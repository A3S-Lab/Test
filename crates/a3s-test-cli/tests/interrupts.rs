#![cfg(unix)]

use std::collections::HashSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_a3s-test"))
}

#[test]
fn first_sigint_cancels_and_reaps_the_command_process_group() {
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

#[test]
fn second_sigint_forces_exit_and_reaps_cleanup_commands() {
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

#[test]
fn sigkill_during_agent_start_triggers_watchdog_and_retains_cleanup_metadata() {
    let _test_guard = process_test_lock().lock().unwrap();
    let runtimes_before = runtime_directories();
    let temp = tempfile::tempdir().expect("tempdir");
    let driver = temp.path().join("fake-agent-browser");

    fs::write(
        &driver,
        r#"#!/bin/sh
case " $* " in
  *" --version "*)
    printf 'agent-browser 0.26.0\n'
    exit 0
    ;;
  *" open "*)
    printf '%s\n' "$$" > "$AGENT_BROWSER_SOCKET_DIR/agent-crash-recovery.pid"
    sleep 30
    ;;
  *" close "*)
    printf '{"success":true}\n'
    exit 0
    ;;
esac
printf '{"success":true}\n'
"#,
    )
    .expect("driver");
    fs::set_permissions(&driver, fs::Permissions::from_mode(0o755)).expect("permissions");

    let mut child = Command::new(binary())
        .args([
            "agent",
            "start",
            "https://example.test",
            "--session",
            "crash-recovery",
            "--goal",
            "Prove exact orphan cleanup",
            "--success",
            "The orphan is reaped",
            "--browser-driver",
            "standalone",
            "--browser-executable",
            driver.to_str().unwrap(),
            "--command-timeout-ms",
            "60000",
            "--idle-timeout-ms",
            "60000",
            "--json",
        ])
        .current_dir(temp.path())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn agent start");

    let state_path = temp
        .path()
        .join(".a3s-test/agent-sessions/crash-recovery/session.json");
    wait_for_file(&state_path, Duration::from_secs(5));
    let state: serde_json::Value =
        serde_json::from_slice(&fs::read(&state_path).expect("recovery state"))
            .expect("recovery state JSON");
    let runtime = PathBuf::from(state["runtime_dir"].as_str().expect("runtime path"));
    let daemon_pid_path = runtime.join("agent-crash-recovery.pid");
    wait_for_file(&daemon_pid_path, Duration::from_secs(5));
    let daemon_pid = fs::read_to_string(&daemon_pid_path)
        .expect("daemon PID")
        .trim()
        .to_string();
    wait_for_child_command(
        child.id(),
        "a3s-test-browser-watchdog",
        Duration::from_secs(5),
    );

    let signal = Command::new("kill")
        .args(["-KILL", &child.id().to_string()])
        .status()
        .expect("kill agent host");
    assert!(signal.success());
    let _ = child.wait();
    let watchdog_stopped = wait_until_not_alive_result(&daemon_pid, Duration::from_secs(3));

    let abort = Command::new(binary())
        .args(["agent", "abort", "--session", "crash-recovery", "--json"])
        .current_dir(temp.path())
        .output()
        .expect("abort interrupted session");
    let stopped = wait_until_not_alive_result(&daemon_pid, Duration::from_secs(3));
    if !stopped {
        let _ = Command::new("kill").args(["-KILL", &daemon_pid]).status();
    }

    assert!(
        watchdog_stopped,
        "browser watchdog did not stop process group {daemon_pid} after host SIGKILL"
    );
    assert!(abort.status.success(), "{abort:?}");
    assert!(
        stopped,
        "browser wrapper {daemon_pid} survived recovery abort"
    );
    assert!(
        !runtime.exists(),
        "interrupted session runtime survived abort"
    );
    assert_no_new_runtimes(&runtimes_before);
}

fn send_sigint(pid: u32) {
    let signal = Command::new("kill")
        .args(["-INT", &pid.to_string()])
        .status()
        .expect("send SIGINT");
    assert!(signal.success());
}

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

fn wait_for_child_command(parent_pid: u32, marker: &str, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let found = Command::new("ps")
            .args(["-axo", "ppid=,command="])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .is_some_and(|output| {
                String::from_utf8_lossy(&output.stdout).lines().any(|line| {
                    let mut fields = line.trim_start().splitn(2, char::is_whitespace);
                    fields.next().and_then(|value| value.parse::<u32>().ok()) == Some(parent_pid)
                        && fields
                            .next()
                            .is_some_and(|command| command.contains(marker))
                })
            });
        if found {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for child command marker {marker:?} under host {parent_pid}");
}

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

fn wait_until_not_alive(pid: &str, timeout: Duration) {
    assert!(
        wait_until_not_alive_result(pid, timeout),
        "grandchild process {pid} survived cancellation"
    );
}

fn wait_until_not_alive_result(pid: &str, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if !process_is_alive(pid) {
            return true;
        }
        thread::sleep(Duration::from_millis(10));
    }
    false
}

fn process_is_alive(pid: &str) -> bool {
    Command::new("kill")
        .args(["-0", pid])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn process_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

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

fn assert_no_new_runtimes(before: &HashSet<PathBuf>) {
    let after = runtime_directories();
    let leaked = after.difference(before).collect::<Vec<_>>();
    assert!(leaked.is_empty(), "leaked runtime directories: {leaked:?}");
}
