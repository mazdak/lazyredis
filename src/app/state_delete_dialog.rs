#[derive(Debug, Default, Clone)]
pub struct DeleteDialogState {
    pub show_confirmation_dialog: bool,
    pub key_to_delete_display_name: Option<String>,
    pub key_to_delete_full_path: Option<String>,
    pub prefix_to_delete: Option<String>,
    pub deletion_is_folder: bool,
    pub keys_to_delete: Vec<String>,
    pub is_multi_delete: bool,
}

impl DeleteDialogState {
    pub fn initiate_delete_selected_item(
        &mut self,
        selected_index: usize,
        visible_keys: &[(String, bool)],
        current_breadcrumb: &[String],
        key_delimiter: char,
        search_active: bool,
    ) {
        if search_active || selected_index >= visible_keys.len() {
            return;
        }
        let (display_name, is_folder) = visible_keys[selected_index].clone();
        self.key_to_delete_display_name = Some(display_name.clone());
        self.deletion_is_folder = is_folder;
        if is_folder {
            let mut prefix_parts = current_breadcrumb.to_vec();
            // Remove the trailing "/" from the display name to get the actual folder name
            prefix_parts.push(display_name.trim_end_matches('/').to_string());
            self.prefix_to_delete = Some(format!("{}{}", prefix_parts.join(&key_delimiter.to_string()), key_delimiter));
            self.key_to_delete_full_path = None;
        } else {
            let mut full_key_parts = current_breadcrumb.to_vec();
            full_key_parts.push(display_name);
            self.key_to_delete_full_path = Some(full_key_parts.join(&key_delimiter.to_string()));
            self.prefix_to_delete = None;
        }
        self.show_confirmation_dialog = true;
    }

    pub fn initiate_delete_multiple_items(
        &mut self,
        selected_indices: &std::collections::HashSet<usize>,
        visible_keys: &[(String, bool)],
        current_breadcrumb: &[String],
        key_delimiter: char,
        search_active: bool,
    ) {
        if search_active || selected_indices.is_empty() {
            return;
        }
        
        self.keys_to_delete.clear();
        self.is_multi_delete = true;
        self.deletion_is_folder = false;
        
        for &index in selected_indices {
            if index < visible_keys.len() {
                let (display_name, is_folder) = &visible_keys[index];
                if *is_folder {
                    let mut prefix_parts = current_breadcrumb.to_vec();
                    prefix_parts.push(display_name.trim_end_matches('/').to_string());
                    let prefix = format!("{}{}", prefix_parts.join(&key_delimiter.to_string()), key_delimiter);
                    self.keys_to_delete.push(format!("folder:{}", prefix));
                } else {
                    let mut full_key_parts = current_breadcrumb.to_vec();
                    full_key_parts.push(display_name.clone());
                    self.keys_to_delete.push(full_key_parts.join(&key_delimiter.to_string()));
                }
            }
        }
        
        if !self.keys_to_delete.is_empty() {
            self.key_to_delete_display_name = Some(format!("{} items", self.keys_to_delete.len()));
            self.show_confirmation_dialog = true;
        }
    }

    pub fn cancel_delete_item(&mut self) {
        self.show_confirmation_dialog = false;
        self.key_to_delete_display_name = None;
        self.key_to_delete_full_path = None;
        self.prefix_to_delete = None;
        self.deletion_is_folder = false;
        self.keys_to_delete.clear();
        self.is_multi_delete = false;
    }
}
