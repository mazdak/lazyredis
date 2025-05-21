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

    if app.is_profile_selector_active {
        // Profile selector takes over the main view
        draw_profile_selector_modal(f, app);
        // Still draw footer and status if they are separate from the main content area that modal covers
        draw_footer_help(f, app, main_layout[2]); // Assuming footer is outside modal coverage or desired
        draw_clipboard_status(f, app, main_layout[3]);
    } else {
        // Normal view
        let content_layout_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
            .split(main_layout[1]);

        draw_profiles_or_db_list(f, app, main_layout[0]);
        draw_key_list_panel(f, app, content_layout_chunks[0]);
        draw_value_display_panel(f, app, content_layout_chunks[1]);
        
        draw_footer_help(f, app, main_layout[2]);
        draw_clipboard_status(f, app, main_layout[3]);

        if app.show_delete_confirmation_dialog {
            draw_delete_confirmation_dialog(f, app);
        }
        if app.is_command_prompt_active {
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

fn draw_key_list_panel(f: &mut Frame, app: &App, area: Rect) {
    let mut key_view_base_title = format!("Keys: {}", app.current_breadcrumb.join(&app.key_delimiter.to_string()));
    if app.is_search_active {
        // For global search, breadcrumb is less relevant in title, show search query
        key_view_base_title = format!("Search Results (Global): {}", app.search_query);
    }

    let key_view_title = if app.is_key_view_focused {
        format!("{} [FOCUSED]", key_view_base_title)
    } else {
        key_view_base_title
    };

    let key_items: Vec<ListItem> = if app.is_search_active {
        app.filtered_keys_in_current_view
            .iter()
            .map(|full_key_name| ListItem::new(full_key_name.as_str()))
            .collect()
    } else {
        app.visible_keys_in_current_view
            .iter()
            .map(|(name, _is_folder)| ListItem::new(name.as_str()))
            .collect()
    };

    let selected_key_index = if app.is_search_active {
        app.selected_filtered_key_index
    } else {
        app.selected_visible_key_index
    };

    let mut list_state = ListState::default(); 
    // Check emptiness and length before moving key_items
    let is_list_empty = key_items.is_empty();
    let list_len = key_items.len();

    let list_widget = List::new(key_items) // key_items is moved here
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
    let mut value_block_title = match &app.active_leaf_key_name {
        Some(name) => format!("Value: {} ({})", name, app.selected_key_type.as_deref().unwrap_or("N/A")),
        None => "Value".to_string(),
    };
    if app.is_value_view_focused {
        value_block_title.push_str(" [FOCUSED]");
    }

    let block = Block::default().borders(Borders::ALL).title(value_block_title)
        .border_style(if app.is_value_view_focused { Style::default().fg(Color::Cyan) } else { Style::default() });

    if let Some(lines) = &app.displayed_value_lines {
        let items: Vec<ListItem> = lines.iter().map(|s| ListItem::new(s.as_str())).collect();
        let mut list_state = ListState::default();
        if !items.is_empty() && app.selected_value_sub_index < items.len() {
            list_state.select(Some(app.selected_value_sub_index));
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
        
        // The List widget itself doesn't directly use app.value_view_scroll in the same way Paragraph does.
        // Ratatui's List state and drawing logic attempt to keep the selected item in view.
        // If fine-grained manual scrolling of a non-selected list is ever needed, it's more complex.
        // For now, relying on selection driving the view is standard.
        f.render_stateful_widget(list_widget, area, &mut list_state);

    } else {
        let value_display_text = app.current_display_value.as_deref().unwrap_or("");
        let value_paragraph = Paragraph::new(value_display_text)
            .block(block)
            .wrap(Wrap { trim: true })
            .scroll(app.value_view_scroll); // Keep scroll for simple paragraph display
        f.render_widget(value_paragraph, area);
    }
}

fn draw_footer_help(f: &mut Frame, app: &App, area: Rect) {
    let mut help_spans = vec![
        Span::styled("q: quit", Style::default().fg(Color::Yellow)),
        Span::raw(" | "),
        Span::styled("p: profiles", Style::default().fg(Color::Yellow)),
        Span::raw(" | "),
        Span::styled("j/k/↑/↓: nav keys/vals", Style::default().fg(Color::Yellow)), // Updated nav help
        Span::raw(" | "),
        Span::styled("PgUp/PgDn: page nav vals", Style::default().fg(Color::Yellow)), // Added page nav for values
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
            let style = if idx == app.selected_profile_list_index {
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
    list_state.select(Some(app.selected_profile_list_index));

    f.render_stateful_widget(list_widget, area, &mut list_state);
}

fn draw_command_prompt_modal(f: &mut Frame, app: &App) {
    let area = centered_rect(70, 30, f.area());
    f.render_widget(Clear, area);

    let input_line = format!("CMD> {}", app.command_input);
    let output = app.command_output.as_deref().unwrap_or("");

    let text = vec![
        Line::from(Span::styled(
            "Custom Command Prompt - use at your own risk!",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ))
        .alignment(Alignment::Center),
        Line::from("").alignment(Alignment::Center),
        Line::from(input_line),
        Line::from("").alignment(Alignment::Center),
        Line::from(output),
    ];

    let block = Block::default().borders(Borders::ALL).title("Command Prompt (: to open, Esc to close)");
    let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: false });
    f.render_widget(paragraph, area);
}
