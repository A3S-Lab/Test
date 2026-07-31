use a3s_test_core::{Action, Target};

#[test]
fn semantic_target_values_round_trip_through_agent_action_json() {
    for (kind, expected) in [
        (
            "test_id",
            Target::TestId {
                value: "checkout".to_string(),
            },
        ),
        (
            "label",
            Target::Label {
                value: "Email".to_string(),
            },
        ),
        (
            "placeholder",
            Target::Placeholder {
                value: "Search".to_string(),
            },
        ),
    ] {
        let encoded = format!(
            r#"{{"type":"click","target":{{"type":"{kind}","value":"{}"}}}}"#,
            match &expected {
                Target::TestId { value }
                | Target::Label { value }
                | Target::Placeholder { value } => value,
                _ => unreachable!(),
            }
        );
        let action: Action = serde_json::from_str(&encoded).expect("typed agent action");
        assert_eq!(action, Action::Click { target: expected });
        let serialized = serde_json::to_value(action).expect("serialize action");
        assert!(serialized["target"]["value"].is_string());
    }
}
