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

    // Fuzzy Search State
    pub is_search_active: bool,
    pub search_query: String,
    pub filtered_keys_in_current_view: Vec<(String, bool)>, // (display_name, is_folder)
    pub selected_filtered_key_index: usize,
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
        if let Some(value_str) = &self.current_display_value {
            if self.active_leaf_key_name.is_some() { // Only copy if a leaf key is active
                match SystemClipboard::new() {
                    Ok(clipboard) => { // clipboard needs to be mut
                        match clipboard.set_string_contents(value_str.clone()) {
                            Ok(_) => self.clipboard_status = Some("Copied value to clipboard!".to_string()),
                            Err(e) => self.clipboard_status = Some(format!("Failed to copy value to clipboard: {}", e)),
                        }
                    }
                    Err(e) => self.clipboard_status = Some(format!("Failed to access clipboard: {}", e)),
                }
            } else {
                self.clipboard_status = Some("No active key value to copy.".to_string()); // Message from app_D, app_C was "No key selected to copy"
            }
        } else {
            self.clipboard_status = Some("No value to copy".to_string());
        }
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

            // Fuzzy Search State
            is_search_active: false,
            search_query: String::new(),
            filtered_keys_in_current_view: Vec::new(),
            selected_filtered_key_index: 0,
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
    }

    // Fetches all keys from Redis and initiates parsing into a tree structure.
    fn fetch_keys_and_build_tree(&mut self) {
        self.raw_keys.clear();
        self.key_tree.clear();
        self.current_breadcrumb.clear();
        self.visible_keys_in_current_view.clear();
        self.selected_visible_key_index = 0;
        self.clear_selected_key_info();

        if let Some(mut con) = self.redis_connection.take() {
            self.connection_status = format!("Fetching keys from DB {}...", self.selected_db_index);
            // Using KEYS for simplicity in this TUI context. SCAN would be better for production.
            match redis::cmd("KEYS").arg("*").query::<Vec<String>>(&mut con) {
                Ok(keys) => {
                    self.raw_keys = keys;
                    if self.raw_keys.is_empty() {
                        self.connection_status = format!("Connected to DB {}. No keys found.", self.selected_db_index);
                    } else {
                        self.connection_status = format!(
                            "Connected to DB {}. Found {} keys. Parsing...",
                            self.selected_db_index,
                            self.raw_keys.len()
                        );
                        self.parse_keys_to_tree(); // Assuming this method exists and works
                        self.update_visible_keys(); // Update the view
                        self.connection_status = format!(
                            "Connected to DB {}. Displaying {} top-level items.",
                            self.selected_db_index,
                            self.visible_keys_in_current_view.len()
                        );
                    }
                }
                Err(e) => {
                    self.connection_status = format!("Failed to fetch keys: {}", e);
                    // Leave key views empty
                }
            }
            self.redis_connection = Some(con); // Put the connection back
        } else {
            self.connection_status = "Not connected. Cannot fetch keys.".to_string();
        }
        // Ensure UI reflects any changes immediately if needed, though drawing is periodic
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
        if self.selected_key_type.as_deref() == Some("hash") {
            if let Some(hash_data) = &self.selected_key_value_hash {
                if hash_data.is_empty() {
                    self.current_display_value = Some("(empty hash)".to_string());
                } else {
                    self.current_display_value = Some(
                        hash_data
                            .iter()
                            .map(|(k, v)| format!("{}: {}\n", k, v))
                            .collect::<String>()
                            .trim_end()
                            .to_string()
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
                    self.current_display_value = Some(
                        zset_data
                            .iter()
                            .map(|(member, score)| format!("Score: {} - Member: {}\n", score, member))
                            .collect::<String>()
                            .trim_end()
                            .to_string()
                    );
                }
            } else {
                self.current_display_value = self.selected_key_value.clone(); // Fallback for ZRANGE error
            }
        } else if self.selected_key_type.as_deref() == Some("list") { // New List handling
            if let Some(list_data) = &self.selected_key_value_list {
                if list_data.is_empty() {
                    self.current_display_value = Some("(empty list)".to_string());
                } else {
                    self.current_display_value = Some(
                        list_data
                            .iter()
                            .enumerate()
                            .map(|(idx, val)| format!("{}: {}\n", idx, val))
                            .collect::<String>()
                            .trim_end()
                            .to_string()
                    );
                }
            } else {
                self.current_display_value = self.selected_key_value.clone(); // Fallback for LRANGE error
            }
        } else if self.selected_key_type.as_deref() == Some("set") { // New Set handling
            if let Some(set_data) = &self.selected_key_value_set {
                if set_data.is_empty() {
                    self.current_display_value = Some("(empty set)".to_string());
                } else {
                    // For sets, order is not guaranteed, so just list members.
                    // Sorting them here for consistent display might be nice for users.
                    let mut sorted_set_data = set_data.clone();
                    sorted_set_data.sort_unstable(); // Sort for consistent display
                    self.current_display_value = Some(
                        sorted_set_data
                            .iter()
                            .map(|val| format!("- {}\n", val))
                            .collect::<String>()
                            .trim_end()
                            .to_string()
                    );
                }
            } else {
                self.current_display_value = self.selected_key_value.clone(); // Fallback for SMEMBERS error
            }
        } else if self.selected_key_type.as_deref() == Some("stream") { // New Stream handling
            if let Some(stream_entries) = &self.selected_key_value_stream {
                if stream_entries.is_empty() {
                    self.current_display_value = Some("(empty stream or an error occurred fetching entries)".to_string());
                } else {
                    let mut display_string = String::new();
                    for entry in stream_entries {
                        display_string.push_str(&format!("ID: {}\n", entry.id));
                        if entry.fields.is_empty(){
                            display_string.push_str("  (no fields)\n");
                        } else {
                            for (field, value) in &entry.fields {
                                display_string.push_str(&format!("  {}: {}\n", field, value));
                            }
                        }
                        display_string.push_str("---\n"); // Separator between entries
                    }
                    self.current_display_value = Some(display_string.trim_end().to_string());
                }
            } else {
                 // This case implies an error during XREADGROUP itself, message should be in selected_key_value
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

    pub fn scroll_value_view_down(&mut self, lines: u16) {
        self.value_view_scroll.0 = self.value_view_scroll.0.saturating_add(lines);
        // TODO: Consider max scroll based on content height
    }

    pub fn scroll_value_view_up(&mut self, lines: u16) {
        self.value_view_scroll.0 = self.value_view_scroll.0.saturating_sub(lines);
    }

    pub fn scroll_value_view_page_down(&mut self) {
        let page_size = 10; // Example page size
        self.scroll_value_view_down(page_size);
    }

    pub fn scroll_value_view_page_up(&mut self) {
        let page_size = 10; // Example page size
        self.scroll_value_view_up(page_size);
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
}

// --- Methods for Fuzzy Search ---
impl App {
    pub fn enter_search_mode(&mut self) {
        self.is_search_active = true;
        self.is_key_view_focused = true; // Search operates on the key view
        self.is_value_view_focused = false;
        self.search_query.clear();
        // Initialize filtered_keys with current visible keys, or perform an initial empty search
        self.filtered_keys_in_current_view = self.visible_keys_in_current_view.clone(); 
        self.selected_filtered_key_index = 0; // Or try to match self.selected_visible_key_index
        self.update_filtered_keys(); // Perform initial filtering (which might be on an empty query)
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
            self.filtered_keys_in_current_view = self.visible_keys_in_current_view.clone();
        } else {
            let matcher = fuzzy_matcher::skim::SkimMatcherV2::default();
            self.filtered_keys_in_current_view = self.visible_keys_in_current_view
                .iter()
                .filter_map(|(name, is_folder)| {
                    matcher.fuzzy_match(name, &self.search_query)
                        .map(|_score| (name.clone(), *is_folder)) // We just care if it matches, not the score for now
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
            let (selected_key_name, _is_folder) = self.filtered_keys_in_current_view[self.selected_filtered_key_index].clone();
            // Find this key in the original `visible_keys_in_current_view` to get its original index
            if let Some(original_index) = self.visible_keys_in_current_view.iter().position(|(name, _)| name == &selected_key_name) {
                self.selected_visible_key_index = original_index;
                self.activate_selected_key(); // Call the original activation logic
            }
            // If not found (shouldn't happen if filtered_keys is a subset), do nothing or log error
        }
    }
} 