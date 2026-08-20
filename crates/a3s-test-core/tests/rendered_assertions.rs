use a3s_test_core::{
    action_uses_observation_target, action_uses_page_context_ref, resolve_page_context_refs,
    Action, Expectation, PageContextBindings, Target, TestSuite, ACTION_PROTOCOL_REVISION,
    MAX_RENDERED_TEXT_ITEMS,
};

#[test]
fn parses_single_and_ordered_collection_rendered_expectations() {
    let suite = TestSuite::from_acl(
        r##"
suite "rendered-state" {
    scenario "catalog" {
        surface = "web"

        expect "total-copy" {
            target = testid("total")
            rendered_text = "Total $42.00"
        }

        expect "three-visible-rows" {
            target = css("[data-row]")
            visible_count = 3
        }

        expect "line-items" {
            target = css("[data-line-item]")
            rendered_texts = ["Keyboard × 1", "Mouse × 2", "Shipping", "Shipping"]
        }

        expect "no-line-items" {
            target = css("[data-missing-line-item]")
            rendered_texts = []
        }

        expect "no-visible-errors" {
            target = role("alert", "Checkout error")
            visible_count = 0
        }
    }
}
"##,
    )
    .expect("rendered assertion suite");

    let steps = &suite.scenarios[0].steps;
    assert_eq!(steps.len(), 5);
    assert_eq!(
        steps[0].action,
        Action::Assert {
            expectation: Expectation::RenderedText {
                target: Target::TestId {
                    value: "total".to_string(),
                },
                value: "Total $42.00".to_string(),
            },
        }
    );
    assert_eq!(
        steps[1].action,
        Action::Assert {
            expectation: Expectation::VisibleCount {
                target: Target::Css {
                    selector: "[data-row]".to_string(),
                },
                count: 3,
            },
        }
    );
    assert_eq!(
        steps[2].action,
        Action::Assert {
            expectation: Expectation::RenderedTexts {
                target: Target::Css {
                    selector: "[data-line-item]".to_string(),
                },
                values: vec![
                    "Keyboard × 1".to_string(),
                    "Mouse × 2".to_string(),
                    "Shipping".to_string(),
                    "Shipping".to_string(),
                ],
            },
        }
    );
    assert_eq!(
        steps[3].action,
        Action::Assert {
            expectation: Expectation::RenderedTexts {
                target: Target::Css {
                    selector: "[data-missing-line-item]".to_string(),
                },
                values: Vec::new(),
            },
        }
    );
    assert_eq!(
        steps[4].action,
        Action::Assert {
            expectation: Expectation::VisibleCount {
                target: Target::Role {
                    role: "alert".to_string(),
                    name: "Checkout error".to_string(),
                },
                count: 0,
            },
        }
    );
}

#[test]
fn rendered_assertion_admission_rejects_ambiguous_or_unbounded_meaning() {
    for (body, code, path) in [
        (
            r#"rendered_text = "Total""#,
            "test.spec.attribute_required",
            ".target",
        ),
        (
            r##"target = css("#total") rendered_text = "Total" visible_count = 1"##,
            "test.spec.condition_ambiguous",
            "rendered",
        ),
        (
            r##"target = ref("@e2") rendered_texts = ["First"]"##,
            "test.spec.rendered_texts_target_unstable",
            ".target",
        ),
        (
            r##"target = visual_point("shot", 10, 20) rendered_texts = []"##,
            "test.spec.rendered_texts_target_unstable",
            ".target",
        ),
        (
            r##"target = css("[data-row]") rendered_texts = "First""##,
            "test.spec.type",
            ".rendered_texts",
        ),
        (
            r##"target = css("[data-row]") rendered_texts = ["First", 2]"##,
            "test.spec.type",
            ".rendered_texts[1]",
        ),
        (
            r##"target = ref("@e2") visible_count = 1"##,
            "test.spec.visible_count_target_unstable",
            ".target",
        ),
        (
            r##"target = visual_point("shot", 10, 20) visible_count = 1"##,
            "test.spec.visible_count_target_unstable",
            ".target",
        ),
        (
            r##"target = css("[data-row]") visible_count = -1"##,
            "test.spec.number_range",
            ".visible_count",
        ),
        (
            r##"target = css("[data-row]") visible_count = 1.5"##,
            "test.spec.number_range",
            ".visible_count",
        ),
        (
            r##"target = css("[data-row]") visible_count = 4294967296"##,
            "test.spec.number_range",
            ".visible_count",
        ),
    ] {
        let acl = format!(
            r#"
suite "invalid-rendered" {{
    scenario "catalog" {{
        surface = "web"
        expect "rendered" {{ {body} }}
    }}
}}
"#
        );
        let error = TestSuite::from_acl(&acl).expect_err("invalid rendered expectation");
        assert_eq!(error.code(), code, "{body}");
        assert!(error.path().contains(path), "{}", error.path());
    }

    let maximum_values = (0..MAX_RENDERED_TEXT_ITEMS)
        .map(|index| format!(r#""item-{index}""#))
        .collect::<Vec<_>>()
        .join(", ");
    let maximum_acl = format!(
        r#"
suite "maximum-rendered-limit" {{
    scenario "catalog" {{
        surface = "web"
        expect "rendered" {{
            target = css("[data-row]")
            rendered_texts = [{maximum_values}]
        }}
    }}
}}
"#
    );
    TestSuite::from_acl(&maximum_acl).expect("maximum rendered text collection is admitted");

    let values = (0..=MAX_RENDERED_TEXT_ITEMS)
        .map(|index| format!(r#""item-{index}""#))
        .collect::<Vec<_>>()
        .join(", ");
    let acl = format!(
        r#"
suite "invalid-rendered-limit" {{
    scenario "catalog" {{
        surface = "web"
        expect "rendered" {{
            target = css("[data-row]")
            rendered_texts = [{values}]
        }}
    }}
}}
"#
    );
    let error = TestSuite::from_acl(&acl).expect_err("oversized rendered text collection");
    assert_eq!(error.code(), "test.spec.rendered_texts_limit");
    assert!(error.path().ends_with(".rendered_texts"));
}

#[test]
fn rendered_assertions_retain_the_revision_ten_wire_contract() {
    assert_eq!(ACTION_PROTOCOL_REVISION, 15);
    assert_eq!(MAX_RENDERED_TEXT_ITEMS, 256);
    let action = Action::Assert {
        expectation: Expectation::RenderedText {
            target: Target::Ref {
                value: "@e4".to_string(),
            },
            value: "Ready".to_string(),
        },
    };
    let encoded = serde_json::to_value(&action).expect("rendered text action JSON");
    assert_eq!(
        encoded,
        serde_json::json!({
            "type": "assert",
            "expectation": {
                "type": "rendered_text",
                "value": {
                    "target": { "type": "ref", "value": "@e4" },
                    "value": "Ready"
                }
            }
        })
    );
    assert_eq!(
        serde_json::from_value::<Action>(encoded).expect("decode rendered text action"),
        action
    );

    let count = Action::Assert {
        expectation: Expectation::VisibleCount {
            target: Target::Css {
                selector: ".row".to_string(),
            },
            count: 2,
        },
    };
    assert_eq!(
        serde_json::to_value(count).expect("visible count action JSON"),
        serde_json::json!({
            "type": "assert",
            "expectation": {
                "type": "visible_count",
                "value": {
                    "target": { "type": "css", "selector": ".row" },
                    "count": 2
                }
            }
        })
    );

    let texts = Action::Assert {
        expectation: Expectation::RenderedTexts {
            target: Target::Css {
                selector: ".line-item".to_string(),
            },
            values: vec!["Keyboard".to_string(), "Keyboard".to_string()],
        },
    };
    let encoded = serde_json::to_value(&texts).expect("rendered texts action JSON");
    assert_eq!(
        encoded,
        serde_json::json!({
            "type": "assert",
            "expectation": {
                "type": "rendered_texts",
                "value": {
                    "target": { "type": "css", "selector": ".line-item" },
                    "values": ["Keyboard", "Keyboard"]
                }
            }
        })
    );
    assert_eq!(
        serde_json::from_value::<Action>(encoded).expect("decode rendered texts action"),
        texts
    );

    let schema = serde_json::to_string(&schemars::schema_for!(Expectation))
        .expect("rendered expectation schema JSON");
    assert!(schema.contains("rendered_texts"));
    assert!(schema.contains(r#""maxItems":256"#));
}

#[test]
fn rendered_expectation_targets_keep_observation_and_page_context_binding() {
    for expectation in [
        Expectation::RenderedText {
            target: Target::Ref {
                value: "@c1".to_string(),
            },
            value: "Ready".to_string(),
        },
        Expectation::RenderedTexts {
            target: Target::Ref {
                value: "@c1".to_string(),
            },
            values: vec!["Ready".to_string()],
        },
        Expectation::VisibleCount {
            target: Target::Ref {
                value: "@c1".to_string(),
            },
            count: 1,
        },
    ] {
        let action = Action::Assert { expectation };
        assert!(action_uses_observation_target(&action));
        assert!(action_uses_page_context_ref(&action));

        let resolved = resolve_page_context_refs(
            action,
            &PageContextBindings {
                revision: Some(7),
                targets: [(
                    "@c1".to_string(),
                    Target::TestId {
                        value: "status".to_string(),
                    },
                )]
                .into_iter()
                .collect(),
                node_fingerprints: Default::default(),
            },
        )
        .expect("resolve rendered assertion context ref");
        assert!(!action_uses_page_context_ref(&resolved));
    }
}
