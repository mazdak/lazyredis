use crate::app::App;
use tokio::task;
use crossclip::{Clipboard, SystemClipboard, ClipboardError};

// Helper function for ellipsizing copied content preview
fn ellipsize(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        format!("{}...", &text[..max_len.saturating_sub(3)])
    }
}

pub async fn copy_selected_key_name_to_clipboard(app: &mut App) {
    app.clipboard_status = None; // Clear previous status
    let mut key_to_copy: Option<String> = None;

    // Prioritize the currently selected item in the visible key list
    if app.selected_visible_key_index < app.visible_keys_in_current_view.len() {
        let (display_name, _is_folder) = app.visible_keys_in_current_view[app.selected_visible_key_index].clone();
        // For folders, display_name often ends with '/'. We might want to trim that.
        key_to_copy = Some(display_name.trim_end_matches('/').to_string());
    }
    
    if let Some(name) = key_to_copy {
        let name_clone_for_closure = name.clone();
        let result: Result<Result<String, ClipboardError>, tokio::task::JoinError> = task::spawn_blocking(move || {
            let clipboard = SystemClipboard::new().map_err(|e| e)?; // Propagate error if SystemClipboard::new() fails
            clipboard.set_string_contents(name_clone_for_closure.clone())?;
            Ok(name_clone_for_closure)
        }).await;

        match result {
            Ok(Ok(copied_name)) => app.clipboard_status = Some(format!("Copied key name '{}' to clipboard!", copied_name)),
            Ok(Err(e)) => app.clipboard_status = Some(format!("Failed to access clipboard: {}", e)),
            Err(e) => app.clipboard_status = Some(format!("Clipboard task failed: {}", e)),
        }
    } else {
        app.clipboard_status = Some("No key selected to copy".to_string());
    }
}

pub async fn copy_selected_key_value_to_clipboard(app: &mut App) {
    app.clipboard_status = None; // Clear previous status
    let mut value_to_copy: Option<String> = None;

    if app.is_value_view_focused {
        // Value view is focused: copy the selected sub-item
        if let Some(lines) = &app.displayed_value_lines {
            if !lines.is_empty() && app.selected_value_sub_index < lines.len() {
                value_to_copy = Some(lines[app.selected_value_sub_index].clone());
            } else {
                app.clipboard_status = Some("No specific value item selected to copy.".to_string());
            }
        } else {
            app.clipboard_status = Some("No multi-line value items to select from.".to_string());
        }
    } else {
        // Key view is focused (or no specific sub-item focus): copy the whole value representation
        if app.active_leaf_key_name.is_some() {
            if let Some(lines) = &app.displayed_value_lines {
                if !lines.is_empty() {
                    value_to_copy = Some(lines.join("\n"));
                } else {
                    if let Some(cvd) = &app.current_display_value {
                        if !cvd.starts_with("(") || !cvd.ends_with(")") {
                            value_to_copy = Some(cvd.clone());
                        } else {
                            app.clipboard_status = Some(format!("Value is an empty placeholder: {}", cvd));
                        }
                    } else {
                         app.clipboard_status = Some("No value content to copy (displayed_value_lines is empty).".to_string());
                    }
                }
            } else if let Some(s_val) = &app.current_display_value {
                value_to_copy = Some(s_val.clone());
            } else {
                app.clipboard_status = Some("No value available to copy for the selected key.".to_string());
            }
        } else {
            app.clipboard_status = Some("No active key selected to copy value from.".to_string());
        }
    }

    if let Some(value_str) = value_to_copy {
        let value_str_clone_for_closure = value_str.clone();
        let result: Result<Result<String, ClipboardError>, tokio::task::JoinError> = task::spawn_blocking(move || {
            let clipboard = SystemClipboard::new().map_err(|e| e)?; // Propagate error
            clipboard.set_string_contents(value_str_clone_for_closure.clone())?;
            Ok(value_str_clone_for_closure)
        }).await;

        match result {
            Ok(Ok(copied_value)) => app.clipboard_status = Some(format!("Copied to clipboard: {}", ellipsize(&copied_value, 50))),
            Ok(Err(e)) => app.clipboard_status = Some(format!("Failed to access clipboard: {}", e)),
            Err(e) => app.clipboard_status = Some(format!("Clipboard task failed: {}", e)),
        }
    }
} 