use a3s_test_core::{Action, ElementState, Expectation, LayoutRelation, Target};
use serde_json::json;

use super::*;

#[test]
fn debug_output_never_contains_registered_values() {
    let redactor =
        ProvenanceRedactor::from_exact_secrets(["top-secret-value"]).expect("valid redactor");

    let debug = format!("{redactor:?}");

    assert!(!debug.contains("top-secret-value"));
    assert!(debug.contains("exact_secret_count: 1"));
}

#[test]
fn redacts_nested_sensitive_keys_and_exact_values() {
    let redactor =
        ProvenanceRedactor::from_exact_secrets(["top-secret-value"]).expect("valid redactor");
    let mut value = json!({
        "safe": "prefix top-secret-value suffix",
        "Authorization": "Bearer implicit-secret",
        "nested": [
            { "api-key": "implicit-api-key" },
            { "name": "Authorization", "value": "Bearer named-secret" },
            { "type": "password", "text": "form-secret" },
        ],
        "key-top-secret-value": "visible",
    });

    redactor.redact_value(&mut value);

    let encoded = value.to_string();
    assert!(!encoded.contains("top-secret-value"));
    assert!(!encoded.contains("implicit-secret"));
    assert!(!encoded.contains("implicit-api-key"));
    assert!(!encoded.contains("named-secret"));
    assert!(!encoded.contains("form-secret"));
    assert!(encoded.contains(REDACTED_VALUE));
    assert!(encoded.contains("visible"));
}

#[test]
fn selects_a_safe_marker_when_a_secret_matches_the_default() {
    let redactor = ProvenanceRedactor::from_exact_secrets(["REDACTED"]).expect("valid redactor");
    let mut value = json!({ "value": "REDACTED" });

    redactor.redact_value(&mut value);

    assert!(!value.to_string().contains("REDACTED"));
    assert_eq!(value["value"], "[FILTERED]");
}

#[test]
fn rejects_empty_and_unbounded_secret_configuration() {
    let empty = ProvenanceRedactor::from_exact_secrets([""]).expect_err("empty secret");
    assert_eq!(empty.code, "test.agent.provenance_redaction_invalid");

    let too_many = (0..=MAX_EXACT_SECRETS).map(|index| format!("secret-{index}"));
    let excessive = ProvenanceRedactor::from_exact_secrets(too_many).expect_err("too many secrets");
    assert_eq!(excessive.code, "test.agent.provenance_redaction_invalid");
}

#[test]
fn strips_url_credentials_query_and_fragment() {
    let redactor = ProvenanceRedactor::default();
    let mut action = Action::Navigate {
        url: "https://user:password@example.test/path?token=value#fragment".to_string(),
    };

    redactor.redact_action(&mut action);

    assert_eq!(
        action,
        Action::Navigate {
            url: "https://example.test/path".to_string(),
        }
    );
}

#[test]
fn strips_sensitive_components_from_nested_url_fields() {
    let redactor = ProvenanceRedactor::default();
    let mut value = json!({
        "initial_url": "https://user:password@example.test/start?token=value#fragment",
        "nested": { "url": "https://example.test/page?session=secret" },
        "visible": "https://example.test/page?not-a-typed-url-field=true"
    });

    redactor.redact_json(&mut value);

    assert_eq!(value["initial_url"], "https://example.test/start");
    assert_eq!(value["nested"]["url"], "https://example.test/page");
    assert_eq!(
        value["visible"],
        "https://example.test/page?not-a-typed-url-field=true"
    );
}

#[test]
fn redacts_assertion_targets_text_values_and_selected_values() {
    let redactor =
        ProvenanceRedactor::from_exact_secrets(["state-secret"]).expect("valid redactor");
    let mut actions = [
        Action::Assert {
            expectation: Expectation::RenderedText {
                target: Target::TestId {
                    value: "state-secret-summary".to_string(),
                },
                value: "Total state-secret".to_string(),
            },
        },
        Action::Assert {
            expectation: Expectation::VisibleCount {
                target: Target::Css {
                    selector: "[data-row=state-secret]".to_string(),
                },
                count: 2,
            },
        },
        Action::Assert {
            expectation: Expectation::InViewport(Target::TestId {
                value: "state-secret-viewport".to_string(),
            }),
        },
        Action::Assert {
            expectation: Expectation::PointerReachable(Target::Css {
                selector: "[data-pointer=state-secret]".to_string(),
            }),
        },
        Action::Assert {
            expectation: Expectation::RenderedTexts {
                target: Target::Css {
                    selector: "[data-row=state-secret]".to_string(),
                },
                values: vec![
                    "public".to_string(),
                    "First state-secret".to_string(),
                    "state-secret".to_string(),
                ],
            },
        },
        Action::Assert {
            expectation: Expectation::State {
                target: Target::Css {
                    selector: "[data-state=state-secret]".to_string(),
                },
                state: ElementState::Checked,
                expected: true,
            },
        },
        Action::Assert {
            expectation: Expectation::State {
                target: Target::TestId {
                    value: "state-secret-focus-scope".to_string(),
                },
                state: ElementState::FocusWithin,
                expected: true,
            },
        },
        Action::Assert {
            expectation: Expectation::State {
                target: Target::TestId {
                    value: "state-secret-semantic-state".to_string(),
                },
                state: ElementState::Expanded,
                expected: true,
            },
        },
        Action::Assert {
            expectation: Expectation::Value {
                target: Target::Label {
                    value: "state-secret field".to_string(),
                },
                value: "prefix-state-secret-suffix".to_string(),
            },
        },
        Action::Assert {
            expectation: Expectation::SelectedValues {
                target: Target::TestId {
                    value: "state-secret-select".to_string(),
                },
                values: vec!["public".to_string(), "state-secret".to_string()],
            },
        },
        Action::Assert {
            expectation: Expectation::Layout {
                target: Target::TestId {
                    value: "state-secret-subject".to_string(),
                },
                relative_to: Target::Css {
                    selector: "[data-reference=state-secret]".to_string(),
                },
                relation: LayoutRelation::Below,
                tolerance_px: 1,
            },
        },
    ];

    for action in &mut actions {
        redactor.redact_action(action);
    }

    let encoded = serde_json::to_string(&actions).expect("redacted actions");
    assert!(!encoded.contains("state-secret"));
    assert!(encoded.contains("public"));
    assert!(encoded.contains(REDACTED_VALUE));
}
