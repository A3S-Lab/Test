use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::File;
use std::io::{Read as _, Seek as _};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use tokio::process::{Child, Command};

use crate::process::{
    attach_command_to_session, register_process_group, terminate_process_group,
    unregister_process_group,
};
use crate::process_tree::{resume_process, BrowserProcessTree};

const MAX_COMMAND_OUTPUT_BYTES: u64 = 8 * 1024 * 1024;

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
        let PreparedCommandOutput {
            capture,
            child_stdout,
            child_stderr,
        } = prepare_command_output().await?;
        let mut command = Command::new(&invocation.program);
        command
            .args(&invocation.args)
            .envs(&invocation.env)
            .stdin(Stdio::null())
            .stdout(child_stdout)
            .stderr(child_stderr)
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

        #[cfg(windows)]
        let standard_handles = StandardHandleInheritanceGuard::clear().map_err(|error| {
            CommandError::unavailable(format!(
                "failed to protect parent output handles from browser inheritance: {error}"
            ))
        })?;
        let spawn_result = command.spawn();
        #[cfg(windows)]
        if let Err(error) = standard_handles.restore() {
            if let Ok(mut child) = spawn_result {
                let _ = child.start_kill();
            }
            return Err(CommandError::unavailable(format!(
                "failed to restore parent output handle inheritance after browser spawn: {error}"
            )));
        }
        let child = spawn_result.map_err(|error| {
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
                return Err(CommandError::timed_out(format!(
                    "browser command exceeded {} ms{}",
                    invocation.timeout.as_millis(),
                    cleanup_detail(cleanup.as_ref())
                )));
            }
        };
        let (stdout, stderr) = read_command_output(capture).await?;

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

struct CommandOutputCapture {
    stdout: File,
    stderr: File,
}

struct PreparedCommandOutput {
    capture: CommandOutputCapture,
    child_stdout: Stdio,
    child_stderr: Stdio,
}

async fn prepare_command_output() -> Result<PreparedCommandOutput, CommandError> {
    tokio::task::spawn_blocking(|| {
        let stdout = tempfile::tempfile()?;
        let stderr = tempfile::tempfile()?;
        let child_stdout = Stdio::from(stdout.try_clone()?);
        let child_stderr = Stdio::from(stderr.try_clone()?);
        Ok::<_, std::io::Error>(PreparedCommandOutput {
            capture: CommandOutputCapture { stdout, stderr },
            child_stdout,
            child_stderr,
        })
    })
    .await
    .map_err(|error| {
        CommandError::unavailable(format!(
            "failed to join browser output capture setup: {error}"
        ))
    })?
    .map_err(|error| {
        CommandError::unavailable(format!("failed to prepare browser output capture: {error}"))
    })
}

async fn read_command_output(
    capture: CommandOutputCapture,
) -> Result<(Vec<u8>, Vec<u8>), CommandError> {
    tokio::task::spawn_blocking(move || {
        Ok::<_, std::io::Error>((
            read_output_file(capture.stdout, "stdout")?,
            read_output_file(capture.stderr, "stderr")?,
        ))
    })
    .await
    .map_err(|error| {
        CommandError::output(format!("failed to join browser output reader: {error}"))
    })?
    .map_err(|error| CommandError::output(format!("failed to read browser output: {error}")))
}

fn read_output_file(mut file: File, stream: &str) -> std::io::Result<Vec<u8>> {
    file.rewind()?;
    let mut output = Vec::new();
    file.by_ref()
        .take(MAX_COMMAND_OUTPUT_BYTES + 1)
        .read_to_end(&mut output)?;
    if output.len() as u64 > MAX_COMMAND_OUTPUT_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("browser command {stream} exceeded {MAX_COMMAND_OUTPUT_BYTES} bytes"),
        ));
    }
    Ok(output)
}

#[cfg(windows)]
struct StandardHandleInheritanceGuard {
    _spawn_lock: std::sync::MutexGuard<'static, ()>,
    handles: Vec<(windows_sys::Win32::Foundation::HANDLE, u32)>,
}

#[cfg(windows)]
impl StandardHandleInheritanceGuard {
    fn clear() -> std::io::Result<Self> {
        use windows_sys::Win32::Foundation::{
            GetHandleInformation, SetHandleInformation, HANDLE_FLAG_INHERIT, INVALID_HANDLE_VALUE,
        };
        use windows_sys::Win32::System::Console::{
            GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
        };

        static SPAWN_LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        let spawn_lock = SPAWN_LOCK
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .map_err(|_| std::io::Error::other("browser spawn handle lock is unavailable"))?;
        let mut guard = Self {
            _spawn_lock: spawn_lock,
            handles: Vec::new(),
        };
        for standard_handle in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
            let handle = unsafe { GetStdHandle(standard_handle) };
            if handle.is_null() || handle == INVALID_HANDLE_VALUE {
                continue;
            }
            let mut flags = 0;
            if unsafe { GetHandleInformation(handle, &mut flags) } == 0 {
                let error = std::io::Error::last_os_error();
                let _ = guard.restore_handles();
                return Err(error);
            }
            if flags & HANDLE_FLAG_INHERIT == 0 {
                continue;
            }
            if unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT, 0) } == 0 {
                let error = std::io::Error::last_os_error();
                let _ = guard.restore_handles();
                return Err(error);
            }
            guard.handles.push((handle, flags));
        }
        Ok(guard)
    }

    fn restore(mut self) -> std::io::Result<()> {
        self.restore_handles()
    }

    fn restore_handles(&mut self) -> std::io::Result<()> {
        use windows_sys::Win32::Foundation::{SetHandleInformation, HANDLE_FLAG_INHERIT};

        let mut first_error = None;
        for (handle, flags) in self.handles.drain(..) {
            if unsafe {
                SetHandleInformation(handle, HANDLE_FLAG_INHERIT, flags & HANDLE_FLAG_INHERIT)
            } == 0
                && first_error.is_none()
            {
                first_error = Some(std::io::Error::last_os_error());
            }
        }
        first_error.map_or(Ok(()), Err)
    }
}

#[cfg(windows)]
impl Drop for StandardHandleInheritanceGuard {
    fn drop(&mut self) {
        let _ = self.restore_handles();
    }
}

#[cfg(test)]
mod tests;
