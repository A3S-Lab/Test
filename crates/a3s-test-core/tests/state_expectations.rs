use a3s_test_core::{
    action_uses_observation_target, action_uses_page_context_ref, resolve_page_context_refs,
    Action, ElementState, Expectation, PageContextBindings, Target, TestSuite,
    ACTION_PROTOCOL_REVISION,
};

#[test]
fn parses_exact_and_composed_focus_ownership_expectations() {
    let suite = TestSuite::from_acl(
        r#"
suite "focus-ownership" {
    scenario "keyboard" {
        surface = "web"

        expect "checkout-focused" {
            focused = role("button", "Checkout")
        }

        expect "cancel-unfocused" {
            unfocused = testid("cancel")
        }

        expect "dialog-owns-focus" {
            focus_within = css("[role=dialog]")
        }

        expect "page-does-not-own-focus" {
            focus_outside = testid("page-shell")
        }
    }
}
"#,
    )
    .expect("typed focus ownership suite");

    let steps = &suite.scenarios[0].steps;
    assert_eq!(steps.len(), 4);
    for (index, state, expected) in [
        (0, ElementState::Focused, true),
        (1, ElementState::Focused, false),
        (2, ElementState::FocusWithin, true),
        (3, ElementState::FocusWithin, false),
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
            panic!("step {index} was not a focus ownership assertion");
        };
        assert_eq!(*actual_state, state);
        assert_eq!(*actual_expected, expected);
    }
}

#[test]
fn focus_ownership_acl_rejects_observation_bound_targets() {
    for condition in ["focused", "unfocused", "focus_within", "focus_outside"] {
        for target in ["ref(\"@e1\")", "visual_point(\"@v1\", 10, 20)"] {
            let acl = format!(
                r#"
suite "unstable-focus" {{
    scenario "keyboard" {{
        surface = "web"
        expect "focus" {{ {condition} = {target} }}
    }}
}}
"#
            );
            let error = TestSuite::from_acl(&acl).expect_err("unstable focus target");
            assert_eq!(
                error.code(),
                "test.spec.focus_target_unstable",
                "{condition}: {target}"
            );
        }
    }
}

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
    assert_eq!(ACTION_PROTOCOL_REVISION, 13);
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
fn focus_ownership_states_have_stable_wire_names() {
    for (state, expected_name) in [
        (ElementState::Focused, "focused"),
        (ElementState::FocusWithin, "focus_within"),
    ] {
        let action = Action::Assert {
            expectation: Expectation::State {
                target: Target::TestId {
                    value: "focus-target".to_string(),
                },
                state,
                expected: true,
            },
        };
        let encoded = serde_json::to_value(&action).expect("focus state action JSON");
        assert_eq!(encoded["expectation"]["value"]["state"], expected_name);
        assert_eq!(
            serde_json::from_value::<Action>(encoded).expect("decode focus state action"),
            action
        );
    }
}

#[test]
fn state_expectation_targets_keep_observation_and_page_context_binding() {
    for expectation in [
        Expectation::State {
            target: Target::Ref {
                value: "@c1".to_string(),
            },
            state: ElementState::FocusWithin,
            expected: true,
        },
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
