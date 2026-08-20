use std::io;
use std::process::{ExitStatus, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use tokio::io::{AsyncRead, AsyncWriteExt as _};
use tokio::process::{Child, Command};

use super::config::{DevServerProfile, VerificationCheckProfile};

const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(10);

pub(super) struct OwnedServer {
    process: OwnedProcess,
}

pub(super) struct OwnedCheck {
    process: OwnedProcess,
}

struct OwnedProcess {
    child: Child,
    tree: ProcessTree,
    output_tasks: Vec<tokio::task::JoinHandle<()>>,
    label: &'static str,
}

impl OwnedServer {
    pub(super) fn spawn(profile: &DevServerProfile) -> Result<Self> {
        Ok(Self {
            process: OwnedProcess::spawn(
                &profile.executable,
                &profile.arguments,
                &profile.working_directory,
                "development server",
            )?,
        })
    }

    pub(super) fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.process.try_wait()
    }

    pub(super) async fn wait(&mut self) -> io::Result<ExitStatus> {
        self.process.wait().await
    }

    pub(super) async fn shutdown(self, timeout: Duration) -> Result<()> {
        self.process.shutdown(timeout).await
    }
}

impl OwnedCheck {
    pub(super) fn spawn(profile: &VerificationCheckProfile) -> Result<Self> {
        Ok(Self {
            process: OwnedProcess::spawn(
                &profile.executable,
                &profile.arguments,
                &profile.working_directory,
                "verification check",
            )?,
        })
    }

    pub(super) async fn complete(
        self,
        timeout: Duration,
        cleanup_timeout: Duration,
    ) -> Result<ExitStatus> {
        self.process.complete(timeout, cleanup_timeout).await
    }
}

impl OwnedProcess {
    fn spawn(
        executable: &str,
        arguments: &[String],
        working_directory: &std::path::Path,
        label: &'static str,
    ) -> Result<Self> {
        let mut command = Command::new(executable);
        command
            .args(arguments)
            .current_dir(working_directory)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        configure_owned_process(&mut command);
        let mut child = command
            .spawn()
            .with_context(|| format!("failed to start {label} executable '{executable}'"))?;
        let tree = match ProcessTree::attach(&child) {
            Ok(tree) => tree,
            Err(error) => {
                terminate_unattached(&mut child);
                return Err(error).with_context(|| format!("failed to contain the {label} tree"));
            }
        };
        let stdout = child
            .stdout
            .take()
            .with_context(|| format!("{label} stdout pipe is unavailable"))?;
        let stderr = child
            .stderr
            .take()
            .with_context(|| format!("{label} stderr pipe is unavailable"))?;
        Ok(Self {
            child,
            tree,
            output_tasks: vec![forward_to_stderr(stdout), forward_to_stderr(stderr)],
            label,
        })
    }

    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.child.try_wait()
    }

    async fn wait(&mut self) -> io::Result<ExitStatus> {
        self.child.wait().await
    }

    async fn shutdown(mut self, timeout: Duration) -> Result<()> {
        let deadline = Instant::now() + timeout;
        self.tree.request_graceful(&mut self.child)?;
        let graceful_budget = timeout.min(Duration::from_secs(5)) / 2;
        if !self.wait_until_stopped(graceful_budget).await? {
            self.tree.terminate_now()?;
            if !self.wait_until_stopped(remaining_until(deadline)).await? {
                anyhow::bail!("{} tree survived forced cleanup", self.label);
            }
        }
        if self.child.try_wait()?.is_none() {
            let _ = self.child.start_kill();
            tokio::time::timeout(remaining_until(deadline), self.child.wait())
                .await
                .with_context(|| {
                    format!(
                        "{} launcher did not exit before cleanup timeout",
                        self.label
                    )
                })??;
        }
        self.tree.disarm()?;
        self.finish_output_tasks(deadline).await;
        Ok(())
    }

    async fn complete(
        mut self,
        timeout: Duration,
        cleanup_timeout: Duration,
    ) -> Result<ExitStatus> {
        let label = self.label;
        let status = match tokio::time::timeout(timeout, self.child.wait()).await {
            Ok(status) => status.with_context(|| format!("failed to wait for {label}"))?,
            Err(_) => {
                self.shutdown(cleanup_timeout)
                    .await
                    .with_context(|| format!("timed-out {label} cleanup failed"))?;
                anyhow::bail!("{label} exceeded its execution timeout");
            }
        };
        let deadline = Instant::now() + cleanup_timeout;
        if !self.wait_until_stopped(Duration::ZERO).await? {
            self.tree.terminate_now()?;
            if !self.wait_until_stopped(remaining_until(deadline)).await? {
                anyhow::bail!("{label} descendants survived forced cleanup");
            }
            self.tree.disarm()?;
            self.finish_output_tasks(deadline).await;
            anyhow::bail!("{label} left descendant processes after its launcher exited");
        }
        self.tree.disarm()?;
        self.finish_output_tasks(deadline).await;
        Ok(status)
    }

    async fn wait_until_stopped(&mut self, timeout: Duration) -> io::Result<bool> {
        let deadline = Instant::now() + timeout;
        loop {
            let _ = self.child.try_wait()?;
            if self.tree.is_empty()? {
                return Ok(true);
            }
            if Instant::now() >= deadline {
                return Ok(false);
            }
            tokio::time::sleep(PROCESS_POLL_INTERVAL).await;
        }
    }

    fn abort_output_tasks(&mut self) {
        for task in self.output_tasks.drain(..) {
            task.abort();
        }
    }

    async fn finish_output_tasks(&mut self, deadline: Instant) {
        for mut task in self.output_tasks.drain(..) {
            if tokio::time::timeout(remaining_until(deadline), &mut task)
                .await
                .is_err()
            {
                task.abort();
                let _ = task.await;
            }
        }
    }
}

impl Drop for OwnedProcess {
    fn drop(&mut self) {
        let _ = self.tree.terminate_now();
        let _ = self.child.start_kill();
        self.abort_output_tasks();
    }
}

fn configure_owned_process(command: &mut Command) {
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
}

fn terminate_unattached(child: &mut Child) {
    #[cfg(unix)]
    if let Some(process_group) = child.id() {
        unix::terminate_group(process_group);
    }
    let _ = child.start_kill();
}

fn forward_to_stderr<R>(mut input: R) -> tokio::task::JoinHandle<()>
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut stderr = tokio::io::stderr();
        let _ = tokio::io::copy(&mut input, &mut stderr).await;
        let _ = stderr.flush().await;
    })
}

fn remaining_until(deadline: Instant) -> Duration {
    deadline.saturating_duration_since(Instant::now())
}

struct ProcessTree {
    #[cfg(unix)]
    process_group: u32,
    #[cfg(unix)]
    watchdog: unix::Watchdog,
    #[cfg(windows)]
    job: windows::Job,
    armed: bool,
}

impl ProcessTree {
    fn attach(child: &Child) -> io::Result<Self> {
        let process_id = valid_process_id(child)?;
        #[cfg(unix)]
        {
            let watchdog = unix::Watchdog::spawn(process_id)?;
            if let Err(error) = unix::register(process_id) {
                drop(watchdog);
                return Err(error);
            }
            Ok(Self {
                process_group: process_id,
                watchdog,
                armed: true,
            })
        }
        #[cfg(windows)]
        {
            let job = windows::Job::attach(child, process_id)?;
            Ok(Self { job, armed: true })
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = process_id;
            Ok(Self { armed: true })
        }
    }

    fn request_graceful(&self, _child: &mut Child) -> io::Result<()> {
        #[cfg(unix)]
        return unix::signal_group(self.process_group, nix::sys::signal::Signal::SIGTERM);

        #[cfg(not(unix))]
        {
            _child.start_kill()
        }
    }

    fn terminate_now(&self) -> io::Result<()> {
        #[cfg(unix)]
        return unix::signal_group(self.process_group, nix::sys::signal::Signal::SIGKILL);

        #[cfg(windows)]
        return self.job.terminate();

        #[cfg(not(any(unix, windows)))]
        Ok(())
    }

    fn is_empty(&self) -> io::Result<bool> {
        #[cfg(unix)]
        return unix::group_is_empty(self.process_group);

        #[cfg(windows)]
        return self.job.is_empty();

        #[cfg(not(any(unix, windows)))]
        Ok(true)
    }

    fn disarm(&mut self) -> io::Result<()> {
        if !self.armed {
            return Ok(());
        }
        #[cfg(unix)]
        {
            unix::unregister(self.process_group);
            self.watchdog.disarm()?;
        }
        #[cfg(windows)]
        self.job.disarm()?;
        self.armed = false;
        Ok(())
    }
}

impl Drop for ProcessTree {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let _ = self.terminate_now();
        #[cfg(unix)]
        unix::unregister(self.process_group);
    }
}

fn valid_process_id(child: &Child) -> io::Result<u32> {
    child
        .id()
        .filter(|process_id| *process_id > 1)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "owned process did not expose a valid process identifier",
            )
        })
}

pub(crate) fn terminate_active_dev_servers() {
    #[cfg(unix)]
    unix::terminate_active();
}

#[cfg(unix)]
mod unix {
    use std::collections::HashSet;
    use std::io::{self, Write as _};
    use std::process::{Child, ChildStdin, Command, Stdio};
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};

    use nix::errno::Errno;
    use nix::sys::signal::{killpg, Signal};
    use nix::unistd::{getpgrp, Pid};

    pub(super) struct Watchdog {
        input: Option<ChildStdin>,
        child: Child,
    }

    impl Watchdog {
        pub(super) fn spawn(process_group: u32) -> io::Result<Self> {
            let script = "if IFS= read -r control && [ \"$control\" = disarm ]; then exit 0; fi; \
                          /bin/kill -KILL -- \"-$1\" 2>/dev/null || true";
            let mut child = Command::new("/bin/sh")
                .args([
                    "-c",
                    script,
                    "a3s-test-process-watchdog",
                    &process_group.to_string(),
                ])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?;
            let input = child
                .stdin
                .take()
                .ok_or_else(|| io::Error::other("owned process watchdog stdin is unavailable"))?;
            Ok(Self {
                input: Some(input),
                child,
            })
        }

        pub(super) fn disarm(&mut self) -> io::Result<()> {
            let write = self
                .input
                .as_mut()
                .ok_or_else(|| io::Error::other("owned process watchdog is closed"))?
                .write_all(b"disarm\n");
            self.input.take();
            write.and(self.wait())
        }

        fn wait(&mut self) -> io::Result<()> {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                match self.child.try_wait()? {
                    Some(_) => return Ok(()),
                    None if Instant::now() < deadline => {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    None => {
                        let _ = self.child.kill();
                        let _ = self.child.wait();
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "owned process watchdog did not stop",
                        ));
                    }
                }
            }
        }
    }

    impl Drop for Watchdog {
        fn drop(&mut self) {
            self.input.take();
            let _ = self.wait();
        }
    }

    pub(super) fn register(process_group: u32) -> io::Result<()> {
        active_groups()
            .lock()
            .map_err(|_| io::Error::other("owned process registry is unavailable"))?
            .insert(process_group);
        Ok(())
    }

    pub(super) fn unregister(process_group: u32) {
        if let Ok(mut groups) = active_groups().lock() {
            groups.remove(&process_group);
        }
    }

    pub(super) fn terminate_active() {
        let groups = active_groups()
            .lock()
            .map(|groups| groups.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        for process_group in groups {
            terminate_group(process_group);
        }
    }

    pub(super) fn terminate_group(process_group: u32) {
        let _ = signal_group(process_group, Signal::SIGKILL);
    }

    pub(super) fn signal_group(process_group: u32, signal: Signal) -> io::Result<()> {
        let process_group = admitted_group(process_group)?;
        match killpg(Pid::from_raw(process_group), signal) {
            Ok(()) | Err(Errno::ESRCH) => Ok(()),
            Err(error) => Err(io::Error::from_raw_os_error(error as i32)),
        }
    }

    pub(super) fn group_is_empty(process_group: u32) -> io::Result<bool> {
        let process_group = admitted_group(process_group)?;
        match killpg(Pid::from_raw(process_group), None) {
            Err(Errno::ESRCH) => Ok(true),
            Ok(()) | Err(Errno::EPERM) => Ok(false),
            Err(error) => Err(io::Error::from_raw_os_error(error as i32)),
        }
    }

    fn admitted_group(process_group: u32) -> io::Result<i32> {
        let process_group = i32::try_from(process_group)
            .map_err(|_| io::Error::other("owned process group is too large"))?;
        if process_group <= 1 || process_group == getpgrp().as_raw() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "owned process group is unsafe",
            ));
        }
        Ok(process_group)
    }

    fn active_groups() -> &'static Mutex<HashSet<u32>> {
        static GROUPS: OnceLock<Mutex<HashSet<u32>>> = OnceLock::new();
        GROUPS.get_or_init(|| Mutex::new(HashSet::new()))
    }
}

#[cfg(windows)]
#[path = "process/windows.rs"]
mod windows;
