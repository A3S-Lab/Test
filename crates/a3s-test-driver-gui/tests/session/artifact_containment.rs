use super::*;

#[cfg(unix)]
fn symlink_directory(target: &std::path::Path, link: &std::path::Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn symlink_directory(target: &std::path::Path, link: &std::path::Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(target, link)
}

#[cfg(unix)]
fn symlink_file(target: &std::path::Path, link: &std::path::Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn symlink_file(target: &std::path::Path, link: &std::path::Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}

fn unavailable_without_host_privilege(error: &std::io::Error) -> bool {
    cfg!(windows)
        && matches!(
            error.kind(),
            std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::Unsupported
        )
}

#[tokio::test]
async fn directory_symlink_escape_is_rejected_before_any_capture_or_write() {
    let temp = TempDir::new().expect("temp dir");
    let scenario = context(&temp);
    let outside = temp.path().join("outside");
    tokio::fs::create_dir_all(&outside)
        .await
        .expect("outside directory");
    let transport = FakeTransport::new(FakeOptions::default());
    let mut session = driver(launch_config(&temp), Arc::clone(&transport))
        .open(&scenario)
        .await
        .expect("GUI session");
    if let Err(error) = symlink_directory(&outside, &scenario.artifacts_dir.join("escape")) {
        if unavailable_without_host_privilege(&error) {
            session.close().await.expect("close skipped session");
            return;
        }
        panic!("failed to create directory symlink: {error}");
    }

    let error = session
        .execute(&TestStep {
            id: "artifact-escape".to_string(),
            action: Action::Screenshot {
                path: "escape/nested/leak.png".to_string(),
            },
            stability: None,
            assertion_mode: Default::default(),
            wait_mode: Default::default(),
        })
        .await
        .expect_err("linked artifact directory must be rejected");
    assert_eq!(error.code(), "test.driver.gui.artifact_path_invalid");
    assert!(!outside.join("nested").exists());
    assert!(transport.calls_for("get_window_state").await.is_empty());

    session.close().await.expect("close session");
}

#[tokio::test]
async fn existing_linked_screenshot_is_rejected_without_touching_its_target() {
    let temp = TempDir::new().expect("temp dir");
    let scenario = context(&temp);
    let outside = temp.path().join("outside-existing.png");
    tokio::fs::write(&outside, b"do-not-replace")
        .await
        .expect("outside file");
    let transport = FakeTransport::new(FakeOptions::default());
    let mut session = driver(launch_config(&temp), Arc::clone(&transport))
        .open(&scenario)
        .await
        .expect("GUI session");
    if let Err(error) = symlink_file(&outside, &scenario.artifacts_dir.join("linked.png")) {
        if unavailable_without_host_privilege(&error) {
            session.close().await.expect("close skipped session");
            return;
        }
        panic!("failed to create screenshot symlink: {error}");
    }

    let error = session
        .execute(&TestStep {
            id: "linked-screenshot".to_string(),
            action: Action::Screenshot {
                path: "linked.png".to_string(),
            },
            stability: None,
            assertion_mode: Default::default(),
            wait_mode: Default::default(),
        })
        .await
        .expect_err("linked screenshot must be rejected");
    assert_eq!(error.code(), "test.driver.gui.artifact_path_invalid");
    assert_eq!(
        tokio::fs::read(&outside).await.expect("outside contents"),
        b"do-not-replace"
    );
    assert!(transport.calls_for("get_window_state").await.is_empty());

    session.close().await.expect("close session");
}

#[tokio::test]
async fn linked_grounding_file_is_rejected_even_when_its_digest_matches() {
    let temp = TempDir::new().expect("temp dir");
    let transport = FakeTransport::new(FakeOptions::default());
    let mut config = launch_config(&temp);
    config.profile = GuiProfile::WindowVision;
    let mut session = driver(config, Arc::clone(&transport))
        .open(&context(&temp))
        .await
        .expect("GUI session");
    let observation = session.observe().await.expect("visual observation");
    let visual_ref = observation.data["visual"]["ref"]
        .as_str()
        .expect("visual ref")
        .to_string();
    let evidence = PathBuf::from(&observation.evidence[0].path);
    let outside = temp.path().join("outside-same-bytes.png");
    tokio::fs::write(&outside, b"fake-png")
        .await
        .expect("outside image");
    tokio::fs::remove_file(&evidence)
        .await
        .expect("remove grounding image");
    if let Err(error) = symlink_file(&outside, &evidence) {
        if unavailable_without_host_privilege(&error) {
            session.close().await.expect("close skipped session");
            return;
        }
        panic!("failed to create grounding symlink: {error}");
    }

    let error = session
        .execute(&TestStep {
            id: "linked-grounding".to_string(),
            action: Action::Click {
                target: Target::VisualPoint {
                    snapshot: visual_ref,
                    x: 10,
                    y: 10,
                },
            },
            stability: None,
            assertion_mode: Default::default(),
            wait_mode: Default::default(),
        })
        .await
        .expect_err("linked grounding image must be stale");
    assert_eq!(error.code(), "test.driver.gui.stale_image");
    assert!(transport.calls_for("click").await.is_empty());

    session.close().await.expect("close session");
}
