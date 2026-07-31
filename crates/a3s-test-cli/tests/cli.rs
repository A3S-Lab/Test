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
