//! Owned application supervisor.
//!
//! The command is an argument list. A spawned process is the leader of its
//! own process group, and that group is reaped on finish, cancel, and timeout.
//! A process that was already listening and was reused is left running.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use a3s_test_core::{DriverError, OwnedApp};
use async_trait::async_trait;
use url::Url;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AppHandle {
    pub pid: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppStop {
    Finish,
    Cancel,
    Timeout,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StartedApp {
    pub handle: Option<AppHandle>,
    pub spawned: bool,
}

#[async_trait]
pub trait AppProcess: Send + Sync {
    async fn ready(&self, url: &str) -> Result<bool, DriverError>;
    async fn spawn(&self, command: &[String]) -> Result<AppHandle, DriverError>;
    async fn reap(&self, handle: AppHandle, stop: AppStop) -> Result<(), DriverError>;
    fn reap_now(&self, handle: &AppHandle);
}

/// Admit and start the suite app. Failure paths do not spawn.
pub async fn start_owned_app(
    app: &OwnedApp,
    host: &dyn AppProcess,
    ci: bool,
) -> Result<StartedApp, DriverError> {
    if app.command.is_empty() || app.command.iter().any(String::is_empty) {
        return Err(DriverError::new(
            "test.run.app_command",
            "owned app command must be a non-empty argument list",
        ));
    }
    if ci && app.reuse {
        return Err(DriverError::new(
            "test.run.app_reuse_denied",
            "continuous integration refuses to reuse an owned app",
        ));
    }
    admit_ready_endpoint(&app.ready_url)?;
    let answering = host.ready(&app.ready_url).await?;
    if answering && !app.reuse {
        return Err(DriverError::new(
            "test.run.app_already_running",
            "owned app ready endpoint already answers",
        ));
    }
    if answering {
        return Ok(StartedApp {
            handle: None,
            spawned: false,
        });
    }
    let handle = host.spawn(&app.command).await?;
    Ok(StartedApp {
        handle: Some(handle),
        spawned: true,
    })
}

fn admit_ready_endpoint(url: &str) -> Result<(), DriverError> {
    let parsed = Url::parse(url).map_err(|_| {
        DriverError::new(
            "test.run.app_ready_url",
            "owned app ready endpoint must be an http(s) URL",
        )
    })?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err(DriverError::new(
            "test.run.app_ready_url",
            "owned app ready endpoint must be an http(s) URL",
        ));
    }
    if parsed.port() == Some(0) {
        return Err(DriverError::new(
            "test.run.app_port",
            "owned app ready endpoint must not use port 0",
        ));
    }
    Ok(())
}

pub struct UnixAppProcess {
    children: Mutex<HashMap<u32, tokio::process::Child>>,
}

impl UnixAppProcess {
    #[must_use]
    pub fn new() -> Self {
        Self {
            children: Mutex::new(HashMap::new()),
        }
    }

    fn children(&self) -> std::sync::MutexGuard<'_, HashMap<u32, tokio::process::Child>> {
        self.children.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Default for UnixAppProcess {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for UnixAppProcess {
    fn drop(&mut self) {
        let pids = self.children().keys().copied().collect::<Vec<_>>();
        for pid in pids {
            self.reap_now(&AppHandle { pid });
        }
    }
}

#[async_trait]
impl AppProcess for UnixAppProcess {
    async fn ready(&self, url: &str) -> Result<bool, DriverError> {
        let parsed = Url::parse(url).map_err(|_| {
            DriverError::new(
                "test.run.app_ready_url",
                "owned app ready endpoint must be an http(s) URL",
            )
        })?;
        let host = parsed.host_str().ok_or_else(|| {
            DriverError::new(
                "test.run.app_ready_url",
                "owned app ready endpoint must be an http(s) URL",
            )
        })?;
        let port = parsed.port_or_known_default().ok_or_else(|| {
            DriverError::new(
                "test.run.app_ready_url",
                "owned app ready endpoint must include a port",
            )
        })?;
        match tokio::time::timeout(
            Duration::from_millis(200),
            tokio::net::TcpStream::connect((host, port)),
        )
        .await
        {
            Ok(Ok(stream)) => {
                drop(stream);
                Ok(true)
            }
            Ok(Err(_)) | Err(_) => Ok(false),
        }
    }

    async fn spawn(&self, command: &[String]) -> Result<AppHandle, DriverError> {
        let Some((program, args)) = command.split_first() else {
            return Err(DriverError::new(
                "test.run.app_command",
                "owned app command must be a non-empty argument list",
            ));
        };
        if program.is_empty() || args.iter().any(String::is_empty) {
            return Err(DriverError::new(
                "test.run.app_command",
                "owned app command must be a non-empty argument list",
            ));
        }
        let mut process = tokio::process::Command::new(program);
        process
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(false);
        #[cfg(unix)]
        {
            process.process_group(0);
        }
        let child = process.spawn().map_err(|_| {
            DriverError::new(
                "test.run.app_spawn_failed",
                "owned app command failed to start",
            )
        })?;
        let pid = child.id().ok_or_else(|| {
            DriverError::new(
                "test.run.app_spawn_failed",
                "owned app command did not produce a process id",
            )
        })?;
        if pid == 0 {
            return Err(DriverError::new(
                "test.run.app_spawn_failed",
                "owned app command did not produce a process id",
            ));
        }
        self.children().insert(pid, child);
        Ok(AppHandle { pid })
    }

    async fn reap(&self, handle: AppHandle, _stop: AppStop) -> Result<(), DriverError> {
        signal_group(handle.pid, true)?;
        tokio::time::sleep(Duration::from_millis(200)).await;
        let _ = signal_group(handle.pid, false);
        let child = self.children().remove(&handle.pid);
        if let Some(mut child) = child {
            let _ = tokio::time::timeout(Duration::from_secs(1), child.wait()).await;
        }
        Ok(())
    }

    fn reap_now(&self, handle: &AppHandle) {
        let _ = signal_group(handle.pid, false);
        if let Some(mut child) = self.children().remove(&handle.pid) {
            let _ = child.start_kill();
            let _ = child.try_wait();
        }
    }
}

fn signal_group(pid: u32, terminate: bool) -> Result<(), DriverError> {
    if pid == 0 {
        return Err(DriverError::new(
            "test.run.app_reap_failed",
            "owned app process id is missing",
        ));
    }
    #[cfg(unix)]
    {
        let raw = i32::try_from(pid).map_err(|_| {
            DriverError::new(
                "test.run.app_reap_failed",
                "owned app process id is outside the supported range",
            )
        })?;
        let signal = if terminate {
            nix::sys::signal::Signal::SIGTERM
        } else {
            nix::sys::signal::Signal::SIGKILL
        };
        match nix::sys::signal::kill(nix::unistd::Pid::from_raw(-raw), signal) {
            Ok(()) => Ok(()),
            Err(nix::errno::Errno::ESRCH) => Ok(()),
            Err(_) => Err(DriverError::new(
                "test.run.app_reap_failed",
                "owned app process group could not be signalled",
            )),
        }
    }
    #[cfg(not(unix))]
    {
        let _ = terminate;
        Err(DriverError::new(
            "test.run.app_reap_failed",
            "owned app process groups require a Unix host",
        ))
    }
}
