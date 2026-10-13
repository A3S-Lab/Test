use a3s_test_core::{verified_session_candidate, Action, DriverError, RecordedStep, StepOutput};
use anyhow::Result;

use super::store::{AgentSessionError, AgentSessionEvent, AgentSessionState, AgentSessionStore};

pub(super) async fn append_success_event(
    store: &AgentSessionStore,
    state: &mut AgentSessionState,
    kind: &str,
    observation_id: Option<u64>,
    action: Action,
    output: StepOutput,
) -> Result<()> {
    let event = AgentSessionEvent {
        sequence: state.next_sequence,
        timestamp_ms: super::unix_ms(),
        kind: kind.to_string(),
        observation_id,
        action: Some(action),
        output: Some(output),
        error: None,
    };
    store.append_event(&event).await?;
    state.next_sequence = state.next_sequence.saturating_add(1);
    state.updated_at_ms = event.timestamp_ms;
    Ok(())
}

pub(super) async fn append_terminal_event(
    store: &AgentSessionStore,
    state: &mut AgentSessionState,
    kind: &str,
) -> Result<()> {
    let event = AgentSessionEvent {
        sequence: state.next_sequence,
        timestamp_ms: super::unix_ms(),
        kind: kind.to_string(),
        observation_id: None,
        action: None,
        output: None,
        error: None,
    };
    store.append_event(&event).await?;
    state.next_sequence = state.next_sequence.saturating_add(1);
    state.updated_at_ms = event.timestamp_ms;
    Ok(())
}

pub(super) async fn record_failure(
    store: &AgentSessionStore,
    state: &mut AgentSessionState,
    kind: &str,
    action: Option<Action>,
    error: &DriverError,
) -> Result<()> {
    let event = AgentSessionEvent {
        sequence: state.next_sequence,
        timestamp_ms: super::unix_ms(),
        kind: kind.to_string(),
        observation_id: None,
        action,
        output: None,
        error: Some(AgentSessionError {
            code: error.code().to_string(),
            message: error.message().to_string(),
        }),
    };
    store.append_event(&event).await?;
    state.next_sequence = state.next_sequence.saturating_add(1);
    state.updated_at_ms = event.timestamp_ms;
    store.save(state).await
}

/// Build a candidate from recorded act events. Secret values are not stored
/// on the event. Names on `fill_secret` and `type_secret` are resolved from
/// the environment so an ordinary fill, type, or assertion that repeats one
/// of those values is omitted. Observation refs and visual points are too.
pub(super) fn candidate_from_events(events: &[AgentSessionEvent]) -> Option<String> {
    let mut steps = Vec::new();
    for event in events {
        if event.kind != "act" {
            continue;
        }
        let Some(action) = event.action.clone() else {
            continue;
        };
        let Ok(index) = u32::try_from(event.sequence) else {
            return None;
        };
        steps.push(RecordedStep {
            index,
            action,
            passed: event.error.is_none(),
        });
    }
    let values = resolved_secret_values(&steps);
    let secrets = values.iter().map(String::as_str).collect::<Vec<_>>();
    verified_session_candidate(&steps, &secrets)
}

fn resolved_secret_values(steps: &[RecordedStep]) -> Vec<String> {
    let mut values = Vec::new();
    for step in steps {
        let name = match &step.action {
            Action::FillSecret { name, .. } | Action::TypeSecret { name, .. } => name.as_str(),
            _ => continue,
        };
        let Ok(value) = std::env::var(name) else {
            continue;
        };
        if value.is_empty() || values.contains(&value) {
            continue;
        }
        values.push(value);
    }
    values
}

#[cfg(test)]
mod tests {
    use a3s_test_core::{Action, Expectation, SecretPurpose, Target};

    use super::{candidate_from_events, AgentSessionError, AgentSessionEvent};

    fn act(sequence: u64, action: Option<Action>, failed: bool) -> AgentSessionEvent {
        AgentSessionEvent {
            sequence,
            timestamp_ms: 0,
            kind: "act".to_string(),
            observation_id: None,
            action,
            output: None,
            error: failed.then(|| AgentSessionError {
                code: "test.driver.web.command_failed".to_string(),
                message: "planned failure".to_string(),
            }),
        }
    }

    #[test]
    fn candidate_from_events_keeps_the_verified_prefix_only() {
        let events = vec![
            act(
                1,
                Some(Action::Navigate {
                    url: "https://example.test/login".to_string(),
                }),
                false,
            ),
            act(
                2,
                Some(Action::FillSecret {
                    target: Target::Css {
                        selector: "#password".to_string(),
                    },
                    name: "APP_PASSWORD".to_string(),
                    purpose: SecretPurpose::Password,
                }),
                false,
            ),
            act(
                3,
                Some(Action::Click {
                    modifiers: Vec::new(),
                    target: Target::Ref {
                        value: "@e4".to_string(),
                    },
                }),
                false,
            ),
            act(
                4,
                Some(Action::Click {
                    modifiers: Vec::new(),
                    target: Target::VisualPoint {
                        snapshot: "shot".to_string(),
                        x: 3,
                        y: 4,
                    },
                }),
                false,
            ),
            act(
                5,
                Some(Action::Click {
                    modifiers: Vec::new(),
                    target: Target::Css {
                        selector: "#failed".to_string(),
                    },
                }),
                true,
            ),
            act(6, None, false),
            AgentSessionEvent {
                sequence: 7,
                timestamp_ms: 0,
                kind: "observe".to_string(),
                observation_id: Some(1),
                action: Some(Action::Snapshot { interactive: true }),
                output: None,
                error: None,
            },
            act(
                8,
                Some(Action::Assert {
                    expectation: Expectation::TextVisible("Welcome".to_string()),
                }),
                false,
            ),
            act(
                9,
                Some(Action::Click {
                    modifiers: Vec::new(),
                    target: Target::Css {
                        selector: "#after".to_string(),
                    },
                }),
                false,
            ),
        ];

        let acl = candidate_from_events(&events).expect("candidate");
        assert!(acl.contains("https://example.test/login"));
        assert!(acl.contains("APP_PASSWORD"));
        assert!(acl.contains("purpose = \"password\""));
        assert!(acl.contains("Welcome"));
        assert!(!acl.contains("@e4"));
        assert!(!acl.contains("visual"));
        assert!(!acl.contains("#failed"));
        assert!(!acl.contains("#after"));
        assert!(!acl.contains("s3cret"));
        assert!(a3s_test_core::TestSuite::from_acl(&acl).is_ok());

        let prose = vec![act(
            1,
            Some(Action::Navigate {
                url: "https://example.test/".to_string(),
            }),
            false,
        )];
        assert!(candidate_from_events(&prose).is_none());
    }

    struct EnvGuard(&'static str);

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            unsafe { std::env::remove_var(self.0) }
        }
    }

    #[test]
    fn candidate_from_events_omits_a_fill_secret_env_value() {
        const NAME: &str = "A3S_TEST_CANDIDATE_SECRET";
        // A quote is escaped by ACL rendering. The parsed suite must still
        // omit the original value, not only the raw ACL spelling.
        const LITERAL: &str = "p\"ass";
        let _guard = EnvGuard(NAME);
        unsafe { std::env::set_var(NAME, LITERAL) };

        let events = vec![
            act(
                1,
                Some(Action::Navigate {
                    url: "https://example.test/login".to_string(),
                }),
                false,
            ),
            act(
                2,
                Some(Action::FillSecret {
                    target: Target::Css {
                        selector: "#password".to_string(),
                    },
                    name: NAME.to_string(),
                    purpose: SecretPurpose::Password,
                }),
                false,
            ),
            act(
                3,
                Some(Action::Fill {
                    target: Target::Css {
                        selector: "#echo".to_string(),
                    },
                    value: LITERAL.to_string(),
                }),
                false,
            ),
            act(
                4,
                Some(Action::Type {
                    target: Target::Css {
                        selector: "#note".to_string(),
                    },
                    value: format!("prefix-{LITERAL}"),
                }),
                false,
            ),
            act(
                5,
                Some(Action::Assert {
                    expectation: Expectation::TextVisible(LITERAL.to_string()),
                }),
                false,
            ),
            act(
                6,
                Some(Action::Assert {
                    expectation: Expectation::TextVisible("Welcome".to_string()),
                }),
                false,
            ),
        ];

        let acl = candidate_from_events(&events).expect("candidate");
        let suite = a3s_test_core::TestSuite::from_acl(&acl).expect("candidate acl");
        let parsed = serde_json::to_value(&suite).expect("suite json");
        assert!(!parsed_suite_contains_secret(&parsed, LITERAL), "{acl}");
        assert!(!acl.contains("p\\\"ass"), "{acl}");
        assert!(acl.contains("https://example.test/login"));
        assert!(acl.contains(NAME));
        assert!(acl.contains("Welcome"));
        assert!(!acl.contains("#echo"));
        assert!(!acl.contains("#note"));
    }

    fn parsed_suite_contains_secret(value: &serde_json::Value, secret: &str) -> bool {
        match value {
            serde_json::Value::String(text) => text.contains(secret),
            serde_json::Value::Array(items) => items
                .iter()
                .any(|item| parsed_suite_contains_secret(item, secret)),
            serde_json::Value::Object(fields) => fields
                .values()
                .any(|item| parsed_suite_contains_secret(item, secret)),
            _ => false,
        }
    }
}
