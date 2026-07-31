use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};

use crate::process::{register_process_group, terminate_process_group, unregister_process_group};

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
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);

        #[cfg(unix)]
        command.process_group(0);

        let child = command.spawn().map_err(|error| {
            CommandError::unavailable(format!("failed to execute browser command: {error}"))
        })?;
        let mut child = ChildGuard::new(child);
        let stdout = child.take_stdout()?;
        let stderr = child.take_stderr()?;
        let stdout_task = tokio::spawn(read_output(stdout));
        let stderr_task = tokio::spawn(read_output(stderr));

        let status = match tokio::time::timeout(invocation.timeout, child.wait()).await {
            Ok(result) => {
                let status = result.map_err(|error| {
                    CommandError::output(format!("failed to wait for browser command: {error}"))
                })?;
                child.finish();
                status
            }
            Err(_) => {
                child.terminate().await;
                stdout_task.abort();
                stderr_task.abort();
                return Err(CommandError::timed_out(format!(
                    "browser command exceeded {} ms",
                    invocation.timeout.as_millis()
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
}

impl ChildGuard {
    fn new(child: Child) -> Self {
        let process_id = child.id().unwrap_or_default();
        register_process_group(process_id);
        Self {
            child: Some(child),
            process_id,
        }
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

    fn finish(&mut self) {
        terminate_process_group(self.process_id);
        unregister_process_group(self.process_id);
        self.child.take();
    }

    async fn terminate(&mut self) {
        terminate_process_group(self.process_id);
        if let Some(child) = self.child.as_mut() {
            let _ = child.start_kill();
            let _ = child.wait().await;
        }
        unregister_process_group(self.process_id);
        self.child.take();
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            terminate_process_group(self.process_id);
            let _ = child.start_kill();
        }
        unregister_process_group(self.process_id);
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
