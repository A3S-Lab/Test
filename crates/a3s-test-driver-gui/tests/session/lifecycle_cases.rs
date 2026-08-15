use super::*;

#[tokio::test]
async fn cleanup_refuses_a_reused_process_identity_but_still_ends_the_session() {
    let temp = TempDir::new().expect("temp dir");
    let transport = FakeTransport::new(FakeOptions::default());
    let mut session = driver(launch_config(&temp), Arc::clone(&transport))
        .open(&context(&temp))
        .await
        .expect("GUI session");
    transport
        .replace_running_identity("com.example.Unrelated")
        .await;

    let error = session.close().await.expect_err("ownership must be lost");
    assert_eq!(error.code(), "test.driver.gui.app_ownership_lost");
    let names = transport.tool_names().await;
    assert!(!names.iter().any(|name| name == "kill_app"));
    assert_eq!(
        names.iter().filter(|name| *name == "end_session").count(),
        1
    );
    assert!(transport.closed().await);
}

#[tokio::test]
async fn transient_owned_app_cleanup_can_be_retried_without_losing_the_session() {
    let temp = TempDir::new().expect("temp dir");
    let transport = FakeTransport::new(FakeOptions {
        kill_failures: 1,
        ..FakeOptions::default()
    });
    let mut session = driver(launch_config(&temp), Arc::clone(&transport))
        .open(&context(&temp))
        .await
        .expect("GUI session");

    let error = session.close().await.expect_err("first cleanup must fail");
    assert_eq!(error.code(), "test.driver.gui.cua_tool_failed");
    assert!(error.retryable());
    assert!(transport.running().await);
    assert!(!transport.closed().await);
    assert!(transport
        .tool_names()
        .await
        .iter()
        .all(|name| name != "end_session"));

    session.close().await.expect("retry cleanup");
    session.close().await.expect("idempotent closed session");
    assert!(!transport.running().await);
    assert!(transport.closed().await);
    let names = transport.tool_names().await;
    assert_eq!(names.iter().filter(|name| *name == "kill_app").count(), 2);
    assert_eq!(
        names.iter().filter(|name| *name == "end_session").count(),
        1
    );
}

#[tokio::test]
async fn cleanup_waits_until_the_owned_application_is_no_longer_running() {
    let temp = TempDir::new().expect("temp dir");
    let transport = FakeTransport::new(FakeOptions {
        kill_visibility_polls: 2,
        ..FakeOptions::default()
    });
    let mut session = driver(launch_config(&temp), Arc::clone(&transport))
        .open(&context(&temp))
        .await
        .expect("GUI session");

    session
        .close()
        .await
        .expect("confirmed application cleanup");

    assert!(!transport.running().await);
    assert_eq!(transport.calls_for("kill_app").await.len(), 1);
    assert_eq!(transport.calls_for("end_session").await.len(), 1);
    assert!(transport.calls_for("list_apps").await.len() >= 5);
    assert!(transport.closed().await);
}

#[tokio::test]
async fn transient_session_end_failure_retries_without_killing_the_app_twice() {
    let temp = TempDir::new().expect("temp dir");
    let transport = FakeTransport::new(FakeOptions {
        end_session_failures: 1,
        ..FakeOptions::default()
    });
    let mut session = driver(launch_config(&temp), Arc::clone(&transport))
        .open(&context(&temp))
        .await
        .expect("GUI session");

    let error = session.close().await.expect_err("first cleanup must fail");
    assert_eq!(error.code(), "test.driver.gui.cua_tool_failed");
    assert!(error.retryable());
    assert!(!transport.running().await);
    assert!(!transport.closed().await);

    session.close().await.expect("retry session end");
    let names = transport.tool_names().await;
    assert_eq!(names.iter().filter(|name| *name == "kill_app").count(), 1);
    assert_eq!(
        names.iter().filter(|name| *name == "end_session").count(),
        2
    );
    assert!(transport.closed().await);
}

#[tokio::test]
async fn dropping_an_owned_session_schedules_exact_cleanup() {
    let temp = TempDir::new().expect("temp dir");
    let transport = FakeTransport::new(FakeOptions::default());
    let session = driver(launch_config(&temp), Arc::clone(&transport))
        .open(&context(&temp))
        .await
        .expect("GUI session");

    drop(session);
    tokio::time::timeout(Duration::from_secs(1), async {
        while !transport.closed().await {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("drop cleanup deadline");
    let names = transport.tool_names().await;
    assert_eq!(names.iter().filter(|name| *name == "kill_app").count(), 1);
    assert_eq!(
        names.iter().filter(|name| *name == "end_session").count(),
        1
    );
}

#[tokio::test]
async fn repeated_open_close_cycles_are_leak_free_and_idempotent() {
    const CYCLES: usize = 32;

    let temp = TempDir::new().expect("temp dir");
    let transport = FakeTransport::new(FakeOptions::default());
    for _ in 0..CYCLES {
        let mut session = driver(launch_config(&temp), Arc::clone(&transport))
            .open(&context(&temp))
            .await
            .expect("GUI session");
        session.close().await.expect("first close");
        session.close().await.expect("idempotent close");
    }

    let names = transport.tool_names().await;
    assert_eq!(
        names.iter().filter(|name| *name == "kill_app").count(),
        CYCLES
    );
    assert_eq!(
        names.iter().filter(|name| *name == "end_session").count(),
        CYCLES
    );
}

#[tokio::test]
async fn cancelled_open_reaps_the_launched_application_and_cua_session() {
    let temp = TempDir::new().expect("temp dir");
    let transport = FakeTransport::new(FakeOptions {
        has_window: false,
        ..FakeOptions::default()
    });
    let driver = driver(launch_config(&temp), Arc::clone(&transport));

    let cancelled =
        tokio::time::timeout(Duration::from_millis(20), driver.open(&context(&temp))).await;
    assert!(
        cancelled.is_err(),
        "open should be cancelled during window discovery"
    );
    tokio::time::timeout(Duration::from_secs(1), async {
        while !transport.closed().await {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("cancelled open cleanup deadline");

    let names = transport.tool_names().await;
    assert_eq!(names.iter().filter(|name| *name == "kill_app").count(), 1);
    assert_eq!(
        names.iter().filter(|name| *name == "end_session").count(),
        1
    );
}

#[tokio::test]
async fn cancelled_open_after_launch_dispatch_reaps_the_application() {
    let temp = TempDir::new().expect("temp dir");
    let transport = FakeTransport::new(FakeOptions {
        launch_response_delay: Duration::from_millis(100),
        ..FakeOptions::default()
    });
    let driver = driver(launch_config(&temp), Arc::clone(&transport));

    let cancelled =
        tokio::time::timeout(Duration::from_millis(20), driver.open(&context(&temp))).await;
    assert!(
        cancelled.is_err(),
        "open should be cancelled after launch dispatch"
    );
    tokio::time::timeout(Duration::from_secs(1), async {
        while !transport.closed().await {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("post-launch cancellation cleanup deadline");

    let names = transport.tool_names().await;
    assert_eq!(names.iter().filter(|name| *name == "launch_app").count(), 1);
    assert_eq!(names.iter().filter(|name| *name == "kill_app").count(), 1);
    assert_eq!(
        names.iter().filter(|name| *name == "end_session").count(),
        1
    );
}
