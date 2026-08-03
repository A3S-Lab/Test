#[cfg(unix)]
use std::collections::HashSet;
#[cfg(unix)]
use std::fs;
use std::path::PathBuf;
use std::process::Command;
#[cfg(unix)]
use std::sync::{Mutex, OnceLock};

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_a3s-test"))
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
fn check_returns_machine_readable_suite() {
    let output = Command::new(binary())
        .args([
            "check",
            workspace_root()
                .join("examples/web-smoke.acl")
                .to_str()
                .unwrap(),
            "--json",
        ])
        .output()
        .expect("run a3s-test check");

    assert!(output.status.success(), "{output:?}");
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON output");
    assert_eq!(value["name"], "web-smoke");
    assert_eq!(value["scenarios"][0]["surface"], "web");
}

#[test]
fn agent_schema_exposes_values_for_semantic_targets() {
    let output = Command::new(binary())
        .args(["agent", "schema"])
        .output()
        .expect("run a3s-test agent schema");

    assert!(output.status.success(), "{output:?}");
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON output");
    assert_eq!(value["planner"], "external_coding_agent");
    assert_eq!(value["protocol_revision"], 5);
    let action_types = value["action_schema"]["oneOf"]
        .as_array()
        .expect("action variants");
    for kind in [
        "hover",
        "focus",
        "double_click",
        "context_click",
        "type",
        "check",
        "uncheck",
        "select",
        "drag",
        "wheel",
        "viewport",
    ] {
        assert!(
            action_types
                .iter()
                .any(|action| action["properties"]["type"]["const"] == kind),
            "missing {kind} action"
        );
    }
    let targets = value["action_schema"]["$defs"]["Target"]["oneOf"]
        .as_array()
        .expect("target variants");
    for kind in ["test_id", "label", "placeholder"] {
        let target = targets
            .iter()
            .find(|target| target["properties"]["type"]["const"] == kind)
            .unwrap_or_else(|| panic!("missing {kind} target"));
        assert_eq!(target["properties"]["value"]["type"], "string");
        assert!(target["required"]
            .as_array()
            .is_some_and(|required| required.iter().any(|field| field == "value")));
    }
}

#[cfg(unix)]
#[test]
fn capabilities_returns_the_admitted_web_protocol() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().expect("tempdir");
    let driver = temp.path().join("fake-agent-browser");
    fs::write(&driver, "#!/bin/sh\nprintf 'agent-browser 0.26.0\\n'\n").expect("driver");
    fs::set_permissions(&driver, fs::Permissions::from_mode(0o755)).expect("permissions");

    let output = Command::new(binary())
        .args([
            "capabilities",
            "--browser-driver",
            "standalone",
            "--browser-executable",
            driver.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run a3s-test capabilities");

    assert!(output.status.success(), "{output:?}");
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON output");
    assert_eq!(value["integration"], "standalone");
    assert_eq!(value["version"], "0.26.0");
    assert_eq!(value["protocol_revision"], 5);
    assert!(value["features"].as_array().is_some_and(|features| {
        features
            .iter()
            .any(|feature| feature.as_str() == Some("tabs"))
    }));
    assert!(value["features"].as_array().is_some_and(|features| {
        features
            .iter()
            .any(|feature| feature.as_str() == Some("context_clicks"))
    }));
    assert!(value["features"].as_array().is_some_and(|features| {
        features
            .iter()
            .any(|feature| feature.as_str() == Some("domain_containment"))
    }));
}

#[cfg(unix)]
#[test]
fn coding_agent_can_drive_a_persistent_typed_test_session() {
    use std::os::unix::fs::PermissionsExt;

    let _test_guard = process_test_lock().lock().unwrap();
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
printf '%s|%s|%s|%s\n' "$AGENT_BROWSER_NAMESPACE" "$AGENT_BROWSER_SOCKET_DIR" "$*" "$AGENT_BROWSER_ALLOWED_DOMAINS" >> "$A3S_TEST_LOG"
case " $* " in
  *" snapshot "*)
    printf '{"success":true,"data":{"origin":"https://example.test/checkout","snapshot":"@e1 [button] Continue"}}\n'
    ;;
  *)
    printf '{"success":true}\n'
    ;;
esac
"#,
    )
    .expect("driver");
    fs::set_permissions(&driver, fs::Permissions::from_mode(0o755)).expect("permissions");

    let start = Command::new(binary())
        .args([
            "agent",
            "start",
            "https://example.test/checkout",
            "--session",
            "checkout",
            "--goal",
            "Complete the checkout smoke test",
            "--success",
            "The Continue action succeeds",
            "--allow-domain",
            "cdn.example.test",
            "--browser-driver",
            "standalone",
            "--browser-executable",
            driver.to_str().unwrap(),
            "--json",
        ])
        .current_dir(temp.path())
        .env("A3S_TEST_LOG", &log)
        .output()
        .expect("start agent session");
    assert!(start.status.success(), "{start:?}");
    let start_json: serde_json::Value = serde_json::from_slice(&start.stdout).expect("start JSON");
    assert_eq!(start_json["session"], "checkout");
    assert_eq!(start_json["status"], "active");
    assert_eq!(
        start_json["browser_allowed_domains"],
        serde_json::json!(["cdn.example.test", "example.test"])
    );

    let observe = Command::new(binary())
        .args([
            "agent",
            "observe",
            "--session",
            "checkout",
            "--interactive",
            "--json",
        ])
        .current_dir(temp.path())
        .env("A3S_TEST_LOG", &log)
        .output()
        .expect("observe agent session");
    assert!(observe.status.success(), "{observe:?}");
    let observation: serde_json::Value =
        serde_json::from_slice(&observe.stdout).expect("observation JSON");
    assert_eq!(observation["observation_id"], 1);
    assert_eq!(
        observation["output"]["data"]["data"]["snapshot"],
        "@e1 [button] Continue"
    );

    let act = Command::new(binary())
        .args([
            "agent",
            "click",
            "@e1",
            "--session",
            "checkout",
            "--observation",
            "1",
            "--json",
        ])
        .current_dir(temp.path())
        .env("A3S_TEST_LOG", &log)
        .output()
        .expect("act in agent session");
    assert!(act.status.success(), "{act:?}");

    let second_observe = Command::new(binary())
        .args([
            "agent",
            "observe",
            "--session",
            "checkout",
            "--interactive",
            "--json",
        ])
        .current_dir(temp.path())
        .env("A3S_TEST_LOG", &log)
        .output()
        .expect("observe agent session after action");
    assert!(second_observe.status.success(), "{second_observe:?}");
    let second_observation: serde_json::Value =
        serde_json::from_slice(&second_observe.stdout).expect("second observation JSON");
    assert_eq!(second_observation["observation_id"], 2);

    let finish = Command::new(binary())
        .args([
            "agent",
            "finish",
            "--session",
            "checkout",
            "--status",
            "passed",
            "--summary",
            "Checkout action completed",
            "--json",
        ])
        .current_dir(temp.path())
        .env("A3S_TEST_LOG", &log)
        .output()
        .expect("finish agent session");
    assert!(finish.status.success(), "{finish:?}");
    let finish_json: serde_json::Value =
        serde_json::from_slice(&finish.stdout).expect("finish JSON");
    assert_eq!(finish_json["status"], "passed");

    let session_root = temp.path().join(".a3s-test/agent-sessions/checkout");
    assert!(session_root.join("report.json").is_file());
    let state: serde_json::Value =
        serde_json::from_slice(&fs::read(session_root.join("session.json")).expect("state"))
            .expect("state JSON");
    let runtime = PathBuf::from(state["runtime_dir"].as_str().expect("runtime path"));
    assert!(!runtime.exists(), "runtime directory survived finish");

    let driver_log = fs::read_to_string(log).expect("driver log");
    assert!(
        driver_log
            .lines()
            .any(|line| line.ends_with(" open https://example.test/checkout")),
        "{driver_log}"
    );
    assert!(
        driver_log
            .lines()
            .any(|line| line.ends_with(" snapshot -i")),
        "{driver_log}"
    );
    assert!(
        driver_log.lines().any(|line| line.ends_with(" click @e1")),
        "{driver_log}"
    );
    assert!(
        driver_log.lines().any(|line| line.ends_with(" close")),
        "{driver_log}"
    );
    let sessions = driver_log
        .lines()
        .filter_map(|line| line.split('|').nth(2))
        .filter_map(|args| {
            let values = args.split_whitespace().collect::<Vec<_>>();
            values
                .iter()
                .position(|value| *value == "--session")
                .and_then(|index| values.get(index + 1))
                .copied()
        })
        .collect::<HashSet<_>>();
    assert_eq!(sessions, HashSet::from(["agent-checkout"]));
    let domain_policies = driver_log
        .lines()
        .filter_map(|line| line.split('|').nth(3))
        .collect::<HashSet<_>>();
    assert_eq!(
        domain_policies,
        HashSet::from(["cdn.example.test,example.test"])
    );
}

#[cfg(unix)]
#[test]
fn compact_agent_commands_cover_advanced_office_interactions() {
    use std::os::unix::fs::PermissionsExt;

    let _test_guard = process_test_lock().lock().unwrap();
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
case " $* " in
  *" get box "*)
    printf '{"success":true,"data":{"x":10,"y":20,"width":100,"height":50}}\n'
    ;;
  *)
    printf '{"success":true}\n'
    ;;
esac
"#,
    )
    .expect("driver");
    fs::set_permissions(&driver, fs::Permissions::from_mode(0o755)).expect("permissions");

    let start = start_agent_session(temp.path(), &driver, &log, "advanced-actions");
    assert!(start.status.success(), "{start:?}");

    let commands = [
        vec!["agent", "hover", "#target"],
        vec!["agent", "focus", "#target"],
        vec!["agent", "double-click", "#target"],
        vec!["agent", "context-click", "#target"],
        vec!["agent", "type", "#target", "more text"],
        vec!["agent", "check", "#target"],
        vec!["agent", "uncheck", "#target"],
        vec!["agent", "select", "#target", "draft", "review"],
        vec!["agent", "drag", "#source", "#target"],
        vec![
            "agent",
            "wheel",
            "-120",
            "--delta-x",
            "4",
            "--modifier",
            "control",
            "--modifier",
            "shift",
        ],
        vec!["agent", "viewport", "1440", "900", "--scale", "2"],
    ];
    for mut command in commands {
        command.extend(["--session", "advanced-actions", "--json"]);
        let output = Command::new(binary())
            .args(command)
            .current_dir(temp.path())
            .env("A3S_TEST_LOG", &log)
            .output()
            .expect("advanced compact action");
        assert!(output.status.success(), "{output:?}");
    }

    let abort = Command::new(binary())
        .args(["agent", "abort", "--session", "advanced-actions", "--json"])
        .current_dir(temp.path())
        .env("A3S_TEST_LOG", &log)
        .output()
        .expect("abort");
    assert!(abort.status.success(), "{abort:?}");

    let driver_log = fs::read_to_string(log).expect("driver log");
    for expected in [
        " hover #target",
        " focus #target",
        " dblclick #target",
        " get box #target",
        " mouse move 60 45",
        " type #target more text",
        " check #target",
        " uncheck #target",
        " select #target draft review",
        " drag #source #target",
        " keydown Control",
        " keydown Shift",
        " mouse wheel -120 4",
        " keyup Shift",
        " keyup Control",
        " set viewport 1440 900 2",
    ] {
        assert!(
            driver_log.lines().any(|line| line.ends_with(expected)),
            "missing {expected:?} in {driver_log}"
        );
    }
    assert!(
        driver_log.lines().any(|line| {
            line.contains(" eval (() => { const target = document.elementFromPoint(60, 45);")
                && line.contains("new MouseEvent('contextmenu'")
        }),
        "missing page-scoped context-menu event in {driver_log}"
    );
}

#[cfg(unix)]
#[test]
fn agent_observe_rejects_a_silently_replaced_browser_page() {
    use std::os::unix::fs::PermissionsExt;

    let _test_guard = process_test_lock().lock().unwrap();
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
case " $* " in
  *" snapshot "*)
    printf '{"success":true,"data":{"origin":"about:blank","snapshot":"(no interactive elements)"}}\n'
    ;;
  *)
    printf '{"success":true}\n'
    ;;
esac
"#,
    )
    .expect("driver");
    fs::set_permissions(&driver, fs::Permissions::from_mode(0o755)).expect("permissions");

    let start = start_agent_session(temp.path(), &driver, &log, "origin-lost");
    assert!(start.status.success(), "{start:?}");

    let observe = Command::new(binary())
        .args([
            "agent",
            "observe",
            "--session",
            "origin-lost",
            "--interactive",
            "--json",
        ])
        .current_dir(temp.path())
        .env("A3S_TEST_LOG", &log)
        .output()
        .expect("observe replaced browser page");
    assert_eq!(observe.status.code(), Some(1), "{observe:?}");
    let value: serde_json::Value = serde_json::from_slice(&observe.stdout).expect("observe JSON");
    assert_eq!(
        value["error"]["code"],
        "test.driver.web.session_origin_lost"
    );
    assert_eq!(value["status"], "active");

    let abort = Command::new(binary())
        .args(["agent", "abort", "--session", "origin-lost", "--json"])
        .current_dir(temp.path())
        .env("A3S_TEST_LOG", &log)
        .output()
        .expect("abort");
    assert!(abort.status.success(), "{abort:?}");
}

#[cfg(unix)]
#[test]
fn failed_agent_action_invalidates_observation_refs_before_retry() {
    use std::os::unix::fs::PermissionsExt;

    let _test_guard = process_test_lock().lock().unwrap();
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
case " $* " in
  *" snapshot "*)
    printf '{"success":true,"data":{"origin":"https://example.test/","snapshot":"@e1 [button] Continue"}}\n'
    ;;
  *" click "*)
    if [ "${A3S_TEST_FAIL_CLICK:-}" = "1" ]; then
      printf '{"success":false,"error":"click failed"}\n'
      exit 1
    fi
    printf '{"success":true}\n'
    ;;
  *)
    printf '{"success":true}\n'
    ;;
esac
"#,
    )
    .expect("driver");
    fs::set_permissions(&driver, fs::Permissions::from_mode(0o755)).expect("permissions");

    let start = start_agent_session(temp.path(), &driver, &log, "failed-action");
    assert!(start.status.success(), "{start:?}");

    let observe = Command::new(binary())
        .args([
            "agent",
            "observe",
            "--session",
            "failed-action",
            "--interactive",
            "--json",
        ])
        .current_dir(temp.path())
        .env("A3S_TEST_LOG", &log)
        .output()
        .expect("observe");
    assert!(observe.status.success(), "{observe:?}");

    let failed_click = Command::new(binary())
        .args([
            "agent",
            "click",
            "@e1",
            "--session",
            "failed-action",
            "--observation",
            "1",
            "--json",
        ])
        .current_dir(temp.path())
        .env("A3S_TEST_LOG", &log)
        .env("A3S_TEST_FAIL_CLICK", "1")
        .output()
        .expect("failed click");
    assert_eq!(failed_click.status.code(), Some(1), "{failed_click:?}");

    let state_path = temp
        .path()
        .join(".a3s-test/agent-sessions/failed-action/session.json");
    let state: serde_json::Value =
        serde_json::from_slice(&fs::read(&state_path).expect("state")).expect("state JSON");
    assert!(state["latest_observation"].is_null());

    let stale_retry = Command::new(binary())
        .args([
            "agent",
            "click",
            "@e1",
            "--session",
            "failed-action",
            "--observation",
            "1",
            "--json",
        ])
        .current_dir(temp.path())
        .env("A3S_TEST_LOG", &log)
        .output()
        .expect("stale retry");
    assert!(!stale_retry.status.success(), "{stale_retry:?}");

    let driver_log = fs::read_to_string(&log).expect("driver log");
    assert_eq!(
        driver_log
            .lines()
            .filter(|line| line.ends_with(" click @e1"))
            .count(),
        1,
        "{driver_log}"
    );

    let abort = Command::new(binary())
        .args(["agent", "abort", "--session", "failed-action", "--json"])
        .current_dir(temp.path())
        .env("A3S_TEST_LOG", &log)
        .output()
        .expect("abort");
    assert!(abort.status.success(), "{abort:?}");
}

#[cfg(unix)]
#[test]
fn failed_finish_preserves_runtime_until_exact_cleanup_can_be_retried() {
    use std::os::unix::fs::PermissionsExt;

    let _test_guard = process_test_lock().lock().unwrap();
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
case " $* " in
  *" close "*)
    if [ "${A3S_TEST_FAIL_CLOSE:-}" = "1" ]; then
      printf '{"success":false,"error":"close failed"}\n'
      exit 1
    fi
    ;;
esac
printf '{"success":true}\n'
"#,
    )
    .expect("driver");
    fs::set_permissions(&driver, fs::Permissions::from_mode(0o755)).expect("permissions");

    let start = start_agent_session(temp.path(), &driver, &log, "cleanup-retry");
    assert!(start.status.success(), "{start:?}");

    let state_path = temp
        .path()
        .join(".a3s-test/agent-sessions/cleanup-retry/session.json");
    let initial_state: serde_json::Value =
        serde_json::from_slice(&fs::read(&state_path).expect("state")).expect("state JSON");
    let runtime = PathBuf::from(initial_state["runtime_dir"].as_str().expect("runtime path"));

    let finish = Command::new(binary())
        .args([
            "agent",
            "finish",
            "--session",
            "cleanup-retry",
            "--status",
            "passed",
            "--summary",
            "Product behavior passed",
            "--json",
        ])
        .current_dir(temp.path())
        .env("A3S_TEST_LOG", &log)
        .env("A3S_TEST_FAIL_CLOSE", "1")
        .output()
        .expect("finish with failed cleanup");
    assert_eq!(finish.status.code(), Some(1), "{finish:?}");
    let finish_json: serde_json::Value =
        serde_json::from_slice(&finish.stdout).expect("finish JSON");
    assert_eq!(finish_json["status"], "failed");
    assert!(finish_json["cleanup_error"].is_object());
    assert!(runtime.is_dir(), "runtime was removed after failed cleanup");

    let abort = Command::new(binary())
        .args(["agent", "abort", "--session", "cleanup-retry", "--json"])
        .current_dir(temp.path())
        .env("A3S_TEST_LOG", &log)
        .output()
        .expect("retry exact cleanup");
    assert!(abort.status.success(), "{abort:?}");
    let abort_json: serde_json::Value = serde_json::from_slice(&abort.stdout).expect("abort JSON");
    assert!(abort_json["cleanup_error"].is_null());
    assert!(
        !runtime.exists(),
        "runtime survived successful cleanup retry"
    );
}

#[cfg(unix)]
fn start_agent_session(
    workspace: &std::path::Path,
    driver: &std::path::Path,
    log: &std::path::Path,
    session: &str,
) -> std::process::Output {
    Command::new(binary())
        .args([
            "agent",
            "start",
            "https://example.test",
            "--session",
            session,
            "--goal",
            "Exercise the agent session lifecycle",
            "--success",
            "The exact browser session is cleaned up",
            "--browser-driver",
            "standalone",
            "--browser-executable",
            driver.to_str().unwrap(),
            "--json",
        ])
        .current_dir(workspace)
        .env("A3S_TEST_LOG", log)
        .output()
        .expect("start agent session")
}

#[cfg(unix)]
fn process_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}
