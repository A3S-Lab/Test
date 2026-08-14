use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::Duration;

use crate::{TuiCommand, TuiSize};

pub(crate) trait PtyProcess: Send {
    fn process_id(&self) -> u32;
    fn resize(&mut self, size: TuiSize) -> io::Result<()>;
    fn try_wait(&mut self) -> io::Result<Option<ProcessStatus>>;
    fn terminate(&mut self) -> io::Result<()>;
    fn wait_for_exit(&mut self, timeout: Duration) -> io::Result<ProcessStatus>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProcessStatus {
    pub code: Option<u32>,
    pub signal: Option<String>,
}

pub(crate) struct SpawnedPty {
    pub process: Box<dyn PtyProcess>,
    pub reader: Box<dyn io::Read + Send>,
    pub writer: Box<dyn io::Write + Send>,
}

pub(crate) async fn spawn(command: TuiCommand, size: TuiSize) -> io::Result<SpawnedPty> {
    tokio::task::spawn_blocking(move || platform::spawn(&command, size))
        .await
        .map_err(|error| io::Error::other(format!("failed to join PTY creation: {error}")))?
}

pub(crate) type SharedProcess = Arc<Mutex<Box<dyn PtyProcess>>>;
type WeakProcess = Weak<Mutex<Box<dyn PtyProcess>>>;
type ProcessRegistry = Mutex<HashMap<u32, WeakProcess>>;

pub(crate) fn register_process(
    process_id: u32,
    process: &SharedProcess,
) -> io::Result<ProcessRegistration> {
    if process_id <= 1 || process_id == std::process::id() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "TUI child did not expose a safe process identifier",
        ));
    }
    active_processes()
        .lock()
        .map_err(|_| io::Error::other("TUI process registry is unavailable"))?
        .insert(process_id, Arc::downgrade(process));
    Ok(ProcessRegistration { process_id })
}

pub(crate) struct ProcessRegistration {
    process_id: u32,
}

impl Drop for ProcessRegistration {
    fn drop(&mut self) {
        if let Ok(mut processes) = active_processes().lock() {
            processes.remove(&self.process_id);
        }
    }
}

pub fn terminate_active_tui_processes() {
    let processes = active_processes()
        .lock()
        .map(|processes| {
            processes
                .iter()
                .map(|(process_id, process)| (*process_id, process.clone()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for (process_id, process) in processes {
        let Some(process) = process.upgrade() else {
            emergency_terminate_process(process_id);
            continue;
        };
        match process.try_lock() {
            Ok(mut process) => {
                if process.terminate().is_err() {
                    emergency_terminate_process(process_id);
                }
            }
            Err(std::sync::TryLockError::Poisoned(error)) => {
                if error.into_inner().terminate().is_err() {
                    emergency_terminate_process(process_id);
                }
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                emergency_terminate_process(process_id);
            }
        };
    }
}

pub(crate) fn emergency_terminate_process(process_id: u32) {
    platform::emergency_terminate(process_id);
}

fn active_processes() -> &'static ProcessRegistry {
    static ACTIVE: OnceLock<ProcessRegistry> = OnceLock::new();
    ACTIVE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(unix)]
fn configure_command(builder: &mut portable_pty::CommandBuilder, command: &TuiCommand) {
    builder.args(&command.arguments);
    if let Some(directory) = &command.working_directory {
        builder.cwd(directory.as_os_str());
    }
    for (name, value) in &command.environment {
        builder.env(name, value);
    }
    builder.env("TERM", "xterm-256color");
    builder.env("COLORTERM", "truecolor");
}

#[cfg(unix)]
mod platform {
    use std::io::{self, Write as _};
    use std::process::{ChildStdin, Command, Stdio};
    use std::time::{Duration, Instant};

    use nix::errno::Errno;
    use nix::sys::signal::{killpg, Signal};
    use nix::unistd::{getpgrp, Pid};
    use portable_pty::{native_pty_system, PtySize};

    use super::{configure_command, ProcessStatus, PtyProcess, SpawnedPty, TuiCommand, TuiSize};

    const POLL_INTERVAL: Duration = Duration::from_millis(10);

    pub(super) fn spawn(command: &TuiCommand, size: TuiSize) -> io::Result<SpawnedPty> {
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: size.rows,
                cols: size.columns,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(other)?;
        let reader = pair.master.try_clone_reader().map_err(other)?;
        let writer = pair.master.take_writer().map_err(other)?;
        let mut builder = portable_pty::CommandBuilder::new(&command.executable);
        configure_command(&mut builder, command);
        let mut child = pair.slave.spawn_command(builder).map_err(other)?;
        drop(pair.slave);
        let process_id = child
            .process_id()
            .filter(|process_id| *process_id > 1)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "PTY child did not expose a valid process identifier",
                )
            })?;
        let watchdog = match Watchdog::spawn(process_id) {
            Ok(watchdog) => watchdog,
            Err(error) => {
                let _ = terminate_group(process_id);
                let _ = child.wait();
                return Err(error);
            }
        };
        let process = UnixPtyProcess {
            process_id,
            child,
            master: pair.master,
            watchdog,
            finished: None,
        };
        Ok(SpawnedPty {
            process: Box::new(process),
            reader,
            writer,
        })
    }

    struct UnixPtyProcess {
        process_id: u32,
        child: Box<dyn portable_pty::Child + Send + Sync>,
        master: Box<dyn portable_pty::MasterPty + Send>,
        watchdog: Watchdog,
        finished: Option<ProcessStatus>,
    }

    impl PtyProcess for UnixPtyProcess {
        fn process_id(&self) -> u32 {
            self.process_id
        }

        fn resize(&mut self, size: TuiSize) -> io::Result<()> {
            self.master
                .resize(PtySize {
                    rows: size.rows,
                    cols: size.columns,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .map_err(other)
        }

        fn try_wait(&mut self) -> io::Result<Option<ProcessStatus>> {
            if let Some(status) = &self.finished {
                return Ok(Some(status.clone()));
            }
            let status = self.child.try_wait()?.map(status_from_portable);
            if let Some(status) = &status {
                self.finished = Some(status.clone());
            }
            Ok(status)
        }

        fn terminate(&mut self) -> io::Result<()> {
            if self.try_wait()?.is_some() && !process_group_exists(self.process_id) {
                return Ok(());
            }
            terminate_group(self.process_id)
        }

        fn wait_for_exit(&mut self, timeout: Duration) -> io::Result<ProcessStatus> {
            let deadline = Instant::now() + timeout;
            let mut root_status = self.finished.clone();
            while process_group_exists(self.process_id) {
                if root_status.is_none() {
                    root_status = self.try_wait()?;
                }
                if Instant::now() >= deadline {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "TUI process group did not stop before the cleanup deadline",
                    ));
                }
                std::thread::sleep(POLL_INTERVAL);
            }
            let status = match root_status.or(self.try_wait()?) {
                Some(status) => status,
                None => self.child.wait().map(status_from_portable)?,
            };
            self.finished = Some(status.clone());
            self.watchdog.disarm()?;
            Ok(status)
        }
    }

    impl Drop for UnixPtyProcess {
        fn drop(&mut self) {
            if self.finished.is_none() {
                let _ = terminate_group(self.process_id);
            }
        }
    }

    struct Watchdog {
        input: Option<ChildStdin>,
        child: std::process::Child,
    }

    impl Watchdog {
        fn spawn(process_group: u32) -> io::Result<Self> {
            let script = "IFS= read -r control || true; \
                          [ \"$control\" = disarm ] || \
                          /bin/kill -KILL -- \"-$1\" 2>/dev/null || true";
            let mut child = Command::new("/bin/sh")
                .args([
                    "-c",
                    script,
                    "a3s-test-tui-watchdog",
                    &process_group.to_string(),
                ])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?;
            let input = child.stdin.take().ok_or_else(|| {
                io::Error::other("TUI process-group watchdog stdin is unavailable")
            })?;
            Ok(Self {
                input: Some(input),
                child,
            })
        }

        fn disarm(&mut self) -> io::Result<()> {
            if let Some(input) = self.input.as_mut() {
                input.write_all(b"disarm\n")?;
            }
            self.input.take();
            self.wait()
        }

        fn trigger(&mut self) -> io::Result<()> {
            self.input.take();
            self.wait()
        }

        fn wait(&mut self) -> io::Result<()> {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                match self.child.try_wait()? {
                    Some(_) => return Ok(()),
                    None if Instant::now() < deadline => std::thread::sleep(POLL_INTERVAL),
                    None => {
                        let _ = self.child.kill();
                        let _ = self.child.wait();
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "TUI process-group watchdog did not exit",
                        ));
                    }
                }
            }
        }
    }

    impl Drop for Watchdog {
        fn drop(&mut self) {
            let _ = self.trigger();
        }
    }

    pub(super) fn emergency_terminate(process_id: u32) {
        let _ = terminate_group(process_id);
    }

    fn status_from_portable(status: portable_pty::ExitStatus) -> ProcessStatus {
        ProcessStatus {
            code: status.signal().is_none().then(|| status.exit_code()),
            signal: status.signal().map(str::to_string),
        }
    }

    fn process_group_exists(process_group: u32) -> bool {
        let Ok(process_group) = i32::try_from(process_group) else {
            return false;
        };
        match killpg(Pid::from_raw(process_group), None) {
            Ok(()) | Err(Errno::EPERM) => true,
            Err(Errno::ESRCH) => false,
            Err(_) => true,
        }
    }

    fn terminate_group(process_group: u32) -> io::Result<()> {
        let current_group = u32::try_from(getpgrp().as_raw()).unwrap_or_default();
        if process_group <= 1 || process_group == current_group {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "refusing to terminate an unsafe TUI process group",
            ));
        }
        let process_group = i32::try_from(process_group)
            .map_err(|_| io::Error::other("TUI process group identifier is too large"))?;
        match killpg(Pid::from_raw(process_group), Signal::SIGKILL) {
            Ok(()) | Err(Errno::ESRCH) => Ok(()),
            Err(Errno::EPERM) if !process_group_exists_u32(process_group) => Ok(()),
            Err(error) => Err(io::Error::from_raw_os_error(error as i32)),
        }
    }

    fn process_group_exists_u32(process_group: i32) -> bool {
        u32::try_from(process_group)
            .ok()
            .is_some_and(process_group_exists)
    }

    fn other(error: impl std::fmt::Display) -> io::Error {
        io::Error::other(error.to_string())
    }
}

#[cfg(windows)]
mod platform {
    use std::io;
    use std::time::Duration;

    use conpty_oxide::{SessionOptions, Size};

    use super::{ProcessStatus, PtyProcess, SpawnedPty, TuiCommand, TuiSize};

    pub(super) fn spawn(command: &TuiCommand, size: TuiSize) -> io::Result<SpawnedPty> {
        let size = Size::try_new(size.columns, size.rows).map_err(other)?;
        let mut builder = conpty_oxide::blocking::Command::new(&command.executable);
        builder.args(&command.arguments);
        if let Some(directory) = &command.working_directory {
            builder.current_dir(directory);
        }
        builder.env("TERM", "xterm-256color");
        builder.env("COLORTERM", "truecolor");
        for (name, value) in &command.environment {
            builder.env(name, value);
        }
        let session = builder
            .spawn_with(SessionOptions::new().size(size))
            .map_err(other)?;
        let parts = session.into_parts();
        let process = WindowsPtyProcess {
            child: parts.child,
            controller: parts.controller,
            finished: None,
        };
        Ok(SpawnedPty {
            process: Box::new(process),
            reader: Box::new(parts.output),
            writer: Box::new(parts.input),
        })
    }

    struct WindowsPtyProcess {
        child: conpty_oxide::blocking::Child,
        controller: conpty_oxide::PtyController,
        finished: Option<ProcessStatus>,
    }

    impl PtyProcess for WindowsPtyProcess {
        fn process_id(&self) -> u32 {
            self.child.id()
        }

        fn resize(&mut self, size: TuiSize) -> io::Result<()> {
            self.controller
                .resize(Size::try_new(size.columns, size.rows).map_err(other)?)
                .map_err(other)
        }

        fn try_wait(&mut self) -> io::Result<Option<ProcessStatus>> {
            if let Some(status) = &self.finished {
                return Ok(Some(status.clone()));
            }
            let status = self
                .child
                .try_wait()
                .map_err(other)?
                .map(|status| ProcessStatus {
                    code: Some(status.code()),
                    signal: None,
                });
            if let Some(status) = &status {
                self.finished = Some(status.clone());
            }
            Ok(status)
        }

        fn terminate(&mut self) -> io::Result<()> {
            self.child.kill().map_err(other)
        }

        fn wait_for_exit(&mut self, timeout: Duration) -> io::Result<ProcessStatus> {
            if let Some(status) = &self.finished {
                return Ok(status.clone());
            }
            let deadline = std::time::Instant::now() + timeout;
            let status = loop {
                if let Some(status) = self.child.try_wait().map_err(other)? {
                    break status;
                }
                if std::time::Instant::now() >= deadline {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "ConPTY child wait timed out",
                    ));
                }
                std::thread::sleep(Duration::from_millis(10));
            };
            let status = ProcessStatus {
                code: Some(status.code()),
                signal: None,
            };
            self.finished = Some(status.clone());
            Ok(status)
        }
    }

    pub(super) fn emergency_terminate(_process_id: u32) {
        // Every Windows session owns a kill-on-close Job. Dropping the
        // session process handle is the emergency boundary.
    }

    fn other(error: impl std::fmt::Display) -> io::Error {
        io::Error::other(error.to_string())
    }
}

#[cfg(not(any(unix, windows)))]
mod platform {
    use std::io;

    use super::{SpawnedPty, TuiCommand, TuiSize};

    pub(super) fn spawn(_command: &TuiCommand, _size: TuiSize) -> io::Result<SpawnedPty> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "this platform has no reviewed PTY backend",
        ))
    }

    pub(super) fn emergency_terminate(_process_id: u32) {}
}
