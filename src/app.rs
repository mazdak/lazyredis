use crate::config::ConnectionProfile;
use redis::{Client, Connection, Value, ConnectionLike};
use std::collections::HashMap;
use crossclip::{Clipboard, SystemClipboard}; // Changed from crossclip::Clipboard for directness, though original likely worked.
use fuzzy_matcher::FuzzyMatcher; // Added import

// StreamEntry struct definition (if it's here, keep it)
// ... (ensure StreamEntry struct is here if it was in the original app_C file)
#[derive(Debug, Clone)] // Assuming StreamEntry was here, it should be kept
pub struct StreamEntry { 
    pub id: String,
    pub fields: Vec<(String, String)>,
}

// KeyTreeNode enum definition (if it's here, keep it)
// ... (ensure KeyTreeNode enum is here if it was in the original app_C file)
#[derive(Debug, Clone)]
pub enum KeyTreeNode {
    Folder(HashMap<String, KeyTreeNode>),
    Leaf { full_key_name: String },
}

pub struct App {
    pub selected_db_index: usize,
    pub db_count: u8,
    pub redis_client: Option<Client>,
    pub redis_connection: Option<Connection>,
    pub connection_status: String,
    pub profiles: Vec<ConnectionProfile>,
    pub current_profile_index: usize,
    pub is_profile_selector_active: bool,
    pub selected_profile_list_index: usize,
    pub raw_keys: Vec<String>,
    pub key_tree: HashMap<String, KeyTreeNode>,
    pub current_breadcrumb: Vec<String>,
    pub visible_keys_in_current_view: Vec<(String, bool)>,
    pub selected_visible_key_index: usize,
    pub key_delimiter: char,
    pub is_key_view_focused: bool,
    pub active_leaf_key_name: Option<String>,
    pub selected_key_type: Option<String>,
    pub selected_key_value: Option<String>, // For simple string values or error messages
    pub selected_key_value_hash: Option<Vec<(String, String)>>,
    pub selected_key_value_zset: Option<Vec<(String, f64)>>, // Or Vec<(String, f64)>, using String for score for now
    pub selected_key_value_list: Option<Vec<String>>,
    pub selected_key_value_set: Option<Vec<String>>,
    pub selected_key_value_stream: Option<Vec<StreamEntry>>,
    pub is_value_view_focused: bool,
    pub value_view_scroll: (u16, u16),
    pub clipboard_status: Option<String>,
    pub current_display_value: Option<String>,
    pub displayed_value_lines: Option<Vec<String>>, // For multi-line values (hashes, lists, etc.)
    pub selected_value_sub_index: usize, // Index for selected item in displayed_value_lines

    // Fuzzy Search State
    pub is_search_active: bool,
    pub search_query: String,
    pub filtered_keys_in_current_view: Vec<String>,
    pub selected_filtered_key_index: usize,

    // Delete Confirmation State
    pub show_delete_confirmation_dialog: bool,
    pub key_to_delete_display_name: Option<String>,
    pub key_to_delete_full_path: Option<String>, // For leaf keys
    pub prefix_to_delete: Option<String>,      // For folders
    pub deletion_is_folder: bool,
}

// REMOVE clipboard functions from global scope if they are here.
// pub fn copy_selected_key_name_to_clipboard(&mut self) { ... }
// pub fn copy_selected_key_value_to_clipboard(&mut self) { ... }

impl App {
    // PASTE the two clipboard functions here
    pub fn copy_selected_key_name_to_clipboard(&mut self) {
        self.clipboard_status = None; // Clear previous status
        let mut key_to_copy: Option<String> = None;

        // Prioritize the currently selected item in the visible key list
        if self.selected_visible_key_index < self.visible_keys_in_current_view.len() {
            let (display_name, _is_folder) = self.visible_keys_in_current_view[self.selected_visible_key_index].clone();
            // For folders, display_name often ends with '/'. We might want to trim that.
            key_to_copy = Some(display_name.trim_end_matches('/').to_string());
        }
        
        // Fallback or alternative: if a leaf key is fully active, its name is also a candidate
        // This could be a user preference or a more complex logic. For now, let's see if the above is sufficient.
        // If key_to_copy is still None AND an active_leaf_key_name exists, consider using it.
        // However, active_leaf_key_name should correspond to a selected key from visible_keys if UI is consistent.
        // The current logic for `display_name` from `visible_keys_in_current_view` should provide the most relevant name.

        if let Some(name) = key_to_copy {
            match SystemClipboard::new() { // Using SystemClipboard directly
                Ok(clipboard) => { // clipboard needs to be mut for set_string_contents
                    match clipboard.set_string_contents(name.clone()) {
                        Ok(_) => self.clipboard_status = Some(format!("Copied key name '{}' to clipboard!", name)),
                        Err(e) => self.clipboard_status = Some(format!("Failed to copy key name to clipboard: {}", e)),
                    }
                }
                Err(e) => self.clipboard_status = Some(format!("Failed to access clipboard: {}", e)),
            }
        } else {
            self.clipboard_status = Some("No key selected to copy".to_string());
        }
    }

    pub fn copy_selected_key_value_to_clipboard(&mut self) {
        self.clipboard_status = None; // Clear previous status
        let mut value_to_copy: Option<String> = None;

        if self.is_value_view_focused {
            // Value view is focused: copy the selected sub-item
            if let Some(lines) = &self.displayed_value_lines {
                if !lines.is_empty() && self.selected_value_sub_index < lines.len() {
                    value_to_copy = Some(lines[self.selected_value_sub_index].clone());
                } else {
                    self.clipboard_status = Some("No specific value item selected to copy.".to_string());
                }
            } else {
                self.clipboard_status = Some("No multi-line value items to select from.".to_string());
            }
        } else {
            // Key view is focused (or no specific sub-item focus): copy the whole value representation
            if self.active_leaf_key_name.is_some() {
                if let Some(lines) = &self.displayed_value_lines {
                    if !lines.is_empty() {
                        value_to_copy = Some(lines.join("\n"));
                    } else {
                        // This case might occur if a complex type is genuinely empty AND update_current_display_value
                        // decided to set displayed_value_lines = Some(vec![]) instead of current_display_value.
                        // For instance, an empty hash might be represented by current_display_value = "(empty hash)".
                        // Let's try current_display_value if displayed_value_lines is Some but empty.
                        if let Some(cvd) = &self.current_display_value {
                             // Check if it's a placeholder like "(empty list)" rather than a real value
                            if !cvd.starts_with("(") || !cvd.ends_with(")") {
                                value_to_copy = Some(cvd.clone());
                            } else {
                                self.clipboard_status = Some(format!("Value is an empty placeholder: {}", cvd));
                            }
                        } else {
                             self.clipboard_status = Some("No value content to copy (displayed_value_lines is empty).".to_string());
                        }
                    }
                } else if let Some(s_val) = &self.current_display_value {
                    // This handles simple strings, (nil), or error messages in current_display_value
                    value_to_copy = Some(s_val.clone());
                } else {
                    self.clipboard_status = Some("No value available to copy for the selected key.".to_string());
                }
            } else {
                self.clipboard_status = Some("No active key selected to copy value from.".to_string());
            }
        }

        if let Some(value_str) = value_to_copy {
            match SystemClipboard::new() {
                Ok(clipboard) => { 
                    match clipboard.set_string_contents(value_str.clone()) {
                        Ok(_) => self.clipboard_status = Some(format!("Copied to clipboard: {}", ellipsize(&value_str, 50))),
                        Err(e) => self.clipboard_status = Some(format!("Failed to copy value to clipboard: {}", e)),
                    }
                }
                Err(e) => self.clipboard_status = Some(format!("Failed to access clipboard: {}", e)),
            }
        } // If value_to_copy is None, a status message should have already been set.
    }

    pub fn new(initial_url: &str, initial_profile_name: &str, profiles: Vec<ConnectionProfile>) -> App {
        let mut app = App {
            selected_db_index: 0,
            db_count: 16, // Default Redis DB count
            redis_client: None,
            redis_connection: None,
            connection_status: format!("Connecting to {} ({})...", initial_profile_name, initial_url),
            profiles,
            current_profile_index: 0,
            is_profile_selector_active: false,
            selected_profile_list_index: 0,
            
            raw_keys: Vec::new(),
            key_tree: HashMap::new(),
            current_breadcrumb: Vec::new(),
            visible_keys_in_current_view: Vec::new(),
            selected_visible_key_index: 0,
            key_delimiter: ':',
            is_key_view_focused: false, 
            active_leaf_key_name: None, 
            selected_key_type: None,    
            selected_key_value: None,   
            selected_key_value_hash: None,
            selected_key_value_zset: None, 
            selected_key_value_list: None,
            selected_key_value_set: None,
            selected_key_value_stream: None,
            is_value_view_focused: false, 
            value_view_scroll: (0, 0),    
            clipboard_status: None, 
            current_display_value: None, 
            displayed_value_lines: None,
            selected_value_sub_index: 0,

            // Fuzzy Search State
            is_search_active: false,
            search_query: String::new(),
            filtered_keys_in_current_view: Vec::new(),
            selected_filtered_key_index: 0,

            // Delete Confirmation State
            show_delete_confirmation_dialog: false,
            key_to_delete_display_name: None,
            key_to_delete_full_path: None,
            prefix_to_delete: None,
            deletion_is_folder: false,
        };

        if !app.profiles.is_empty() {
            app.current_profile_index = app.profiles.iter().position(|p| p.url == initial_url).unwrap_or(0);
            app.selected_profile_list_index = app.current_profile_index;
        }

        app.connect_to_profile(app.current_profile_index);
        app
    }

    fn connect_to_profile(&mut self, profile_index: usize) {
        if profile_index >= self.profiles.len() {
            self.connection_status = format!("Error: Profile index {} out of bounds.", profile_index);
            self.redis_client = None;
            self.redis_connection = None;
            return;
        }

        let profile = &self.profiles[profile_index];
        self.connection_status = format!("Connecting to {} ({})...", profile.name, profile.url);
        self.selected_db_index = profile.db.map_or(self.selected_db_index, |db| db as usize); // Use profile's DB if set, else keep current app selection

        match Client::open(profile.url.as_str()) {
            Ok(client) => {
                self.redis_client = Some(client);
                match self.redis_client.as_ref().unwrap().get_connection() {
                    Ok(mut connection) => {
                        // Select the database
                        let db_to_select = profile.db.unwrap_or(self.selected_db_index as u8);
                        match redis::cmd("SELECT").arg(db_to_select).query::<()>(&mut connection) {
                            Ok(_) => {
                                self.selected_db_index = db_to_select as usize; // Ensure app state matches selected DB
                                self.redis_connection = Some(connection);
                                self.connection_status = format!(
                                    "Connected to {} ({}), DB {}",
                                    profile.name, profile.url, self.selected_db_index
                                );
                                self.fetch_keys_and_build_tree(); // Fetch keys for the new connection
                            }
                            Err(e) => {
                                self.connection_status = format!(
                                    "Failed to select DB {} on {}: {}",
                                    db_to_select, profile.name, e
                                );
                                self.redis_client = None;
                                self.redis_connection = None;
                            }
                        }
                    }
                    Err(e) => {
                        self.connection_status =
                            format!("Failed to get connection for {}: {}", profile.name, e);
                        self.redis_client = None;
                        self.redis_connection = None;
                    }
                }
            }
            Err(e) => {
                self.connection_status = format!("Failed to create client for {}: {}", profile.name, e);
                self.redis_client = None;
                self.redis_connection = None;
            }
        }
    }

    fn clear_selected_key_info(&mut self) {
        self.active_leaf_key_name = None;
        self.selected_key_type = None;
        self.selected_key_value = None;
        self.selected_key_value_hash = None;
        self.selected_key_value_zset = None;
        self.selected_key_value_list = None;
        self.selected_key_value_set = None;
        self.selected_key_value_stream = None;
        self.value_view_scroll = (0, 0);
        self.is_value_view_focused = false;
        self.current_display_value = None;
        self.displayed_value_lines = None;
        self.selected_value_sub_index = 0;
    }

    // Fetches all keys from Redis using SCAN and incrementally updates the tree and view.
    fn fetch_keys_and_build_tree(&mut self) {
        self.raw_keys.clear();
        self.key_tree.clear();
        self.current_breadcrumb.clear();
        self.visible_keys_in_current_view.clear();
        self.selected_visible_key_index = 0;
        self.clear_selected_key_info();

        if let Some(mut con) = self.redis_connection.take() {
            self.connection_status = format!("Fetching keys from DB {}...", self.selected_db_index);
            let mut cursor: u64 = 0;
            // Iteratively SCAN to avoid blocking Redis and allow incremental UI updates
            loop {
                match redis::cmd("SCAN")
                    .arg(cursor)
                    .arg("MATCH").arg("*")
                    .arg("COUNT").arg(1000)
                    .query::<(u64, Vec<String>)>(&mut con)
                {
                    Ok((next_cursor, batch)) => {
                        cursor = next_cursor;
                        self.raw_keys.extend(batch);
                        if !self.raw_keys.is_empty() {
                            self.parse_keys_to_tree();
                            self.update_visible_keys();
                        }
                        self.connection_status = format!(
                            "Connected to DB {}. Found {} keys (cursor {}).",
                            self.selected_db_index,
                            self.raw_keys.len(),
                            cursor
                        );
                        if cursor == 0 {
                            break;
                        }
                    }
                    Err(e) => {
                        self.connection_status = format!("Failed during SCAN: {}", e);
                        break;
                    }
                }
            }
            if self.raw_keys.is_empty() {
                self.connection_status = format!("Connected to DB {}. No keys found.", self.selected_db_index);
            } else {
                self.connection_status = format!(
                    "Connected to DB {}. Found {} keys. Displaying {} top-level items.",
                    self.selected_db_index,
                    self.raw_keys.len(),
                    self.visible_keys_in_current_view.len()
                );
            }
            self.redis_connection = Some(con);
        } else {
            self.connection_status = "Not connected. Cannot fetch keys.".to_string();
        }
    }
    
    // Placeholder for parse_keys_to_tree if it doesn't exist or needs adjustment
    // Ensure this method correctly populates self.key_tree from self.raw_keys
    fn parse_keys_to_tree(&mut self) {
        let mut tree = HashMap::new();
        for full_key_name in &self.raw_keys {
            let parts: Vec<&str> = full_key_name.split(self.key_delimiter).collect();
            let mut current_level = &mut tree;
            for (i, part) in parts.iter().enumerate() {
                if i == parts.len() - 1 { // Last part is a leaf
                    current_level.entry(part.to_string()).or_insert_with(|| {
                        KeyTreeNode::Leaf {
                            full_key_name: full_key_name.to_string(),
                        }
                    });
                } else { // Intermediate part is a folder
                    let node = current_level
                        .entry(part.to_string())
                        .or_insert_with(|| KeyTreeNode::Folder(HashMap::new()));

                    // If the node was a Leaf, but we need a Folder (because it's an intermediate part),
                    // we must ensure it becomes a Folder.
                    // The or_insert_with above only inserts if vacant. If it was occupied by a Leaf, it's still a Leaf.
                    if matches!(node, KeyTreeNode::Leaf { .. }) {
                        *node = KeyTreeNode::Folder(HashMap::new());
                    }

                    // Now, it must be a Folder, either newly inserted, pre-existing, or just promoted.
                    if let KeyTreeNode::Folder(sub_map) = node {
                        current_level = sub_map;
                    } else {
                        // This should be unreachable if the logic above is correct.
                        // If `node` was a `Leaf` it should have been replaced by a `Folder`.
                        unreachable!("Node should have been converted to a Folder if it was a Leaf");
                    }
                }
            }
        }
        self.key_tree = tree;
    }


    pub fn previous_key_in_view(&mut self) {
        if !self.visible_keys_in_current_view.is_empty() {
            let new_idx = if self.selected_visible_key_index > 0 {
                self.selected_visible_key_index - 1
            } else {
                self.visible_keys_in_current_view.len() - 1
            };
            if new_idx != self.selected_visible_key_index {
                self.selected_visible_key_index = new_idx;
                self.clear_selected_key_info(); // Clear old value and value view state
            }
        }
    }

    pub fn activate_selected_key(&mut self) {
        if self.selected_visible_key_index < self.visible_keys_in_current_view.len() {
            let (display_name, is_folder) = self.visible_keys_in_current_view[self.selected_visible_key_index].clone();
            self.clear_selected_key_info(); // Clear previous key's info

            if is_folder {
                let folder_name = display_name.trim_end_matches('/').to_string();
                self.current_breadcrumb.push(folder_name);
                self.update_visible_keys(); 
            } else {
                // This is a leaf key. Find its full name from the key_tree.
                let mut current_node_map_for_leaf = &self.key_tree; 
                for segment in &self.current_breadcrumb {
                    if let Some(KeyTreeNode::Folder(sub_map)) = current_node_map_for_leaf.get(segment) {
                        current_node_map_for_leaf = sub_map;
                    } else {
                        self.selected_key_value = Some("Error: Invalid breadcrumb path while finding leaf.".to_string());
                        self.update_current_display_value();
                        return;
                    }
                }
                
                let actual_full_key_name_opt: Option<String> = current_node_map_for_leaf
                    .get(&display_name)
                    .and_then(|node| match node {
                        KeyTreeNode::Leaf { full_key_name } => Some(full_key_name.clone()),
                        _ => None, 
                    });

                if let Some(actual_full_key_name) = actual_full_key_name_opt {
                    self.active_leaf_key_name = Some(actual_full_key_name.clone());
                    self.selected_key_type = Some("fetching...".to_string()); 
                    self.selected_value_sub_index = 0; // Reset sub-index
                    self.value_view_scroll = (0, 0); // Reset scroll

                    if let Some(mut con) = self.redis_connection.take() {
                        // Try GET first, as it's common
                        match redis::cmd("GET").arg(&actual_full_key_name).query::<Option<String>>(&mut con) {
                            Ok(Some(value)) => { // Successfully got a string
                                self.selected_key_type = Some("string".to_string());
                                self.selected_key_value = Some(value);
                            }
                            Ok(None) => { // Key exists but is nil (still string type contextually for GET)
                                self.selected_key_type = Some("string".to_string());
                                self.selected_key_value = Some("(nil)".to_string());
                            }
                            Err(e_get) => { // GET failed, could be WRONGTYPE or other error
                                let mut is_wrong_type_error = false;
                                if e_get.kind() == redis::ErrorKind::TypeError {
                                    is_wrong_type_error = true;
                                } else if e_get.kind() == redis::ErrorKind::ExtensionError {
                                    if let Some(code) = e_get.code() {
                                        if code == "WRONGTYPE" {
                                            is_wrong_type_error = true;
                                        }
                                    }
                                }

                                if is_wrong_type_error {
                                    // GET failed due to WRONGTYPE, so fetch the actual type
                                    match redis::cmd("TYPE").arg(&actual_full_key_name).query::<String>(&mut con) {
                                        Ok(key_type) => {
                                            self.selected_key_type = Some(key_type.clone());
                                            match key_type.as_str() {
                                                "hash" => self.fetch_and_set_hash_value(&actual_full_key_name, &mut con),
                                                "zset" => self.fetch_and_set_zset_value(&actual_full_key_name, &mut con),
                                                "list" => self.fetch_and_set_list_value(&actual_full_key_name, &mut con),
                                                "set" => self.fetch_and_set_set_value(&actual_full_key_name, &mut con),
                                                "stream" => self.fetch_and_set_stream_value(&actual_full_key_name, &mut con),
                                                _ => {
                                                    self.selected_key_value = Some(format!(
                                                        "Key is of type '{}'. Value view for this type not yet implemented.",
                                                        key_type
                                                    ));
                                                }
                                            }
                                        }
                                        Err(e_type) => { // TYPE command failed
                                            self.selected_key_type = Some("error (TYPE failed)".to_string());
                                            self.selected_key_value = Some(format!(
                                                "GET for '{}' failed (WRONGTYPE). Subsequent TYPE command also failed: {}",
                                                actual_full_key_name, e_type
                                            ));
                                        }
                                    }
                                } else { // GET failed for a reason other than WRONGTYPE
                                    self.selected_key_type = Some("error (GET failed)".to_string());
                                    self.selected_key_value = Some(format!(
                                        "Failed to GET key '{}': {} (Kind: {:?}, Code: {:?})", 
                                        actual_full_key_name, e_get, e_get.kind(), e_get.code()
                                    ));
                                }
                            }
                        }
                        self.redis_connection = Some(con); 
                    } else { // No Redis connection
                        self.selected_key_type = Some("error".to_string());
                        self.selected_key_value = Some("Error: No Redis connection to fetch key value.".to_string());
                    }
                } else { // Key not found as leaf in tree
                    self.selected_key_type = Some("error".to_string());
                    self.selected_key_value = Some(format!("Error: Key '{}' not found as leaf in tree at current level after traversal.", display_name));
                }
            }
        }
        self.update_current_display_value(); // Call update_current_display_value once at the end
    }

    // --- Helper methods for fetching and setting values for specific key types ---

    fn fetch_and_set_hash_value(&mut self, key_name: &str, con: &mut Connection) {
        match redis::cmd("HGETALL").arg(key_name).query::<Vec<String>>(con) {
            Ok(pairs) => {
                if pairs.is_empty() { 
                    self.selected_key_value_hash = Some(Vec::new());
                } else {
                    let mut hash_data: Vec<(String, String)> = Vec::new();
                    for chunk in pairs.chunks(2) {
                        if chunk.len() == 2 {
                            hash_data.push((chunk[0].clone(), chunk[1].clone()));
                        } else {
                            self.selected_key_value = Some(format!(
                                "HGETALL for '{}' (hash) returned malformed pair data.", key_name
                            ));
                            self.selected_key_value_hash = None; 
                            return; 
                        }
                    }
                    self.selected_key_value_hash = Some(hash_data);
                }
                self.selected_key_value = None; // Clear generic error/value holder if successful
            }
            Err(e) => {
                self.selected_key_value = Some(format!(
                    "Failed to HGETALL for '{}' (hash): {}", key_name, e
                ));
                self.selected_key_value_hash = None;
            }
        }
    }

    fn fetch_and_set_zset_value(&mut self, key_name: &str, con: &mut Connection) {
        match redis::cmd("ZRANGE").arg(key_name).arg(0).arg(-1).arg("WITHSCORES").query::<Vec<String>>(con) {
            Ok(pairs) => {
                if pairs.is_empty() { 
                    self.selected_key_value_zset = Some(Vec::new());
                } else {
                    let mut zset_data: Vec<(String, f64)> = Vec::new();
                    for chunk in pairs.chunks(2) {
                        if chunk.len() == 2 {
                            let member = chunk[0].clone();
                            match chunk[1].parse::<f64>() {
                                Ok(score) => zset_data.push((member, score)),
                                Err(_) => {
                                    self.selected_key_value = Some(format!(
                                        "ZRANGE for '{}' (zset) failed to parse score for member '{}'.", key_name, member
                                    ));
                                    self.selected_key_value_zset = None; 
                                    return;
                                }
                            }
                        } else {
                            self.selected_key_value = Some(format!(
                                "ZRANGE for '{}' (zset) returned malformed pair data.", key_name
                            ));
                            self.selected_key_value_zset = None; 
                            return;
                        }
                    }
                    self.selected_key_value_zset = Some(zset_data);
                }
                self.selected_key_value = None;
            }
            Err(e) => {
                self.selected_key_value = Some(format!(
                    "Failed to ZRANGE for '{}' (zset): {}", key_name, e
                ));
                self.selected_key_value_zset = None;
            }
        }
    }

    fn fetch_and_set_list_value(&mut self, key_name: &str, con: &mut Connection) {
        match redis::cmd("LRANGE").arg(key_name).arg(0).arg(-1).query::<Vec<String>>(con) {
            Ok(elements) => {
                self.selected_key_value_list = Some(elements);
                self.selected_key_value = None;
            }
            Err(e) => {
                self.selected_key_value = Some(format!(
                    "Failed to LRANGE for '{}' (list): {}", key_name, e
                ));
                self.selected_key_value_list = None;
            }
        }
    }

    fn fetch_and_set_set_value(&mut self, key_name: &str, con: &mut Connection) {
        match redis::cmd("SMEMBERS").arg(key_name).query::<Vec<String>>(con) {
            Ok(members) => {
                self.selected_key_value_set = Some(members);
                self.selected_key_value = None;
            }
            Err(e) => {
                self.selected_key_value = Some(format!(
                    "Failed to SMEMBERS for '{}' (set): {}", key_name, e
                ));
                self.selected_key_value_set = None;
            }
        }
    }

    fn fetch_and_set_stream_value(&mut self, key_name: &str, con: &mut Connection) {
        const GROUP_NAME: &str = "lazyredis_group"; // Consider making these configurable or dynamic
        const CONSUMER_NAME: &str = "lazyredis_consumer";

        // Attempt to create the group; ignore error if it already exists.
        // A more robust solution might check the error type specifically for "BUSYGROUP".
        let _ = redis::cmd("XGROUP")
            .arg("CREATE").arg(key_name).arg(GROUP_NAME).arg("$").arg("MKSTREAM")
            .query::<()>(con); // Error ignored for simplicity here.

        match con.req_command(&redis::cmd("XREADGROUP")
            .arg("GROUP").arg(GROUP_NAME).arg(CONSUMER_NAME)
            .arg("COUNT").arg(100) // Fetch more entries for a better view
            .arg("STREAMS").arg(key_name).arg(">")) // Read new messages for this consumer
        {
            Ok(Value::Nil) => {
                self.selected_key_value_stream = Some(Vec::new()); // No new messages
                self.selected_key_value = None;
            }
            Ok(Value::Array(mut stream_results)) => {
                if stream_results.is_empty() { // Should be covered by Nil, but as a safeguard
                    self.selected_key_value_stream = Some(Vec::new());
                    self.selected_key_value = None;
                    return;
                }
                // XREADGROUP returns an array of streams, each stream is [key_name, entries_array]
                let stream_data_val = stream_results.swap_remove(0); // We asked for one stream
                if let Value::Array(mut stream_data_parts) = stream_data_val {
                    if stream_data_parts.len() == 2 {
                        let entries_val = stream_data_parts.pop().unwrap(); // entries_array
                        if let Value::Array(entries_array) = entries_val {
                            let mut parsed_entries: Vec<StreamEntry> = Vec::new();
                            for entry_value in &entries_array {
                                if let Value::Array(entry_parts) = entry_value {
                                    if entry_parts.len() == 2 {
                                        if let (Value::BulkString(id_bytes), Value::Array(field_pairs_value)) = (&entry_parts[0], &entry_parts[1]) {
                                            let entry_id = String::from_utf8_lossy(id_bytes).to_string();
                                            let mut fields_map: Vec<(String, String)> = Vec::new();
                                            for i in (0..field_pairs_value.len()).step_by(2) {
                                                if i + 1 < field_pairs_value.len() {
                                                    if let (Value::BulkString(f_bytes), Value::BulkString(v_bytes)) = (&field_pairs_value[i], &field_pairs_value[i+1]) {
                                                        fields_map.push((
                                                            String::from_utf8_lossy(f_bytes).to_string(),
                                                            String::from_utf8_lossy(v_bytes).to_string()
                                                        ));
                                                    } else { /* Malformed field/value pair */ break; }
                                                } else { /* Malformed field pairs (odd number) */ break; }
                                            }
                                            parsed_entries.push(StreamEntry { id: entry_id, fields: fields_map });
                                        } else { /* Malformed ID or fields structure */ }
                                    } else { /* Malformed entry structure */ }
                                } else { /* Stream entry not an array */ }
                            }
                            if parsed_entries.len() < entries_array.len() && self.selected_key_value.is_none() {
                                // Partial parse, indicate an issue
                                self.selected_key_value = Some(format!("Partially parsed stream data for '{}'. Some entries might be malformed.", key_name));
                            } else if self.selected_key_value.is_none() {
                                self.selected_key_value = None; // Clear if no errors during parsing.
                            }
                            self.selected_key_value_stream = Some(parsed_entries);
                            return; // Successfully parsed
                        }
                    }
                }
                // If we reach here, parsing the XREADGROUP response failed at some point.
                self.selected_key_value = Some(format!("Malformed XREADGROUP response structure for '{}' (stream).", key_name));
                self.selected_key_value_stream = None;
            }
            Ok(_) => { // Any other Value type is unexpected
                self.selected_key_value = Some(format!("Unexpected XREADGROUP response type for '{}' (stream).", key_name));
                self.selected_key_value_stream = None;
            }
            Err(e) => {
                self.selected_key_value = Some(format!(
                    "Failed to XREADGROUP for '{}' (stream): {}", key_name, e
                ));
                self.selected_key_value_stream = None;
            }
        }
    }

    // Method to update self.current_display_value based on current key type and value
    fn update_current_display_value(&mut self) {
        self.current_display_value = None; // Clear simple display value
        self.displayed_value_lines = None; // Clear line-based display value
        self.selected_value_sub_index = 0; // Reset sub-index
        self.value_view_scroll = (0,0); // Reset scroll for new value display

        if self.selected_key_type.as_deref() == Some("hash") {
            if let Some(hash_data) = &self.selected_key_value_hash {
                if hash_data.is_empty() {
                    self.current_display_value = Some("(empty hash)".to_string());
                } else {
                    self.displayed_value_lines = Some(
                        hash_data
                            .iter()
                            .map(|(k, v)| format!("{}: {}", k, v))
                            .collect::<Vec<String>>(),
                    );
                }
            } else {
                self.current_display_value = self.selected_key_value.clone(); // Fallback for HGETALL error
            }
        } else if self.selected_key_type.as_deref() == Some("zset") {
            if let Some(zset_data) = &self.selected_key_value_zset {
                if zset_data.is_empty() {
                    self.current_display_value = Some("(empty zset)".to_string());
                } else {
                    self.displayed_value_lines = Some(
                        zset_data
                            .iter()
                            .map(|(member, score)| format!("Score: {} - Member: {}", score, member))
                            .collect::<Vec<String>>(),
                    );
                }
            } else {
                self.current_display_value = self.selected_key_value.clone(); // Fallback for ZRANGE error
            }
        } else if self.selected_key_type.as_deref() == Some("list") { 
            if let Some(list_data) = &self.selected_key_value_list {
                if list_data.is_empty() {
                    self.current_display_value = Some("(empty list)".to_string());
                } else {
                    self.displayed_value_lines = Some(
                        list_data
                            .iter()
                            .enumerate()
                            .map(|(idx, val)| format!("{}: {}", idx, val))
                            .collect::<Vec<String>>(),
                    );
                }
            } else {
                self.current_display_value = self.selected_key_value.clone(); // Fallback for LRANGE error
            }
        } else if self.selected_key_type.as_deref() == Some("set") { 
            if let Some(set_data) = &self.selected_key_value_set {
                if set_data.is_empty() {
                    self.current_display_value = Some("(empty set)".to_string());
                } else {
                    let mut sorted_set_data = set_data.clone();
                    sorted_set_data.sort_unstable(); 
                    self.displayed_value_lines = Some(
                        sorted_set_data
                            .iter()
                            .map(|val| format!("- {}", val))
                            .collect::<Vec<String>>(),
                    );
                }
            } else {
                self.current_display_value = self.selected_key_value.clone(); // Fallback for SMEMBERS error
            }
        } else if self.selected_key_type.as_deref() == Some("stream") { 
            if let Some(stream_entries) = &self.selected_key_value_stream {
                if stream_entries.is_empty() {
                    self.current_display_value = Some("(empty stream or an error occurred fetching entries)".to_string());
                } else {
                    let mut lines: Vec<String> = Vec::new();
                    for entry in stream_entries {
                        lines.push(format!("ID: {}", entry.id));
                        if entry.fields.is_empty(){
                            lines.push("  (no fields)".to_string());
                        } else {
                            for (field, value) in &entry.fields {
                                lines.push(format!("  {}: {}", field, value));
                            }
                        }
                        lines.push("---".to_string()); // Separator between entries
                    }
                    if lines.last().map_or(false, |l| l == "---") { // Remove trailing separator
                        lines.pop();
                    }
                    self.displayed_value_lines = Some(lines);
                }
            } else {
                self.current_display_value = self.selected_key_value.clone();
            }
        } else {
            // For string type, errors, or "not implemented" messages for other types
            self.current_display_value = self.selected_key_value.clone();
        }
    }

    pub fn navigate_key_tree_up(&mut self) {
        if !self.current_breadcrumb.is_empty() {
            self.current_breadcrumb.pop();
            self.update_visible_keys(); // This will also reset selected_visible_key_index to 0
            self.clear_selected_key_info(); // Clear details of any previously selected leaf key
        }
        // If breadcrumb is empty, we are at the root, do nothing.
    }

    pub fn update_visible_keys(&mut self) {
        // This method should update self.visible_keys_in_current_view based on self.key_tree and self.current_breadcrumb
        // and reset self.selected_visible_key_index.
        // The actual logic for traversing self.key_tree based on self.current_breadcrumb
        // and populating self.visible_keys_in_current_view (Vec<(String, bool)>) needs to be implemented.
        // The bool indicates if the entry is a folder or a leaf.
        
        let mut current_level = &self.key_tree;
        for segment in &self.current_breadcrumb {
            if let Some(KeyTreeNode::Folder(next_level)) = current_level.get(segment) {
                current_level = next_level;
            } else {
                // Breadcrumb is invalid or points to a leaf prematurely.
                self.visible_keys_in_current_view.clear();
                self.selected_visible_key_index = 0;
                // Optionally, set an error status or log this.
                return;
            }
        }

        self.visible_keys_in_current_view = current_level.iter().map(|(name, node)| {
            let display_name = match node {
                KeyTreeNode::Folder(_) => format!("{}/", name),
                KeyTreeNode::Leaf { .. } => name.clone(),
            };
            (display_name, matches!(node, KeyTreeNode::Folder(_)))
        }).collect();

        // Sort, folders first, then by name
        self.visible_keys_in_current_view.sort_by(|(name_a, is_folder_a), (name_b, is_folder_b)| {
            match (is_folder_a, is_folder_b) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => name_a.cmp(name_b),
            }
        });
        self.selected_visible_key_index = 0;
    }

    pub fn toggle_profile_selector(&mut self) {
        self.is_profile_selector_active = !self.is_profile_selector_active;
        if self.is_profile_selector_active {
            // Reset selection to current profile when opening
            self.selected_profile_list_index = self.current_profile_index;
        }
    }

    pub fn next_profile_in_list(&mut self) {
        if !self.profiles.is_empty() {
            self.selected_profile_list_index = (self.selected_profile_list_index + 1) % self.profiles.len();
        }
    }

    pub fn previous_profile_in_list(&mut self) {
        if !self.profiles.is_empty() {
            if self.selected_profile_list_index > 0 {
                self.selected_profile_list_index -= 1;
            } else {
                self.selected_profile_list_index = self.profiles.len() - 1;
            }
        }
    }

    pub fn select_profile_and_connect(&mut self) {
        if self.selected_profile_list_index < self.profiles.len() {
            self.current_profile_index = self.selected_profile_list_index;
            self.is_profile_selector_active = false; 
            // The connect_to_profile method will update connection_status and other relevant fields.
            self.connect_to_profile(self.current_profile_index); 
        }
    }

    pub fn cycle_focus_backward(&mut self) {
        if self.is_value_view_focused {
            self.is_value_view_focused = false;
            self.is_key_view_focused = true;
        } else if self.is_key_view_focused {
            self.is_key_view_focused = false;
            // Focus DB view (implicit: neither key nor value view focused)
        } else { // DB view is focused or no focus
            self.is_value_view_focused = true; // Cycle to value view
        }
    }

    pub fn cycle_focus_forward(&mut self) {
        if self.is_key_view_focused {
            self.is_key_view_focused = false;
            self.is_value_view_focused = true;
        } else if self.is_value_view_focused {
            self.is_value_view_focused = false;
            // Focus DB view (implicit)
        } else { // DB view is focused or no focus
            self.is_key_view_focused = true;
        }
    }

    pub fn next_key_in_view(&mut self) {
        if !self.visible_keys_in_current_view.is_empty() {
            let new_idx = (self.selected_visible_key_index + 1) % self.visible_keys_in_current_view.len();
            if new_idx != self.selected_visible_key_index { // Check to avoid clearing info if selection doesn't change (e.g. single item list)
                self.selected_visible_key_index = new_idx;
                self.clear_selected_key_info(); 
                // Value update will be triggered by activate_selected_key or similar action by user
            }
        }
    }

    pub fn next_db(&mut self) {
        if self.db_count > 0 {
            self.selected_db_index = (self.selected_db_index + 1) % (self.db_count as usize);
            self.clear_selected_key_info();
            self.current_breadcrumb.clear();
            self.raw_keys.clear(); // Clear old keys
            self.key_tree.clear();
            self.visible_keys_in_current_view.clear();
            self.selected_visible_key_index = 0;
            // Re-establish connection or select DB and fetch keys
            if let Some(profile_idx) = self.profiles.iter().position(|p| p.db == Some(self.selected_db_index as u8) || (p.db.is_none() && self.selected_db_index ==0)) {
                 self.connect_to_profile(profile_idx); // This will eventually call fetch_keys_and_build_tree
            } else {
                // Attempt to connect to current profile, which should handle DB selection
                self.connect_to_profile(self.current_profile_index);
            }
        }
    }

    pub fn previous_db(&mut self) {
        if self.db_count > 0 {
            if self.selected_db_index > 0 {
                self.selected_db_index -= 1;
            } else {
                self.selected_db_index = (self.db_count as usize).saturating_sub(1);
            }
            self.clear_selected_key_info();
            self.current_breadcrumb.clear();
            self.raw_keys.clear();
            self.key_tree.clear();
            self.visible_keys_in_current_view.clear();
            self.selected_visible_key_index = 0;
            // Re-establish connection or select DB and fetch keys
            if let Some(profile_idx) = self.profiles.iter().position(|p| p.db == Some(self.selected_db_index as u8) || (p.db.is_none() && self.selected_db_index ==0)) {
                 self.connect_to_profile(profile_idx);
            } else {
                 self.connect_to_profile(self.current_profile_index);
            }
        }
    }

    pub fn navigate_to_key_tree_root(&mut self) {
        self.current_breadcrumb.clear();
        self.update_visible_keys(); // This will reset selected_visible_key_index to 0
        self.clear_selected_key_info(); // Clear details of any previously selected leaf key
    }

    pub fn initiate_delete_selected_item(&mut self) {
        if self.is_search_active || self.selected_visible_key_index >= self.visible_keys_in_current_view.len() {
            // Do not initiate delete if in search mode or selection is invalid
            return;
        }

        let (display_name, is_folder) = self.visible_keys_in_current_view[self.selected_visible_key_index].clone();
        self.key_to_delete_display_name = Some(display_name.clone());
        self.deletion_is_folder = is_folder;

        if is_folder {
            let mut prefix_parts = self.current_breadcrumb.clone();
            prefix_parts.push(display_name.trim_end_matches(self.key_delimiter).to_string()); // Use delimiter here
            self.prefix_to_delete = Some(format!("{}{}", prefix_parts.join(&self.key_delimiter.to_string()), self.key_delimiter));
            self.key_to_delete_full_path = None;
        } else {
            // Construct full key name for leaf
            let mut full_key_parts = self.current_breadcrumb.clone();
            full_key_parts.push(display_name);
            self.key_to_delete_full_path = Some(full_key_parts.join(&self.key_delimiter.to_string()));
            self.prefix_to_delete = None;
        }
        self.show_delete_confirmation_dialog = true;
    }

    pub fn confirm_delete_item(&mut self) {
        if !self.show_delete_confirmation_dialog {
            return;
        }

        let mut deleted_count = 0;
        let mut deletion_error: Option<String> = None;

        if let Some(mut con) = self.redis_connection.take() {
            if self.deletion_is_folder {
                if let Some(prefix) = self.prefix_to_delete.clone() { // Clone to avoid borrow issues
                    // For folders, we need to find all keys matching the prefix
                    match redis::cmd("KEYS").arg(format!("{}*", prefix)).query::<Vec<String>>(&mut con) {
                        Ok(keys_to_delete) => {
                            if !keys_to_delete.is_empty() {
                                match redis::cmd("DEL").arg(keys_to_delete.as_slice()).query::<i32>(&mut con) {
                                    Ok(count) => deleted_count = count,
                                    Err(e) => deletion_error = Some(format!("Failed to DEL keys for prefix '{}': {}", prefix, e)),
                                }
                            } else {
                                // No keys matched the prefix, arguably not an error, but nothing deleted.
                                self.clipboard_status = Some(format!("No keys found matching prefix '{}' to delete.", prefix));
                            }
                        }
                        Err(e) => deletion_error = Some(format!("Failed to KEYS for prefix '{}': {}", prefix, e)),
                    }
                }
            } else if let Some(full_key) = self.key_to_delete_full_path.clone() { // Clone for similar reasons
                match redis::cmd("DEL").arg(&full_key).query::<i32>(&mut con) {
                    Ok(count) => deleted_count = count,
                    Err(e) => deletion_error = Some(format!("Failed to DEL key '{}': {}", full_key, e)),
                }
            }
            self.redis_connection = Some(con);
        } else {
            deletion_error = Some("No Redis connection to perform delete.".to_string());
        }

        if let Some(err_msg) = deletion_error {
            self.clipboard_status = Some(err_msg); // Use clipboard_status to show error
        } else if deleted_count > 0 {
            self.clipboard_status = Some(format!("Successfully deleted {} key(s).", deleted_count));
            self.fetch_keys_and_build_tree(); // Refresh
        } else if self.prefix_to_delete.is_some() && deleted_count == 0 && self.clipboard_status.is_none() {
             // Handled the "No keys matched prefix" case already by setting clipboard_status
        } else if self.key_to_delete_full_path.is_some() && deleted_count == 0 {
            self.clipboard_status = Some(format!("Key '{}' not found or already deleted.", self.key_to_delete_display_name.as_deref().unwrap_or("selected")));
        }


        // Reset confirmation state
        self.cancel_delete_item();
    }

    pub fn cancel_delete_item(&mut self) {
        self.show_delete_confirmation_dialog = false;
        self.key_to_delete_display_name = None;
        self.key_to_delete_full_path = None;
        self.prefix_to_delete = None;
        self.deletion_is_folder = false;
    }
}

// --- Methods for Fuzzy Search ---
impl App {
    pub fn enter_search_mode(&mut self) {
        self.is_search_active = true;
        self.is_key_view_focused = true; // Search operates on the key view
        self.is_value_view_focused = false;
        self.search_query.clear();
        self.filtered_keys_in_current_view.clear(); // Initialize as empty for global search
        self.selected_filtered_key_index = 0;
        self.update_filtered_keys(); // Populate based on (empty) query from raw_keys
    }

    pub fn exit_search_mode(&mut self) {
        self.is_search_active = false;
        self.search_query.clear();
        self.filtered_keys_in_current_view.clear();
        self.selected_filtered_key_index = 0;
        // Key view should remain focused or handled by normal focus cycle
    }

    pub fn update_filtered_keys(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_keys_in_current_view.clear(); // Clear results if query is empty
        } else {
            let matcher = fuzzy_matcher::skim::SkimMatcherV2::default();
            self.filtered_keys_in_current_view = self.raw_keys // Search all raw_keys
                .iter()
                .filter_map(|full_key_name| {
                    matcher.fuzzy_match(full_key_name, &self.search_query)
                        .map(|_score| full_key_name.clone()) // Store the full_key_name if it matches
                })
                .collect();
        }
        // Reset selection in filtered list if it goes out of bounds
        if self.selected_filtered_key_index >= self.filtered_keys_in_current_view.len() {
            self.selected_filtered_key_index = self.filtered_keys_in_current_view.len().saturating_sub(1);
        }
         if self.filtered_keys_in_current_view.is_empty() && !self.search_query.is_empty(){
            self.selected_filtered_key_index = 0; // No items, index must be 0
        } else if self.filtered_keys_in_current_view.len() == 1 {
            self.selected_filtered_key_index = 0; // Only one item, must be selected
        }
    }

    pub fn select_next_filtered_key(&mut self) {
        if !self.filtered_keys_in_current_view.is_empty() {
            self.selected_filtered_key_index = (self.selected_filtered_key_index + 1) % self.filtered_keys_in_current_view.len();
        }
    }

    pub fn select_previous_filtered_key(&mut self) {
        if !self.filtered_keys_in_current_view.is_empty() {
            if self.selected_filtered_key_index > 0 {
                self.selected_filtered_key_index -= 1;
            } else {
                self.selected_filtered_key_index = self.filtered_keys_in_current_view.len() - 1;
            }
        }
    }

    pub fn activate_selected_filtered_key(&mut self) {
        if self.selected_filtered_key_index < self.filtered_keys_in_current_view.len() {
            let full_key_path = self.filtered_keys_in_current_view[self.selected_filtered_key_index].clone();
            let path_segments: Vec<String> = full_key_path.split(self.key_delimiter).map(|s| s.to_string()).collect();

            if path_segments.is_empty() {
                self.exit_search_mode();
                return;
            }

            // Determine if the selected path is effectively a folder or a leaf
            // A path is a folder if it has children in the key_tree or if other raw_keys start with this path + delimiter
            let mut is_folder_in_tree = false;
            let mut current_level = &self.key_tree;
            for (i, segment) in path_segments.iter().enumerate() {
                if i < path_segments.len() -1 { // Not the last segment
                    if let Some(KeyTreeNode::Folder(sub_map)) = current_level.get(segment) {
                        current_level = sub_map;
                    } else {
                        // Path segment not found as folder, cannot be a folder in tree this way
                        is_folder_in_tree = false;
                        break;
                    }
                } else { // Last segment
                    if let Some(KeyTreeNode::Folder(_)) = current_level.get(segment) {
                        is_folder_in_tree = true; // Exact match is a folder node
                    }
                    // If it's a Leaf node, is_folder_in_tree remains false
                }
            }
            
            // Alternative check: if any *other* raw key starts with this full_key_path + delimiter
            // This handles cases where a key itself might be a leaf (e.g. `seed:user:1`) but also a prefix for others (`seed:user:1:name`)
            if !is_folder_in_tree {
                let prefix_to_check = format!("{}{}", full_key_path, self.key_delimiter);
                if self.raw_keys.iter().any(|k| k.starts_with(&prefix_to_check)) {
                    is_folder_in_tree = true;
                }
            }

            let leaf_name_if_leaf = if !is_folder_in_tree { path_segments.last().cloned() } else { None };

            if is_folder_in_tree {
                self.current_breadcrumb = path_segments;
            } else {
                // It's a leaf key. Breadcrumb is its parent path.
                self.current_breadcrumb = if path_segments.len() > 1 {
                    path_segments[0..path_segments.len()-1].to_vec()
                } else {
                    Vec::new() // Root level leaf key
                };
            }

            self.update_visible_keys(); // Update view to the new breadcrumb path

            if !is_folder_in_tree {
                if let Some(leaf_name) = leaf_name_if_leaf { // Use the captured leaf_name
                    if let Some(idx) = self.visible_keys_in_current_view.iter().position(|(name, is_folder)| name == &leaf_name && !*is_folder) {
                        self.selected_visible_key_index = idx;
                        self.activate_selected_key(); // Load its value
                    }
                }
            } else {
                // If it was a folder, clear any active leaf selection from before search
                self.clear_selected_key_info();
            }

            self.exit_search_mode();
        }
    }

    // New value navigation methods
    pub fn select_next_value_item(&mut self) {
        if let Some(lines) = &self.displayed_value_lines {
            if !lines.is_empty() {
                self.selected_value_sub_index = (self.selected_value_sub_index + 1) % lines.len();
            }
        }
    }

    pub fn select_previous_value_item(&mut self) {
        if let Some(lines) = &self.displayed_value_lines {
            if !lines.is_empty() {
                if self.selected_value_sub_index > 0 {
                    self.selected_value_sub_index -= 1;
                } else {
                    self.selected_value_sub_index = lines.len() - 1;
                }
            }
        }
    }

    pub fn select_page_down_value_item(&mut self, page_size: usize) {
        if let Some(lines) = &self.displayed_value_lines {
            if !lines.is_empty() {
                self.selected_value_sub_index = (self.selected_value_sub_index + page_size).min(lines.len() - 1);
            }
        }
    }

    pub fn select_page_up_value_item(&mut self, page_size: usize) {
        if let Some(lines) = &self.displayed_value_lines {
            if !lines.is_empty() {
                self.selected_value_sub_index = self.selected_value_sub_index.saturating_sub(page_size);
            }
        }
    }
}

// Helper function for ellipsizing copied content preview (optional)
fn ellipsize(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        format!("{}...", &text[..max_len.saturating_sub(3)])
    }
}