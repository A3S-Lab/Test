#[cfg(unix)]
mod unix {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::process::Command;

    fn binary() -> PathBuf {
        PathBuf::from(env!("CARGO_BIN_EXE_a3s-test"))
    }

    #[test]
    fn long_external_session_names_use_socket_safe_driver_ids() {
        let temp = tempfile::tempdir().expect("tempdir");
        let driver = temp.path().join("fake-agent-browser");
        let log = temp.path().join("driver.log");
        fs::write(
            &driver,
            r#"#!/bin/sh
case " $* " in
  *" --version "*)
    printf 'agent-browser 0.26.0\n'
    exit 0
    ;;
esac
printf '%s\n' "$*" >> "$A3S_TEST_LOG"
printf '{"success":true}\n'
"#,
        )
        .expect("driver");
        fs::set_permissions(&driver, fs::Permissions::from_mode(0o755)).expect("permissions");

        let session = "office-presentation-font-v041";
        let start = Command::new(binary())
            .args([
                "agent",
                "start",
                "https://example.test",
                "--session",
                session,
                "--goal",
                "Verify long session names",
                "--success",
                "The browser session starts",
                "--browser-driver",
                "standalone",
                "--browser-executable",
                driver.to_str().expect("driver path"),
                "--json",
            ])
            .current_dir(temp.path())
            .env("A3S_TEST_LOG", &log)
            .output()
            .expect("start");
        assert!(start.status.success(), "{start:?}");

        let state_path = temp
            .path()
            .join(".a3s-test/agent-sessions")
            .join(session)
            .join("session.json");
        let state: serde_json::Value =
            serde_json::from_slice(&fs::read(state_path).expect("state")).expect("state JSON");
        let driver_session = state["driver_session"].as_str().expect("driver session");
        assert_eq!(driver_session.len(), 28);
        assert!(
            driver_session.starts_with("agent-offic-"),
            "{driver_session}"
        );

        let driver_log = fs::read_to_string(&log).expect("driver log");
        assert!(
            driver_log
                .lines()
                .any(|line| line.contains(&format!("--session {driver_session} --json open"))),
            "{driver_log}"
        );

        let abort = Command::new(binary())
            .args(["agent", "abort", "--session", session, "--json"])
            .current_dir(temp.path())
            .env("A3S_TEST_LOG", &log)
            .output()
            .expect("abort");
        assert!(abort.status.success(), "{abort:?}");
    }
}
