//! Admission of a verified-session ACL candidate before it is written as a suite.
//!
//! `finish` keeps the candidate on the session report. Adoption re-reads that
//! text, rejects observation refs, visual points, and supplied secret literals,
//! and returns the suite source a caller asked to name.

use thiserror::Error;

use crate::candidate::{text_contains_observation_ref, value_contains_secret};
use crate::{Action, Surface, TestSuite};

const VERIFIED_SUITE_NAME: &str = "verified-session";

#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("{code}: {message}")]
pub struct AdoptError {
    code: &'static str,
    message: String,
}

impl AdoptError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn code(&self) -> &'static str {
        self.code
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Re-validate a finish-report candidate and, when requested, give the suite
/// the caller's name. The returned text is the only bytes adoption may write.
pub fn adopted_suite_acl(
    candidate: &str,
    suite_name: Option<&str>,
    secret_values: &[&str],
) -> Result<String, AdoptError> {
    let suite = admit_verified_candidate(candidate, secret_values)?;
    let Some(name) = suite_name else {
        return Ok(candidate.to_string());
    };
    if name == suite.name {
        return Ok(candidate.to_string());
    }
    if suite.name != VERIFIED_SUITE_NAME {
        return Err(AdoptError::new(
            "test.adopt.suite_name",
            "only the verified-session candidate can be given a caller suite name",
        ));
    }
    let needle = format!("suite \"{VERIFIED_SUITE_NAME}\"");
    if candidate.matches(&needle).count() != 1 {
        return Err(AdoptError::new(
            "test.adopt.suite_name",
            "the verified-session candidate does not have one suite label to rename",
        ));
    }
    let renamed = candidate.replacen(&needle, &format!("suite \"{name}\""), 1);
    let renamed_suite = admit_verified_candidate(&renamed, secret_values)?;
    if renamed_suite.name != name {
        return Err(AdoptError::new(
            "test.adopt.suite_name",
            "the caller suite name was not admitted",
        ));
    }
    Ok(renamed)
}

fn admit_verified_candidate(
    candidate: &str,
    secret_values: &[&str],
) -> Result<TestSuite, AdoptError> {
    if candidate.trim().is_empty() {
        return Err(AdoptError::new(
            "test.adopt.prose_only",
            "a prose-only success has no verified-session candidate",
        ));
    }
    if text_contains_observation_ref(candidate) {
        return Err(AdoptError::new(
            "test.adopt.observation_ref",
            "the candidate still contains an observation ref",
        ));
    }
    if candidate.contains("visual_point(") {
        return Err(AdoptError::new(
            "test.adopt.visual_point",
            "the candidate still contains a visual point",
        ));
    }
    let suite = TestSuite::from_acl(candidate).map_err(|error| {
        if error.code() == "test.spec.identifier_invalid" {
            AdoptError::new(
                "test.adopt.suite_name",
                "the suite name must contain only ASCII letters, digits, '-' or '_' and be at most 64 bytes",
            )
        } else {
            AdoptError::new(
                "test.adopt.candidate_invalid",
                format!("the candidate is not an admissible ACL suite ({})", error.code()),
            )
        }
    })?;
    if let Some(kind) = unstable_target_kind(&suite) {
        return Err(match kind {
            UnstableTarget::ObservationRef => AdoptError::new(
                "test.adopt.observation_ref",
                "the candidate still contains an observation ref",
            ),
            UnstableTarget::VisualPoint => AdoptError::new(
                "test.adopt.visual_point",
                "the candidate still contains a visual point",
            ),
        });
    }
    if !suite_has_assertion(&suite) {
        return Err(AdoptError::new(
            "test.adopt.unverified",
            "the candidate has no assertion that confirmed the session",
        ));
    }
    if suite.scenarios.is_empty()
        || suite
            .scenarios
            .iter()
            .any(|scenario| scenario.surface != Surface::Web)
    {
        return Err(AdoptError::new(
            "test.adopt.surface",
            "a verified-session candidate must be a web suite",
        ));
    }
    if contains_secret_literal(candidate, &suite, secret_values) {
        return Err(AdoptError::new(
            "test.adopt.secret_literal",
            "the candidate still contains a secret literal",
        ));
    }
    Ok(suite)
}

fn contains_secret_literal(candidate: &str, suite: &TestSuite, secret_values: &[&str]) -> bool {
    let secrets = secret_values
        .iter()
        .copied()
        .filter(|secret| !secret.is_empty())
        .collect::<Vec<_>>();
    if secrets.is_empty() {
        return false;
    }
    if secrets.iter().any(|secret| candidate.contains(secret)) {
        return true;
    }
    match serde_json::to_value(suite) {
        Ok(value) => value_contains_secret(&value, &secrets),
        Err(_) => true,
    }
}

enum UnstableTarget {
    ObservationRef,
    VisualPoint,
}

fn unstable_target_kind(suite: &TestSuite) -> Option<UnstableTarget> {
    let Ok(value) = serde_json::to_value(suite) else {
        return Some(UnstableTarget::ObservationRef);
    };
    json_unstable_target(&value)
}

fn json_unstable_target(value: &serde_json::Value) -> Option<UnstableTarget> {
    match value {
        serde_json::Value::Object(fields) => {
            if let Some(serde_json::Value::String(kind)) = fields.get("type") {
                if kind == "ref" {
                    return Some(UnstableTarget::ObservationRef);
                }
                if kind == "visual_point" {
                    return Some(UnstableTarget::VisualPoint);
                }
            }
            fields.values().find_map(json_unstable_target)
        }
        serde_json::Value::Array(items) => items.iter().find_map(json_unstable_target),
        _ => None,
    }
}

fn suite_has_assertion(suite: &TestSuite) -> bool {
    suite.scenarios.iter().any(|scenario| {
        scenario
            .steps
            .iter()
            .any(|step| matches!(step.action, Action::Assert { .. }))
    })
}

#[cfg(test)]
mod tests {
    use crate::{verified_session_candidate, Action, Expectation, RecordedStep, Target, TestSuite};

    use super::adopted_suite_acl;

    fn candidate_with_text(text: &str) -> String {
        verified_session_candidate(
            &[
                RecordedStep {
                    index: 1,
                    action: Action::Navigate {
                        url: "https://example.test/adopt".to_string(),
                    },
                    passed: true,
                },
                RecordedStep {
                    index: 2,
                    action: Action::Assert {
                        expectation: Expectation::TextVisible(text.to_string()),
                    },
                    passed: true,
                },
            ],
            &[],
        )
        .expect("verified candidate")
    }

    #[test]
    fn names_a_verified_candidate_for_the_caller() {
        let candidate = candidate_with_text("Saved");
        let adopted =
            adopted_suite_acl(&candidate, Some("checkout-adopted"), &[]).expect("named suite");
        let suite = TestSuite::from_acl(&adopted).expect("parse adopted suite");
        assert_eq!(suite.name, "checkout-adopted");
        assert!(adopted.contains("https://example.test/adopt"));
        assert!(adopted.contains("Saved"));
        assert!(!adopted.contains("long_press"));
        assert!(!adopted.contains("network_fulfill"));
    }

    #[test]
    fn refuses_prose_refs_visual_points_and_secret_literals() {
        let prose = adopted_suite_acl("   ", None, &[]).expect_err("blank candidate");
        assert_eq!(prose.code(), "test.adopt.prose_only");

        let with_ref = r#"
suite "verified-session" {
    version = 1
    scenario "verified" {
        name = "Verified session"
        surface = "web"
        timeout_ms = 60000
        click "open" { target = ref("@e4") }
        expect "saved" { text = "Saved" }
    }
}
"#;
        let refused = adopted_suite_acl(with_ref, Some("checkout"), &[]).expect_err("ref");
        assert_eq!(refused.code(), "test.adopt.observation_ref");

        let with_point = r#"
suite "verified-session" {
    version = 1
    scenario "verified" {
        name = "Verified session"
        surface = "web"
        timeout_ms = 60000
        click "open" { target = visual_point("shot", 3, 4) }
        expect "saved" { text = "Saved" }
    }
}
"#;
        let refused = adopted_suite_acl(with_point, None, &[]).expect_err("visual point");
        assert_eq!(refused.code(), "test.adopt.visual_point");

        let candidate = candidate_with_text("s3cret-literal");
        let refused = adopted_suite_acl(&candidate, None, &["s3cret-literal"]).expect_err("secret");
        assert_eq!(refused.code(), "test.adopt.secret_literal");
        assert!(!refused.message().contains("s3cret-literal"));
    }

    #[test]
    fn keeps_a_candidate_that_already_omitted_the_secret_literal() {
        let steps = [
            RecordedStep {
                index: 1,
                action: Action::Navigate {
                    url: "https://example.test/login".to_string(),
                },
                passed: true,
            },
            RecordedStep {
                index: 2,
                action: Action::Fill {
                    target: Target::Css {
                        selector: "#echo".to_string(),
                    },
                    value: "s3cret-literal".to_string(),
                },
                passed: true,
            },
            RecordedStep {
                index: 3,
                action: Action::Assert {
                    expectation: Expectation::TextVisible("Welcome".to_string()),
                },
                passed: true,
            },
        ];
        let candidate = verified_session_candidate(&steps, &["s3cret-literal"]).expect("candidate");
        assert!(!candidate.contains("s3cret-literal"));
        let adopted =
            adopted_suite_acl(&candidate, Some("login"), &["s3cret-literal"]).expect("adopt");
        assert!(!adopted.contains("s3cret-literal"));
        assert!(adopted.contains("Welcome"));
    }

    #[test]
    fn rendered_text_assertion_confirms_a_candidate() {
        let candidate = verified_session_candidate(
            &[
                RecordedStep {
                    index: 1,
                    action: Action::Navigate {
                        url: "https://example.test/adopt".to_string(),
                    },
                    passed: true,
                },
                RecordedStep {
                    index: 2,
                    action: Action::Assert {
                        expectation: Expectation::RenderedText {
                            target: Target::Role {
                                role: "button".to_string(),
                                name: "Save profile".to_string(),
                            },
                            value: "Save profile".to_string(),
                        },
                    },
                    passed: true,
                },
            ],
            &[],
        )
        .expect("rendered-text candidate");
        assert!(candidate.contains("rendered_text"));
        assert!(candidate.contains("role(\"button\", \"Save profile\")"));
        let adopted = adopted_suite_acl(&candidate, Some("save-profile"), &[]).expect("adopt");
        let suite = TestSuite::from_acl(&adopted).expect("parse");
        assert!(matches!(
            suite.scenarios[0].steps[1].action,
            Action::Assert {
                expectation: Expectation::RenderedText { .. }
            }
        ));
    }
}
