use super::*;
use a3s_test_core::StepOutput;

#[test]
fn ref_actions_require_the_latest_observation() {
    let state = test_state(Some(7));
    let actions = [
        Action::Click {
            target: Target::Ref {
                value: "@e3".to_string(),
            },
        },
        Action::ContextClick {
            target: Target::Ref {
                value: "@e4".to_string(),
            },
        },
        Action::Drag {
            source: Target::Css {
                selector: "#source".to_string(),
            },
            target: Target::Ref {
                value: "@e5".to_string(),
            },
        },
        Action::Wheel {
            target: Some(Target::Ref {
                value: "@e6".to_string(),
            }),
            delta_x: 0,
            delta_y: 120,
            modifiers: Vec::new(),
        },
    ];

    for action in actions {
        assert!(validate_action(&state, &action, Some(7)).is_ok());
        assert!(validate_action(&state, &action, Some(6)).is_err());
        assert!(validate_action(&test_state(None), &action, Some(7)).is_err());
    }
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

#[test]
fn observations_must_remain_on_an_admitted_web_origin() {
    let state = test_state(None);
    let allowed = StepOutput::new("snapshot").with_data(json!({
        "success": true,
        "data": {
            "origin": "https://example.test/document#section"
        }
    }));
    assert!(validate_observation_origin(&state, &allowed).is_ok());

    let replaced = StepOutput::new("snapshot").with_data(json!({
        "success": true,
        "data": {
            "origin": "about:blank"
        }
    }));
    let replaced_error =
        validate_observation_origin(&state, &replaced).expect_err("about:blank must be rejected");
    assert_eq!(replaced_error.code(), "test.driver.web.session_origin_lost");

    let outside = StepOutput::new("snapshot").with_data(json!({
        "success": true,
        "data": {
            "url": "https://outside.test/document"
        }
    }));
    let outside_error = validate_observation_origin(&state, &outside)
        .expect_err("an unapproved Web origin must be rejected");
    assert_eq!(
        outside_error.code(),
        "test.driver.web.navigation_origin_denied"
    );

    let missing = StepOutput::new("snapshot").with_data(json!({
        "success": true,
        "data": {
            "snapshot": "(no interactive elements)"
        }
    }));
    let missing_error = validate_observation_origin(&state, &missing)
        .expect_err("a snapshot without its page URL must be rejected");
    assert_eq!(missing_error.code(), "test.driver.web.output_invalid");
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
