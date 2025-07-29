use crate::app::StreamEntry;

#[derive(Debug, Default, Clone)]
pub struct ValueViewer {
    pub active_leaf_key_name: Option<String>,
    pub selected_key_type: Option<String>,
    pub selected_key_value: Option<String>,
    pub selected_key_value_hash: Option<Vec<(String, String)>>,
    pub selected_key_value_zset: Option<Vec<(String, f64)>>,
    pub selected_key_value_list: Option<Vec<String>>,
    pub selected_key_value_set: Option<Vec<String>>,
    pub selected_key_value_json: Option<String>,
    pub selected_key_value_stream: Option<Vec<StreamEntry>>,
    pub current_display_value: Option<String>,
    pub displayed_value_lines: Option<Vec<String>>,
    pub selected_value_sub_index: usize,
    pub value_view_scroll: (u16, u16),
}

impl ValueViewer {
    pub fn clear(&mut self) {
        self.active_leaf_key_name = None;
        self.selected_key_type = None;
        self.selected_key_value = None;
        self.selected_key_value_hash = None;
        self.selected_key_value_zset = None;
        self.selected_key_value_list = None;
        self.selected_key_value_set = None;
        self.selected_key_value_stream = None;
        self.current_display_value = None;
        self.displayed_value_lines = None;
        self.selected_value_sub_index = 0;
        self.value_view_scroll = (0, 0);
    }

    pub fn update_current_display_value(&mut self) {
        self.current_display_value = None;
        self.displayed_value_lines = None;
        self.selected_value_sub_index = 0;
        self.value_view_scroll = (0, 0);

        match self
            .selected_key_type
            .as_mut()
            .map(|v| v.to_uppercase())
            .as_deref()
        {
            Some("HASH") => {
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
                    self.current_display_value = self.selected_key_value.clone();
                }
            }
            Some("ZSET") => {
                if let Some(zset_data) = &self.selected_key_value_zset {
                    if zset_data.is_empty() {
                        self.current_display_value = Some("(empty zset)".to_string());
                    } else {
                        self.displayed_value_lines = Some(
                            zset_data
                                .iter()
                                .map(|(member, score)| {
                                    format!("Score: {} - Member: {}", score, member)
                                })
                                .collect::<Vec<String>>(),
                        );
                    }
                } else {
                    self.current_display_value = self.selected_key_value.clone();
                }
            }
            Some("LIST") => {
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
                    self.current_display_value = self.selected_key_value.clone();
                }
            }
            Some("SET") => {
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
                    self.current_display_value = self.selected_key_value.clone();
                }
            }
            Some("STREAM") => {
                if let Some(stream_entries) = &self.selected_key_value_stream {
                    if stream_entries.is_empty() {
                        self.current_display_value = Some(
                            "(empty stream or an error occurred fetching entries)".to_string(),
                        );
                    } else {
                        let mut lines: Vec<String> = Vec::new();
                        for entry in stream_entries {
                            lines.push(format!("ID: {}", entry.id));
                            if entry.fields.is_empty() {
                                lines.push("  (no fields)".to_string());
                            } else {
                                for (field, value) in &entry.fields {
                                    lines.push(format!("  {}: {}", field, value));
                                }
                            }
                            lines.push("---".to_string());
                        }
                        if lines.last().map_or(false, |l| l == "---") {
                            lines.pop();
                        }
                        self.displayed_value_lines = Some(lines);
                    }
                } else {
                    self.current_display_value = self.selected_key_value.clone();
                }
            }
            Some("REJSON-RL") => self.current_display_value = self.selected_key_value_json.take(),
            _ => self.current_display_value = self.selected_key_value.clone(),
        }
    }
}
