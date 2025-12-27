use crate::app::{KeyTreeNode};
use fuzzy_matcher::FuzzyMatcher; // Added import
use std::collections::HashMap;

#[derive(Debug)]
pub struct SearchState {
    pub is_active: bool,
    pub query: String,
    pub filtered_keys: Vec<String>,
    pub selected_index: usize,
}

#[derive(Debug)] // Added derive Debug for easier inspection if needed
pub struct SearchActivationInfo {
    pub full_key_path: String,
    pub path_segments: Vec<String>,
    pub is_folder: bool,
    // leaf_name: Option<String>, // This can be derived from path_segments if !is_folder
}

impl SearchState {
    pub fn new() -> Self {
        SearchState {
            is_active: false,
            query: String::new(),
            filtered_keys: Vec::new(),
            selected_index: 0,
        }
    }

    pub fn enter(&mut self) {
        self.is_active = true;
        self.query.clear();
        self.filtered_keys.clear();
        self.selected_index = 0;
    }

    pub fn exit(&mut self) {
        self.is_active = false;
        self.query.clear();
        self.filtered_keys.clear();
        self.selected_index = 0;
    }

    pub fn update_filtered_keys(&mut self, raw_keys: &[String]) {
        if self.query.is_empty() {
            self.filtered_keys.clear();
            self.selected_index = 0;
            return;
        }

        let matcher = fuzzy_matcher::skim::SkimMatcherV2::default();
        self.filtered_keys = raw_keys
            .iter()
            .filter_map(|full_key_name| {
                matcher
                    .fuzzy_match(full_key_name, &self.query)
                    .map(|_score| full_key_name.clone())
            })
            .collect();

        if self.filtered_keys.is_empty() {
            self.selected_index = 0;
            return;
        }

        if self.selected_index >= self.filtered_keys.len() {
            self.selected_index = self.filtered_keys.len() - 1;
        }
    }

    pub fn select_next_filtered(&mut self) {
        if !self.filtered_keys.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.filtered_keys.len();
        }
    }

    pub fn select_previous_filtered(&mut self) {
        if !self.filtered_keys.is_empty() {
            if self.selected_index > 0 {
                self.selected_index -= 1;
            } else {
                self.selected_index = self.filtered_keys.len() - 1;
            }
        }
    }

    // Takes necessary App data as read-only references or copies
    // Returns information needed by App to complete the activation
pub fn activate_selected_filtered(&self, key_delimiter: char, key_tree: &HashMap<String, KeyTreeNode>, raw_keys: &[String]) -> Option<SearchActivationInfo> {
        if self.selected_index < self.filtered_keys.len() {
            let full_key_path = self.filtered_keys[self.selected_index].clone();
            let path_segments: Vec<String> = full_key_path.split(key_delimiter).map(|s| s.to_string()).collect();

            if path_segments.is_empty() {
                return None; // Activation failed or not possible
            }

            let mut is_folder_in_tree = false;
            let mut current_level = key_tree;
            for (i, segment) in path_segments.iter().enumerate() {
                if i < path_segments.len() -1 { 
                    if let Some(KeyTreeNode::Folder(sub_map)) = current_level.get(segment) {
                        current_level = sub_map;
                    } else {
                        is_folder_in_tree = false; // Path segment not found as a folder
                        break;
                    }
                } else { // Last segment
                    if let Some(KeyTreeNode::Folder(_)) = current_level.get(segment) {
                        is_folder_in_tree = true; 
                    }
                    // If it's a Leaf, is_folder_in_tree remains false, which is correct.
                    // If it's not present at all, is_folder_in_tree remains false.
                }
            }
            
            // Additional check: Even if not a KeyTreeNode::Folder, if other keys start with this path + delimiter, treat as folder
            if !is_folder_in_tree {
                let prefix_to_check = format!("{}{}", full_key_path, key_delimiter);
                if raw_keys.iter().any(|k| k.starts_with(&prefix_to_check)) {
                    is_folder_in_tree = true;
                }
            }

            Some(SearchActivationInfo {
                full_key_path,
                path_segments,
                is_folder: is_folder_in_tree,
            })
        } else {
            None
        }
    }
}

impl Default for SearchState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::KeyTreeNode;
    use std::collections::HashMap;

    #[test]
    fn activate_selected_filtered_detects_folder_by_prefix() {
        let mut state = SearchState::new();
        state.filtered_keys = vec!["foo".to_string()];
        state.selected_index = 0;

        let key_tree: HashMap<String, KeyTreeNode> = HashMap::new();
        let raw_keys = vec!["foo:bar".to_string()];

        let info = state
            .activate_selected_filtered(':', &key_tree, &raw_keys)
            .expect("activation");

        assert!(info.is_folder);
    }

    #[test]
    fn activate_selected_filtered_detects_leaf() {
        let mut state = SearchState::new();
        state.filtered_keys = vec!["alpha".to_string()];
        state.selected_index = 0;

        let mut key_tree = HashMap::new();
        key_tree.insert(
            "alpha".to_string(),
            KeyTreeNode::Leaf {
                full_key_name: "alpha".to_string(),
            },
        );
        let raw_keys = vec!["alpha".to_string()];

        let info = state
            .activate_selected_filtered(':', &key_tree, &raw_keys)
            .expect("activation");

        assert!(!info.is_folder);
    }
}
