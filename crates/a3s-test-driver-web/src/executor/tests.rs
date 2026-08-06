use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::Write as _;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::Duration;

#[cfg(unix)]
use nix::sys::signal::{kill, Signal};
#[cfg(unix)]
use nix::unistd::Pid;

use super::{CommandErrorKind, CommandExecutor, CommandInvocation, TokioCommandExecutor};
use crate::process::{terminate_process_group, SessionRegistration};
use crate::runtime::RuntimeDirectory;

const DESCENDANT_FIXTURE_TEST: &str = "executor::tests::successful_command_descendant_fixture";
const DESCENDANT_MODE_ENV: &str = "A3S_TEST_BROWSER_DESCENDANT_MODE";
const DESCENDANT_GATE_ENV: &str = "A3S_TEST_BROWSER_DESCENDANT_GATE";
const DESCENDANT_PARENT_ENV: &str = "A3S_TEST_BROWSER_PARENT_FILE";
const DESCENDANT_LEAF_ENV: &str = "A3S_TEST_BROWSER_LEAF_FILE";
const DESCENDANT_ATTACHED_ENV: &str = "A3S_TEST_BROWSER_ATTACHED_FILE";
const DESCENDANT_OUTPUT_PID_ENV: &str = "A3S_TEST_BROWSER_OUTPUT_PID_FILE";
#[cfg(windows)]
const DESCENDANT_EXE_ENV: &str = "A3S_TEST_BROWSER_DESCENDANT_EXE";
#[cfg(windows)]
const DESCENDANT_TEST_ENV: &str = "A3S_TEST_BROWSER_DESCENDANT_TEST";
#[cfg(windows)]
const CONSOLE_PROBE_TEST: &str = "executor::tests::windows_browser_command_console_probe_fixture";
#[cfg(windows)]
const CONSOLE_PROBE_ENV: &str = "A3S_TEST_BROWSER_CONSOLE_PROBE";

#[tokio::test]
#[cfg(unix)]
async fn successful_command_keeps_its_persistent_descendant_alive() {
    let temp = tempfile::tempdir().expect("tempdir");
    let pid_file = temp.path().join("daemon.pid");
    let script = format!(
        "sleep 30 >/dev/null 2>&1 & echo $! > '{}' && printf '{{}}'",
        pid_file.display()
    );
    let invocation = CommandInvocation {
        program: PathBuf::from("/bin/sh"),
        args: vec![OsString::from("-c"), OsString::from(script)],
        env: Default::default(),
        timeout: Duration::from_secs(5),
    };

    let output = TokioCommandExecutor
        .run(invocation)
        .await
        .expect("successful launcher command");
    assert_eq!(output.exit_code, 0);

    let process_id = std::fs::read_to_string(pid_file)
        .expect("daemon PID")
        .trim()
        .parse::<i32>()
        .expect("numeric daemon PID");
    let process_id = Pid::from_raw(process_id);
    assert!(
        kill(process_id, None).is_ok(),
        "successful command cleanup killed the persistent daemon"
    );
    let _ = kill(process_id, Signal::SIGKILL);
}

#[tokio::test]
async fn successful_command_does_not_wait_for_inherited_output_handles() {
    let _test_guard = process_tree_test_lock().lock().await;
    let temp = tempfile::tempdir().expect("tempdir");
    let process_id_file = temp.path().join("output-descendant.pid");
    let mut env = BTreeMap::new();
    env.insert(
        OsString::from(DESCENDANT_MODE_ENV),
        OsString::from("output-parent"),
    );
    env.insert(
        OsString::from(DESCENDANT_OUTPUT_PID_ENV),
        process_id_file.clone().into_os_string(),
    );
    let invocation = CommandInvocation {
        program: std::env::current_exe().expect("current test executable"),
        args: vec![
            OsString::from(DESCENDANT_FIXTURE_TEST),
            OsString::from("--ignored"),
            OsString::from("--exact"),
            OsString::from("--nocapture"),
        ],
        env,
        timeout: Duration::from_secs(5),
    };

    let result =
        tokio::time::timeout(Duration::from_secs(3), TokioCommandExecutor.run(invocation)).await;
    wait_for_file(&process_id_file).await;
    let process_id = std::fs::read_to_string(&process_id_file)
        .expect("output descendant PID")
        .trim()
        .parse::<u32>()
        .expect("numeric output descendant PID");
    cleanup_output_descendant(process_id);

    let output = result
        .expect("browser command waited for a persistent descendant's output handle")
        .expect("successful browser command");
    assert_eq!(output.exit_code, 0);
    assert!(output.stdout.contains("{}"), "{:?}", output.stdout);
}

#[tokio::test]
async fn browser_descendant_does_not_inherit_its_callers_output_pipes() {
    let _test_guard = process_tree_test_lock().lock().await;
    let temp = tempfile::tempdir().expect("tempdir");
    let process_id_file = temp.path().join("nested-output-descendant.pid");
    let mut command =
        std::process::Command::new(std::env::current_exe().expect("current test executable"));
    command
        .args([
            DESCENDANT_FIXTURE_TEST,
            "--ignored",
            "--exact",
            "--nocapture",
        ])
        .env(DESCENDANT_MODE_ENV, "nested-output-parent")
        .env(DESCENDANT_OUTPUT_PID_ENV, &process_id_file)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let captured = tokio::task::spawn_blocking(move || command.output());
    let result = tokio::time::timeout(Duration::from_secs(3), captured).await;
    wait_for_file(&process_id_file).await;
    let process_id = std::fs::read_to_string(&process_id_file)
        .expect("nested output descendant PID")
        .trim()
        .parse::<u32>()
        .expect("numeric nested output descendant PID");
    cleanup_output_descendant(process_id);

    let output = result
        .expect("browser descendant retained its caller's output pipe")
        .expect("join captured browser command")
        .expect("capture browser command output");
    assert!(output.status.success(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("nested output complete"),
        "{:?}",
        output.stdout
    );
}

#[tokio::test]
async fn oversized_browser_command_output_is_rejected() {
    let mut env = BTreeMap::new();
    env.insert(
        OsString::from(DESCENDANT_MODE_ENV),
        OsString::from("oversized-output"),
    );
    let invocation = CommandInvocation {
        program: std::env::current_exe().expect("current test executable"),
        args: vec![
            OsString::from(DESCENDANT_FIXTURE_TEST),
            OsString::from("--ignored"),
            OsString::from("--exact"),
            OsString::from("--nocapture"),
        ],
        env,
        timeout: Duration::from_secs(5),
    };

    let error = TokioCommandExecutor
        .run(invocation)
        .await
        .expect_err("oversized browser output must fail");
    assert_eq!(error.kind(), CommandErrorKind::Output);
    assert!(error.to_string().contains("exceeded 8388608 bytes"));
}

#[cfg(windows)]
#[tokio::test]
async fn windows_browser_command_and_cmd_shim_run_without_a_console() {
    let temp = tempfile::tempdir().expect("tempdir");
    let executable = std::env::current_exe().expect("current test executable");
    let cmd_shim = temp.path().join("agent-browser.cmd");
    std::fs::write(&cmd_shim, "@echo off\r\n\"%~1\" %2 %3 %4 %5\r\n")
        .expect("write browser cmd shim");
    let invocation = CommandInvocation {
        program: cmd_shim,
        args: vec![
            executable.into_os_string(),
            OsString::from(CONSOLE_PROBE_TEST),
            OsString::from("--ignored"),
            OsString::from("--exact"),
            OsString::from("--nocapture"),
        ],
        env: BTreeMap::from([(OsString::from(CONSOLE_PROBE_ENV), OsString::from("1"))]),
        timeout: Duration::from_secs(5),
    };

    let output = TokioCommandExecutor
        .run(invocation)
        .await
        .expect("hidden cmd browser shim");
    assert_eq!(output.exit_code, 0, "{}", output.stderr);
    assert!(
        output.stdout.contains("console-window=none"),
        "{}",
        output.stdout
    );
}

#[tokio::test]
async fn dropping_a_session_reaps_a_successful_command_descendant_and_its_socket() {
    let _test_guard = process_tree_test_lock().lock().await;
    let temp = tempfile::tempdir().expect("tempdir");
    let (runtime, registration) = registered_session(&temp, "owned").await;
    let (leaf_pid, address) = launch_successful_descendant(&runtime, &temp, "owned").await;

    drop(registration);

    let released = wait_until_socket_released(address).await;
    if !released {
        terminate_process_group(leaf_pid);
    }
    assert!(
        released,
        "successful browser-command descendant and socket survived session drop"
    );
}

#[tokio::test]
async fn session_cleanup_never_terminates_an_independent_browser_tree() {
    let _test_guard = process_tree_test_lock().lock().await;
    let first_temp = tempfile::tempdir().expect("first tempdir");
    let second_temp = tempfile::tempdir().expect("second tempdir");
    let (first_runtime, first_registration) = registered_session(&first_temp, "first").await;
    let (second_runtime, second_registration) = registered_session(&second_temp, "second").await;
    let (first_pid, first_address) =
        launch_successful_descendant(&first_runtime, &first_temp, "first").await;
    let (second_pid, second_address) =
        launch_successful_descendant(&second_runtime, &second_temp, "second").await;

    drop(first_registration);

    let first_released = wait_until_socket_released(first_address).await;
    if !first_released {
        terminate_process_group(first_pid);
    }
    let second_alive =
        TcpStream::connect_timeout(&second_address, Duration::from_millis(250)).is_ok();
    drop(second_registration);
    let second_released = wait_until_socket_released(second_address).await;
    if !second_released {
        terminate_process_group(second_pid);
    }

    assert!(first_released, "first browser tree survived its cleanup");
    assert!(
        second_alive,
        "cleaning the first session terminated an independent browser tree"
    );
    assert!(second_released, "second browser tree survived its cleanup");
}

#[tokio::test]
async fn command_timeout_reaps_the_complete_descendant_tree_and_socket() {
    let _test_guard = process_tree_test_lock().lock().await;
    let temp = tempfile::tempdir().expect("tempdir");
    let (runtime, registration) = registered_session(&temp, "timeout").await;
    let gate = temp.path().join("timeout-spawn-leaf");
    let attached_file = temp.path().join("timeout-attached");
    let parent_file = temp.path().join("timeout-parent.pid");
    let leaf_file = temp.path().join("timeout-leaf.txt");
    let mut invocation = descendant_fixture_invocation(
        runtime.path().to_path_buf(),
        gate.clone(),
        attached_file.clone(),
        parent_file.clone(),
        leaf_file.clone(),
    );
    invocation.env.insert(
        OsString::from(DESCENDANT_MODE_ENV),
        OsString::from("parent-hang"),
    );
    invocation.timeout = Duration::from_secs(3);

    let command = tokio::spawn(async move { TokioCommandExecutor.run(invocation).await });
    wait_for_file(&parent_file).await;
    wait_for_file(&attached_file).await;
    tokio::fs::write(&gate, b"spawn")
        .await
        .expect("release descendant fixture");
    let (leaf_pid, address) = wait_for_leaf(&leaf_file).await;
    let error = command
        .await
        .expect("join timed-out browser command")
        .expect_err("browser command must time out");
    assert_eq!(error.kind(), super::CommandErrorKind::TimedOut);

    let released_before_session_cleanup = wait_until_socket_released(address).await;
    drop(registration);
    if !released_before_session_cleanup {
        terminate_process_group(leaf_pid);
    }
    assert!(
        released_before_session_cleanup,
        "browser command timeout left a descendant or socket alive"
    );
}

#[tokio::test]
async fn cancelling_a_command_future_reaps_the_complete_descendant_tree_and_socket() {
    let _test_guard = process_tree_test_lock().lock().await;
    let temp = tempfile::tempdir().expect("tempdir");
    let (runtime, registration) = registered_session(&temp, "cancel").await;
    let gate = temp.path().join("cancel-spawn-leaf");
    let attached_file = temp.path().join("cancel-attached");
    let parent_file = temp.path().join("cancel-parent.pid");
    let leaf_file = temp.path().join("cancel-leaf.txt");
    let mut invocation = descendant_fixture_invocation(
        runtime.path().to_path_buf(),
        gate.clone(),
        attached_file.clone(),
        parent_file.clone(),
        leaf_file.clone(),
    );
    invocation.env.insert(
        OsString::from(DESCENDANT_MODE_ENV),
        OsString::from("parent-hang"),
    );
    invocation.timeout = Duration::from_secs(30);

    let command = tokio::spawn(async move { TokioCommandExecutor.run(invocation).await });
    wait_for_file(&parent_file).await;
    wait_for_file(&attached_file).await;
    tokio::fs::write(&gate, b"spawn")
        .await
        .expect("release descendant fixture");
    let (leaf_pid, address) = wait_for_leaf(&leaf_file).await;
    command.abort();
    let cancellation = command
        .await
        .expect_err("browser command must be cancelled");
    assert!(cancellation.is_cancelled());

    let released_before_session_cleanup = wait_until_socket_released(address).await;
    drop(registration);
    if !released_before_session_cleanup {
        terminate_process_group(leaf_pid);
    }
    assert!(
        released_before_session_cleanup,
        "browser command cancellation left a descendant or socket alive"
    );
}

#[tokio::test]
async fn cancelling_an_unregistered_persistent_command_reaps_its_descendants() {
    let _test_guard = process_tree_test_lock().lock().await;
    let temp = tempfile::tempdir().expect("tempdir");
    let runtime_path = temp.path().join("persistent-runtime");
    std::fs::create_dir(&runtime_path).expect("persistent runtime");
    let gate = temp.path().join("persistent-cancel-spawn-leaf");
    let attached_file = temp.path().join("persistent-cancel-attached");
    let parent_file = temp.path().join("persistent-cancel-parent.pid");
    let leaf_file = temp.path().join("persistent-cancel-leaf.txt");
    let mut invocation = descendant_fixture_invocation(
        runtime_path,
        gate.clone(),
        attached_file.clone(),
        parent_file.clone(),
        leaf_file.clone(),
    );
    invocation.env.insert(
        OsString::from(DESCENDANT_MODE_ENV),
        OsString::from("parent-hang"),
    );
    invocation.timeout = Duration::from_secs(30);

    let command = tokio::spawn(async move { TokioCommandExecutor.run(invocation).await });
    wait_for_file(&parent_file).await;
    wait_for_file(&attached_file).await;
    tokio::fs::write(&gate, b"spawn")
        .await
        .expect("release descendant fixture");
    let (leaf_pid, address) = wait_for_leaf(&leaf_file).await;
    command.abort();
    let cancellation = command
        .await
        .expect_err("browser command must be cancelled");
    assert!(cancellation.is_cancelled());

    let released = wait_until_socket_released(address).await;
    if !released {
        terminate_process_group(leaf_pid);
    }
    assert!(
        released,
        "cancelled persistent command left a reparented descendant alive"
    );
}

#[tokio::test]
async fn nonzero_command_exit_reaps_persistent_descendants_before_returning() {
    let _test_guard = process_tree_test_lock().lock().await;
    let temp = tempfile::tempdir().expect("tempdir");
    let runtime_path = temp.path().join("persistent-runtime");
    std::fs::create_dir(&runtime_path).expect("persistent runtime");
    let gate = temp.path().join("persistent-failure-spawn-leaf");
    let attached_file = temp.path().join("persistent-failure-attached");
    let parent_file = temp.path().join("persistent-failure-parent.pid");
    let leaf_file = temp.path().join("persistent-failure-leaf.txt");
    let mut invocation = descendant_fixture_invocation(
        runtime_path,
        gate.clone(),
        attached_file.clone(),
        parent_file.clone(),
        leaf_file.clone(),
    );
    invocation.env.insert(
        OsString::from(DESCENDANT_MODE_ENV),
        OsString::from("parent-fail"),
    );

    let command = tokio::spawn(async move { TokioCommandExecutor.run(invocation).await });
    wait_for_file(&attached_file).await;
    tokio::fs::write(&gate, b"spawn")
        .await
        .expect("release descendant fixture");
    let (leaf_pid, address) = wait_for_leaf(&leaf_file).await;
    let output = command
        .await
        .expect("join failed browser command")
        .expect("failed browser command output");
    let released = wait_until_socket_released(address).await;
    if !released {
        cleanup_descendant(&parent_file, leaf_pid);
    }

    assert_eq!(output.exit_code, 23);
    assert!(
        released,
        "nonzero browser command left a persistent descendant or socket alive"
    );
}

#[tokio::test]
async fn successful_unregistered_command_preserves_its_persistent_descendant() {
    let _test_guard = process_tree_test_lock().lock().await;
    let temp = tempfile::tempdir().expect("tempdir");
    let runtime_path = temp.path().join("persistent-runtime");
    std::fs::create_dir(&runtime_path).expect("persistent runtime");
    let gate = temp.path().join("persistent-success-spawn-leaf");
    let attached_file = temp.path().join("persistent-success-attached");
    let parent_file = temp.path().join("persistent-success-parent.pid");
    let leaf_file = temp.path().join("persistent-success-leaf.txt");
    let invocation = descendant_fixture_invocation(
        runtime_path,
        gate.clone(),
        attached_file.clone(),
        parent_file.clone(),
        leaf_file.clone(),
    );

    let command = tokio::spawn(async move { TokioCommandExecutor.run(invocation).await });
    wait_for_file(&parent_file).await;
    wait_for_file(&attached_file).await;
    tokio::fs::write(&gate, b"spawn")
        .await
        .expect("release descendant fixture");
    let (leaf_pid, address) = wait_for_leaf(&leaf_file).await;
    let output = command
        .await
        .expect("join persistent browser command")
        .expect("persistent browser command");
    assert_eq!(output.exit_code, 0);
    let remained_alive = TcpStream::connect_timeout(&address, Duration::from_millis(250)).is_ok();

    cleanup_descendant(&parent_file, leaf_pid);
    let released = wait_until_socket_released(address).await;
    assert!(
        remained_alive,
        "successful persistent command killed its long-lived descendant"
    );
    assert!(released, "persistent descendant cleanup failed");
}

async fn registered_session(
    temp: &tempfile::TempDir,
    name: &str,
) -> (RuntimeDirectory, SessionRegistration) {
    let runtime_path = temp.path().join("runtime");
    std::fs::create_dir(&runtime_path).expect("runtime directory");
    let runtime = RuntimeDirectory::bind_existing(&runtime_path)
        .await
        .expect("bind runtime");
    let registration = SessionRegistration::new(
        runtime.clone(),
        name.to_string(),
        "browser".to_string(),
        vec!["fixture".to_string()],
    )
    .expect("register owned browser session");
    (runtime, registration)
}

async fn launch_successful_descendant(
    runtime: &RuntimeDirectory,
    temp: &tempfile::TempDir,
    name: &str,
) -> (u32, SocketAddr) {
    let gate = temp.path().join(format!("{name}-spawn-leaf"));
    let attached_file = temp.path().join(format!("{name}-attached"));
    let parent_file = temp.path().join(format!("{name}-parent.pid"));
    let leaf_file = temp.path().join(format!("{name}-leaf.txt"));
    let invocation = descendant_fixture_invocation(
        runtime.path().to_path_buf(),
        gate.clone(),
        attached_file.clone(),
        parent_file.clone(),
        leaf_file.clone(),
    );

    let command = tokio::spawn(async move { TokioCommandExecutor.run(invocation).await });
    wait_for_file(&parent_file).await;
    wait_for_file(&attached_file).await;
    tokio::fs::write(&gate, b"spawn")
        .await
        .expect("release descendant fixture");
    let descendant = wait_for_leaf(&leaf_file).await;
    let output = command
        .await
        .expect("join browser command")
        .expect("browser command");
    assert_eq!(output.exit_code, 0);
    assert!(
        TcpStream::connect_timeout(&descendant.1, Duration::from_millis(250)).is_ok(),
        "fixture descendant never owned its socket"
    );
    descendant
}

fn process_tree_test_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

fn cleanup_descendant(parent_file: &std::path::Path, _leaf_process_id: u32) {
    #[cfg(unix)]
    {
        let parent_process_id = std::fs::read_to_string(parent_file)
            .expect("fixture parent PID")
            .trim()
            .parse::<u32>()
            .expect("numeric fixture parent PID");
        terminate_process_group(parent_process_id);
    }
    #[cfg(not(unix))]
    {
        let _ = parent_file;
        terminate_process_group(_leaf_process_id);
    }
}

fn cleanup_output_descendant(process_id: u32) {
    #[cfg(unix)]
    if let Ok(process_id) = i32::try_from(process_id) {
        let _ = kill(Pid::from_raw(process_id), Signal::SIGKILL);
    }
    #[cfg(windows)]
    terminate_process_group(process_id);
}

fn descendant_fixture_invocation(
    runtime: PathBuf,
    gate: PathBuf,
    attached_file: PathBuf,
    parent_file: PathBuf,
    leaf_file: PathBuf,
) -> CommandInvocation {
    let mut env = BTreeMap::new();
    env.insert(
        OsString::from("AGENT_BROWSER_SOCKET_DIR"),
        runtime.into_os_string(),
    );
    env.insert(
        OsString::from(DESCENDANT_MODE_ENV),
        OsString::from("parent"),
    );
    env.insert(OsString::from(DESCENDANT_GATE_ENV), gate.into_os_string());
    env.insert(
        OsString::from(DESCENDANT_ATTACHED_ENV),
        attached_file.into_os_string(),
    );
    env.insert(
        OsString::from(DESCENDANT_PARENT_ENV),
        parent_file.into_os_string(),
    );
    env.insert(
        OsString::from(DESCENDANT_LEAF_ENV),
        leaf_file.into_os_string(),
    );
    CommandInvocation {
        program: std::env::current_exe().expect("current test executable"),
        args: vec![
            OsString::from(DESCENDANT_FIXTURE_TEST),
            OsString::from("--ignored"),
            OsString::from("--exact"),
        ],
        env,
        timeout: Duration::from_secs(5),
    }
}

async fn wait_for_file(path: &std::path::Path) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while !path.is_file() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for {}",
            path.display()
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn wait_for_leaf(path: &std::path::Path) -> (u32, SocketAddr) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(source) = tokio::fs::read_to_string(path).await {
            let mut lines = source.lines();
            if let (Some(process_id), Some(address)) = (lines.next(), lines.next()) {
                if let (Ok(process_id), Ok(address)) =
                    (process_id.parse::<u32>(), address.parse::<SocketAddr>())
                {
                    return (process_id, address);
                }
            }
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for descendant fixture"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn wait_until_socket_released(address: SocketAddr) -> bool {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        if TcpListener::bind(address).is_ok() {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[cfg(windows)]
#[test]
#[ignore = "helper process for the hidden Windows browser-command test"]
fn windows_browser_command_console_probe_fixture() {
    use windows_sys::Win32::System::Console::GetConsoleWindow;

    assert_eq!(
        std::env::var(CONSOLE_PROBE_ENV).as_deref(),
        Ok("1"),
        "console probe must only run through its owning test"
    );
    let console_window = unsafe { GetConsoleWindow() };
    assert!(
        console_window.is_null(),
        "browser command inherited or created a Windows console"
    );
    println!("console-window=none");
}

#[test]
#[ignore = "helper process for browser process-tree lifecycle tests"]
#[allow(
    clippy::zombie_processes,
    reason = "the fixture descendant must outlive its launcher and is reaped by the parent test"
)]
fn successful_command_descendant_fixture() {
    let mode = std::env::var(DESCENDANT_MODE_ENV).expect("descendant fixture mode");
    if mode == "oversized-output" {
        std::io::stdout()
            .write_all(&vec![b'x'; super::MAX_COMMAND_OUTPUT_BYTES as usize + 1])
            .expect("write oversized output fixture");
        return;
    }
    if mode == "nested-output-parent" {
        let process_id_file = std::env::var_os(DESCENDANT_OUTPUT_PID_ENV)
            .map(PathBuf::from)
            .expect("nested output descendant PID file");
        let mut env = BTreeMap::new();
        env.insert(
            OsString::from(DESCENDANT_MODE_ENV),
            OsString::from("output-parent"),
        );
        env.insert(
            OsString::from(DESCENDANT_OUTPUT_PID_ENV),
            process_id_file.into_os_string(),
        );
        let invocation = CommandInvocation {
            program: std::env::current_exe().expect("current nested fixture executable"),
            args: vec![
                OsString::from(DESCENDANT_FIXTURE_TEST),
                OsString::from("--ignored"),
                OsString::from("--exact"),
                OsString::from("--nocapture"),
            ],
            env,
            timeout: Duration::from_secs(5),
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("nested fixture runtime");
        let output = runtime
            .block_on(TokioCommandExecutor.run(invocation))
            .expect("nested browser command");
        assert_eq!(output.exit_code, 0);
        println!("nested output complete");
        return;
    }
    if mode == "output-child" {
        std::thread::sleep(Duration::from_secs(30));
        return;
    }
    if mode == "output-parent" {
        let child = std::process::Command::new(
            std::env::current_exe().expect("current descendant fixture executable"),
        )
        .args([DESCENDANT_FIXTURE_TEST, "--ignored", "--exact"])
        .env(DESCENDANT_MODE_ENV, "output-child")
        .stdin(Stdio::null())
        .spawn()
        .expect("spawn output-holding descendant");
        let process_id_file = std::env::var_os(DESCENDANT_OUTPUT_PID_ENV)
            .map(PathBuf::from)
            .expect("output descendant PID file");
        std::fs::write(process_id_file, child.id().to_string())
            .expect("publish output descendant PID");
        println!("{{}}");
        return;
    }
    if mode == "leaf" {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind fixture socket");
        let leaf_file = std::env::var_os(DESCENDANT_LEAF_ENV)
            .map(PathBuf::from)
            .expect("descendant fixture leaf file");
        std::fs::write(
            leaf_file,
            format!(
                "{}\n{}\n",
                std::process::id(),
                listener.local_addr().unwrap()
            ),
        )
        .expect("publish descendant fixture");
        std::thread::sleep(Duration::from_secs(30));
        return;
    }
    assert!(matches!(
        mode.as_str(),
        "parent" | "parent-fail" | "parent-hang"
    ));

    let gate = std::env::var_os(DESCENDANT_GATE_ENV)
        .map(PathBuf::from)
        .expect("descendant fixture gate");
    let parent_file = std::env::var_os(DESCENDANT_PARENT_ENV)
        .map(PathBuf::from)
        .expect("descendant fixture parent file");
    std::fs::write(parent_file, std::process::id().to_string()).expect("publish fixture parent");
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !gate.is_file() {
        assert!(
            std::time::Instant::now() < deadline,
            "descendant fixture gate was not released"
        );
        std::thread::sleep(Duration::from_millis(10));
    }

    spawn_descendant_fixture();
    if mode == "parent-hang" {
        std::thread::sleep(Duration::from_secs(30));
        return;
    }
    if mode == "parent-fail" {
        let leaf_file = std::env::var_os(DESCENDANT_LEAF_ENV)
            .map(PathBuf::from)
            .expect("descendant fixture leaf file");
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !leaf_file.is_file() {
            assert!(
                std::time::Instant::now() < deadline,
                "failed-command descendant fixture never started"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        std::process::exit(23);
    }
    println!("{{}}");
}

#[cfg(unix)]
fn spawn_descendant_fixture() {
    let status = std::process::Command::new("/bin/sh")
        .args([
            "-c",
            "\"$1\" \"$2\" --ignored --exact </dev/null >/dev/null 2>&1 &",
            "a3s-test-descendant-launcher",
        ])
        .arg(std::env::current_exe().expect("current descendant fixture executable"))
        .arg(DESCENDANT_FIXTURE_TEST)
        .env(DESCENDANT_MODE_ENV, "leaf")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run descendant fixture launcher");
    assert!(status.success(), "descendant fixture launcher failed");
}

#[cfg(windows)]
fn spawn_descendant_fixture() {
    use std::os::windows::process::CommandExt as _;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let powershell = PathBuf::from(std::env::var_os("SystemRoot").expect("SystemRoot"))
        .join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
    let script = format!(
        "Start-Process -FilePath $env:{DESCENDANT_EXE_ENV} -ArgumentList \
         @($env:{DESCENDANT_TEST_ENV}, '--ignored', '--exact') -WindowStyle Hidden"
    );
    let status = std::process::Command::new(powershell)
        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command"])
        .arg(script)
        .env(DESCENDANT_MODE_ENV, "leaf")
        .env(
            DESCENDANT_EXE_ENV,
            std::env::current_exe().expect("current descendant fixture executable"),
        )
        .env(DESCENDANT_TEST_ENV, DESCENDANT_FIXTURE_TEST)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .expect("run descendant fixture launcher");
    assert!(status.success(), "descendant fixture launcher failed");
}
