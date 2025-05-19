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
use url::Url;

/// A simple TUI for Redis
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct CliArgs {
    /// Seed the Redis instance with test data
    #[arg(long)]
    seed: bool,
}

// Add a page size constant for value navigation
const VALUE_NAVIGATION_PAGE_SIZE: usize = 10;

fn main() -> Result<(), Box<dyn Error>> {
    let args = CliArgs::parse();

    if args.seed {
        println!("Seeding Redis with test data...");
        let app_config = config::Config::load();

        let target_profile = app_config.profiles.iter().find(|p| {
            p.dev.unwrap_or(false) ||
            if let Ok(url) = Url::parse(&p.url) {
                url.host_str().map_or(false, |host| host == "localhost" || host == "127.0.0.1")
            } else {
                false
            }
        }).or_else(|| {
            app_config.profiles.first()
        });

        if let Some(profile) = target_profile {
            println!("Targeting profile: {} ({}) for seeding.", profile.name, profile.url);
            
            println!("This will delete ALL KEYS in database {} on {} and add a large amount of test data.", profile.db.unwrap_or(0), profile.url);
            println!("Are you sure you want to proceed? (yes/no)");
            let mut confirmation = String::new();
            io::stdin().read_line(&mut confirmation)?;
            if confirmation.trim().to_lowercase() != "yes" {
                println!("Seeding cancelled by user.");
                return Ok(());
            }

            match seed_redis_data(&profile.url, profile.db.unwrap_or(0)) {
                Ok(_) => {
                    println!("Redis seeded successfully for profile '{}'.", profile.name);
                }
                Err(e) => {
                    eprintln!("Error seeding Redis for profile '{}': {}", profile.name, e);
                }
            }
        } else {
            eprintln!("No suitable profile found for seeding (dev=true or localhost/127.0.0.1). Please check your lazyredis.toml");
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

fn seed_redis_data(redis_url: &str, db_index: u8) -> Result<(), Box<dyn Error>> {
    println!("Connecting to {} (DB {}) to seed data...", redis_url, db_index);
    let client = Client::open(redis_url)?;
    let mut con = client.get_connection()?;

    redis::cmd("SELECT").arg(db_index).query::<()>(&mut con)?;
    println!("Selected database {}.", db_index);

    println!("Flushing database {}...", db_index);
    redis::cmd("FLUSHDB").query::<()>(&mut con)?;
    println!("Database {} flushed.", db_index);

    println!("Seeding a large volume of keys...");

    for i in 0..1000 {
        let _: () = con.set(format!("seed:simple:{}", i), format!("Simple value {}", i))?;
    }
    if 1000 % 100 == 0 { println!("Seeded 1000 simple keys...");}

    for i in 0..50 {
        for j in 0..20 {
            for k in 0..10 {
                let key = format!("seed:level1:{}:level2:{}:key:{}", i, j, k);
                let _: () = con.set(&key, format!("Value for {}", key))?;
            }
        }
        if (i+1) % 10 == 0 { println!("Seeded hierarchy for level1 up to {}...", i+1); }
    }
    println!("Seeded nested keys (50*20*10 = 10,000 keys).");

    for i in 0..100 {
        let _: () = con.set(format!("seed/path/num_{}", i), format!("Path value {}", i))?;
        let _: () = con.set(format!("seed.dot.num_{}", i), format!("Dot value {}", i))?;
        let _: () = con.set(format!("seed-dash-num_{}", i), format!("Dash value {}", i))?;
    }
    println!("Seeded 300 keys with various delimiters.");

    for i in 0..50 {
        let mut fields = Vec::new();
        for j in 0..200 {
            fields.push((format!("field_{}", j), format!("value_for_hash_{}_field_{}", i, j)));
        }
        let _: () = con.hset_multiple(format!("seed:large_hash:{}", i), &fields)?;
        if (i+1) % 10 == 0 { println!("Seeded large hash {}...", i+1); }
    }
    println!("Seeded 50 large hashes (50 * 200 fields).");

    for i in 0..50 {
        let mut items = Vec::new();
        for j in 0..500 {
            items.push(format!("list_{}_item_{}", i, j));
        }
        let _: () = con.rpush(format!("seed:large_list:{}", i), items)?;
        if (i+1) % 10 == 0 { println!("Seeded large list {}...", i+1); }
    }
    println!("Seeded 50 large lists (50 * 500 items).");
    
    for i in 0..50 {
        let mut members = Vec::new();
        for j in 0..300 {
            members.push(format!("set_{}_member_{}", i, j));
        }
        let _: () = con.sadd(format!("seed:large_set:{}", i), members)?;
         if (i+1) % 10 == 0 { println!("Seeded large set {}...", i+1); }
    }
    println!("Seeded 50 large sets (50 * 300 members).");

    for i in 0..50 {
        let mut members_scores = Vec::new();
        for j in 0..400 {
            members_scores.push(((j * 10) as f64, format!("zset_{}_member_{}", i, j)));
        }
        let _: () = con.zadd_multiple(format!("seed:large_zset:{}", i), &members_scores)?;
        if (i+1) % 10 == 0 { println!("Seeded large zset {}...", i+1); }
    }
    println!("Seeded 50 large zsets (50 * 400 members/scores).");

    for i in 0..10 {
        for j in 0..1000 {
            let _: String = con.xadd(format!("seed:large_stream:{}", i), "*", &[
                ("event_id", format!("{}-{}", i, j)),
                ("sensor_id", format!("sensor_{}", i % 5)),
                ("timestamp", (j * 1000).to_string()),
                ("payload", format!("Some data payload for event {}-{}, could be JSON or any string.", i,j))
            ])?;
        }
        println!("Seeded stream seed:large_stream:{} with 1000 entries.", i);
    }
    println!("Seeded 10 streams with 1000 entries each.");
    
    println!("Seeding original specific test keys...");
    let _: () = con.set("seed:string", "Hello from LazyRedis Seeder!")?;
    let _: () = con.set("seed:another_string", "This string is a bit longer and might require scrolling to see fully in the value panel if it is narrow enough.")?;
    let _: () = con.hset_multiple("seed:hash", &[("field1", "Value1"), ("field2", "Another Value"), ("long_field_name_for_testing_wrapping", "This value is also quite long to test how wrapping behaves in the TUI for hash values.")])?;
    let _: () = con.rpush("seed:list", &["Item 1", "Item 2", "Item 3", "Yet another item", "And one more for good measure"])?;
    let _: () = con.sadd("seed:set", &["MemberA", "MemberB", "MemberC", "MemberD", "MemberE", "MemberA"])?;
    let _: () = con.zadd_multiple("seed:zset", &[ (10.0, "Ten"), (1.0, "One"), (30.0, "Thirty"), (20.0, "Twenty"), (5.0, "Five"), (100.0, "One Hundred"), (15.0, "Fifteen")])?;
    let _: String = con.xadd("seed:stream", "*", &[("fieldA", "valueA1"), ("fieldB", "valueB1")])?;
    let _: String = con.xadd("seed:stream", "*", &[("sensor-id", "1234"), ("temperature", "19.8")])?;
    let _: String = con.xadd("seed:stream", "*", &[("message", "Hello World"), ("user", "Alice"), ("timestamp", "1678886400000")])?;
    println!("Seeding empty types for testing views...");
    let _: () = con.hset("seed:empty_hash", "placeholder_field", "placeholder_value")?;
    let _: i32 = con.hdel("seed:empty_hash", "placeholder_field")?;
    let _: () = con.rpush("seed:empty_list", "placeholder")?;
    let _: String = con.lpop("seed:empty_list", Default::default())?;
    let _: () = con.sadd("seed:empty_set", "placeholder")?;
    let _: i32 = con.srem("seed:empty_set", "placeholder")?;
    let _: () = con.zadd("seed:empty_zset", "placeholder", 1.0)?;
    let _: i32 = con.zrem("seed:empty_zset", "placeholder")?;

    println!("Finished seeding data.");
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
                    } else if app.show_delete_confirmation_dialog {
                        match key.code {
                            KeyCode::Enter => app.confirm_delete_item(),
                            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => app.cancel_delete_item(),
                            KeyCode::Char('y') | KeyCode::Char('Y') => app.confirm_delete_item(),
                            _ => {}
                        }
                    } else if app.is_search_active {
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
                                KeyCode::Char('y') => app.copy_selected_key_name_to_clipboard(), 
                                KeyCode::Char('Y') => app.copy_selected_key_value_to_clipboard(), 
                                KeyCode::Char('d') => {
                                    if app.is_key_view_focused {
                                        app.initiate_delete_selected_item();
                                    }
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
                                    if app.is_key_view_focused {
                                        app.activate_selected_key();
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
                }
            }
        }
    }
} 