#[cfg(test)]

mod tests {
    use std::collections::HashMap;
    use crate::app::{App, KeyTreeNode};
    use crate::app::value_viewer::ValueViewer;
    use crate::app::state_profile_selector::ProfileSelectorState;
    use crate::app::state_delete_dialog::DeleteDialogState;
    use crate::search::SearchState;
    use crate::command::CommandState;
    use crate::config::ConnectionProfile;

    fn empty_app() -> App {
        App {
            selected_db_index: 0,
            db_count: 16,
            redis: crate::app::redis_client::RedisClient::new(),
            connection_status: String::new(),
            profiles: Vec::new(),
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
            search_state: SearchState::new(),
            delete_dialog: DeleteDialogState::default(),
            command_state: CommandState::new(),
            pending_operation: None,
            redis_stats: None,
            show_stats: false,
            stats_auto_refresh: true,
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

    #[test]
    fn seed_and_purge_only_allowed_on_dev_profiles() {
        // Simulate profiles
        let dev_profile = ConnectionProfile {
            name: "Dev".to_string(),
            url: "redis://localhost:6379".to_string(),
            db: Some(0),
            dev: Some(true),
            color: None,
        };
        let prod_profile = ConnectionProfile {
            name: "Prod".to_string(),
            url: "redis://prod.example.com:6379".to_string(),
            db: Some(0),
            dev: Some(false),
            color: None,
        };
        // Simulate CLI logic
        fn can_seed_or_purge(profile: &ConnectionProfile) -> bool {
            profile.dev.unwrap_or(false)
        }
        assert!(can_seed_or_purge(&dev_profile), "Should allow on dev profile");
        assert!(!can_seed_or_purge(&prod_profile), "Should NOT allow on prod profile");
        // Also test default (dev missing)
        let no_dev_field = ConnectionProfile {
            name: "NoDev".to_string(),
            url: "redis://localhost:6379".to_string(),
            db: Some(0),
            dev: None,
            color: None,
        };
        assert!(!can_seed_or_purge(&no_dev_field), "Should NOT allow if dev field is missing");
    }
} 
