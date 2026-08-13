use a3s_test_core::{Action, ModifierKey, RepairActor, Target};

#[test]
fn semantic_target_values_round_trip_through_agent_action_json() {
    for (kind, expected) in [
        (
            "automation_id",
            Target::AutomationId {
                value: "checkout-button".to_string(),
            },
        ),
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
                Target::AutomationId { value }
                | Target::TestId { value }
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

#[test]
fn visual_point_round_trips_with_its_grounding_snapshot() {
    let action = Action::Click {
        target: Target::VisualPoint {
            snapshot: "@v7".to_string(),
            x: 240,
            y: 160,
        },
    };
    let encoded = serde_json::to_string(&action).expect("serialize visual action");
    let decoded: Action = serde_json::from_str(&encoded).expect("deserialize visual action");

    assert_eq!(decoded, action);
    assert!(encoded.contains("@v7"));
}

#[test]
fn a3s_test_actor_uses_the_public_hyphenated_wire_name() {
    assert_eq!(
        serde_json::to_string(&RepairActor::A3sTest).expect("serialize repair actor"),
        "\"a3s-test\""
    );
    assert_eq!(
        serde_json::from_str::<RepairActor>("\"a3s-test\"").expect("public repair actor"),
        RepairActor::A3sTest
    );
    assert_eq!(
        serde_json::from_str::<RepairActor>("\"a3s_test\"").expect("legacy repair actor"),
        RepairActor::A3sTest
    );
}

#[test]
fn repair_conflict_relation_has_a_typed_non_keyword_wire_contract() {
    use a3s_test_core::RepairRelation;

    let relation = RepairRelation::ConflictsWith {
        finding_id: "finding-2".to_string(),
    };
    let encoded = serde_json::to_value(&relation).expect("serialize repair relation");
    assert_eq!(
        encoded,
        serde_json::json!({
            "kind": "conflicts_with",
            "findingId": "finding-2"
        })
    );
    assert_eq!(
        serde_json::from_value::<RepairRelation>(encoded).expect("deserialize repair relation"),
        relation
    );
    assert!(serde_json::from_value::<RepairRelation>(serde_json::json!({
        "kind": "conflicts_with",
        "findingId": "finding-2",
        "instructionKeywords": ["black", "white"]
    }))
    .is_err());
}

#[test]
fn visual_viewport_is_additive_and_preserves_css_pixel_geometry() {
    use a3s_test_core::{PageContextViewport, PageContextVisualViewport};

    let viewport = PageContextViewport {
        width: 1280.0,
        height: 720.0,
        dpr: 2.0,
        visual: Some(PageContextVisualViewport {
            x: 0.0,
            y: 0.0,
            width: 853.333,
            height: 480.0,
            scale: 1.5,
        }),
    };
    let encoded = serde_json::to_value(&viewport).expect("serialize visual viewport");
    assert_eq!(encoded["width"], 1280.0);
    assert_eq!(encoded["dpr"], 2.0);
    assert_eq!(encoded["visual"]["width"], 853.333);
    assert_eq!(encoded["visual"]["scale"], 1.5);
    assert_eq!(
        serde_json::from_value::<PageContextViewport>(encoded)
            .expect("deserialize visual viewport"),
        viewport
    );

    let legacy: PageContextViewport = serde_json::from_value(serde_json::json!({
        "width": 1280.0,
        "height": 720.0,
        "dpr": 1.0
    }))
    .expect("deserialize legacy viewport without visual metadata");
    assert!(legacy.visual.is_none());
}
