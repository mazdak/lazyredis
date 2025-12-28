pub mod app_clipboard;
mod app_fetch;
mod value_format;
pub mod redis_client;
pub mod redis_stats;
pub mod state_delete_dialog;
pub mod state_profile_selector;
pub mod value_viewer;

// use crate::search::SearchState;

// REMOVE: pub mod app;

use crate::command::CommandState;
use crate::config::ConnectionProfile;
use crate::search::SearchState;
// REMOVE: use redis::{Client};
pub use redis::aio::MultiplexedConnection; // Re-export for other modules
                                           // use tokio::task; // Moved to app_clipboard.rs, check if needed elsewhere here.
use std::collections::HashMap;
// use crossclip::{Clipboard, SystemClipboard}; // Moved to app_clipboard.rs
use crate::app::redis_client::RedisClient;
use crate::app::redis_stats::RedisStats;
use crate::app::state_delete_dialog::DeleteDialogState;
use crate::app::state_profile_selector::ProfileSelectorState;
use crate::app::value_viewer::ValueViewer;
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
    AutoPreviewCurrentKey,
}

const DELETE_BATCH_SIZE: usize = 500;

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
    pub selected_indices: std::collections::HashSet<usize>,
    pub multi_select_anchor: Option<usize>,
    pub key_delimiter: char,
    pub is_key_view_focused: bool,
    pub value_viewer: ValueViewer,
    pub is_value_view_focused: bool,
    pub value_is_pinned: bool,
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

    pub fn new(
        initial_url: &str,
        initial_profile_name: &str,
        profiles: Vec<ConnectionProfile>,
    ) -> App {
        let mut app = App {
            selected_db_index: 0,
            db_count: 16,
            redis: RedisClient::new(),
            connection_status: format!(
                "Initializing for {} ({})...",
                initial_profile_name, initial_url
            ),
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
            selected_indices: std::collections::HashSet::new(),
            multi_select_anchor: None,
            key_delimiter: ':',
            is_key_view_focused: false,
            value_viewer: ValueViewer::default(),
            is_value_view_focused: false,
            value_is_pinned: false,
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
        self.connection_status = "Preparing initial connection...".to_string();
        self.pending_operation = Some(PendingOperation::InitialConnect);
    }

    pub async fn execute_initial_connect(&mut self) {
        self.connect_to_profile(self.current_profile_index, true)
            .await;
        self.pending_operation = None;
    }

    async fn connect_to_profile(&mut self, profile_index: usize, use_profile_db: bool) {
        if profile_index >= self.profiles.len() {
            self.connection_status =
                format!("Error: Profile index {} out of bounds.", profile_index);
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
        match self
            .redis
            .connect_to_profile(profile, use_profile_db, target_db_override)
            .await
        {
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
        self.value_is_pinned = false;
    }

    pub fn clear_selected_key_info_if_not_pinned(&mut self) {
        if !self.value_is_pinned {
            self.value_viewer.clear();
            self.is_value_view_focused = false;
        }
    }

    async fn fetch_value_for_key(
        &mut self,
        full_key_name: &str,
        con: &mut MultiplexedConnection,
    ) {
        let ttl = redis::cmd("TTL")
            .arg(full_key_name)
            .query_async::<i64>(con)
            .await
            .unwrap_or(-2);
        self.ttl_map.insert(full_key_name.to_string(), ttl);

        let key_type = match redis::cmd("TYPE")
            .arg(full_key_name)
            .query_async::<String>(con)
            .await
        {
            Ok(key_type) => key_type,
            Err(e) => {
                self.value_viewer.selected_key_type = Some("error".to_string());
                self.value_viewer.selected_key_value = Some(format!(
                    "Failed to TYPE key '{}': {}",
                    full_key_name, e
                ));
                self.value_viewer.update_current_display_value();
                return;
            }
        };

        self.type_map
            .insert(full_key_name.to_string(), key_type.clone());
        let key_type_upper = key_type.to_uppercase();
        self.value_viewer.selected_key_type = Some(key_type_upper.clone());

        match key_type_upper.as_str() {
            "STRING" => self.fetch_string_value(full_key_name, con).await,
            "NONE" => {
                self.value_viewer.selected_key_value =
                    Some("(nil)".to_string());
            }
            "HASH" => {
                self.fetch_and_set_hash_value(full_key_name, con).await;
            }
            "ZSET" => {
                self.fetch_and_set_zset_value(full_key_name, con).await;
            }
            "LIST" => {
                self.fetch_and_set_list_value(full_key_name, con).await;
            }
            "SET" => {
                self.fetch_and_set_set_value(full_key_name, con).await;
            }
            "STREAM" => {
                self.fetch_and_set_stream_value(full_key_name, con).await;
            }
            "REJSON-RL" | "JSON" => {
                self.fetch_and_set_json_value(full_key_name, con).await;
            }
            _ => {
                self.value_viewer.selected_key_value = Some(format!(
                    "Key is of type '{}'. Value view for this type not yet implemented.",
                    key_type
                ));
            }
        }

        self.value_viewer.update_current_display_value();
    }

    async fn fetch_string_value(
        &mut self,
        full_key_name: &str,
        con: &mut MultiplexedConnection,
    ) {
        match redis::cmd("GET")
            .arg(full_key_name)
            .query_async::<Option<Vec<u8>>>(con)
            .await
        {
            Ok(Some(bytes)) => {
                self.value_viewer.selected_key_value =
                    Some(value_format::format_bytes_block(&bytes));
            }
            Ok(None) => {
                self.value_viewer.selected_key_value =
                    Some("(nil)".to_string());
            }
            Err(e) => {
                self.value_viewer.selected_key_type = Some("error".to_string());
                self.value_viewer.selected_key_value = Some(format!(
                    "Failed to GET key '{}': {}",
                    full_key_name, e
                ));
            }
        }
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
                .arg("MATCH")
                .arg("*")
                .arg("COUNT")
                .arg(1000)
                .query_async::<(u64, Vec<String>)>(&mut con)
                .await
            {
                Ok((next_cursor, batch)) => {
                    cursor = next_cursor;
                    for key in &batch {
                        self.insert_key_into_tree(key);
                    }
                    self.raw_keys.extend(batch);
                    if !self.raw_keys.is_empty() {
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
            self.connection_status =
                format!("Connected to DB {}. No keys found.", self.selected_db_index);
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

    #[cfg(test)]
    fn parse_keys_to_tree(&mut self) {
        self.key_tree.clear();
        let raw_keys = self.raw_keys.clone();
        for full_key_name in &raw_keys {
            self.insert_key_into_tree(full_key_name);
        }
    }

    fn insert_key_into_tree(&mut self, full_key_name: &str) {
        let parts: Vec<&str> = full_key_name.split(self.key_delimiter).collect();
        let mut current_level = &mut self.key_tree;
        for (i, part) in parts.iter().enumerate() {
            if i == parts.len() - 1 {
                current_level
                    .entry(part.to_string())
                    .or_insert_with(|| KeyTreeNode::Leaf {
                        full_key_name: full_key_name.to_string(),
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
                    unreachable!(
                        "Node should have been converted to a Folder if it was a Leaf"
                    );
                }
            }
        }
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
                self.clear_selected_key_info_if_not_pinned();
            }
        }
    }

    pub async fn activate_selected_key(&mut self) {
        if self.selected_visible_key_index < self.visible_keys_in_current_view.len() {
            let (display_name, is_folder) =
                self.visible_keys_in_current_view[self.selected_visible_key_index].clone();
            self.clear_selected_key_info();
            if is_folder {
                let folder_name = display_name.trim_end_matches('/').to_string();
                self.current_breadcrumb.push(folder_name);
                self.update_visible_keys();
            } else {
                let mut current_node_map_for_leaf = &self.key_tree;
                for segment in &self.current_breadcrumb {
                    if let Some(KeyTreeNode::Folder(sub_map)) =
                        current_node_map_for_leaf.get(segment)
                    {
                        current_node_map_for_leaf = sub_map;
                    } else {
                        self.value_viewer.selected_key_value =
                            Some("Error: Invalid breadcrumb path while finding leaf.".to_string());
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
                    self.value_viewer.clear();
                    self.value_viewer.active_leaf_key_name = Some(actual_full_key_name.clone());
                    self.value_viewer.selected_key_type = Some("fetching...".to_string());
                    let mut con = match self.redis.connection.take() {
                        Some(con) => con,
                        None => {
                            self.value_viewer.selected_key_type = Some("error".to_string());
                            self.value_viewer.selected_key_value =
                                Some("Error: No Redis connection to fetch key value.".to_string());
                            self.value_viewer.update_current_display_value();
                            return;
                        }
                    };
                    self.fetch_value_for_key(&actual_full_key_name, &mut con)
                        .await;
                    self.redis.connection = Some(con);
                } else {
                    self.value_viewer.selected_key_type = Some("error".to_string());
                    self.value_viewer.selected_key_value = Some(format!("Error: Key '{}' not found as leaf in tree at current level after traversal.", display_name));
                }
            }
        }
        self.value_viewer.update_current_display_value();
        // Mark value as pinned when explicitly activated with Enter
        if self.value_viewer.active_leaf_key_name.is_some() {
            self.value_is_pinned = true;
        }
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

        self.visible_keys_in_current_view
            .sort_by(
                |(a_name, a_folder), (b_name, b_folder)| match (a_folder, b_folder) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a_name.cmp(b_name),
                },
            );
        self.selected_visible_key_index = 0;
    }

    pub fn toggle_profile_selector(&mut self) {
        self.profile_state.toggle(self.current_profile_index);
    }

    pub fn focus_db(&mut self) {
        self.is_key_view_focused = false;
        self.is_value_view_focused = false;
    }

    pub fn focus_keys(&mut self) {
        self.is_key_view_focused = true;
        self.is_value_view_focused = false;
    }

    pub fn focus_values(&mut self) {
        self.is_key_view_focused = false;
        self.is_value_view_focused = true;
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
            self.connect_to_profile(self.current_profile_index, true)
                .await;
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
            let new_idx =
                (self.selected_visible_key_index + 1) % self.visible_keys_in_current_view.len();
            if new_idx != self.selected_visible_key_index {
                self.selected_visible_key_index = new_idx;
                self.clear_selected_key_info_if_not_pinned();
            }
        }
    }

    pub fn next_key_in_view_with_shift(&mut self) {
        if !self.visible_keys_in_current_view.is_empty() {
            let anchor = self
                .multi_select_anchor
                .unwrap_or(self.selected_visible_key_index);
            let new_idx =
                (self.selected_visible_key_index + 1) % self.visible_keys_in_current_view.len();

            if new_idx != self.selected_visible_key_index {
                self.selected_visible_key_index = new_idx;
                self.multi_select_anchor = Some(anchor);
                self.update_selection_range(anchor, new_idx);
                self.clear_selected_key_info_if_not_pinned();
            }
        }
    }

    pub fn previous_key_in_view_with_shift(&mut self) {
        if !self.visible_keys_in_current_view.is_empty() {
            let anchor = self
                .multi_select_anchor
                .unwrap_or(self.selected_visible_key_index);
            let new_idx = if self.selected_visible_key_index > 0 {
                self.selected_visible_key_index - 1
            } else {
                self.visible_keys_in_current_view.len() - 1
            };

            if new_idx != self.selected_visible_key_index {
                self.selected_visible_key_index = new_idx;
                self.multi_select_anchor = Some(anchor);
                self.update_selection_range(anchor, new_idx);
                self.clear_selected_key_info_if_not_pinned();
            }
        }
    }

    fn update_selection_range(&mut self, anchor: usize, current: usize) {
        self.selected_indices.clear();
        let start = anchor.min(current);
        let end = anchor.max(current);
        for i in start..=end {
            self.selected_indices.insert(i);
        }
    }

    pub fn clear_multi_selection(&mut self) {
        self.selected_indices.clear();
        self.multi_select_anchor = None;
    }

    pub fn toggle_current_selection(&mut self) {
        if self
            .selected_indices
            .contains(&self.selected_visible_key_index)
        {
            self.selected_indices
                .remove(&self.selected_visible_key_index);
        } else {
            self.selected_indices
                .insert(self.selected_visible_key_index);
        }
        self.multi_select_anchor = Some(self.selected_visible_key_index);
    }

    pub async fn auto_preview_current_key(&mut self) {
        if !self.value_is_pinned
            && self.selected_visible_key_index < self.visible_keys_in_current_view.len()
        {
            let (display_name, is_folder) =
                self.visible_keys_in_current_view[self.selected_visible_key_index].clone();

            if !is_folder {
                let mut current_node_map_for_leaf = &self.key_tree;
                for segment in &self.current_breadcrumb {
                    if let Some(KeyTreeNode::Folder(sub_map)) =
                        current_node_map_for_leaf.get(segment)
                    {
                        current_node_map_for_leaf = sub_map;
                    } else {
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
                    self.value_viewer.clear();
                    self.value_viewer.active_leaf_key_name = Some(actual_full_key_name.clone());
                    self.value_viewer.selected_key_type = Some("fetching...".to_string());

                    let mut con = match self.redis.connection.take() {
                        Some(con) => con,
                        None => return,
                    };

                    self.fetch_value_for_key(&actual_full_key_name, &mut con)
                        .await;
                    self.redis.connection = Some(con);
                }
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
        self.connect_to_profile(self.current_profile_index, false)
            .await;
        self.pending_operation = None;
    }

    pub fn navigate_to_key_tree_root(&mut self) {
        self.current_breadcrumb.clear();
        self.update_visible_keys();
        self.clear_selected_key_info();
    }

    pub fn initiate_delete_selected_item(&mut self) {
        if !self.selected_indices.is_empty() {
            // Multi-select delete
            self.delete_dialog.initiate_delete_multiple_items(
                &self.selected_indices,
                &self.visible_keys_in_current_view,
                &self.current_breadcrumb,
                self.key_delimiter,
                self.search_state.is_active,
            );
        } else {
            // Single item delete
            self.delete_dialog.initiate_delete_selected_item(
                self.selected_visible_key_index,
                &self.visible_keys_in_current_view,
                &self.current_breadcrumb,
                self.key_delimiter,
                self.search_state.is_active,
            );
        }
    }

    pub fn cancel_delete_item(&mut self) {
        self.delete_dialog.show_confirmation_dialog = false;
        self.delete_dialog.key_to_delete_display_name = None;
        self.delete_dialog.key_to_delete_full_path = None;
        self.delete_dialog.prefix_to_delete = None;
        self.delete_dialog.deletion_is_folder = false;
    }

    pub async fn confirm_delete_item(&mut self) {
        let result = if self.delete_dialog.is_multi_delete {
            self.delete_multiple_items_async().await
        } else if self.delete_dialog.deletion_is_folder {
            if let Some(prefix) = self.delete_dialog.prefix_to_delete.clone() {
                self.delete_redis_prefix_async(&prefix).await
            } else {
                Err("Prefix to delete was None".to_string())
            }
        } else if let Some(key_path) = self.delete_dialog.key_to_delete_full_path.clone() {
            self.delete_redis_key_async(&key_path).await
        } else {
            Err("Key path to delete was None".to_string())
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
        self.delete_dialog.keys_to_delete.clear();
        self.delete_dialog.is_multi_delete = false;

        // Clear multi-selection after deletion
        self.clear_multi_selection();

        self.fetch_keys_and_build_tree().await;
        self.update_visible_keys();
        self.value_viewer.active_leaf_key_name = None;
        self.clear_selected_key_info();
    }

    async fn delete_keys_batch(
        &self,
        con: &mut MultiplexedConnection,
        keys: &[String],
        prefer_unlink: &mut bool,
    ) -> Result<i64, String> {
        if keys.is_empty() {
            return Ok(0);
        }

        let result = if *prefer_unlink {
            redis::cmd("UNLINK").arg(keys).query_async::<i64>(con).await
        } else {
            redis::cmd("DEL").arg(keys).query_async::<i64>(con).await
        };

        match result {
            Ok(count) => Ok(count),
            Err(e) => {
                if *prefer_unlink && is_unknown_command_error(&e) {
                    *prefer_unlink = false;
                    redis::cmd("DEL")
                        .arg(keys)
                        .query_async::<i64>(con)
                        .await
                        .map_err(|err| format!("Error deleting keys: {}", err))
                } else {
                    Err(format!("Error deleting keys: {}", e))
                }
            }
        }
    }

    async fn delete_prefix_keys(
        &self,
        con: &mut MultiplexedConnection,
        prefix: &str,
        prefer_unlink: &mut bool,
    ) -> Result<i64, String> {
        let pattern = format!("{}*", prefix);
        let mut cursor: u64 = 0;
        let mut batch = Vec::new();
        let mut total_deleted: i64 = 0;

        loop {
            let (next_cursor, keys) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&pattern)
                .arg("COUNT")
                .arg(1000)
                .query_async::<(u64, Vec<String>)>(con)
                .await
                .map_err(|e| format!("Error scanning keys for prefix {}: {}", prefix, e))?;

            for key in keys {
                batch.push(key);
                if batch.len() >= DELETE_BATCH_SIZE {
                    total_deleted += self.delete_keys_batch(con, &batch, prefer_unlink).await?;
                    batch.clear();
                }
            }

            if next_cursor == 0 {
                break;
            }
            cursor = next_cursor;
        }

        if !batch.is_empty() {
            total_deleted += self.delete_keys_batch(con, &batch, prefer_unlink).await?;
        }

        Ok(total_deleted)
    }

    async fn delete_redis_prefix_async(&mut self, prefix: &str) -> Result<String, String> {
        let mut con = match self.redis.connection.take() {
            Some(con) => con,
            None => return Err("No Redis connection available for deleting prefix.".to_string()),
        };

        let mut prefer_unlink = true;
        let result = match self.delete_prefix_keys(&mut con, prefix, &mut prefer_unlink).await {
            Ok(0) => Ok(format!("No keys found matching prefix '{}'.", prefix)),
            Ok(count) => Ok(format!("Deleted {} keys matching prefix '{}'.", count, prefix)),
            Err(e) => Err(e),
        };

        self.redis.connection = Some(con);
        result
    }

    async fn delete_redis_key_async(&mut self, full_key: &str) -> Result<String, String> {
        let mut con = match self.redis.connection.take() {
            Some(con) => con,
            None => return Err("No Redis connection available for deleting key.".to_string()),
        };

        let mut prefer_unlink = true;
        let result = match self
            .delete_keys_batch(&mut con, &[full_key.to_string()], &mut prefer_unlink)
            .await
        {
            Ok(count) => {
                if count > 0 {
                    Ok(format!("Deleted key '{}'.", full_key))
                } else {
                    Ok(format!("Key '{}' not found or already deleted.", full_key))
                }
            }
            Err(e) => Err(e),
        };

        self.redis.connection = Some(con);
        result
    }

    async fn delete_multiple_items_async(&mut self) -> Result<String, String> {
        let mut con = match self.redis.connection.take() {
            Some(con) => con,
            None => return Err("No Redis connection available for multi-delete.".to_string()),
        };

        let mut total_deleted: i64 = 0;
        let mut errors = Vec::new();
        let mut prefer_unlink = true;
        let mut key_batch = Vec::new();

        for item in &self.delete_dialog.keys_to_delete {
            if let Some(prefix) = item.strip_prefix("folder:") {
                if !key_batch.is_empty() {
                    match self
                        .delete_keys_batch(&mut con, &key_batch, &mut prefer_unlink)
                        .await
                    {
                        Ok(count) => total_deleted += count,
                        Err(e) => errors.push(e),
                    }
                    key_batch.clear();
                }
                // Handle folder deletion
                match self
                    .delete_prefix_keys(&mut con, prefix, &mut prefer_unlink)
                    .await
                {
                    Ok(count) => total_deleted += count,
                    Err(e) => errors.push(e),
                }
            } else {
                // Handle single key deletion
                key_batch.push(item.clone());
                if key_batch.len() >= DELETE_BATCH_SIZE {
                    match self
                        .delete_keys_batch(&mut con, &key_batch, &mut prefer_unlink)
                        .await
                    {
                        Ok(count) => total_deleted += count,
                        Err(e) => errors.push(e),
                    }
                    key_batch.clear();
                }
            }
        }

        if !key_batch.is_empty() {
            match self
                .delete_keys_batch(&mut con, &key_batch, &mut prefer_unlink)
                .await
            {
                Ok(count) => total_deleted += count,
                Err(e) => errors.push(e),
            }
        }

        self.redis.connection = Some(con);

        if errors.is_empty() {
            Ok(format!("Deleted {} items.", total_deleted))
        } else {
            Err(format!(
                "Deleted {} items, but encountered errors: {}",
                total_deleted,
                errors.join("; ")
            ))
        }
    }

    pub fn enter_search_mode(&mut self) {
        self.search_state.enter();
        self.is_key_view_focused = true;
        self.is_value_view_focused = false;
        self.search_state
            .update_filtered_keys(&self.raw_keys);
    }

    pub fn exit_search_mode(&mut self) {
        self.search_state.exit();
    }

    pub fn update_filtered_keys(&mut self) {
        self.search_state
            .update_filtered_keys(&self.raw_keys);
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
            &self.raw_keys,
        );

        if let Some(info) = activation_info_opt {
            if info.is_folder {
                self.current_breadcrumb = info.path_segments;
                self.update_visible_keys();
                self.clear_selected_key_info_if_not_pinned();
            } else {
                self.current_breadcrumb = if info.path_segments.len() > 1 {
                    info.path_segments[0..info.path_segments.len() - 1].to_vec()
                } else {
                    Vec::new()
                };
                self.update_visible_keys();

                if let Some(leaf_name) = info.path_segments.last() {
                    if let Some(idx) = self
                        .visible_keys_in_current_view
                        .iter()
                        .position(|(name, is_folder)| name == leaf_name && !*is_folder)
                    {
                        self.selected_visible_key_index = idx;
                        self.activate_selected_key().await;
                    } else {
                        self.clear_selected_key_info_if_not_pinned();
                    }
                } else {
                    self.clear_selected_key_info_if_not_pinned();
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
                self.value_viewer.selected_value_sub_index =
                    (self.value_viewer.selected_value_sub_index + 1) % lines.len();
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
                self.value_viewer.selected_value_sub_index =
                    (self.value_viewer.selected_value_sub_index + page_size).min(lines.len() - 1);
            }
        }
    }

    pub fn select_page_up_value_item(&mut self, page_size: usize) {
        if let Some(lines) = &self.value_viewer.displayed_value_lines {
            if !lines.is_empty() {
                self.value_viewer.selected_value_sub_index = self
                    .value_viewer
                    .selected_value_sub_index
                    .saturating_sub(page_size);
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
        self.command_state
            .execute_command(&mut self.redis.connection)
            .await;
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

fn is_unknown_command_error(err: &redis::RedisError) -> bool {
    err.kind() == redis::ErrorKind::Extension
        && err.to_string().to_lowercase().contains("unknown command")
}

#[cfg(test)]
mod tests;
