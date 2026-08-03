use a3s_test_core::Action;
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
