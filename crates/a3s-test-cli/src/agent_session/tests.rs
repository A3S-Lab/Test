use super::*;

#[test]
fn ref_actions_require_the_latest_observation() {
    let state = test_state(Some(7));
    let action = Action::Click {
        target: Target::Ref {
            value: "@e3".to_string(),
        },
    };
    assert!(validate_action(&state, &action, Some(7)).is_ok());
    assert!(validate_action(&state, &action, Some(6)).is_err());
    assert!(validate_action(&test_state(None), &action, Some(7)).is_err());
}

#[test]
fn navigation_is_limited_to_admitted_origins() {
    let state = test_state(None);
    assert!(validate_action(
        &state,
        &Action::Navigate {
            url: "https://example.test/next".to_string(),
        },
        None,
    )
    .is_ok());
    assert!(validate_action(
        &state,
        &Action::Navigate {
            url: "https://outside.test/".to_string(),
        },
        None,
    )
    .is_err());
}

fn test_state(latest_observation: Option<u64>) -> AgentSessionState {
    AgentSessionState {
        schema_version: SESSION_SCHEMA_VERSION,
        session: "test".to_string(),
        workspace: PathBuf::from("/workspace"),
        surface: Surface::Web,
        status: AgentSessionStatus::Active,
        goal: "Test".to_string(),
        success_criteria: vec!["Pass".to_string()],
        allowed_origins: vec!["https://example.test".to_string()],
        browser: StoredBrowserConfig {
            driver: StoredBrowserDriver::Standalone,
            executable: PathBuf::from("agent-browser"),
            headed: false,
            command_timeout_ms: 30_000,
            idle_timeout_ms: 300_000,
        },
        namespace: "namespace".to_string(),
        driver_session: "agent-test".to_string(),
        runtime_dir: PathBuf::from("/tmp/a3st-test"),
        artifacts_dir: PathBuf::from("/workspace/.a3s-test"),
        active_video_path: None,
        next_sequence: 1,
        next_observation_id: latest_observation
            .and_then(|value| value.checked_add(1))
            .unwrap_or(1),
        latest_observation,
        started_at_ms: 0,
        updated_at_ms: 0,
        summary: None,
    }
}
