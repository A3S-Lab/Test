use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};

use crate::process::{
    attach_command_to_session, register_process_group, terminate_process_group,
    unregister_process_group,
};
use crate::process_tree::{resume_process, BrowserProcessTree};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandInvocation {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub env: BTreeMap<OsString, OsString>,
    pub timeout: Duration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandErrorKind {
    Unavailable,
    TimedOut,
    Output,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandError {
    kind: CommandErrorKind,
    message: String,
}

impl CommandError {
    #[must_use]
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self {
            kind: CommandErrorKind::Unavailable,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn timed_out(message: impl Into<String>) -> Self {
        Self {
            kind: CommandErrorKind::TimedOut,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn output(message: impl Into<String>) -> Self {
        Self {
            kind: CommandErrorKind::Output,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn kind(&self) -> CommandErrorKind {
        self.kind
    }

    #[must_use]
    pub fn retryable(&self) -> bool {
        self.kind == CommandErrorKind::Unavailable
    }
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for CommandError {}

#[async_trait]
pub trait CommandExecutor: Send + Sync {
    async fn run(&self, invocation: CommandInvocation) -> Result<CommandOutput, CommandError>;
}

#[derive(Default)]
pub struct TokioCommandExecutor;

#[async_trait]
impl CommandExecutor for TokioCommandExecutor {
    async fn run(&self, invocation: CommandInvocation) -> Result<CommandOutput, CommandError> {
        let mut command = Command::new(&invocation.program);
        command
            .args(&invocation.args)
            .envs(&invocation.env)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);

        #[cfg(unix)]
        command.process_group(0);

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt as _;

            const CREATE_SUSPENDED: u32 = 0x0000_0004;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command
                .as_std_mut()
                .creation_flags(CREATE_SUSPENDED | CREATE_NO_WINDOW);
        }

        let child = command.spawn().map_err(|error| {
            CommandError::unavailable(format!("failed to execute browser command: {error}"))
        })?;
        let mut child = ChildGuard::new(child);
        if let Err(error) = child.attach_to_session(&invocation.env) {
            let cleanup = child.terminate().await.err();
            return Err(CommandError::unavailable(format!(
                "failed to contain browser command process tree: {error}{}",
                cleanup_detail(cleanup.as_ref())
            )));
        }
        if let Err(error) = child.resume() {
            let cleanup = child.terminate().await.err();
            return Err(CommandError::unavailable(format!(
                "failed to resume contained browser command: {error}{}",
                cleanup_detail(cleanup.as_ref())
            )));
        }
        let stdout = child.take_stdout()?;
        let stderr = child.take_stderr()?;
        let stdout_task = tokio::spawn(read_output(stdout));
        let stderr_task = tokio::spawn(read_output(stderr));

        let status = match tokio::time::timeout(invocation.timeout, child.wait()).await {
            Ok(result) => {
                let status = result.map_err(|error| {
                    CommandError::output(format!("failed to wait for browser command: {error}"))
                })?;
                if status.success() {
                    child.finish().await.map_err(|error| {
                        CommandError::output(format!(
                            "failed to release successful browser command containment: {error}"
                        ))
                    })?;
                } else {
                    child.terminate().await.map_err(|error| {
                        CommandError::output(format!(
                            "browser command failed and its process tree could not be cleaned: {error}"
                        ))
                    })?;
                }
                status
            }
            Err(_) => {
                let cleanup = child.terminate().await.err();
                stdout_task.abort();
                stderr_task.abort();
                return Err(CommandError::timed_out(format!(
                    "browser command exceeded {} ms{}",
                    invocation.timeout.as_millis(),
                    cleanup_detail(cleanup.as_ref())
                )));
            }
        };
        let stdout = join_output(stdout_task).await?;
        let stderr = join_output(stderr_task).await?;

        Ok(CommandOutput {
            exit_code: status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&stdout).into_owned(),
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
        })
    }
}

struct ChildGuard {
    child: Option<Child>,
    process_id: u32,
    session_process_tree: Option<std::sync::Arc<BrowserProcessTree>>,
    command_process_tree: Option<std::sync::Arc<BrowserProcessTree>>,
}

impl ChildGuard {
    fn new(child: Child) -> Self {
        let process_id = child.id().unwrap_or_default();
        register_process_group(process_id);
        Self {
            child: Some(child),
            process_id,
            session_process_tree: None,
            command_process_tree: None,
        }
    }

    fn attach_to_session(
        &mut self,
        environment: &BTreeMap<OsString, OsString>,
    ) -> std::io::Result<()> {
        let child = self
            .child
            .as_ref()
            .ok_or_else(|| std::io::Error::other("browser command child is no longer available"))?;
        self.session_process_tree = attach_command_to_session(environment, child)?;
        if self.session_process_tree.is_none() {
            let process_tree = std::sync::Arc::new(BrowserProcessTree::new()?);
            process_tree.attach(child)?;
            self.command_process_tree = Some(process_tree);
        }
        #[cfg(test)]
        if let Some(path) = environment.get(std::ffi::OsStr::new("A3S_TEST_BROWSER_ATTACHED_FILE"))
        {
            std::fs::write(path, b"attached")?;
        }
        Ok(())
    }

    fn resume(&self) -> std::io::Result<()> {
        resume_process(self.process_id)
    }

    fn take_stdout(&mut self) -> Result<tokio::process::ChildStdout, CommandError> {
        self.child
            .as_mut()
            .and_then(|child| child.stdout.take())
            .ok_or_else(|| CommandError::output("browser command stdout is unavailable"))
    }

    fn take_stderr(&mut self) -> Result<tokio::process::ChildStderr, CommandError> {
        self.child
            .as_mut()
            .and_then(|child| child.stderr.take())
            .ok_or_else(|| CommandError::output("browser command stderr is unavailable"))
    }

    async fn wait(&mut self) -> std::io::Result<std::process::ExitStatus> {
        match self.child.as_mut() {
            Some(child) => child.wait().await,
            None => Err(std::io::Error::other(
                "browser command child is no longer available",
            )),
        }
    }

    async fn finish(&mut self) -> std::io::Result<()> {
        if let Some(process_tree) = &self.session_process_tree {
            process_tree.prune_exited()?;
        } else if let Some(process_tree) = &self.command_process_tree {
            let process_tree = std::sync::Arc::clone(process_tree);
            tokio::task::spawn_blocking(move || process_tree.disarm())
                .await
                .map_err(|error| {
                    std::io::Error::other(format!("browser watchdog cleanup task failed: {error}"))
                })??;
        }
        unregister_process_group(self.process_id);
        self.child.take();
        Ok(())
    }

    async fn terminate(&mut self) -> std::io::Result<()> {
        let mut first_error = None;
        if let Some(process_tree) = self.owned_process_tree() {
            if let Err(error) = process_tree.terminate() {
                first_error = Some(error);
            }
        } else {
            terminate_process_group(self.process_id);
        }
        if let Some(child) = self.child.as_mut() {
            let _ = child.start_kill();
            if let Err(error) = child.wait().await {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
        if let Some(process_tree) = self.owned_process_tree() {
            let wait_result = tokio::task::spawn_blocking(move || process_tree.wait_for_exit())
                .await
                .map_err(|error| {
                    std::io::Error::other(format!("browser process cleanup task failed: {error}"))
                })
                .and_then(std::convert::identity);
            if let Err(error) = wait_result {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
        if let Some(process_tree) = &self.session_process_tree {
            if let Err(error) = process_tree.prune_exited() {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
        unregister_process_group(self.process_id);
        self.child.take();
        first_error.map_or(Ok(()), Err)
    }

    fn owned_process_tree(&self) -> Option<std::sync::Arc<BrowserProcessTree>> {
        self.session_process_tree
            .as_ref()
            .or(self.command_process_tree.as_ref())
            .map(std::sync::Arc::clone)
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let process_tree = self
                .session_process_tree
                .as_ref()
                .or(self.command_process_tree.as_ref());
            if let Some(process_tree) = process_tree {
                let _ = process_tree.terminate();
            } else {
                terminate_process_group(self.process_id);
            }
            let _ = child.start_kill();
            reap_child(child);
            if let Some(process_tree) = process_tree {
                let _ = process_tree.wait_for_exit();
            }
        }
        unregister_process_group(self.process_id);
    }
}

fn cleanup_detail(error: Option<&std::io::Error>) -> String {
    error.map_or_else(String::new, |error| {
        format!("; process cleanup also failed: {error}")
    })
}

fn reap_child(child: &mut Child) {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(_)) | Err(_) => return,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(None) => return,
        }
    }
}

async fn read_output<R>(mut reader: R) -> std::io::Result<Vec<u8>>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut output = Vec::new();
    reader.read_to_end(&mut output).await?;
    Ok(output)
}

async fn join_output(
    task: tokio::task::JoinHandle<std::io::Result<Vec<u8>>>,
) -> Result<Vec<u8>, CommandError> {
    task.await
        .map_err(|error| CommandError::output(format!("browser output reader failed: {error}")))?
        .map_err(|error| {
            CommandError::output(format!("failed to read browser command output: {error}"))
        })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::ffi::OsString;
    use std::net::{SocketAddr, TcpListener, TcpStream};
    use std::path::PathBuf;
    use std::process::Stdio;
    use std::sync::OnceLock;
    use std::time::Duration;

    #[cfg(unix)]
    use nix::sys::signal::{kill, Signal};
    #[cfg(unix)]
    use nix::unistd::Pid;

    use super::{CommandExecutor, CommandInvocation, TokioCommandExecutor};
    use crate::process::{terminate_process_group, SessionRegistration};
    use crate::runtime::RuntimeDirectory;

    const DESCENDANT_FIXTURE_TEST: &str = "executor::tests::successful_command_descendant_fixture";
    const DESCENDANT_MODE_ENV: &str = "A3S_TEST_BROWSER_DESCENDANT_MODE";
    const DESCENDANT_GATE_ENV: &str = "A3S_TEST_BROWSER_DESCENDANT_GATE";
    const DESCENDANT_PARENT_ENV: &str = "A3S_TEST_BROWSER_PARENT_FILE";
    const DESCENDANT_LEAF_ENV: &str = "A3S_TEST_BROWSER_LEAF_FILE";
    const DESCENDANT_ATTACHED_ENV: &str = "A3S_TEST_BROWSER_ATTACHED_FILE";
    #[cfg(windows)]
    const DESCENDANT_EXE_ENV: &str = "A3S_TEST_BROWSER_DESCENDANT_EXE";
    #[cfg(windows)]
    const DESCENDANT_TEST_ENV: &str = "A3S_TEST_BROWSER_DESCENDANT_TEST";

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
        let (second_runtime, second_registration) =
            registered_session(&second_temp, "second").await;
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
        let remained_alive =
            TcpStream::connect_timeout(&address, Duration::from_millis(250)).is_ok();

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

    #[test]
    #[ignore = "helper process for browser process-tree lifecycle tests"]
    fn successful_command_descendant_fixture() {
        let mode = std::env::var(DESCENDANT_MODE_ENV).expect("descendant fixture mode");
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
        std::fs::write(parent_file, std::process::id().to_string())
            .expect("publish fixture parent");
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
}
