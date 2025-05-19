pub mod app;
pub mod ui;
pub mod config;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event as CEvent, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};
use std::{error::Error, io, time::Duration};
use clap::Parser;
use redis::{Client, Commands};

/// A simple TUI for Redis
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct CliArgs {
    /// Seed the Redis instance with test data
    #[arg(long)]
    seed: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = CliArgs::parse();

    if args.seed {
        println!("Seeding Redis with test data...");
        let app_config = config::Config::load();
        let redis_url = app_config.profiles.first().map_or("redis://127.0.0.1:6379", |p| &p.url);
        
        match seed_redis_data(redis_url) {
            Ok(_) => {
                println!("Redis seeded successfully.");
            }
            Err(e) => {
                eprintln!("Error seeding Redis: {}", e);
            }
        }
        return Ok(());
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let app_config_tui = config::Config::load();
    let initial_url = app_config_tui.profiles.first().map_or("redis://127.0.0.1:6379", |p| &p.url).to_string();
    let initial_profile_name = app_config_tui.profiles.first().map_or("Default", |p| &p.name).to_string();
    let app = app::App::new(&initial_url, &initial_profile_name, app_config_tui.profiles.clone());

    let res = run_app(&mut terminal, app);

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

fn seed_redis_data(redis_url: &str) -> Result<(), Box<dyn Error>> {
    println!("Connecting to {} to seed data...", redis_url);
    let client = Client::open(redis_url)?;
    let mut con = client.get_connection()?;

    println!("Seeding basic keys...");
    let _: () = con.set("seed:string", "Hello from LazyRedis Seeder!")?;
    let _: () = con.set("seed:another_string", "This string is a bit longer and might require scrolling to see fully in the value panel if it is narrow enough.")?;
    let _: () = con.hset_multiple("seed:hash", &[("field1", "Value1"), ("field2", "Another Value"), ("long_field_name_for_testing_wrapping", "This value is also quite long to test how wrapping behaves in the TUI for hash values.")])?;
    
    println!("Seeding list...");
    let _: () = con.rpush("seed:list", &["Item 1", "Item 2", "Item 3", "Yet another item", "And one more for good measure"])?;
    let mut list_for_scrolling = Vec::new();
    for i in 1..=30 {
        list_for_scrolling.push(format!("Scrollable List Item {}", i));
    }
    let _: () = con.rpush("seed:scroll_list", list_for_scrolling)?;

    println!("Seeding set...");
    let _: () = con.sadd("seed:set", &["MemberA", "MemberB", "MemberC", "MemberD", "MemberE", "MemberA"])?;

    println!("Seeding sorted set (zset)...");
    let _: () = con.zadd_multiple("seed:zset", &[ (10.0, "Ten"), (1.0, "One"), (30.0, "Thirty"), (20.0, "Twenty"), (5.0, "Five"), (100.0, "One Hundred"), (15.0, "Fifteen")])?;

    println!("Seeding stream...");
    let _: String = con.xadd("seed:stream", "*", &[("fieldA", "valueA1"), ("fieldB", "valueB1")])?;
    let _: String = con.xadd("seed:stream", "*", &[("sensor-id", "1234"), ("temperature", "19.8")])?;
    let _: String = con.xadd("seed:stream", "*", &[("message", "Hello World"), ("user", "Alice"), ("timestamp", "1678886400000")])?;

    println!("Seeding nested/namespaced keys...");
    let _: () = con.set("seed:user:1:name", "Alice Wonderland")?;
    let _: () = con.set("seed:user:1:email", "alice@example.com")?;
    let _: () = con.set("seed:user:1:settings:theme", "dark")?;
    let _: () = con.set("seed:user:2:name", "Bob The Builder")?;
    let _: () = con.set("seed:user:2:email", "bob@example.com")?;
    let _: () = con.hset_multiple("seed:product:100", &[("name", "Awesome Gadget"), ("price", "99.99"), ("stock", "250")])?;
    let _: () = con.rpush("seed:logs:app1", &["INFO: Startup complete", "WARN: Low disk space", "ERROR: Connection timeout"])?;
    let _: () = con.sadd("seed:followers:user:1", &["user:2", "user:3", "user:4"])?;
    let _: () = con.zadd("seed:leaderboard:game1", "Alice", 1500.0)?;
    let _: () = con.zadd("seed:leaderboard:game1", "Bob", 1200.0)?;

    println!("Seeding keys with special characters or delimiters in name...");
    let _: () = con.set("seed:key:with:colons", "value for key with colons")?;
    let _: () = con.set("seed:key/with/slashes", "value for key with slashes")?;
    let _: () = con.set("seed:key with spaces", "value for key with spaces")?;
    let _: () = con.set("seed:!@#$%^&*()", "value for key with special chars")?;
    
    println!("Seeding an empty hash for testing empty view");
    let _: () = con.hset("seed:empty_hash", "placeholder_field", "placeholder_value")?;
    let _: i32 = con.hdel("seed:empty_hash", "placeholder_field")?;

    println!("Seeding an empty list for testing empty view");
    let _: () = con.rpush("seed:empty_list", "placeholder")?;
    let _: String = con.lpop("seed:empty_list", Default::default())?;


    println!("Seeding an empty set for testing empty view");
    let _: () = con.sadd("seed:empty_set", "placeholder")?;
    let _: i32 = con.srem("seed:empty_set", "placeholder")?;

    println!("Seeding an empty zset for testing empty view");
    let _: () = con.zadd("seed:empty_zset", "placeholder", 1.0)?;
    let _: i32 = con.zrem("seed:empty_zset", "placeholder")?;
    
    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: app::App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui::ui(f, &app))?;

        if event::poll(Duration::from_millis(100))? {
            if let CEvent::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press { 
                    app.clipboard_status = None; 

                    if app.is_profile_selector_active {
                        match key.code {
                            KeyCode::Char('q') => return Ok(()),
                            KeyCode::Char('p') | KeyCode::Esc => app.toggle_profile_selector(),
                            KeyCode::Char('j') | KeyCode::Down => app.next_profile_in_list(),
                            KeyCode::Char('k') | KeyCode::Up => app.previous_profile_in_list(),
                            KeyCode::Enter => app.select_profile_and_connect(),
                            _ => {}
                        }
                    } else if app.is_search_active {
                        // Handle input for search mode
                        match key.code {
                            KeyCode::Char(c) => {
                                app.search_query.push(c);
                                app.update_filtered_keys();
                            }
                            KeyCode::Backspace => {
                                app.search_query.pop();
                                app.update_filtered_keys();
                            }
                            KeyCode::Esc => {
                                app.exit_search_mode();
                            }
                            KeyCode::Enter => {
                                app.activate_selected_filtered_key(); 
                                app.exit_search_mode(); 
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
                        // Normal mode input handling (not profile selector, not search)
                        // Check for Shift+Tab first due to potential Tab match
                        if key.modifiers == KeyModifiers::SHIFT && key.code == KeyCode::Tab { // Note: BackTab might not be reliably caught by all terminals as a unique KeyCode
                             app.cycle_focus_backward();
                        } else { // Regular key codes without specific shift handling
                            match key.code {
                                KeyCode::Char('q') => return Ok(()),
                                KeyCode::Char('/') => { // Enter search mode
                                    app.enter_search_mode();
                                }
                                KeyCode::Char('p') => app.toggle_profile_selector(),
                                KeyCode::Tab => app.cycle_focus_forward(), 
                                KeyCode::Char('y') => app.copy_selected_key_name_to_clipboard(), 
                                KeyCode::Char('Y') => app.copy_selected_key_value_to_clipboard(), 
                                KeyCode::Char('j') | KeyCode::Down => {
                                    if app.is_value_view_focused {
                                        app.scroll_value_view_down(1);
                                    } else if app.is_key_view_focused {
                                        app.next_key_in_view();
                                    } else {
                                        app.next_db();
                                    }
                                }
                                KeyCode::Char('k') | KeyCode::Up => {
                                    if app.is_value_view_focused {
                                        app.scroll_value_view_up(1);
                                    } else if app.is_key_view_focused {
                                        app.previous_key_in_view();
                                    } else {
                                        app.previous_db();
                                    }
                                }
                                KeyCode::PageDown => { 
                                    if app.is_value_view_focused {
                                        app.scroll_value_view_page_down();
                                    }
                                }
                                KeyCode::PageUp => { 
                                    if app.is_value_view_focused {
                                        app.scroll_value_view_page_up();
                                    }
                                }
                                KeyCode::Enter => {
                                    if app.is_key_view_focused {
                                        app.activate_selected_key();
                                    } else if !app.is_value_view_focused { // Neither key_view nor value_view focused, so DB view is active
                                        // DB is already selected by j/k. Just switch focus to key view.
                                        app.is_key_view_focused = true;
                                        app.is_value_view_focused = false; // Ensure value view isn't also focused
                                    }
                                }
                                KeyCode::Backspace => { 
                                    if app.is_key_view_focused {
                                        app.navigate_key_tree_up();
                                    }
                                }
                                KeyCode::Esc => { // New: Navigate to root of key tree
                                    if app.is_key_view_focused {
                                        app.navigate_to_key_tree_root();
                                    }
                                    // If other views are focused, Esc might have other meanings or do nothing here
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }
} 