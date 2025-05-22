pub mod app;
pub mod ui;
pub mod config;
pub mod seed;
pub mod search;
pub mod command;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event as CEvent, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};
use std::{io, time::Duration};
use anyhow::Result;
use clap::Parser;
use redis::Client;
use url::Url;
use tokio;

/// A simple TUI for Redis
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct CliArgs {
    /// Profile name to connect on startup, or to select for seeding/purging
    #[arg(long, value_name = "PROFILE")]
    profile: Option<String>,

    /// Seed the Redis instance with test data
    #[arg(long)]
    seed: bool,

    /// Purge (delete) all keys in the Redis instance
    #[arg(long)]
    purge: bool,
}

// Add a page size constant for value navigation
const VALUE_NAVIGATION_PAGE_SIZE: usize = 10;

#[tokio::main]
async fn main() -> Result<()> {
    let args = CliArgs::parse();

    if args.seed || args.purge {
        let action = if args.purge { "purge" } else { "seed" };
        let verb = if args.purge { "Purging" } else { "Seeding" };
        let noun = if args.purge { "keys" } else { "with test data" };
        println!("{} Redis {}...", verb, noun);
        let app_config = config::Config::load(None);

        let target_profile = if let Some(profile_name) = &args.profile {
            app_config
                .profiles
                .iter()
                .find(|p| &p.name == profile_name)
                .or_else(|| {
                    eprintln!("Profile '{}' not found in configuration.", profile_name);
                    std::process::exit(1);
                })
        } else {
            app_config
                .profiles
                .iter()
                .find(|p| {
                    p.dev.unwrap_or(false)
                        || if let Ok(url) = Url::parse(&p.url) {
                            url.host_str()
                                .map_or(false, |host| host == "localhost" || host == "127.0.0.1")
                        } else {
                            false
                        }
                })
                .or_else(|| app_config.profiles.first())
        };

        if let Some(profile) = target_profile {
            if !profile.dev.unwrap_or(false) {
                eprintln!(
                    "Profile '{}' is not marked dev=true; refusing to {}.",
                    profile.name,
                    action
                );
                std::process::exit(1);
            }

            println!(
                "Targeting profile: {} ({}) for {}.",
                profile.name,
                profile.url,
                action
            );
            if args.purge {
                println!(
                    "This will delete ALL KEYS in database {} on {}.",
                    profile.db.unwrap_or(0),
                    profile.url
                );
            } else {
                println!(
                    "This will delete ALL KEYS in database {} on {} and add a large amount of test data.",
                    profile.db.unwrap_or(0),
                    profile.url
                );
            }
            println!("Are you sure you want to proceed? (yes/no)");
            let mut confirmation = String::new();
            io::stdin().read_line(&mut confirmation)?;
            if confirmation.trim().to_lowercase() != "yes" {
                println!("{} cancelled by user.", if args.purge { "Purge" } else { "Seeding" });
                return Ok(());
            }

            if args.purge {
                match purge_redis_data(&profile.url, profile.db.unwrap_or(0)).await {
                    Ok(_) => println!("Redis purged successfully for profile '{}'.", profile.name),
                    Err(e) => eprintln!("Error purging Redis for profile '{}': {}", profile.name, e),
                }
            } else {
                match seed::seed_redis_data(&profile.url, profile.db.unwrap_or(0)).await {
                    Ok(_) => println!("Redis seeded successfully for profile '{}'.", profile.name),
                    Err(e) => eprintln!("Error seeding Redis for profile '{}': {}", profile.name, e),
                }
            }
        } else {
            eprintln!(
                "No suitable profile found for {} (dev=true or localhost/127.0.0.1). Please check your lazyredis.toml",
                action
            );
        }
        return Ok(());
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let app_config_tui = config::Config::load(None);
    let (initial_url, initial_profile_name) = if let Some(profile_name) = &args.profile {
        match app_config_tui.profiles.iter().find(|p| &p.name == profile_name) {
            Some(p) => (p.url.clone(), p.name.clone()),
            None => {
                eprintln!("Profile '{}' not found in configuration.", profile_name);
                std::process::exit(1);
            }
        }
    } else {
        (
            app_config_tui.profiles.first().map_or("redis://127.0.0.1:6379".to_string(), |p| p.url.clone()),
            app_config_tui.profiles.first().map_or("Default".to_string(), |p| p.name.clone()),
        )
    };
    let app = app::App::new(&initial_url, &initial_profile_name, app_config_tui.profiles.clone());

    let res = run_app(&mut terminal, app).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err);
    }
    
    Ok(())
}

/// Purge (flush) all keys in the specified Redis database
async fn purge_redis_data(redis_url: &str, db_index: u8) -> Result<()> {
    println!("Connecting to {} (DB {}) to purge keys...", redis_url, db_index);
    let client = Client::open(redis_url)?;
    let mut con = client.get_multiplexed_async_connection().await?;

    redis::cmd("SELECT").arg(db_index).query_async::<()>(&mut con).await?;
    println!("Selected database {}.", db_index);

    println!("Purging database {}...", db_index);
    redis::cmd("FLUSHDB").query_async::<()>(&mut con).await?;
    println!("Database {} purged.", db_index);

    Ok(())
}

async fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: app::App) -> io::Result<()> {
    // Trigger initial connect, status will be set by this sync call
    app.trigger_initial_connect(); 
    // First draw will show "Preparing initial connection..."
    terminal.draw(|f| ui::ui(f, &app))?; 
    // Removed: app.initial_connect_and_fetch().await; We handle this in the loop now

    loop {
        // Handle pending asynchronous operations if any
        // We clone it because the execute methods take `&mut self` and might clear it.
        let mut did_async_op = false;
        let operation_to_execute = app.pending_operation.take();
        if let Some(operation_to_execute) = operation_to_execute {
            match operation_to_execute {
                app::PendingOperation::InitialConnect => {
                    app.execute_initial_connect().await;
                    did_async_op = true;
                }
                app::PendingOperation::ApplySelectedDb => {
                    app.execute_apply_selected_db().await;
                    did_async_op = true;
                }
                app::PendingOperation::SelectProfileAndConnect => {
                    app.select_profile_and_connect().await; 
                    did_async_op = true;
                }
                app::PendingOperation::ConfirmDeleteItem => {
                    app.confirm_delete_item().await;
                    did_async_op = true;
                }
                app::PendingOperation::ExecuteCommand => {
                    app.execute_command_input().await;
                    did_async_op = true;
                }
                app::PendingOperation::ActivateSelectedKey => {
                    app.activate_selected_key().await;
                    did_async_op = true;
                }
                app::PendingOperation::CopyKeyNameToClipboard => {
                    crate::app::app_clipboard::copy_selected_key_name_to_clipboard(&mut app).await;
                    did_async_op = true;
                }
                app::PendingOperation::CopyKeyValueToClipboard => {
                    crate::app::app_clipboard::copy_selected_key_value_to_clipboard(&mut app).await;
                    did_async_op = true;
                }
                app::PendingOperation::ActivateSelectedFilteredKey => {
                    app.activate_selected_filtered_key().await;
                    did_async_op = true;
                }
            }
        }
        if did_async_op {
            continue;
        }
        terminal.draw(|f| ui::ui(f, &app))?;

        // Now handle events in a separate block (mutable borrow)
        if event::poll(Duration::from_millis(100))? {
            if let CEvent::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press { 
                    app.clipboard_status = None; 

                    // Only process key events if no async operation is pending
                    // This prevents inputs from interfering with an ongoing async task's state changes
                    // or triggering new operations while one is in progress.
                    if app.pending_operation.is_none() {
                        if app.profile_state.is_active {
                            match key.code {
                                KeyCode::Char('q') => return Ok(()),
                                KeyCode::Char('p') | KeyCode::Esc => app.toggle_profile_selector(),
                                KeyCode::Char('j') | KeyCode::Down => app.next_profile_in_list(),
                                KeyCode::Char('k') | KeyCode::Up => app.previous_profile_in_list(),
                                KeyCode::Enter => {
                                    app.pending_operation = Some(app::PendingOperation::SelectProfileAndConnect);
                                }
                                _ => {}
                            }
                        } else if app.delete_dialog.show_confirmation_dialog {
                            match key.code {
                                KeyCode::Enter => {
                                    app.pending_operation = Some(app::PendingOperation::ConfirmDeleteItem);
                                }
                                KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => app.cancel_delete_item(),
                                KeyCode::Char('y') | KeyCode::Char('Y') => {
                                    app.pending_operation = Some(app::PendingOperation::ConfirmDeleteItem);
                                }
                                _ => {}
                            }
                        } else if app.command_state.is_active {
                            match key.code {
                                KeyCode::Esc => {
                                    app.close_command_prompt();
                                    terminal.hide_cursor()?;
                                }
                                KeyCode::Backspace => {
                                    app.command_state.input_buffer.pop();
                                }
                                KeyCode::Char(c) => {
                                    app.command_state.input_buffer.push(c);
                                }
                                KeyCode::Enter => {
                                    app.pending_operation = Some(app::PendingOperation::ExecuteCommand);
                                }
                                _ => {}
                            }
                        } else if app.search_state.is_active {
                            match key.code {
                                KeyCode::Char(c) => {
                                    app.search_state.query.push(c);
                                    app.update_filtered_keys();
                                }
                                KeyCode::Backspace => {
                                    app.search_state.query.pop();
                                    app.update_filtered_keys();
                                }
                                KeyCode::Esc => {
                                    app.exit_search_mode();
                                }
                                KeyCode::Enter => {
                                    app.pending_operation = Some(app::PendingOperation::ActivateSelectedFilteredKey);
                                }
                                KeyCode::Down => {
                                    app.select_next_filtered_key();
                                }
                                KeyCode::Up => {
                                    app.select_previous_filtered_key();
                                }
                                _ => {}
                            }
                        } else {
                            if (key.modifiers == KeyModifiers::SHIFT && key.code == KeyCode::Tab) || key.code == KeyCode::BackTab {
                                app.cycle_focus_backward();
                            } else {
                                match key.code {
                                    KeyCode::Char('q') => return Ok(()),
                                    KeyCode::Char('/') => {
                                        app.enter_search_mode();
                                    }
                                    KeyCode::Char('p') => app.toggle_profile_selector(),
                                    KeyCode::Tab => app.cycle_focus_forward(), 
                                    KeyCode::Char('y') => app.pending_operation = Some(app::PendingOperation::CopyKeyNameToClipboard),
                                    KeyCode::Char('Y') => app.pending_operation = Some(app::PendingOperation::CopyKeyValueToClipboard),
                                    KeyCode::Char('d') => {
                                        if app.is_key_view_focused {
                                            app.initiate_delete_selected_item(); // This is sync, sets up dialog
                                        }
                                    }
                                    KeyCode::Char(':') => {
                                        app.open_command_prompt(); // Sync
                                        terminal.show_cursor()?;
                                    }
                                    KeyCode::Char('j') | KeyCode::Down => {
                                        if app.is_value_view_focused {
                                            app.select_next_value_item();
                                        } else if app.is_key_view_focused {
                                            app.next_key_in_view();
                                        } else {
                                            app.next_db();
                                        }
                                    }
                                    KeyCode::Char('k') | KeyCode::Up => {
                                        if app.is_value_view_focused {
                                            app.select_previous_value_item();
                                        } else if app.is_key_view_focused {
                                            app.previous_key_in_view();
                                        } else {
                                            app.previous_db();
                                        }
                                    }
                                    KeyCode::PageDown => { 
                                        if app.is_value_view_focused {
                                            app.select_page_down_value_item(VALUE_NAVIGATION_PAGE_SIZE);
                                        }
                                    }
                                    KeyCode::PageUp => { 
                                        if app.is_value_view_focused {
                                            app.select_page_up_value_item(VALUE_NAVIGATION_PAGE_SIZE);
                                        }
                                    }
                                    KeyCode::Enter => {
                                        if app.search_state.is_active {
                                            app.pending_operation = Some(app::PendingOperation::ActivateSelectedFilteredKey);
                                        } else if app.is_key_view_focused {
                                            app.pending_operation = Some(app::PendingOperation::ActivateSelectedKey);
                                        } else if !app.is_value_view_focused && !app.is_key_view_focused {
                                            app.trigger_apply_selected_db();
                                        } else if !app.is_value_view_focused {
                                            app.is_key_view_focused = true;
                                            app.is_value_view_focused = false;
                                        }
                                    }
                                    KeyCode::Backspace => { 
                                        if app.is_key_view_focused {
                                            app.navigate_key_tree_up();
                                        }
                                    }
                                    KeyCode::Esc => {
                                        if app.is_key_view_focused {
                                            app.navigate_to_key_tree_root();
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    } // End of if app.pending_operation.is_none()
                }
            }
        }
    }
} 