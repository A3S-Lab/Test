#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_a3s-test"))
}

#[test]
fn legacy_sessions_reject_turns_but_preserve_abort_and_finish_cleanup() {
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
printf '%s|%s\n' "$*" "${AGENT_BROWSER_ALLOWED_DOMAINS-}" >> "$A3S_TEST_LOG"
printf '{"success":true}\n'
"#,
    )
    .expect("driver");
    fs::set_permissions(&driver, fs::Permissions::from_mode(0o755)).expect("permissions");

    let abort_session = "legacy-abort";
    let start = start_session(temp.path(), &driver, &log, abort_session);
    assert!(start.status.success(), "{start:?}");
    let abort_runtime = strip_browser_policy(temp.path(), abort_session);
    let log_before_observe = fs::read_to_string(&log).expect("driver log");

    let observe = run_agent(
        temp.path(),
        &log,
        &[
            "agent",
            "observe",
            "--session",
            abort_session,
            "--interactive",
            "--json",
        ],
    );
    assert_eq!(observe.status.code(), Some(1), "{observe:?}");
    let observe_json: serde_json::Value =
        serde_json::from_slice(&observe.stdout).expect("observe JSON");
    assert_eq!(
        observe_json["error"]["code"],
        "test.session.browser_network_policy_missing"
    );
    assert_eq!(
        observe_json["next"],
        "a3s-test agent abort --session legacy-abort --json"
    );
    assert_eq!(
        fs::read_to_string(&log).expect("driver log after rejected observe"),
        log_before_observe,
        "rejected observe dispatched a browser command"
    );

    let abort = run_agent(
        temp.path(),
        &log,
        &["agent", "abort", "--session", abort_session, "--json"],
    );
    assert!(abort.status.success(), "{abort:?}");
    assert!(!abort_runtime.exists(), "abort left its runtime directory");

    let finish_session = "legacy-finish";
    let start = start_session(temp.path(), &driver, &log, finish_session);
    assert!(start.status.success(), "{start:?}");
    let finish_runtime = strip_browser_policy(temp.path(), finish_session);
    let log_before_action = fs::read_to_string(&log).expect("driver log");

    let action = run_agent(
        temp.path(),
        &log,
        &[
            "agent",
            "click",
            "#continue",
            "--session",
            finish_session,
            "--json",
        ],
    );
    assert_eq!(action.status.code(), Some(1), "{action:?}");
    let action_json: serde_json::Value =
        serde_json::from_slice(&action.stdout).expect("action JSON");
    assert_eq!(
        action_json["error"]["code"],
        "test.session.browser_network_policy_missing"
    );
    assert_eq!(
        fs::read_to_string(&log).expect("driver log after rejected action"),
        log_before_action,
        "rejected action dispatched a browser command"
    );

    let finish = run_agent(
        temp.path(),
        &log,
        &[
            "agent",
            "finish",
            "--session",
            finish_session,
            "--status",
            "passed",
            "--summary",
            "Legacy session was closed without another browser turn",
            "--json",
        ],
    );
    assert!(finish.status.success(), "{finish:?}");
    let finish_json: serde_json::Value =
        serde_json::from_slice(&finish.stdout).expect("finish JSON");
    assert_eq!(finish_json["status"], "passed");
    assert_eq!(
        finish_json["report"]["browser_allowed_domains"],
        serde_json::json!([])
    );
    assert!(
        !finish_runtime.exists(),
        "finish left its runtime directory"
    );

    let driver_log = fs::read_to_string(&log).expect("final driver log");
    assert!(!driver_log.contains(" snapshot "), "{driver_log}");
    assert!(!driver_log.contains(" click "), "{driver_log}");
    let close_lines = driver_log
        .lines()
        .filter(|line| {
            line.split('|')
                .next()
                .is_some_and(|args| args.ends_with(" close"))
        })
        .collect::<Vec<_>>();
    assert_eq!(close_lines.len(), 2, "{driver_log}");
    assert!(
        close_lines.iter().all(|line| line.ends_with('|')),
        "legacy cleanup must not claim a retrofitted domain policy: {driver_log}"
    );

    for session in [abort_session, finish_session] {
        let events = fs::read_to_string(session_root(temp.path(), session).join("events.jsonl"))
            .expect("event log");
        assert!(
            events.contains("test.session.browser_network_policy_missing"),
            "missing policy failure event for {session}: {events}"
        );
    }
}

fn start_session(workspace: &Path, driver: &Path, log: &Path, session: &str) -> Output {
    run_agent(
        workspace,
        log,
        &[
            "agent",
            "start",
            "https://example.test",
            "--session",
            session,
            "--goal",
            "Verify legacy session migration behavior",
            "--success",
            "The browser runtime is closed without an uncontained turn",
            "--browser-driver",
            "standalone",
            "--browser-executable",
            driver.to_str().expect("driver path"),
            "--json",
        ],
    )
}

fn strip_browser_policy(workspace: &Path, session: &str) -> PathBuf {
    let state_path = session_root(workspace, session).join("session.json");
    let mut state: serde_json::Value =
        serde_json::from_slice(&fs::read(&state_path).expect("session state"))
            .expect("session JSON");
    let runtime = PathBuf::from(state["runtime_dir"].as_str().expect("runtime directory"));
    assert!(
        state
            .as_object_mut()
            .expect("session object")
            .remove("browser_allowed_domains")
            .is_some(),
        "new session did not persist a browser policy"
    );
    fs::write(
        state_path,
        serde_json::to_vec_pretty(&state).expect("encoded session state"),
    )
    .expect("legacy session state");
    runtime
}

fn run_agent(workspace: &Path, log: &Path, args: &[&str]) -> Output {
    Command::new(binary())
        .args(args)
        .current_dir(workspace)
        .env("A3S_TEST_LOG", log)
        .env_remove("AGENT_BROWSER_ALLOWED_DOMAINS")
        .output()
        .expect("run a3s-test agent command")
}

fn session_root(workspace: &Path, session: &str) -> PathBuf {
    workspace
        .join(".a3s-test")
        .join("agent-sessions")
        .join(session)
}
