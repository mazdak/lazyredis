#[cfg(test)]

mod tests {
    use super::*;
    use std::collections::HashMap;

    fn empty_app() -> App {
        App {
            selected_db_index: 0,
            db_count: 16,
            redis_client: None,
            redis_connection: None,
            connection_status: String::new(),
            profiles: Vec::new(),
            current_profile_index: 0,
            is_profile_selector_active: false,
            selected_profile_list_index: 0,
            raw_keys: Vec::new(),
            key_tree: HashMap::new(),
            current_breadcrumb: Vec::new(),
            visible_keys_in_current_view: Vec::new(),
            ttl_map: HashMap::new(),
            type_map: HashMap::new(),
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
            search_state: SearchState::new(),
            show_delete_confirmation_dialog: false,
            key_to_delete_display_name: None,
            key_to_delete_full_path: None,
            prefix_to_delete: None,
            deletion_is_folder: false,
            command_state: CommandState::new(),
            pending_operation: None,
        }
    }

    #[test]
    fn builds_tree_with_nested_keys() {
        let mut app = empty_app();
        app.raw_keys = vec![
            "foo:bar".to_string(),
            "foo:baz".to_string(),
            "foo:qux:1".to_string(),
            "alpha".to_string(),
            "beta:g1:h1".to_string(),
        ];
        app.parse_keys_to_tree();

        assert!(matches!(
            app.key_tree.get("alpha").unwrap(),
            KeyTreeNode::Leaf { full_key_name } if full_key_name == "alpha"
        ));

        if let KeyTreeNode::Folder(foo_map) = app.key_tree.get("foo").unwrap() {
            assert!(matches!(
                foo_map.get("bar").unwrap(),
                KeyTreeNode::Leaf { full_key_name } if full_key_name == "foo:bar"
            ));
            if let KeyTreeNode::Folder(qux_map) = foo_map.get("qux").unwrap() {
                assert!(matches!(
                    qux_map.get("1").unwrap(),
                    KeyTreeNode::Leaf { full_key_name } if full_key_name == "foo:qux:1"
                ));
            } else {
                panic!("qux should be a folder");
            }
        } else {
            panic!("foo should be a folder");
        }
    }

    #[test]
    fn promotes_leaf_to_folder_when_needed() {
        let mut app = empty_app();
        app.raw_keys = vec!["foo".to_string(), "foo:bar".to_string()];
        app.parse_keys_to_tree();
        if let KeyTreeNode::Folder(map) = app.key_tree.get("foo").unwrap() {
            assert!(matches!(
                map.get("bar").unwrap(),
                KeyTreeNode::Leaf { full_key_name } if full_key_name == "foo:bar"
            ));
            assert_eq!(map.len(), 1);
        } else {
            panic!("foo should be folder");
        }
    }
} 
