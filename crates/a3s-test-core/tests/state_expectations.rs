use a3s_test_core::{
    action_uses_observation_target, action_uses_page_context_ref, resolve_page_context_refs,
    Action, ElementState, Expectation, PageContextBindings, Target, TestSuite,
    ACTION_PROTOCOL_REVISION,
};

#[test]
fn parses_typed_control_state_value_and_selection_expectations() {
    let suite = TestSuite::from_acl(
        r##"
suite "control-state" {
    scenario "form" {
        surface = "web"

        expect "name-value" {
            target = label("Display name")
            value = "Ada"
        }

        expect "submit-enabled" {
            enabled = role("button", "Submit")
        }

        expect "submit-disabled" {
            disabled = testid("submit")
        }

        expect "terms-checked" {
            checked = label("Terms")
        }

        expect "terms-unchecked" {
            unchecked = css("#terms")
        }

        expect "review-selected" {
            selected = role("option", "Review")
        }

        expect "draft-unselected" {
            unselected = css("#status option[value=draft]")
        }

        expect "status-values" {
            target = css("#status")
            selected_values = ["review", "published"]
        }

        expect "nothing-selected" {
            target = css("#empty-status")
            selected_values = []
        }
    }
}
"##,
    )
    .expect("typed state expectation suite");

    let steps = &suite.scenarios[0].steps;
    assert_eq!(steps.len(), 9);
    assert_eq!(
        steps[0].action,
        Action::Assert {
            expectation: Expectation::Value {
                target: Target::Label {
                    value: "Display name".to_string(),
                },
                value: "Ada".to_string(),
            },
        }
    );

    for (index, state, expected) in [
        (1, ElementState::Enabled, true),
        (2, ElementState::Enabled, false),
        (3, ElementState::Checked, true),
        (4, ElementState::Checked, false),
        (5, ElementState::Selected, true),
        (6, ElementState::Selected, false),
    ] {
        let Action::Assert {
            expectation:
                Expectation::State {
                    state: actual_state,
                    expected: actual_expected,
                    ..
                },
        } = &steps[index].action
        else {
            panic!("step {index} was not a state assertion");
        };
        assert_eq!(*actual_state, state);
        assert_eq!(*actual_expected, expected);
    }

    assert_eq!(
        steps[7].action,
        Action::Assert {
            expectation: Expectation::SelectedValues {
                target: Target::Css {
                    selector: "#status".to_string(),
                },
                values: vec!["published".to_string(), "review".to_string()],
            },
        }
    );
    assert_eq!(
        steps[8].action,
        Action::Assert {
            expectation: Expectation::SelectedValues {
                target: Target::Css {
                    selector: "#empty-status".to_string(),
                },
                values: Vec::new(),
            },
        }
    );
}

#[test]
fn state_expectation_admission_rejects_unknown_or_ambiguous_meaning() {
    for (body, code, path) in [
        (
            r#"value = "Ada""#,
            "test.spec.attribute_required",
            ".target",
        ),
        (
            r##"target = css("#name") value = "Ada" checked = css("#terms")"##,
            "test.spec.condition_ambiguous",
            "name-value",
        ),
        (
            r##"target = css("#extra") enabled = css("#submit")"##,
            "test.spec.attribute_unexpected",
            ".target",
        ),
        (
            r##"target = css("#status") selected_values = ["review", "review"]"##,
            "test.spec.selected_value_duplicate",
            "selected_values[1]",
        ),
        (
            r##"target = css("#status") selected_values = "review""##,
            "test.spec.type",
            ".selected_values",
        ),
    ] {
        let acl = format!(
            r#"
suite "invalid-state" {{
    scenario "form" {{
        surface = "web"
        expect "name-value" {{ {body} }}
    }}
}}
"#
        );
        let error = TestSuite::from_acl(&acl).expect_err("invalid state expectation");
        assert_eq!(error.code(), code, "{body}");
        assert!(error.path().contains(path), "{}", error.path());
    }
}

#[test]
fn state_expectations_remain_wire_compatible_after_revision_nine() {
    assert_eq!(ACTION_PROTOCOL_REVISION, 11);
    let action = Action::Assert {
        expectation: Expectation::State {
            target: Target::Ref {
                value: "@e4".to_string(),
            },
            state: ElementState::Checked,
            expected: false,
        },
    };
    let encoded = serde_json::to_value(&action).expect("state action JSON");
    assert_eq!(
        encoded,
        serde_json::json!({
            "type": "assert",
            "expectation": {
                "type": "state",
                "value": {
                    "target": { "type": "ref", "value": "@e4" },
                    "state": "checked",
                    "expected": false
                }
            }
        })
    );
    assert_eq!(
        serde_json::from_value::<Action>(encoded).expect("decode state action"),
        action
    );
}

#[test]
fn state_expectation_targets_keep_observation_and_page_context_binding() {
    for expectation in [
        Expectation::State {
            target: Target::Ref {
                value: "@c1".to_string(),
            },
            state: ElementState::Enabled,
            expected: true,
        },
        Expectation::Value {
            target: Target::Ref {
                value: "@c1".to_string(),
            },
            value: "Ada".to_string(),
        },
        Expectation::SelectedValues {
            target: Target::Ref {
                value: "@c1".to_string(),
            },
            values: vec!["review".to_string()],
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
                        value: "control".to_string(),
                    },
                )]
                .into_iter()
                .collect(),
            },
        )
        .expect("resolve state assertion context ref");
        assert!(!action_uses_page_context_ref(&resolved));
    }
}
