use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Clear, Wrap},
    Frame,
};
use crate::app::App;

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
    // Overall layout: status bar, main content, help bar
    let overall_layout_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Status bar
            Constraint::Min(0),    // Main content area
            Constraint::Length(1), // Help bar
        ].as_ref())
        .split(f.area());

    let status_area = overall_layout_chunks[0];
    let main_content_area = overall_layout_chunks[1];
    let help_area = overall_layout_chunks[2];

    // 1. Status Bar
    let status_text = app.clipboard_status.as_deref().unwrap_or(&app.connection_status);
    let status_paragraph = Paragraph::new(status_text);
    f.render_widget(status_paragraph, status_area);
    // Consider clearing app.clipboard_status here or in App after a short delay/next event

    // 2. Main Content (either profile selector or DB view)
    if app.is_profile_selector_active {
        let popup_area = centered_rect(60, 50, main_content_area); // Centered within main_content_area

        let profiles: Vec<ListItem> = app
            .profiles
            .iter()
            .map(|p| ListItem::new(p.name.as_str()))
            .collect();

        let profiles_list = List::new(profiles)
            .block(Block::default().borders(Borders::ALL).title("Select Connection Profile"))
            .highlight_style(
                Style::default()
                    .bg(Color::LightBlue)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");

        let mut list_state = ListState::default();
        list_state.select(Some(app.selected_profile_list_index));
        
        f.render_widget(Clear, popup_area); 
        f.render_stateful_widget(profiles_list, popup_area, &mut list_state);
    } else {
        // Main view: DB list on left, Key List + Key Value on right
        let main_horizontal_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(20), Constraint::Percentage(80)].as_ref())
            .split(main_content_area);
        
        let db_pane_area = main_horizontal_chunks[0];
        let right_pane_area = main_horizontal_chunks[1];

        // DB List Pane (left)
        let db_list_title = if app.is_key_view_focused || app.is_value_view_focused { 
            "Databases" 
        } else { 
            "Databases (Focused)" 
        };
        let mut db_items: Vec<ListItem> = Vec::new();
        if app.db_count > 0 { 
            for i in 0..app.db_count {
                let item_text = format!("DB {}", i);
                db_items.push(ListItem::new(item_text));
            }
        }
        let db_list = List::new(db_items)
            .block(Block::default().borders(Borders::ALL).title(db_list_title))
            .highlight_style(
                Style::default()
                    .bg(if app.is_key_view_focused || app.is_value_view_focused { Color::DarkGray } else { Color::LightGreen })
                    .add_modifier(Modifier::BOLD)
            )
            .highlight_symbol(if app.is_key_view_focused || app.is_value_view_focused { "  " } else { "-> " });
        
        let mut db_list_state = ListState::default();
        if app.db_count > 0 { 
            db_list_state.select(Some(app.selected_db_index));
        }

        f.render_stateful_widget(db_list, db_pane_area, &mut db_list_state);

        // Right Pane: Key List (top) and Key Value (bottom)
        let right_vertical_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(50), // Key List
                Constraint::Percentage(50), // Key Value
            ].as_ref())
            .split(right_pane_area);
        
        let key_list_area = right_vertical_chunks[0];
        let key_value_area = right_vertical_chunks[1];

        // Key List (top-right)
        let mut key_view_base_title = format!("Keys: {}", app.current_breadcrumb.join(&app.key_delimiter.to_string()));
        if app.is_search_active {
            key_view_base_title = format!("Search Keys ({}): {} [/]", app.search_query, app.current_breadcrumb.join(&app.key_delimiter.to_string()));
        }

        let key_view_title = if app.is_key_view_focused { 
            format!("{} (Focused)", key_view_base_title) 
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
        let key_list_widget = List::new(key_items) // Renamed to avoid conflict
            .block(Block::default().borders(Borders::ALL).title(key_view_title))
            .highlight_style(
                Style::default()
                    .bg(if app.is_key_view_focused { Color::Yellow } else { Color::DarkGray })
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(if app.is_key_view_focused { ">> " } else { "  " });
        let mut key_list_state = ListState::default();
        if !items_to_display.is_empty() {
             key_list_state.select(Some(selected_key_index));
        }
        f.render_stateful_widget(key_list_widget, key_list_area, &mut key_list_state);

        // Key Value Display (bottom-right)
        let mut value_block_title = match &app.active_leaf_key_name {
            Some(name) => format!("Value: {} ({})", name, app.selected_key_type.as_deref().unwrap_or("N/A")),
            None => "Value".to_string(),
        };
        if app.is_value_view_focused {
            value_block_title.push_str(" (Focused)");
        }

        // Use the pre-formatted current_display_value from App
        let value_display_text = app.current_display_value.as_deref().unwrap_or("");

        let value_paragraph = Paragraph::new(value_display_text)
            .block(Block::default().borders(Borders::ALL).title(value_block_title)
                .border_style(if app.is_value_view_focused { Style::default().fg(Color::Cyan) } else { Style::default() }))
            .wrap(Wrap { trim: true })
            .scroll(app.value_view_scroll);
        f.render_widget(value_paragraph, key_value_area);
    }

    // 3. Help Bar
    let help_text = if app.is_profile_selector_active {
        "Help: Esc/p: Close | j/k/↑/↓: Nav | Enter: Select | q: Quit"
    } else if app.is_search_active {
        "Search: Esc: Cancel | Enter: Activate | ↑/↓: Nav | Type to filter"
    } else if app.is_value_view_focused {
        "Help: Tab: Focus DBs | Shift+Tab: Focus Keys | ↑/↓/PgUp/PgDn: Scroll | 'y': Copy Name | 'Y': Copy Value | q: Quit"
    } else if app.is_key_view_focused {
        "Help: Tab: Focus Value | Shift+Tab: Focus DBs | j/k/↑/↓: Nav Keys | Enter: Select | Backspace: Up | 'y': Copy Name | 'Y': Copy Value | q: Quit"
    } else {
        "Help: Tab: Focus Keys | Shift+Tab: Focus Value (N/A) | j/k/↑/↓: Nav DBs | Enter: Select DB | p: Profiles | q: Quit"
    };
    let help_paragraph = Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray)); // Optional: style the help text
    f.render_widget(help_paragraph, help_area);
} 