use super::*;

#[tokio::test]
async fn application_identity_drift_blocks_observation_and_input_before_dispatch() {
    let temp = TempDir::new().expect("temp dir");
    let transport = FakeTransport::new(FakeOptions::default());
    let mut session = driver(launch_config(&temp), Arc::clone(&transport))
        .open(&context(&temp))
        .await
        .expect("GUI session");
    session.observe().await.expect("initial observation");
    let snapshots_before_drift = transport.calls_for("get_window_state").await.len();

    transport
        .replace_running_identity("com.example.Unrelated")
        .await;
    let input_error = session
        .execute(&TestStep {
            id: "identity-drift-input".to_string(),
            action: Action::Press {
                key: "ENTER".to_string(),
            },
            stability: None,
            assertion_mode: Default::default(),
            wait_mode: Default::default(),
        })
        .await
        .expect_err("identity drift must block input");
    assert_eq!(
        input_error.code(),
        "test.driver.gui.application_binding_lost"
    );
    assert!(!input_error.retryable());
    assert!(transport.calls_for("press_key").await.is_empty());

    let observation_error = session
        .observe()
        .await
        .expect_err("identity drift must block observation");
    assert_eq!(
        observation_error.code(),
        "test.driver.gui.application_binding_lost"
    );
    assert_eq!(
        transport.calls_for("get_window_state").await.len(),
        snapshots_before_drift
    );

    let cleanup_error = session
        .close()
        .await
        .expect_err("reused process identity must not be killed");
    assert_eq!(cleanup_error.code(), "test.driver.gui.app_ownership_lost");
    assert!(transport.calls_for("kill_app").await.is_empty());
    assert_eq!(transport.calls_for("end_session").await.len(), 1);
    assert!(transport.closed().await);
}

#[tokio::test]
async fn window_binding_drift_blocks_observation_and_input_before_dispatch() {
    let temp = TempDir::new().expect("temp dir");
    let transport = FakeTransport::new(FakeOptions::default());
    let mut session = driver(launch_config(&temp), Arc::clone(&transport))
        .open(&context(&temp))
        .await
        .expect("GUI session");
    let observation = session.observe().await.expect("initial observation");
    let initial_ref = observation.data["elements"][0]["ref"]
        .as_str()
        .expect("initial ref")
        .to_string();
    let snapshots_before_drift = transport.calls_for("get_window_state").await.len();

    transport.set_window_available(false).await;
    let input_error = session
        .execute(&TestStep {
            id: "window-drift-input".to_string(),
            action: Action::Press {
                key: "ENTER".to_string(),
            },
            stability: None,
            assertion_mode: Default::default(),
            wait_mode: Default::default(),
        })
        .await
        .expect_err("window drift must block input");
    assert_eq!(input_error.code(), "test.driver.gui.window_binding_lost");
    assert!(!input_error.retryable());
    assert!(transport.calls_for("press_key").await.is_empty());

    let observation_error = session
        .observe()
        .await
        .expect_err("window drift must block observation");
    assert_eq!(
        observation_error.code(),
        "test.driver.gui.window_binding_lost"
    );
    assert_eq!(
        transport.calls_for("get_window_state").await.len(),
        snapshots_before_drift
    );

    transport.set_window_available(true).await;
    let stale_error = session
        .execute(&TestStep {
            id: "old-window-ref".to_string(),
            action: Action::Click {
                target: Target::Ref { value: initial_ref },
            },
            stability: None,
            assertion_mode: Default::default(),
            wait_mode: Default::default(),
        })
        .await
        .expect_err("binding failure must invalidate the old window snapshot");
    assert_eq!(stale_error.code(), "test.driver.gui.stale_reference");
    assert!(transport.calls_for("click").await.is_empty());
    session.close().await.expect("identity-safe cleanup");
    assert_eq!(transport.calls_for("kill_app").await.len(), 1);
    assert_eq!(transport.calls_for("end_session").await.len(), 1);
    assert!(transport.closed().await);
}
