use super::*;
use a3s_test_core::{StepOutput, Target};

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
    let mut state = test_state(None);
    state.browser_allowed_domains = Some(vec![
        "cdn.example.test".to_string(),
        "example.test".to_string(),
    ]);
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
            url: "https://cdn.example.test/".to_string(),
        },
        None,
    )
    .is_err());
}

#[test]
fn runner_owned_contract_actions_are_not_interactive_turns() {
    let error = validate_action(
        &test_state(None),
        &Action::VerifyContract {
            contract: "./contract.acl".to_string(),
            variant: "desktop".to_string(),
            state: "ready".to_string(),
        },
        None,
    )
    .expect_err("runner action must be denied");
    assert!(error.to_string().contains("deterministic ACL runs"));
}

#[test]
fn browser_network_policy_combines_origin_hosts_and_network_only_domains() {
    let policy = browser_network_policy(
        &[
            "https://example.test".to_string(),
            "https://api.example.test:8443".to_string(),
        ],
        &["*.cdn.example.test".to_string(), "EXAMPLE.TEST".to_string()],
    )
    .expect("browser network policy");

    assert_eq!(
        policy.allowed_domains(),
        ["*.cdn.example.test", "api.example.test", "example.test"]
    );
}

#[test]
fn legacy_session_metadata_remains_readable_but_cannot_execute_turns() {
    let mut encoded = serde_json::to_value(test_state(Some(7))).expect("session JSON");
    encoded
        .as_object_mut()
        .expect("session object")
        .remove("browser_allowed_domains");

    let legacy: AgentSessionState =
        serde_json::from_value(encoded).expect("legacy session metadata");
    assert!(legacy.browser_allowed_domains.is_none());

    let turn_error = validate_turn_browser_network_policy(&legacy)
        .expect_err("legacy session turn must fail closed");
    assert_eq!(
        turn_error.code(),
        "test.session.browser_network_policy_missing"
    );

    let cleanup_policy = stored_browser_network_policy(&legacy, BrowserConnectionPurpose::Cleanup)
        .expect("legacy cleanup policy");
    assert!(cleanup_policy.allowed_domains().is_empty());
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

#[tokio::test]
async fn failed_start_cleanup_preserves_a_retryable_session_record() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = temp.path().join("workspace");
    tokio::fs::create_dir(&workspace).await.expect("workspace");
    let store = AgentSessionStore::for_workspace(&workspace, "failed-start");
    store
        .create_directories()
        .await
        .expect("session directories");
    let runtime = temp.path().join("owned-runtime");
    tokio::fs::create_dir(&runtime)
        .await
        .expect("owned runtime");
    let mut state = test_state(None);
    state.workspace = workspace;
    state.session = "failed-start".to_string();
    state.runtime_dir = runtime.clone();
    state.artifacts_dir = store.artifacts_dir().to_path_buf();
    let cleanup_error = DriverError::new(
        "test.driver.web.process_cleanup_failed",
        "owned browser tree did not stop",
    );

    preserve_failed_start(&store, &mut state, None, &cleanup_error)
        .await
        .expect("preserve failed start");

    let stored = store.load().await.expect("load preserved state");
    assert_eq!(stored.status, AgentSessionStatus::Failed);
    assert!(stored.latest_observation.is_none());
    assert!(stored
        .summary
        .as_deref()
        .is_some_and(|summary| summary.contains("cleanup must be retried")));
    assert!(runtime.is_dir(), "cleanup evidence was removed");
    assert!(
        store.events_path().is_file(),
        "cleanup failure was not recorded"
    );
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
        auto_resolve_repairs: false,
        allowed_origins: vec!["https://example.test".to_string()],
        browser_allowed_domains: Some(vec!["example.test".to_string()]),
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
        page_context_bindings: None,
        started_at_ms: 0,
        updated_at_ms: 0,
        summary: None,
    }
}
