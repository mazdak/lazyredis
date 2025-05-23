use ratatui::{
    layout::{Constraint, Direction, Layout, Rect, Alignment, Position},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Clear, Wrap, Gauge},
    Frame,
    text::{Line, Span},
};
use crate::app::{App};

/// helper function to create a centered rect using up certain percentage of the available rect `r`
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ].as_ref())
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ].as_ref())
        .split(popup_layout[1])[1]
}

pub fn ui(f: &mut Frame, app: &App) {
    // Define main layout areas for when modals are NOT fully obscuring
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Increased height for DB list and status
            Constraint::Min(0),    // For key/value panels
            Constraint::Length(1), // For footer help
            Constraint::Length(1), // For clipboard status
        ].as_ref())
        .split(f.area());

    if app.profile_state.is_active {
        // Profile selector takes over the main view
        draw_profile_selector_modal(f, app);
        // Still draw footer and status if they are separate from the main content area that modal covers
        draw_footer_help(f, app, main_layout[2]); // Assuming footer is outside modal coverage or desired
        draw_clipboard_status(f, app, main_layout[3]);
    } else {
        // Normal view
        let content_layout_chunks = if app.show_stats {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(25), Constraint::Percentage(50), Constraint::Percentage(25)].as_ref())
                .split(main_layout[1])
        } else {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
                .split(main_layout[1])
        };

        draw_profiles_or_db_list(f, app, main_layout[0]);
        draw_key_list_panel(f, app, content_layout_chunks[0]);
        
        if app.show_stats {
            draw_value_display_panel(f, app, content_layout_chunks[1]);
            draw_redis_stats_panel(f, app, content_layout_chunks[2]);
        } else {
            draw_value_display_panel(f, app, content_layout_chunks[1]);
        }
        
        draw_footer_help(f, app, main_layout[2]);
        draw_clipboard_status(f, app, main_layout[3]);

        if app.delete_dialog.show_confirmation_dialog {
            draw_delete_confirmation_dialog(f, app);
        }
        if app.command_state.is_active {
            draw_command_prompt_modal(f, app);
        }
    }
}

fn draw_profiles_or_db_list(f: &mut Frame, app: &App, area: Rect) {
    let is_focused = !app.is_key_view_focused && !app.is_value_view_focused;

    let current_profile = app.profiles.get(app.current_profile_index);
    let profile_name_str = current_profile.map_or("Unknown", |p| p.name.as_str());
    let profile_color = current_profile.map_or(Color::White, |p| p.resolved_color());

    let base_title_text = format!("Profile: {} - Databases / Connection", profile_name_str);
    let block_title = if is_focused {
        format!("{} [FOCUSED]", base_title_text)
    } else {
        base_title_text
    };

    let outer_block = Block::default()
        .borders(Borders::ALL)
        .title(block_title)
        .border_style(Style::default().fg(profile_color));

    f.render_widget(outer_block.clone(), area); // Render the block itself
    let inner_area = outer_block.inner(area); // Get area inside borders

    // Split inner_area for a vertical DB list and a status message area
    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),       // For DB List
            Constraint::Length(1),    // For Connection Status (adjust if wrapping needed)
        ].as_ref())
        .split(inner_area);
    
    let db_list_area = vertical_chunks[0];
    let status_area = vertical_chunks[1];

    // Render DB List (Vertical)
    let dbs: Vec<ListItem> = (0..app.db_count)
        .map(|i| {
            let display_text = format!("DB {}", i);
            let style = if i as usize == app.selected_db_index {
                if is_focused {
                    Style::default().fg(Color::Black).bg(Color::White) // Focused and selected
                } else {
                    Style::default().fg(Color::Cyan) // Selected but not focused
                }
            } else {
                Style::default()
            };
            ListItem::new(display_text).style(style)
        })
        .collect();

    let db_list_widget = List::new(dbs)
        .block(Block::default()) // No title for inner list, outer block has it
        .highlight_style(Style::default().add_modifier(Modifier::BOLD)) // Style for when navigating, if any
        .highlight_symbol(if is_focused { ">> " } else { "  " });
    
    let mut db_list_state = ListState::default();
    if app.db_count > 0 && (app.selected_db_index as u8) < app.db_count { // Ensure index is valid
        db_list_state.select(Some(app.selected_db_index));
    }
    f.render_stateful_widget(db_list_widget, db_list_area, &mut db_list_state);

    // Render Status
    let connection_status_paragraph = Paragraph::new(app.connection_status.as_str())
        .style(Style::default().fg(profile_color))
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Center); // Center status text
    f.render_widget(connection_status_paragraph, status_area);
}

fn format_ttl(ttl: i64) -> String {
    if ttl < 0 {
        "No Expiry".to_string()
    } else {
        let mins = ttl / 60;
        let secs = ttl % 60;
        if mins > 0 {
            format!("Expires in {}m {}s", mins, secs)
        } else {
            format!("Expires in {}s", secs)
        }
    }
}

fn draw_key_list_panel(f: &mut Frame, app: &App, area: Rect) {
    let mut key_view_base_title = format!("Keys: {}", app.current_breadcrumb.join(&app.key_delimiter.to_string()));
    if app.search_state.is_active {
        key_view_base_title = format!("Search Results (Global): {}", app.search_state.query);
    }
    let key_view_title = if app.is_key_view_focused {
        format!("{} [FOCUSED]", key_view_base_title)
    } else {
        key_view_base_title
    };
    let key_items: Vec<ListItem> = if app.search_state.is_active {
        app.search_state.filtered_keys
            .iter()
            .map(|full_key_name| ListItem::new(full_key_name.as_str()))
            .collect()
    } else {
        app.visible_keys_in_current_view
            .iter()
            .map(|(name, _is_folder)| ListItem::new(name.as_str()))
            .collect()
    };
    let selected_key_index = if app.search_state.is_active {
        app.search_state.selected_index
    } else {
        app.selected_visible_key_index
    };
    let mut list_state = ListState::default();
    let is_list_empty = key_items.is_empty();
    let list_len = key_items.len();
    let list_widget = List::new(key_items)
        .block(Block::default().borders(Borders::ALL).title(key_view_title))
        .highlight_style(
            Style::default()
                .bg(if app.is_key_view_focused { Color::Yellow } else { Color::DarkGray })
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(if app.is_key_view_focused { ">> " } else { "  " });
    if !is_list_empty && selected_key_index < list_len {
        list_state.select(Some(selected_key_index));
    }
    f.render_stateful_widget(list_widget, area, &mut list_state);
}

fn draw_value_display_panel(f: &mut Frame, app: &App, area: Rect) {
    let mut value_block_title = match &app.value_viewer.active_leaf_key_name {
        Some(name) => {
            let ttl = app.ttl_map.get(name).copied().unwrap_or(-2);
            let ttl_str = format_ttl(ttl);
            format!("Value: {} ({}) | TTL: {}", name, app.value_viewer.selected_key_type.as_deref().unwrap_or("N/A"), ttl_str)
        },
        None => "Value".to_string(),
    };
    if app.is_value_view_focused {
        value_block_title.push_str(" [FOCUSED]");
    }
    let block = Block::default().borders(Borders::ALL).title(value_block_title)
        .border_style(if app.is_value_view_focused { Style::default().fg(Color::Cyan) } else { Style::default() });
    if let Some(lines) = &app.value_viewer.displayed_value_lines {
        let items: Vec<ListItem> = lines.iter().map(|s| ListItem::new(s.as_str())).collect();
        let mut list_state = ListState::default();
        if !items.is_empty() && app.value_viewer.selected_value_sub_index < items.len() {
            list_state.select(Some(app.value_viewer.selected_value_sub_index));
        }
        let list_widget = List::new(items)
            .block(block)
            .highlight_style(
                Style::default()
                    .bg(if app.is_value_view_focused { Color::Yellow } else { Color::DarkGray })
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(if app.is_value_view_focused { ">> " } else { "  " });
        f.render_stateful_widget(list_widget, area, &mut list_state);
    } else {
        let value_display_text = app.value_viewer.current_display_value.as_deref().unwrap_or("");
        let value_paragraph = Paragraph::new(value_display_text)
            .block(block)
            .wrap(Wrap { trim: true })
            .scroll(app.value_viewer.value_view_scroll);
        f.render_widget(value_paragraph, area);
    }
}

fn draw_footer_help(f: &mut Frame, app: &App, area: Rect) {
    let mut help_spans = vec![
        Span::styled("q: quit", Style::default().fg(Color::Yellow)),
        Span::raw(" | "),
        Span::styled("p: profiles", Style::default().fg(Color::Yellow)),
        Span::raw(" | "),
        Span::styled("j/k/↑/↓: nav keys/vals", Style::default().fg(Color::Yellow)),
        Span::raw(" | "),
        Span::styled("PgUp/PgDn: page nav vals", Style::default().fg(Color::Yellow)),
        Span::raw(" | "),
        Span::styled("Tab/S-Tab: focus", Style::default().fg(Color::Yellow)),
        Span::raw(" | "),
        Span::styled("Enter: select", Style::default().fg(Color::Yellow)),
        Span::raw(" | "),
        Span::styled("Esc: up/root", Style::default().fg(Color::Yellow)),
        Span::raw(" | "),
        Span::styled("y: copy name", Style::default().fg(Color::Yellow)),
        Span::raw(" | "),
        Span::styled("Y: copy val", Style::default().fg(Color::Yellow)),
        Span::raw(" | "),
        Span::styled("/: search", Style::default().fg(Color::Yellow)),
        Span::raw(" | "),
        Span::styled("d: del", Style::default().fg(Color::Yellow)),
        Span::raw(" | "),
        Span::styled("s: stats", Style::default().fg(Color::Yellow)),
    ];

    if app.search_state.is_active {
        help_spans.extend(vec![
            Span::raw(" | "),
            Span::styled("Esc: exit search", Style::default().fg(Color::Cyan)),
            Span::raw(" | "),
            Span::styled("Enter: activate", Style::default().fg(Color::Cyan)),
        ]);
    } else if app.delete_dialog.show_confirmation_dialog {
        help_spans = vec![
            Span::styled("Confirm Deletion: ", Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD)),
            Span::styled("[Y]es", Style::default().fg(Color::Green)),
            Span::raw(" / "),
            Span::styled("[N]o (Esc)", Style::default().fg(Color::Red)),
        ];
    } else if !app.command_state.is_active {
        help_spans.extend(vec![
            Span::raw(" | "),
            Span::styled(":: cmd", Style::default().fg(Color::Cyan)),
        ]);
    }


    let help_line = Line::from(help_spans);
    let footer_paragraph = Paragraph::new(help_line)
        .block(Block::default())
        .alignment(Alignment::Center);
    f.render_widget(footer_paragraph, area);
}

fn draw_clipboard_status(f: &mut Frame, app: &App, area: Rect) {
    if let Some(status) = &app.clipboard_status {
        let status_text = Paragraph::new(status.as_str())
            .style(Style::default().fg(Color::LightCyan))
            .alignment(Alignment::Center);
        f.render_widget(status_text, area);
    }
}

fn draw_delete_confirmation_dialog(f: &mut Frame, app: &App) {
    let area = centered_rect(60, 25, f.area());
    f.render_widget(Clear, area); // Clear the background

    let item_type = if app.delete_dialog.deletion_is_folder { "folder" } else { "key" };
    let item_name = app
        .delete_dialog
        .key_to_delete_display_name
        .as_deref()
        .unwrap_or("unknown");

    let text = vec![
        Line::from(Span::styled(
            format!("Delete {} '{}'?", item_type, item_name),
            Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD)
        )).alignment(Alignment::Center),
        Line::from("").alignment(Alignment::Center),
        if app.delete_dialog.deletion_is_folder {
            Line::from(Span::raw(format!("This will delete the folder and ALL keys within prefix '{}'.", app.delete_dialog.prefix_to_delete.as_deref().unwrap_or("N/A")))).alignment(Alignment::Center)
        } else {
            Line::from(Span::raw(format!("This will permanently delete the key '{}'.", app.delete_dialog.key_to_delete_full_path.as_deref().unwrap_or(item_name)))).alignment(Alignment::Center)
        },
        Line::from("").alignment(Alignment::Center),
        Line::from(Span::raw("This action CANNOT be undone.")).alignment(Alignment::Center),
        Line::from("").alignment(Alignment::Center),
        Line::from(vec![
            Span::raw("Press "),
            Span::styled("[Y]es", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(" or "),
            Span::styled("[N]o (Esc)", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        ]).alignment(Alignment::Center),
    ];

    let block = Block::default()
        .title("Confirm Deletion")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));

    let paragraph = Paragraph::new(text)
        .block(block)
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

fn draw_profile_selector_modal(f: &mut Frame, app: &App) {
    // Define a centered area for the modal, e.g., 60% width, 50% height
    let area = centered_rect(60, 50, f.area());
    f.render_widget(Clear, area); // Clear the background

    let profiles: Vec<ListItem> = app
        .profiles
        .iter()
        .enumerate()
        .map(|(idx, profile)| {
            let item_color = profile.resolved_color();
            let style = if idx == app.profile_state.selected_index {
                Style::default()
                    .fg(Color::Black)
                    .bg(item_color)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(item_color)
            };
            ListItem::new(format!("{} ({})", profile.name, profile.url)).style(style)
        })
        .collect();

    let list_widget = List::new(profiles)
        .block(Block::default().borders(Borders::ALL).title("Select Connection Profile (p/Esc to close)"))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .highlight_symbol(">> ");
    
    let mut list_state = ListState::default();
    list_state.select(Some(app.profile_state.selected_index));

    f.render_stateful_widget(list_widget, area, &mut list_state);
}

fn draw_command_prompt_modal(f: &mut Frame, app: &App) {
    let area = centered_rect(70, 30, f.area());
    f.render_widget(Clear, area);

    let input_line_text = format!("CMD> {}", app.command_state.input_buffer);
    // Calculate cursor position: area.x + "CMD> ".len() + current command_input length
    // Ensure cursor position is within the bounds of the modal.
    let cursor_x = area.x + 6 + app.command_state.input_buffer.chars().count() as u16;
    let cursor_y = area.y + 3; // Corrected: Was area.y + 4, should be on the input line

    // Only set cursor if the command prompt is active and focused (implicitly handled by modal display)
    f.set_cursor_position(Position::new(cursor_x, cursor_y));

    let output = app.command_state.last_result.as_deref().unwrap_or("");

    let text = vec![
        Line::from(Span::styled(
            "Custom Command Prompt - use at your own risk!",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ))
        .alignment(Alignment::Center),
        Line::from("").alignment(Alignment::Center),
        Line::from(input_line_text),
        Line::from("").alignment(Alignment::Center),
        Line::from(output),
    ];

    let block = Block::default().borders(Borders::ALL).title("Command Prompt (: to open, Esc to close)");
    let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: false });
    f.render_widget(paragraph, area);
}

fn draw_redis_stats_panel(f: &mut Frame, app: &App, area: Rect) {
    let title = if app.stats_auto_refresh {
        "Redis Stats [Auto] (s: toggle)"
    } else {
        "Redis Stats [Manual] (s: toggle)"
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(Color::Cyan));

    if let Some(stats) = &app.redis_stats {
        // Split the area into sections for different stat categories
        let inner_area = block.inner(area);
        let sections = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(6),  // Server info
                Constraint::Length(8),  // Memory stats
                Constraint::Length(6),  // Client stats
                Constraint::Length(6),  // Performance stats
                Constraint::Min(0),     // Additional space
            ])
            .split(inner_area);

        // Server Information Section
        let server_info = vec![
            Line::from(vec![
                Span::styled("Server: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw(format!("Redis {} ({})", stats.redis_version, stats.redis_mode)),
            ]),
            Line::from(vec![
                Span::styled("Role: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw(format!("{} ({} slaves)", stats.role, stats.connected_slaves)),
            ]),
            Line::from(vec![
                Span::styled("Uptime: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw(&stats.uptime_human),
            ]),
            Line::from(vec![
                Span::styled("Updated: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw(format!("{:.1}s ago", stats.age().as_secs_f64())),
            ]),
        ];

        let server_paragraph = Paragraph::new(server_info)
            .block(Block::default().borders(Borders::ALL).title("Server").border_style(Style::default().fg(Color::Green)))
            .wrap(Wrap { trim: true });
        f.render_widget(server_paragraph, sections[0]);

        // Memory Section with btop-style bars
        let memory_usage_ratio = if stats.memory_peak > 0 {
            (stats.memory_used as f64 / stats.memory_peak as f64).min(1.0)
        } else {
            0.0
        };

        let memory_gauge = Gauge::default()
            .block(Block::default().borders(Borders::ALL).title("Memory Usage").border_style(Style::default().fg(Color::Red)))
            .gauge_style(Style::default().fg(Color::Red).bg(Color::Black))
            .ratio(memory_usage_ratio)
            .label(format!("{} / {} ({:.1}%)", 
                stats.memory_used_human, 
                stats.memory_peak_human,
                memory_usage_ratio * 100.0
            ));
        f.render_widget(memory_gauge, sections[1]);

        // Client Stats
        let client_info = vec![
            Line::from(vec![
                Span::styled("Connected: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(stats.connected_clients.to_string(), Style::default().fg(Color::Green)),
            ]),
            Line::from(vec![
                Span::styled("Blocked: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(stats.blocked_clients.to_string(), Style::default().fg(Color::Red)),
            ]),
            Line::from(vec![
                Span::styled("Hit Rate: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{:.1}%", stats.hit_rate), 
                    if stats.hit_rate > 90.0 { Style::default().fg(Color::Green) } 
                    else if stats.hit_rate > 70.0 { Style::default().fg(Color::Yellow) } 
                    else { Style::default().fg(Color::Red) }
                ),
            ]),
        ];

        let client_paragraph = Paragraph::new(client_info)
            .block(Block::default().borders(Borders::ALL).title("Clients").border_style(Style::default().fg(Color::Blue)))
            .wrap(Wrap { trim: true });
        f.render_widget(client_paragraph, sections[2]);

        // Performance Stats
        let perf_info = vec![
            Line::from(vec![
                Span::styled("Ops/sec: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(stats.instantaneous_ops_per_sec.to_string(), 
                    if stats.instantaneous_ops_per_sec > 1000 { Style::default().fg(Color::Green) }
                    else if stats.instantaneous_ops_per_sec > 100 { Style::default().fg(Color::Yellow) }
                    else { Style::default().fg(Color::White) }
                ),
            ]),
            Line::from(vec![
                Span::styled("Total Cmds: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw(format_large_number(stats.total_commands_processed)),
            ]),
            Line::from(vec![
                Span::styled("CPU: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw(format!("sys:{:.2} usr:{:.2}", stats.used_cpu_sys, stats.used_cpu_user)),
            ]),
        ];

        let perf_paragraph = Paragraph::new(perf_info)
            .block(Block::default().borders(Borders::ALL).title("Performance").border_style(Style::default().fg(Color::Magenta)))
            .wrap(Wrap { trim: true });
        f.render_widget(perf_paragraph, sections[3]);

    } else {
        // No stats available
        let loading_text = vec![
            Line::from(""),
            Line::from(Span::styled("Loading Redis stats...", Style::default().fg(Color::Yellow))).alignment(Alignment::Center),
            Line::from(""),
            Line::from(Span::raw("Press 's' to toggle stats view")).alignment(Alignment::Center),
        ];

        let loading_paragraph = Paragraph::new(loading_text)
            .block(block)
            .wrap(Wrap { trim: true });
        f.render_widget(loading_paragraph, area);
        return;
    }

    f.render_widget(block, area);
}

fn format_large_number(num: u64) -> String {
    if num >= 1_000_000_000 {
        format!("{:.1}B", num as f64 / 1_000_000_000.0)
    } else if num >= 1_000_000 {
        format!("{:.1}M", num as f64 / 1_000_000.0)
    } else if num >= 1_000 {
        format!("{:.1}K", num as f64 / 1_000.0)
    } else {
        num.to_string()
    }
}
