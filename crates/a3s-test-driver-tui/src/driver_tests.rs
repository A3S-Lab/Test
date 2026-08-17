#[cfg(any(unix, windows))]
use std::ffi::OsString;
#[cfg(windows)]
use std::io::Write as _;
#[cfg(any(unix, windows))]
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::process::Stdio;
#[cfg(any(unix, windows))]
use std::time::Duration;

#[cfg(any(unix, windows))]
use a3s_test_core::{
    Action, Expectation, ScenarioContext, SurfaceDriver as _, Target, TestStep, WaitCondition,
};
use serde_json::Value;

use super::*;
#[cfg(any(unix, windows))]
use crate::TuiCommand;

#[cfg(windows)]
const CONPTY_FIXTURE_TEST: &str = "driver::tests::windows_conpty_fixture";
#[cfg(windows)]
const CONPTY_FIXTURE_MODE_ENV: &str = "A3S_TEST_TUI_CONPTY_FIXTURE_MODE";
#[cfg(windows)]
const CONPTY_FIXTURE_PID_ENV: &str = "A3S_TEST_TUI_CONPTY_FIXTURE_PID_FILE";

#[cfg(unix)]
fn shell_command(script: &str) -> TuiCommand {
    let mut command = TuiCommand::new("/bin/sh");
    command.arguments = vec![OsString::from("-c"), OsString::from(script)];
    command
}

#[cfg(any(unix, windows))]
fn test_config(command: TuiCommand) -> TuiDriverConfig {
    TuiDriverConfig {
        command,
        initial_size: TuiSize::default(),
        command_timeout: if cfg!(windows) {
            Duration::from_secs(15)
        } else {
            Duration::from_secs(5)
        },
        cleanup_timeout: Duration::from_secs(5),
        scrollback_rows: 100,
        max_output_bytes: 64 * 1024,
    }
}

#[cfg(any(unix, windows))]
async fn open_session(command: TuiCommand, artifacts: PathBuf) -> Box<dyn DriverSession> {
    TuiDriver::new(test_config(command))
        .open(&ScenarioContext {
            run_id: "run".to_string(),
            scenario_id: "scenario".to_string(),
            artifacts_dir: artifacts,
        })
        .await
        .expect("open terminal session")
}

#[cfg(any(unix, windows))]
fn step(id: &str, action: Action) -> TestStep {
    TestStep {
        id: id.to_string(),
        action,
        stability: None,
        assertion_mode: Default::default(),
        wait_mode: Default::default(),
    }
}

#[cfg(any(unix, windows))]
async fn wait_text(session: &mut dyn DriverSession, text: &str) {
    session
        .execute(&step(
            "wait",
            Action::Wait {
                condition: WaitCondition::Text(text.to_string()),
            },
        ))
        .await
        .expect("wait for terminal text");
}

#[cfg(unix)]
#[tokio::test]
async fn target_bound_rendered_assertions_fail_closed_on_terminal_surfaces() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut session = open_session(
        shell_command("printf 'ready'; sleep 30"),
        temp.path().join("artifacts"),
    )
    .await;
    wait_text(session.as_mut(), "ready").await;

    let target = Target::Css {
        selector: "#status".to_string(),
    };
    for expectation in [
        Expectation::RenderedText {
            target: target.clone(),
            value: "Ready".to_string(),
        },
        Expectation::VisibleCount { target, count: 1 },
    ] {
        let error = session
            .execute(&step(
                "unsupported-rendered-assertion",
                Action::Assert { expectation },
            ))
            .await
            .expect_err("terminal surfaces must reject browser-rendered assertions");
        assert_eq!(error.code(), "test.driver.tui.action_unsupported");
    }

    session.close().await.expect("close terminal");
}

#[cfg(unix)]
#[tokio::test]
async fn real_pty_supports_input_resize_modes_waits_and_recording() {
    let temp = tempfile::tempdir().expect("temp dir");
    let script = "trap 'exit 0' TERM; \
                  stty -echo; \
                  printf '\\033[?1049h\\033[?1h\\033[?2004hready\\033[2;3H'; \
                  IFS= read -r line; \
                  printf '\\033[?2004l\\033[?1l\\033[?1049linput:%s size:' \"$line\"; \
                  stty size; \
                  sleep 30";
    let mut session = open_session(shell_command(script), temp.path().join("artifacts")).await;

    wait_text(session.as_mut(), "ready").await;
    let observation = session.observe().await.expect("observe modes");
    assert_eq!(observation.data["viewport"]["alternate_screen"], true);
    assert_eq!(observation.data["viewport"]["application_cursor"], true);
    assert_eq!(observation.data["viewport"]["bracketed_paste"], true);
    assert_eq!(observation.data["viewport"]["cursor"]["row"], 1);
    assert_eq!(observation.data["viewport"]["cursor"]["column"], 2);

    session
        .execute(&step(
            "resize",
            Action::TerminalResize {
                columns: 100,
                rows: 30,
            },
        ))
        .await
        .expect("resize terminal");
    session
        .execute(&step(
            "paste",
            Action::TerminalPaste {
                text: "hello".to_string(),
            },
        ))
        .await
        .expect("paste terminal input");
    session
        .execute(&step(
            "enter",
            Action::Press {
                key: "Enter".to_string(),
            },
        ))
        .await
        .expect("press enter");
    session
        .execute(&step(
            "regex",
            Action::Wait {
                condition: WaitCondition::Regex("input:hello.*size:30 100".to_string()),
            },
        ))
        .await
        .expect("wait for terminal regex");
    let restored = session.observe().await.expect("observe restored modes");
    assert_eq!(restored.data["viewport"]["alternate_screen"], false);
    assert_eq!(restored.data["viewport"]["application_cursor"], false);
    assert_eq!(restored.data["viewport"]["bracketed_paste"], false);
    let output = session
        .execute(&step(
            "record",
            Action::TerminalRecording {
                path: "terminal/session.vt".to_string(),
            },
        ))
        .await
        .expect("record terminal");
    let recording = &output.evidence[0].path;
    let bytes = tokio::fs::read(recording).await.expect("read recording");
    assert!(bytes
        .windows(b"size:30 100".len())
        .any(|part| part == b"size:30 100"));
    assert!(bytes
        .windows(b"\x1b[?1049h".len())
        .any(|part| part == b"\x1b[?1049h"));
    session.close().await.expect("close terminal");
}

#[cfg(windows)]
#[tokio::test]
async fn real_conpty_supports_input_and_reaps_its_job_tree() {
    let temp = tempfile::tempdir().expect("temp dir");
    let pid_file = temp.path().join("descendant.pid");
    let mut command =
        TuiCommand::new(std::env::current_exe().expect("current ConPTY fixture executable"));
    command.arguments = [CONPTY_FIXTURE_TEST, "--ignored", "--exact", "--nocapture"]
        .into_iter()
        .map(OsString::from)
        .collect();
    command.environment.insert(
        OsString::from(CONPTY_FIXTURE_MODE_ENV),
        OsString::from("parent"),
    );
    command.environment.insert(
        OsString::from(CONPTY_FIXTURE_PID_ENV),
        pid_file.clone().into_os_string(),
    );
    let mut session = open_session(command, temp.path().join("artifacts")).await;

    wait_text(session.as_mut(), "ready").await;
    session
        .execute(&step(
            "paste",
            Action::TerminalPaste {
                text: "hello".to_string(),
            },
        ))
        .await
        .expect("paste terminal input");
    session
        .execute(&step(
            "enter",
            Action::Press {
                key: "Enter".to_string(),
            },
        ))
        .await
        .expect("press enter");
    wait_text(session.as_mut(), "input:hello").await;
    let descendant = wait_for_pid(&pid_file).await;
    assert!(
        windows_process_is_running(descendant),
        "descendant exited before containment was tested"
    );

    session.close().await.expect("close ConPTY tree");
    let stopped = wait_until_stopped(descendant).await;
    if !stopped {
        terminate_fixture(descendant);
    }
    assert!(
        stopped,
        "Windows Job descendant {descendant} survived terminal close"
    );
}

#[cfg(windows)]
#[test]
#[ignore = "helper process for the real ConPTY lifecycle test"]
#[allow(
    clippy::zombie_processes,
    reason = "the fixture descendant must remain alive until the owning ConPTY Job closes"
)]
fn windows_conpty_fixture() {
    let mode = std::env::var(CONPTY_FIXTURE_MODE_ENV).expect("ConPTY fixture mode");
    if mode == "leaf" {
        std::thread::sleep(Duration::from_secs(30));
        return;
    }
    assert_eq!(mode, "parent", "unsupported ConPTY fixture mode");
    let pid_file = std::env::var_os(CONPTY_FIXTURE_PID_ENV)
        .map(PathBuf::from)
        .expect("ConPTY fixture descendant PID file");
    let mut child = std::process::Command::new(
        std::env::current_exe().expect("current ConPTY descendant executable"),
    )
    .args([CONPTY_FIXTURE_TEST, "--ignored", "--exact"])
    .env(CONPTY_FIXTURE_MODE_ENV, "leaf")
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .spawn()
    .expect("spawn ConPTY descendant fixture");
    std::fs::write(pid_file, child.id().to_string()).expect("publish ConPTY descendant PID");

    println!("ready");
    std::io::stdout().flush().expect("flush ConPTY readiness");
    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .expect("read ConPTY input");
    println!("input:{}", line.trim_end_matches(&['\r', '\n'][..]));
    std::io::stdout().flush().expect("flush ConPTY input echo");
    child
        .wait()
        .expect("wait for the ConPTY descendant fixture");
}

#[cfg(unix)]
#[tokio::test]
async fn wait_can_match_text_retained_only_in_scrollback() {
    let temp = tempfile::tempdir().expect("temp dir");
    let script = "printf 'scrollback-marker\\n'; i=0; while [ $i -lt 40 ]; do printf 'line-%s\\n' \"$i\"; i=$((i + 1)); done; sleep 30";
    let mut session = open_session(shell_command(script), temp.path().join("artifacts")).await;
    wait_text(session.as_mut(), "line-39").await;
    wait_text(session.as_mut(), "scrollback-marker").await;
    let observation = session.observe().await.expect("observe scrollback");
    assert!(observation.data["viewport"]["text"]
        .as_str()
        .is_some_and(|text| text.contains("scrollback-marker")));
    session.close().await.expect("close terminal");
}

#[cfg(unix)]
#[tokio::test]
async fn wait_drains_final_output_after_a_clean_process_exit() {
    let temp = tempfile::tempdir().expect("temp dir");
    for attempt in 0..25 {
        let mut session = open_session(
            shell_command("printf 'final-output\\n'"),
            temp.path().join(format!("artifacts-{attempt}")),
        )
        .await;
        wait_text(session.as_mut(), "final-output").await;
        session.close().await.expect("close terminal");
    }
}

#[cfg(unix)]
#[tokio::test]
async fn close_terminates_descendant_after_root_has_exited() {
    let temp = tempfile::tempdir().expect("temp dir");
    let pid_file = temp.path().join("descendant.pid");
    let script = format!(
        ": a3s-test-tui-descendant-fixture; trap '' HUP; sleep 30 & child=$!; printf '%s\\n' \"$child\" > {}; printf 'spawned\\n'; exit 0",
        shell_quote(&pid_file)
    );
    let mut session = open_session(shell_command(&script), temp.path().join("artifacts")).await;
    wait_text(session.as_mut(), "spawned").await;
    let descendant = wait_for_pid(&pid_file).await;
    session.close().await.expect("close terminal tree");
    let stopped = wait_until_stopped(descendant).await;
    if !stopped {
        terminate_fixture(descendant);
    }
    assert!(stopped, "descendant {descendant} survived terminal close");
}

#[cfg(unix)]
#[tokio::test]
async fn drop_terminates_the_exact_owned_process_group() {
    let temp = tempfile::tempdir().expect("temp dir");
    let pid_file = temp.path().join("descendant.pid");
    let script = format!(
        ": a3s-test-tui-descendant-fixture; sleep 30 & child=$!; printf '%s\\n' \"$child\" > {}; printf 'spawned\\n'; wait",
        shell_quote(&pid_file)
    );
    let mut session = open_session(shell_command(&script), temp.path().join("artifacts")).await;
    wait_text(session.as_mut(), "spawned").await;
    let descendant = wait_for_pid(&pid_file).await;
    drop(session);
    let stopped = wait_until_stopped(descendant).await;
    if !stopped {
        terminate_fixture(descendant);
    }
    assert!(stopped, "descendant {descendant} survived terminal drop");
}

#[cfg(unix)]
fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "'\\''"))
}

#[cfg(any(unix, windows))]
async fn wait_for_pid(path: &Path) -> u32 {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(source) = tokio::fs::read_to_string(path).await {
            if let Ok(process_id) = source.trim().parse::<u32>() {
                return process_id;
            }
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "PID was not published"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[cfg(any(unix, windows))]
async fn wait_until_stopped(process_id: u32) -> bool {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while process_is_running(process_id) {
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    true
}

#[cfg(unix)]
fn process_is_running(process_id: u32) -> bool {
    let Ok(process_id) = i32::try_from(process_id) else {
        return false;
    };
    match nix::sys::signal::kill(nix::unistd::Pid::from_raw(process_id), None) {
        Ok(()) => !is_zombie(process_id),
        Err(nix::errno::Errno::ESRCH) => false,
        Err(_) => true,
    }
}

#[cfg(windows)]
fn process_is_running(process_id: u32) -> bool {
    windows_process_is_running(process_id)
}

#[cfg(windows)]
fn windows_process_is_running(process_id: u32) -> bool {
    use std::os::windows::io::{AsRawHandle as _, FromRawHandle as _, OwnedHandle};
    use windows_sys::Win32::Foundation::STILL_ACTIVE;
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    let raw_handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id) };
    if raw_handle.is_null() {
        return false;
    }
    let handle = unsafe { OwnedHandle::from_raw_handle(raw_handle) };
    let mut exit_code = 0_u32;
    let queried = unsafe { GetExitCodeProcess(handle.as_raw_handle(), &mut exit_code) };
    queried != 0 && exit_code == u32::try_from(STILL_ACTIVE).expect("STILL_ACTIVE is positive")
}

#[cfg(target_os = "linux")]
fn is_zombie(process_id: i32) -> bool {
    std::fs::read_to_string(format!("/proc/{process_id}/stat"))
        .ok()
        .and_then(|source| source.rsplit_once(") ").map(|(_, tail)| tail.to_string()))
        .is_some_and(|tail| tail.starts_with("Z "))
}

#[cfg(all(unix, not(target_os = "linux")))]
fn is_zombie(_process_id: i32) -> bool {
    false
}

#[cfg(unix)]
fn terminate_fixture(process_id: u32) {
    if let Ok(process_id) = i32::try_from(process_id) {
        let _ = nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(process_id),
            nix::sys::signal::Signal::SIGKILL,
        );
    }
}

#[cfg(windows)]
fn terminate_fixture(process_id: u32) {
    use std::os::windows::io::{AsRawHandle as _, FromRawHandle as _, OwnedHandle};
    use windows_sys::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};

    let raw_handle = unsafe { OpenProcess(PROCESS_TERMINATE, 0, process_id) };
    if !raw_handle.is_null() {
        let handle = unsafe { OwnedHandle::from_raw_handle(raw_handle) };
        let _ = unsafe { TerminateProcess(handle.as_raw_handle(), 1) };
    }
}

#[test]
fn observation_shape_remains_bounded_and_typed() {
    let mut state = TerminalState::new(TuiSize::default(), 10, 1024);
    state.process(b"hello");
    let data: Value = state.data_with_history();
    assert_eq!(data["surface"], "tui");
    assert_eq!(data["viewport"]["columns"], 80);
    assert_eq!(data["process"]["running"], true);
}
