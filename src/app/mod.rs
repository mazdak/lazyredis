pub mod app_clipboard;
mod app_fetch;
pub mod redis_client;
pub mod redis_stats;
pub mod value_viewer;
pub mod state_profile_selector;
pub mod state_delete_dialog;

// use crate::search::SearchState;

// REMOVE: pub mod app; 

use crate::config::ConnectionProfile;
use crate::search::SearchState;
use crate::command::CommandState;
// REMOVE: use redis::{Client};
pub use redis::aio::MultiplexedConnection; // Re-export for other modules
// use tokio::task; // Moved to app_clipboard.rs, check if needed elsewhere here.
use std::collections::HashMap;
// use crossclip::{Clipboard, SystemClipboard}; // Moved to app_clipboard.rs
use crate::app::redis_client::RedisClient;
use crate::app::redis_stats::RedisStats;
use crate::app::value_viewer::ValueViewer;
use crate::app::state_profile_selector::ProfileSelectorState;
use crate::app::state_delete_dialog::DeleteDialogState;
// REMOVE: use crate::app::app_fetch::{
//     fetch_and_set_hash_value,
//     fetch_and_set_zset_value,
//     fetch_and_set_list_value,
//     fetch_and_set_set_value,
//     fetch_and_set_stream_value,
// };

// StreamEntry struct definition
#[derive(Debug, Clone)] 
pub struct StreamEntry { 
    pub id: String,
    pub fields: Vec<(String, String)>,
}

// KeyTreeNode enum definition
#[derive(Debug, Clone)]
pub enum KeyTreeNode {
    Folder(HashMap<String, KeyTreeNode>),
    Leaf { full_key_name: String },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PendingOperation {
    InitialConnect,
    ApplySelectedDb,
    SelectProfileAndConnect,
    ConfirmDeleteItem,
    ExecuteCommand,
    ActivateSelectedKey,
    ActivateSelectedFilteredKey,
    CopyKeyNameToClipboard,
    CopyKeyValueToClipboard,
    FetchRedisStats,
}

pub struct App {
    pub selected_db_index: usize,
    pub db_count: u8,
    pub redis: RedisClient,
    pub connection_status: String,
    pub profiles: Vec<ConnectionProfile>,
    pub current_profile_index: usize,
    pub profile_state: ProfileSelectorState,
    pub raw_keys: Vec<String>,
    pub key_tree: HashMap<String, KeyTreeNode>,
    pub current_breadcrumb: Vec<String>,
    pub visible_keys_in_current_view: Vec<(String, bool)>,
    pub ttl_map: HashMap<String, i64>,
    pub type_map: HashMap<String, String>,
    pub selected_visible_key_index: usize,
    pub key_delimiter: char,
    pub is_key_view_focused: bool,
    pub value_viewer: ValueViewer,
    pub is_value_view_focused: bool,
    pub scan_cursor: u64,
    pub keys_fully_loaded: bool,
    pub clipboard_status: Option<String>,

    // Fuzzy Search State
    pub search_state: SearchState,

    // Delete Confirmation State
    pub delete_dialog: DeleteDialogState,

    // Command prompt state
    pub command_state: CommandState,
    pub pending_operation: Option<PendingOperation>,

    // Redis stats state
    pub redis_stats: Option<RedisStats>,
    pub show_stats: bool,
    pub stats_auto_refresh: bool,
}

impl App {
    // Clipboard functions are now in app::app_clipboard
    // Calls would be: crate::app::app_clipboard::copy_selected_key_name_to_clipboard(self).await;
    // And: crate::app::app_clipboard::copy_selected_key_value_to_clipboard(self).await;

    pub fn new(initial_url: &str, initial_profile_name: &str, profiles: Vec<ConnectionProfile>) -> App {
        let mut app = App {
            selected_db_index: 0, 
            db_count: 16, 
            redis: RedisClient::new(),
            connection_status: format!("Initializing for {} ({})...", initial_profile_name, initial_url),
            profiles,
            current_profile_index: 0, 
            profile_state: ProfileSelectorState::default(),
            
            raw_keys: Vec::new(),
            key_tree: HashMap::new(),
            current_breadcrumb: Vec::new(),
            visible_keys_in_current_view: Vec::new(),
            ttl_map: HashMap::new(),
            type_map: HashMap::new(),
            selected_visible_key_index: 0,
            key_delimiter: ':',
            is_key_view_focused: false, 
            value_viewer: ValueViewer::default(),
            is_value_view_focused: false,
            scan_cursor: 0,
            keys_fully_loaded: false,
            clipboard_status: None,

            // Fuzzy Search State
            search_state: SearchState::new(),

            // Delete Confirmation State
            delete_dialog: DeleteDialogState::default(),

            // Command prompt state
            command_state: CommandState::new(),
            pending_operation: None,

            // Redis stats state
            redis_stats: None,
            show_stats: false,
            stats_auto_refresh: true,
        };

        if !app.profiles.is_empty() {
            app.current_profile_index = app
                .profiles
                .iter()
                .position(|p| p.url == initial_url)
                .unwrap_or(0);
            app.profile_state.selected_index = app.current_profile_index;
            if let Some(db) = app.profiles[app.current_profile_index].db {
                app.selected_db_index = db as usize;
            }
        }
        app
    }

    pub fn trigger_initial_connect(&mut self) {
        self.connection_status = format!("Preparing initial connection...");
        self.pending_operation = Some(PendingOperation::InitialConnect);
    }

    pub async fn execute_initial_connect(&mut self) {
        self.connect_to_profile(self.current_profile_index, true).await;
        self.pending_operation = None; 
    }

    async fn connect_to_profile(&mut self, profile_index: usize, use_profile_db: bool) {
        if profile_index >= self.profiles.len() {
            self.connection_status = format!("Error: Profile index {} out of bounds.", profile_index);
            return;
        }

        let profile = &self.profiles[profile_index];
        self.connection_status = format!("Connecting to {} ({})...", profile.name, profile.url);
        tokio::task::yield_now().await;

        // Determine the target_db_index_override based on use_profile_db
        let target_db_override = if use_profile_db {
            None // When using profile_db, no override is needed
        } else {
            Some(self.selected_db_index) // When not using profile_db (i.e. manual DB select), pass current app selection
        };

        // Use the new RedisClient abstraction
        match self.redis.connect_to_profile(profile, use_profile_db, target_db_override).await {
            Ok(()) => {
                self.selected_db_index = self.redis.db_index;
                self.connection_status = self.redis.connection_status.clone();
                self.fetch_keys_and_build_tree().await;
            }
            Err(e) => {
                self.connection_status = format!("Failed to connect: {}", e);
            }
        }
    }

    pub fn clear_selected_key_info(&mut self) {
        self.value_viewer.clear();
        self.is_value_view_focused = false;
    }

    async fn fetch_keys_and_build_tree(&mut self) {
        self.raw_keys.clear();
        self.key_tree.clear();
        self.current_breadcrumb.clear();
        self.visible_keys_in_current_view.clear();
        self.selected_visible_key_index = 0;
        self.clear_selected_key_info();

        self.scan_cursor = 0;
        self.keys_fully_loaded = false;

        let mut cursor: u64 = self.scan_cursor;
        let mut con = match self.redis.connection.take() {
            Some(con) => con,
            None => {
                self.connection_status = "Not connected. Cannot fetch keys.".to_string();
                return;
            }
        };
        loop {
            match redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH").arg("*")
                .arg("COUNT").arg(1000)
                .query_async::<(u64, Vec<String>)>(&mut con).await
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
                    self.scan_cursor = cursor;
                    if cursor == 0 {
                        self.keys_fully_loaded = true;
                        break;
                    }
                    tokio::task::yield_now().await;
                }
                Err(e) => {
                    self.connection_status = format!("Failed during SCAN: {}", e);
                    break;
                }
            }
        }
        self.redis.connection = Some(con);
        if self.raw_keys.is_empty() {
            self.connection_status = format!("Connected to DB {}. No keys found.", self.selected_db_index);
        } else if !self.keys_fully_loaded {
            self.connection_status = format!(
                "Connected to DB {}. Loaded {} keys so far...",
                self.selected_db_index,
                self.raw_keys.len()
            );
        } else {
            self.connection_status = format!(
                "Connected to DB {}. Found {} keys. Displaying {} top-level items.",
                self.selected_db_index,
                self.raw_keys.len(),
                self.visible_keys_in_current_view.len()
            );
        }
    }
    
    fn parse_keys_to_tree(&mut self) {
        let mut tree = HashMap::new();
        for full_key_name in &self.raw_keys {
            let parts: Vec<&str> = full_key_name.split(self.key_delimiter).collect();
            let mut current_level = &mut tree;
            for (i, part) in parts.iter().enumerate() {
                if i == parts.len() - 1 { 
                    current_level.entry(part.to_string()).or_insert_with(|| {
                        KeyTreeNode::Leaf {
                            full_key_name: full_key_name.to_string(),
                        }
                    });
                } else { 
                    let node = current_level
                        .entry(part.to_string())
                        .or_insert_with(|| KeyTreeNode::Folder(HashMap::new()));

                    if matches!(node, KeyTreeNode::Leaf { .. }) {
                        *node = KeyTreeNode::Folder(HashMap::new());
                    }

                    if let KeyTreeNode::Folder(sub_map) = node {
                        current_level = sub_map;
                    } else {
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
                self.clear_selected_key_info(); 
            }
        }
    }

    pub async fn activate_selected_key(&mut self) {
        if self.selected_visible_key_index < self.visible_keys_in_current_view.len() {
            let (display_name, is_folder) = self.visible_keys_in_current_view[self.selected_visible_key_index].clone();
            self.clear_selected_key_info();
            if is_folder {
                let folder_name = display_name.trim_end_matches('/').to_string();
                self.current_breadcrumb.push(folder_name);
                self.update_visible_keys();
            } else {
                let mut current_node_map_for_leaf = &self.key_tree;
                for segment in &self.current_breadcrumb {
                    if let Some(KeyTreeNode::Folder(sub_map)) = current_node_map_for_leaf.get(segment) {
                        current_node_map_for_leaf = sub_map;
                    } else {
                        self.value_viewer.selected_key_value = Some("Error: Invalid breadcrumb path while finding leaf.".to_string());
                        self.value_viewer.update_current_display_value();
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
                    self.value_viewer.active_leaf_key_name = Some(actual_full_key_name.clone());
                    self.value_viewer.selected_key_type = Some("fetching...".to_string());
                    self.value_viewer.selected_value_sub_index = 0;
                    self.value_viewer.value_view_scroll = (0, 0);
                    let mut con = match self.redis.connection.take() {
                        Some(con) => con,
                        None => {
                            self.value_viewer.selected_key_type = Some("error".to_string());
                            self.value_viewer.selected_key_value = Some("Error: No Redis connection to fetch key value.".to_string());
                            self.value_viewer.update_current_display_value();
                            return;
                        }
                    };
                    // Fetch TTL and type for the selected key only
                    let ttl = redis::cmd("TTL").arg(&actual_full_key_name).query_async::<i64>(&mut con).await.unwrap_or(-2);
                    self.ttl_map.insert(actual_full_key_name.clone(), ttl);
                    let key_type = redis::cmd("TYPE").arg(&actual_full_key_name).query_async::<String>(&mut con).await.unwrap_or("unknown".to_string());
                    self.type_map.insert(actual_full_key_name.clone(), key_type.clone());
                    match redis::cmd("GET").arg(&actual_full_key_name).query_async::<Option<String>>(&mut con).await {
                        Ok(Some(value)) => {
                            self.value_viewer.selected_key_type = Some("string".to_string());
                            self.value_viewer.selected_key_value = Some(value);
                        }
                        Ok(None) => {
                            self.value_viewer.selected_key_type = Some("string".to_string());
                            self.value_viewer.selected_key_value = Some("(nil)".to_string());
                        }
                        Err(e_get) => {
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
                                match redis::cmd("TYPE").arg(&actual_full_key_name).query_async::<String>(&mut con).await {
                                    Ok(key_type) => {
                                        self.value_viewer.selected_key_type = Some(key_type.clone());
                                        match key_type.as_str() {
                                            "hash" => self.fetch_and_set_hash_value(&actual_full_key_name, &mut con).await,
                                            "zset" => self.fetch_and_set_zset_value(&actual_full_key_name, &mut con).await,
                                            "list" => self.fetch_and_set_list_value(&actual_full_key_name, &mut con).await,
                                            "set" => self.fetch_and_set_set_value(&actual_full_key_name, &mut con).await,
                                            "stream" => self.fetch_and_set_stream_value(&actual_full_key_name, &mut con).await,
                                            _ => {
                                                self.value_viewer.selected_key_value = Some(format!(
                                                    "Key is of type '{}'. Value view for this type not yet implemented.",
                                                    key_type
                                                ));
                                            }
                                        }
                                    }
                                    Err(e_type) => {
                                        self.value_viewer.selected_key_type = Some("error (TYPE failed)".to_string());
                                        self.value_viewer.selected_key_value = Some(format!(
                                            "GET for '{}' failed (WRONGTYPE). Subsequent TYPE command also failed: {}",
                                            actual_full_key_name, e_type
                                        ));
                                    }
                                }
                            } else {
                                self.value_viewer.selected_key_type = Some("error (GET failed)".to_string());
                                self.value_viewer.selected_key_value = Some(format!(
                                    "Failed to GET key '{}': {} (Kind: {:?}, Code: {:?})",
                                    actual_full_key_name, e_get, e_get.kind(), e_get.code()
                                ));
                            }
                        }
                    }
                    self.redis.connection = Some(con);
                } else {
                    self.value_viewer.selected_key_type = Some("error".to_string());
                    self.value_viewer.selected_key_value = Some(format!("Error: Key '{}' not found as leaf in tree at current level after traversal.", display_name));
                }
            }
        }
        self.value_viewer.update_current_display_value();
    }

    pub fn navigate_key_tree_up(&mut self) {
        if !self.current_breadcrumb.is_empty() {
            self.current_breadcrumb.pop();
            self.update_visible_keys(); 
            self.clear_selected_key_info(); 
        }
    }

    pub fn update_visible_keys(&mut self) {
        let mut current_level = &self.key_tree;
        for segment in &self.current_breadcrumb {
            if let Some(KeyTreeNode::Folder(next_level)) = current_level.get(segment) {
                current_level = next_level;
            } else {
                self.visible_keys_in_current_view.clear();
                self.selected_visible_key_index = 0;
                return;
            }
        }

        self.visible_keys_in_current_view = current_level
            .iter()
            .map(|(name, node)| {
                let display_name = match node {
                    KeyTreeNode::Folder(_) => format!("{}/", name),
                    KeyTreeNode::Leaf { .. } => name.clone(),
                };
                (display_name, matches!(node, KeyTreeNode::Folder(_)))
            })
            .collect();
        
        self.visible_keys_in_current_view.sort_by(|(a_name, a_folder), (b_name, b_folder)| {
            match (a_folder, b_folder) {
                (true, false) => std::cmp::Ordering::Less,    
                (false, true) => std::cmp::Ordering::Greater, 
                _ => a_name.cmp(b_name),                     
            }
        });
        self.selected_visible_key_index = 0;
    }

    pub fn toggle_profile_selector(&mut self) {
        self.profile_state.toggle(self.current_profile_index);
    }

    pub fn next_profile_in_list(&mut self) {
        self.profile_state.next(self.profiles.len());
    }

    pub fn previous_profile_in_list(&mut self) {
        self.profile_state.previous(self.profiles.len());
    }

    pub async fn select_profile_and_connect(&mut self) {
        if self.profile_state.selected_index < self.profiles.len() {
            self.current_profile_index = self.profile_state.selected_index;
            self.profile_state.is_active = false;
            self.connect_to_profile(self.current_profile_index, true).await;
        }
    }

    pub fn cycle_focus_backward(&mut self) {
        if self.is_value_view_focused {
            self.is_value_view_focused = false;
            self.is_key_view_focused = false; // DB selector focus
        } else if self.is_key_view_focused {
            self.is_key_view_focused = false;
            self.is_value_view_focused = true;
        } else { 
            self.is_key_view_focused = true; 
            self.is_value_view_focused = false;
        }
    }

    pub fn cycle_focus_forward(&mut self) {
        if self.is_key_view_focused {
            self.is_key_view_focused = false;
            self.is_value_view_focused = true;
        } else if self.is_value_view_focused {
            self.is_value_view_focused = false;
            // Now, neither is focused: DB selector focus
        } else { 
            self.is_key_view_focused = true;
        }
    }

    pub fn next_key_in_view(&mut self) {
        if !self.visible_keys_in_current_view.is_empty() {
            let new_idx = (self.selected_visible_key_index + 1) % self.visible_keys_in_current_view.len();
            if new_idx != self.selected_visible_key_index { 
                self.selected_visible_key_index = new_idx;
                self.clear_selected_key_info(); 
            }
        }
    }

    pub fn next_db(&mut self) {
        if self.db_count > 0 {
            self.selected_db_index = (self.selected_db_index + 1) % (self.db_count as usize);
        }
    }

    pub fn previous_db(&mut self) {
        if self.db_count > 0 {
            if self.selected_db_index > 0 {
                self.selected_db_index -= 1;
            } else {
                self.selected_db_index = (self.db_count as usize).saturating_sub(1);
            }
        }
    }

    pub fn trigger_apply_selected_db(&mut self) {
        self.connection_status = format!("Preparing to switch to DB {}...", self.selected_db_index);
        self.pending_operation = Some(PendingOperation::ApplySelectedDb);
    }

    pub async fn execute_apply_selected_db(&mut self) {
        self.clear_selected_key_info();
        self.current_breadcrumb.clear();
        self.raw_keys.clear();
        self.key_tree.clear();
        self.visible_keys_in_current_view.clear();
        self.selected_visible_key_index = 0;
        self.connect_to_profile(self.current_profile_index, false).await;
        self.pending_operation = None; 
    }

    pub fn navigate_to_key_tree_root(&mut self) {
        self.current_breadcrumb.clear();
        self.update_visible_keys(); 
        self.clear_selected_key_info(); 
    }

    pub fn initiate_delete_selected_item(&mut self) {
        self.delete_dialog.initiate_delete_selected_item(
            self.selected_visible_key_index,
            &self.visible_keys_in_current_view,
            &self.current_breadcrumb,
            self.key_delimiter,
            self.search_state.is_active,
        );
    }

    pub fn cancel_delete_item(&mut self) {
        self.delete_dialog.show_confirmation_dialog = false;
        self.delete_dialog.key_to_delete_display_name = None;
        self.delete_dialog.key_to_delete_full_path = None;
        self.delete_dialog.prefix_to_delete = None;
        self.delete_dialog.deletion_is_folder = false;
    }

    pub async fn confirm_delete_item(&mut self) {
        let result = if self.delete_dialog.deletion_is_folder {
            if let Some(prefix) = self.delete_dialog.prefix_to_delete.clone() {
                self.delete_redis_prefix_async(&prefix).await
            } else {
                Err("Prefix to delete was None".to_string())
            }
        } else {
            if let Some(key_path) = self.delete_dialog.key_to_delete_full_path.clone() {
                self.delete_redis_key_async(&key_path).await
            } else {
                Err("Key path to delete was None".to_string())
            }
        };

        match result {
            Ok(msg) => self.clipboard_status = Some(msg),
            Err(e) => self.clipboard_status = Some(format!("Error deleting: {}", e)),
        }
        
        self.delete_dialog.show_confirmation_dialog = false;
        self.delete_dialog.key_to_delete_display_name = None;
        self.delete_dialog.key_to_delete_full_path = None;
        self.delete_dialog.prefix_to_delete = None;
        self.delete_dialog.deletion_is_folder = false;

        self.fetch_keys_and_build_tree().await;
        self.update_visible_keys(); 
        self.value_viewer.active_leaf_key_name = None;
        self.clear_selected_key_info(); 
    }

    async fn delete_redis_prefix_async(&mut self, prefix: &str) -> Result<String, String> {
        let pattern = format!("{}{}", prefix, if prefix.ends_with(self.key_delimiter) { "*" } else { "*" });
        let mut keys_to_delete: Vec<String> = Vec::new();
        let mut cursor: u64 = 0;
        let mut con = match self.redis.connection.take() {
            Some(con) => con,
            None => return Err("No Redis connection available for deleting prefix.".to_string()),
        };
        loop {
            match redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH").arg(&pattern)
                .arg("COUNT").arg(100)
                .query_async::<(u64, Vec<String>)>(&mut con).await
            {
                Ok((next_cursor, batch)) => {
                    keys_to_delete.extend(batch);
                    if next_cursor == 0 {
                        break;
                    }
                    cursor = next_cursor;
                }
                Err(e) => return Err(format!("Error scanning keys for prefix {}: {}", prefix, e.to_string())),
            }
        }
        if keys_to_delete.is_empty() {
            return Ok(format!("No keys found matching prefix '{}'.", prefix));
        }
        match redis::cmd("DEL").arg(keys_to_delete.as_slice()).query_async::<i32>(&mut con).await {
            Ok(count) => Ok(format!("Deleted {} keys matching prefix '{}'.", count, prefix)),
            Err(e) => Err(format!("Error deleting keys for prefix {}: {}", prefix, e.to_string())),
        }
    }

    async fn delete_redis_key_async(&mut self, full_key: &str) -> Result<String, String> {
        let mut con = match self.redis.connection.take() {
            Some(con) => con,
            None => return Err("No Redis connection available for deleting key.".to_string()),
        };
        match redis::cmd("DEL").arg(full_key).query_async::<i32>(&mut con).await {
            Ok(count) => {
                if count > 0 {
                    Ok(format!("Deleted key '{}'.", full_key))
                } else {
                    Ok(format!("Key '{}' not found or already deleted.", full_key))
                }
            }
            Err(e) => Err(format!("Error deleting key {}: {}", full_key, e.to_string())),
        }
    }

    pub fn enter_search_mode(&mut self) {
        self.search_state.enter();
        self.is_key_view_focused = true; 
        self.is_value_view_focused = false;
        self.search_state.update_filtered_keys(&self.raw_keys.clone()); 
    }

    pub fn exit_search_mode(&mut self) {
        self.search_state.exit();
    }

    pub fn update_filtered_keys(&mut self) {
        self.search_state.update_filtered_keys(&self.raw_keys.clone()); 
    }

    pub fn select_next_filtered_key(&mut self) {
        self.search_state.select_next_filtered();
    }

    pub fn select_previous_filtered_key(&mut self) {
        self.search_state.select_previous_filtered();
    }

    pub async fn activate_selected_filtered_key(&mut self) {
        let activation_info_opt = self.search_state.activate_selected_filtered(
            self.key_delimiter, 
            &self.key_tree, 
            &self.raw_keys
        );

        if let Some(info) = activation_info_opt {
            if info.is_folder {
                self.current_breadcrumb = info.path_segments;
                self.update_visible_keys();
                self.clear_selected_key_info();
            } else {
                self.current_breadcrumb = if info.path_segments.len() > 1 {
                    info.path_segments[0..info.path_segments.len()-1].to_vec()
                } else {
                    Vec::new() 
                };
                self.update_visible_keys(); 

                if let Some(leaf_name) = info.path_segments.last() { 
                    if let Some(idx) = self.visible_keys_in_current_view.iter().position(|(name, is_folder)| name == leaf_name && !*is_folder) {
                        self.selected_visible_key_index = idx;
                        self.activate_selected_key().await; 
                    } else {
                        self.clear_selected_key_info();
                    }
                } else {
                    self.clear_selected_key_info();
                }
            }
            self.search_state.exit();
            self.is_key_view_focused = true;
            self.is_value_view_focused = false;
        } else {
            self.search_state.exit(); 
        }
    }

    pub fn select_next_value_item(&mut self) {
        if let Some(lines) = &self.value_viewer.displayed_value_lines {
            if !lines.is_empty() {
                self.value_viewer.selected_value_sub_index = (self.value_viewer.selected_value_sub_index + 1) % lines.len();
            }
        }
    }

    pub fn select_previous_value_item(&mut self) {
        if let Some(lines) = &self.value_viewer.displayed_value_lines {
            if !lines.is_empty() {
                if self.value_viewer.selected_value_sub_index > 0 {
                    self.value_viewer.selected_value_sub_index -= 1;
                } else {
                    self.value_viewer.selected_value_sub_index = lines.len() - 1;
                }
            }
        }
    }

    pub fn select_page_down_value_item(&mut self, page_size: usize) {
        if let Some(lines) = &self.value_viewer.displayed_value_lines {
            if !lines.is_empty() {
                self.value_viewer.selected_value_sub_index = (self.value_viewer.selected_value_sub_index + page_size).min(lines.len() - 1);
            }
        }
    }

    pub fn select_page_up_value_item(&mut self, page_size: usize) {
        if let Some(lines) = &self.value_viewer.displayed_value_lines {
            if !lines.is_empty() {
                self.value_viewer.selected_value_sub_index = self.value_viewer.selected_value_sub_index.saturating_sub(page_size);
            }
        }
    }

    pub fn open_command_prompt(&mut self) {
        self.command_state.open();
    }

    pub fn close_command_prompt(&mut self) {
        self.command_state.close();
    }

    pub async fn execute_command_input(&mut self) {
        self.command_state.execute_command(&mut self.redis.connection).await;
    }

    pub fn toggle_stats_view(&mut self) {
        self.show_stats = !self.show_stats;
        if self.show_stats && self.redis_stats.is_none() {
            self.pending_operation = Some(PendingOperation::FetchRedisStats);
        }
    }

    pub fn toggle_stats_auto_refresh(&mut self) {
        self.stats_auto_refresh = !self.stats_auto_refresh;
    }

    pub fn trigger_fetch_redis_stats(&mut self) {
        self.pending_operation = Some(PendingOperation::FetchRedisStats);
    }

    pub async fn execute_fetch_redis_stats(&mut self) {
        match self.redis.get_info().await {
            Ok(info_string) => {
                self.redis_stats = Some(RedisStats::from_info_string(&info_string));
            }
            Err(e) => {
                // Could set an error state here if needed
                eprintln!("Failed to fetch Redis stats: {}", e);
            }
        }
        self.pending_operation = None;
    }

    pub fn should_refresh_stats(&self) -> bool {
        if !self.show_stats || !self.stats_auto_refresh {
            return false;
        }
        
        match &self.redis_stats {
            None => true,
            Some(stats) => stats.is_stale(std::time::Duration::from_secs(2)),
        }
    }
}


#[cfg(test)]
mod tests;
