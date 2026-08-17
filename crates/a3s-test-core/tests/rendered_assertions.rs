use a3s_test_core::{
    action_uses_observation_target, action_uses_page_context_ref, resolve_page_context_refs,
    Action, Expectation, PageContextBindings, Target, TestSuite, ACTION_PROTOCOL_REVISION,
};

#[test]
fn parses_target_bound_rendered_text_and_visible_count_expectations() {
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
    assert_eq!(steps.len(), 3);
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
}

#[test]
fn rendered_assertions_have_a_revision_nine_wire_contract() {
    assert_eq!(ACTION_PROTOCOL_REVISION, 9);
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
            },
        )
        .expect("resolve rendered assertion context ref");
        assert!(!action_uses_page_context_ref(&resolved));
    }
}
