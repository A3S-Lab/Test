use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

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
    assert_eq!(value["protocol_revision"], 1);
    assert!(value["features"].as_array().is_some_and(|features| {
        features
            .iter()
            .any(|feature| feature.as_str() == Some("tabs"))
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
printf '%s|%s|%s\n' "$AGENT_BROWSER_NAMESPACE" "$AGENT_BROWSER_SOCKET_DIR" "$*" >> "$A3S_TEST_LOG"
case " $* " in
  *" snapshot "*)
    printf '{"success":true,"data":{"snapshot":"@e1 [button] Continue"}}\n'
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
    printf '{"success":true,"data":{"snapshot":"@e1 [button] Continue"}}\n'
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
#[test]
fn first_sigint_cancels_and_reaps_the_command_process_group() {
    use std::os::unix::fs::PermissionsExt;

    let _test_guard = process_test_lock().lock().unwrap();
    let runtimes_before = runtime_directories();
    let temp = tempfile::tempdir().expect("tempdir");
    let driver = temp.path().join("fake-agent-browser");
    let log = temp.path().join("driver.log");
    let grandchild_pid = temp.path().join("grandchild.pid");
    let suite = temp.path().join("suite.acl");

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
  *" open "*)
    sleep 30 &
    printf '%s\n' "$!" > "$A3S_TEST_GRANDCHILD_PID"
    wait
    ;;
esac
printf '{"success":true}\n'
"#,
    )
    .expect("driver");
    fs::set_permissions(&driver, fs::Permissions::from_mode(0o755)).expect("permissions");
    fs::write(
        &suite,
        r#"
suite "interrupt" {
    scenario "browser" {
        surface = "web"
        timeout_ms = 60000
        navigate "open" {
            url = "https://example.test"
        }
    }
}
"#,
    )
    .expect("suite");

    let mut child = Command::new(binary())
        .args([
            "run",
            suite.to_str().unwrap(),
            "--browser-driver",
            "standalone",
            "--browser-executable",
            driver.to_str().unwrap(),
            "--command-timeout-ms",
            "60000",
            "--cleanup-timeout-ms",
            "2000",
            "--json",
        ])
        .current_dir(temp.path())
        .env("A3S_TEST_LOG", &log)
        .env("A3S_TEST_GRANDCHILD_PID", &grandchild_pid)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn a3s-test");

    wait_for_file(&grandchild_pid, Duration::from_secs(5));
    let signal = Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .expect("send SIGINT");
    assert!(signal.success());

    wait_for_exit(&mut child, Duration::from_secs(5));
    let output = child.wait_with_output().expect("collect output");
    assert_eq!(output.status.code(), Some(130), "{output:?}");
    let log = fs::read_to_string(&log).expect("driver log");
    assert!(log.lines().any(|line| line.ends_with(" close")), "{log}");

    let pid = fs::read_to_string(&grandchild_pid)
        .expect("grandchild pid")
        .trim()
        .to_string();
    wait_until_not_alive(&pid, Duration::from_secs(2));
    assert_no_new_runtimes(&runtimes_before);
}

#[cfg(unix)]
#[test]
fn second_sigint_forces_exit_and_reaps_cleanup_commands() {
    use std::os::unix::fs::PermissionsExt;

    let _test_guard = process_test_lock().lock().unwrap();
    let runtimes_before = runtime_directories();
    let temp = tempfile::tempdir().expect("tempdir");
    let driver = temp.path().join("hanging-agent-browser");
    let log = temp.path().join("driver.log");
    let active_pid = temp.path().join("active.pid");
    let suite = temp.path().join("suite.acl");

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
sleep 30 &
printf '%s\n' "$!" > "$A3S_TEST_ACTIVE_PID"
wait
"#,
    )
    .expect("driver");
    fs::set_permissions(&driver, fs::Permissions::from_mode(0o755)).expect("permissions");
    fs::write(
        &suite,
        r#"
suite "interrupt-twice" {
    scenario "browser" {
        surface = "web"
        timeout_ms = 60000
        navigate "open" {
            url = "https://example.test"
        }
    }
}
"#,
    )
    .expect("suite");

    let mut child = Command::new(binary())
        .args([
            "run",
            suite.to_str().unwrap(),
            "--browser-driver",
            "standalone",
            "--browser-executable",
            driver.to_str().unwrap(),
            "--command-timeout-ms",
            "60000",
            "--cleanup-timeout-ms",
            "60000",
            "--json",
        ])
        .current_dir(temp.path())
        .env("A3S_TEST_LOG", &log)
        .env("A3S_TEST_ACTIVE_PID", &active_pid)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn a3s-test");

    wait_for_file(&active_pid, Duration::from_secs(5));
    send_sigint(child.id());
    wait_for_log_line(&log, " close", Duration::from_secs(5));
    send_sigint(child.id());

    wait_for_exit(&mut child, Duration::from_secs(5));
    let output = child.wait_with_output().expect("collect output");
    assert_eq!(output.status.code(), Some(130), "{output:?}");

    let pid = fs::read_to_string(&active_pid)
        .expect("active pid")
        .trim()
        .to_string();
    wait_until_not_alive(&pid, Duration::from_secs(2));
    assert_no_new_runtimes(&runtimes_before);
}

#[cfg(unix)]
fn send_sigint(pid: u32) {
    let signal = Command::new("kill")
        .args(["-INT", &pid.to_string()])
        .status()
        .expect("send SIGINT");
    assert!(signal.success());
}

#[cfg(unix)]
fn wait_for_file(path: &std::path::Path, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if path.is_file() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for {}", path.display());
}

#[cfg(unix)]
fn wait_for_log_line(path: &std::path::Path, suffix: &str, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let found = fs::read_to_string(path)
            .ok()
            .is_some_and(|log| log.lines().any(|line| line.ends_with(suffix)));
        if found {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for {suffix:?} in {}", path.display());
}

#[cfg(unix)]
fn wait_for_exit(child: &mut std::process::Child, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if child.try_wait().expect("poll child").is_some() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    let _ = child.kill();
    panic!("a3s-test did not stop after SIGINT");
}

#[cfg(unix)]
fn wait_until_not_alive(pid: &str, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let alive = Command::new("kill")
            .args(["-0", pid])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        if !alive {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("grandchild process {pid} survived cancellation");
}

#[cfg(unix)]
fn process_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[cfg(unix)]
fn runtime_directories() -> HashSet<PathBuf> {
    fs::read_dir("/tmp")
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("a3st-"))
        })
        .collect()
}

#[cfg(unix)]
fn assert_no_new_runtimes(before: &HashSet<PathBuf>) {
    let after = runtime_directories();
    let leaked = after.difference(before).collect::<Vec<_>>();
    assert!(leaked.is_empty(), "leaked runtime directories: {leaked:?}");
}
