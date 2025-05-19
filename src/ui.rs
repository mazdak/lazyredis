use ratatui::{
    layout::{Constraint, Direction, Layout, Rect, Alignment},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Clear, Wrap},
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
    // Define main layout areas
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // For profiles/DB list and connection status
            Constraint::Min(0),    // For key/value panels
            Constraint::Length(1), // For footer help
            Constraint::Length(1), // For clipboard status
        ].as_ref())
        .split(f.area());

    // Split the main content area for key list and value display
    let content_layout_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
        .split(main_layout[1]);

    // Draw top panel (profiles or DB list)
    draw_profiles_or_db_list(f, app, main_layout[0]);

    // Draw main content panels (keys and value)
    // These are drawn unless a full-screen modal like profile selector might be covering them.
    // For now, profile selector is part of draw_profiles_or_db_list and doesn't cover these.
    draw_key_list_panel(f, app, content_layout_chunks[0]);
    draw_value_display_panel(f, app, content_layout_chunks[1]);
    
    // Draw footer and status messages
    draw_footer_help(f, app, main_layout[2]);
    draw_clipboard_status(f, app, main_layout[3]);

    // Modals should be drawn last so they appear on top
    if app.is_profile_selector_active {
        // Profile selector is currently integrated into draw_profiles_or_db_list.
        // If it were a separate modal, it would be drawn here:
        // draw_profile_selector_modal(f, app, f.size()); 
    }

    if app.show_delete_confirmation_dialog {
        draw_delete_confirmation_dialog(f, app); // Draw delete confirmation dialog if active
    }
}

fn draw_profiles_or_db_list(f: &mut Frame, app: &App, area: Rect) {
    let outer_block_title = if app.is_profile_selector_active {
        "Select Connection Profile (p to close)"
    } else {
        "Databases / Connection"
    };
    let outer_block = Block::default().borders(Borders::ALL).title(outer_block_title);
    f.render_widget(outer_block, area);

    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)].as_ref()) // Content and status line
        .margin(1)
        .split(area);

    if app.is_profile_selector_active {
        let profiles: Vec<ListItem> = app.profiles.iter().enumerate().map(|(idx, profile)| {
            let style = if idx == app.selected_profile_list_index {
                Style::default().fg(Color::Black).bg(Color::White)
            } else {
                Style::default()
            };
            ListItem::new(format!("{} ({})", profile.name, profile.url)).style(style)
        }).collect();
        let profiles_list = List::new(profiles)
            .block(Block::default())
            .highlight_style(Style::default().add_modifier(Modifier::BOLD))
            .highlight_symbol(">> ");
        f.render_widget(profiles_list, inner_chunks[0]);
    } else {
        // Display DB list and current connection status
        let db_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(20), Constraint::Percentage(80)].as_ref())
            .split(inner_chunks[0]);

        let dbs: Vec<ListItem> = (0..app.db_count)
            .map(|i| {
                let display_text = if i as usize == app.selected_db_index {
                    format!("DB {} <<<", i)
                } else {
                    format!("DB {}", i)
                };
                let style = if i as usize == app.selected_db_index && !app.is_key_view_focused && !app.is_value_view_focused {
                    Style::default().fg(Color::Black).bg(Color::White) // Highlight if DB panel focused
                } else if i as usize == app.selected_db_index {
                    Style::default().fg(Color::Cyan) // Indicate selected but not focused
                } else {
                    Style::default()
                };
                ListItem::new(display_text).style(style)
            })
            .collect();
        let db_list = List::new(dbs)
            .block(Block::default().title(if !app.is_key_view_focused && !app.is_value_view_focused {"[DBs]"} else {"DBs"}));
        f.render_widget(db_list, db_chunks[0]);
        
        let connection_status_paragraph = Paragraph::new(app.connection_status.as_str())
            .wrap(Wrap { trim: true })
            .block(Block::default().title("Status"));
        f.render_widget(connection_status_paragraph, db_chunks[1]);
    }
}

fn draw_key_list_panel(f: &mut Frame, app: &App, area: Rect) {
    let mut key_view_base_title = format!("Keys: {}", app.current_breadcrumb.join(&app.key_delimiter.to_string()));
    if app.is_search_active {
        key_view_base_title = format!("Search Keys ({}): {} [/]", app.search_query, app.current_breadcrumb.join(&app.key_delimiter.to_string()));
    }

    let key_view_title = if app.is_key_view_focused {
        format!("{} [FOCUSED]", key_view_base_title)
    } else {
        key_view_base_title
    };

    let items_to_display = if app.is_search_active {
        &app.filtered_keys_in_current_view
    } else {
        &app.visible_keys_in_current_view
    };
    let selected_key_index = if app.is_search_active {
        app.selected_filtered_key_index
    } else {
        app.selected_visible_key_index
    };

    let key_items: Vec<ListItem> = items_to_display
        .iter()
        .map(|(name, _is_folder)| ListItem::new(name.as_str()))
        .collect();

    let list_widget = List::new(key_items)
        .block(Block::default().borders(Borders::ALL).title(key_view_title))
        .highlight_style(
            Style::default()
                .bg(if app.is_key_view_focused { Color::Yellow } else { Color::DarkGray })
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(if app.is_key_view_focused { ">> " } else { "  " });

    let mut list_state = ListState::default(); 
    if !items_to_display.is_empty() && selected_key_index < items_to_display.len() {
        list_state.select(Some(selected_key_index));
    }

    f.render_stateful_widget(list_widget, area, &mut list_state);
}

fn draw_value_display_panel(f: &mut Frame, app: &App, area: Rect) {
    let mut value_block_title = match &app.active_leaf_key_name {
        Some(name) => format!("Value: {} ({})", name, app.selected_key_type.as_deref().unwrap_or("N/A")),
        None => "Value".to_string(),
    };
    if app.is_value_view_focused {
        value_block_title.push_str(" [FOCUSED]");
    }

    let value_display_text = app.current_display_value.as_deref().unwrap_or("");

    let value_paragraph = Paragraph::new(value_display_text)
        .block(Block::default().borders(Borders::ALL).title(value_block_title)
            .border_style(if app.is_value_view_focused { Style::default().fg(Color::Cyan) } else { Style::default() }))
        .wrap(Wrap { trim: true })
        .scroll(app.value_view_scroll);
    f.render_widget(value_paragraph, area);
}

fn draw_footer_help(f: &mut Frame, app: &App, area: Rect) {
    let mut help_spans = vec![
        Span::styled("q: quit", Style::default().fg(Color::Yellow)),
        Span::raw(" | "),
        Span::styled("p: profiles", Style::default().fg(Color::Yellow)),
        Span::raw(" | "),
        Span::styled("j/k/↑/↓: nav", Style::default().fg(Color::Yellow)),
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
        Span::styled("d: del", Style::default().fg(Color::Yellow)), // Added delete help
    ];

    if app.is_search_active {
        help_spans.extend(vec![
            Span::raw(" | "),
            Span::styled("Esc: exit search", Style::default().fg(Color::Cyan)),
            Span::raw(" | "),
            Span::styled("Enter: activate", Style::default().fg(Color::Cyan)),
        ]);
    } else if app.show_delete_confirmation_dialog {
        help_spans = vec![
            Span::styled("Confirm Deletion: ", Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD)),
            Span::styled("[Y]es", Style::default().fg(Color::Green)),
            Span::raw(" / "),
            Span::styled("[N]o (Esc)", Style::default().fg(Color::Red)),
        ];
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

    let item_type = if app.deletion_is_folder { "folder" } else { "key" };
    let item_name = app.key_to_delete_display_name.as_deref().unwrap_or("unknown");

    let text = vec![
        Line::from(Span::styled(
            format!("Delete {} '{}'?", item_type, item_name),
            Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD)
        )).alignment(Alignment::Center),
        Line::from("").alignment(Alignment::Center),
        if app.deletion_is_folder {
            Line::from(Span::raw(format!("This will delete the folder and ALL keys within prefix '{}'.", app.prefix_to_delete.as_deref().unwrap_or("N/A")))).alignment(Alignment::Center)
        } else {
            Line::from(Span::raw(format!("This will permanently delete the key '{}'.", app.key_to_delete_full_path.as_deref().unwrap_or(item_name)))).alignment(Alignment::Center)
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