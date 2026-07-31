use a3s_test_core::{Action, ModifierKey, Target};

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

#[test]
fn advanced_web_actions_round_trip_through_agent_action_json() {
    let target = Target::Ref {
        value: "@e7".to_string(),
    };
    let actions = [
        Action::Hover {
            target: target.clone(),
        },
        Action::Focus {
            target: target.clone(),
        },
        Action::DoubleClick {
            target: target.clone(),
        },
        Action::ContextClick {
            target: target.clone(),
        },
        Action::Type {
            target: target.clone(),
            value: "more text".to_string(),
        },
        Action::Check {
            target: target.clone(),
        },
        Action::Uncheck {
            target: target.clone(),
        },
        Action::Select {
            target: target.clone(),
            values: vec!["draft".to_string(), "review".to_string()],
        },
        Action::Drag {
            source: target.clone(),
            target: Target::Css {
                selector: "#drop-zone".to_string(),
            },
        },
        Action::Wheel {
            target: Some(target),
            delta_x: 4,
            delta_y: -120,
            modifiers: vec![ModifierKey::Control, ModifierKey::Shift],
        },
        Action::Viewport {
            width: 1440,
            height: 900,
            scale: Some(2),
        },
    ];

    for action in actions {
        let encoded = serde_json::to_string(&action).expect("serialize action");
        let decoded: Action = serde_json::from_str(&encoded).expect("deserialize action");
        assert_eq!(decoded, action);
    }
}
