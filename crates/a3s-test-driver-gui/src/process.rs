use std::io;
use std::time::Duration;

use tokio::process::{Child, Command};

pub(crate) fn configure_owned_process(command: &mut Command) {
    command.kill_on_drop(true);

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

pub(crate) struct OwnedProcessTree {
    #[cfg(unix)]
    process_group: u32,
    #[cfg(unix)]
    watchdog: UnixWatchdog,
    #[cfg(windows)]
    job: windows::Job,
    armed: bool,
}

impl OwnedProcessTree {
    pub(crate) fn attach(child: &Child) -> io::Result<Self> {
        #[cfg(unix)]
        {
            let process_id = valid_process_id(child)?;
            let watchdog = UnixWatchdog::spawn(process_id)?;
            register_process_group(process_id)?;
            Ok(Self {
                process_group: process_id,
                watchdog,
                armed: true,
            })
        }

        #[cfg(windows)]
        {
            valid_process_id(child)?;
            let job = windows::Job::attach(child)?;
            Ok(Self { job, armed: true })
        }

        #[cfg(not(any(unix, windows)))]
        {
            valid_process_id(child)?;
            Ok(Self { armed: true })
        }
    }

    #[cfg(test)]
    fn terminate(mut self) {
        self.terminate_inner();
        let _ = self.disarm();
    }

    pub(crate) fn terminate_now(&self) {
        self.terminate_inner();
    }

    pub(crate) fn terminate_and_wait(mut self, timeout: Duration) -> io::Result<()> {
        self.terminate_inner();
        let result = self.wait_for_exit(timeout);
        let disarm_result = self.disarm();
        result.and(disarm_result)
    }

    fn terminate_inner(&self) {
        #[cfg(unix)]
        terminate_process_group(self.process_group);

        #[cfg(windows)]
        self.job.terminate();
    }

    fn wait_for_exit(&self, timeout: Duration) -> io::Result<()> {
        #[cfg(unix)]
        return wait_for_process_group(self.process_group, timeout);

        #[cfg(windows)]
        return self.job.wait_for_exit(timeout);

        #[cfg(not(any(unix, windows)))]
        {
            let _ = timeout;
            Ok(())
        }
    }

    fn disarm(&mut self) -> io::Result<()> {
        if !self.armed {
            return Ok(());
        }
        self.armed = false;

        #[cfg(unix)]
        {
            unregister_process_group(self.process_group);
            self.watchdog.trigger()
        }

        #[cfg(not(unix))]
        Ok(())
    }
}

fn valid_process_id(child: &Child) -> io::Result<u32> {
    child
        .id()
        .filter(|process_id| *process_id > 1)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "CUA MCP proxy did not expose a valid process identifier",
            )
        })
}

impl Drop for OwnedProcessTree {
    fn drop(&mut self) {
        if self.armed {
            self.terminate_inner();
            let _ = self.disarm();
        }
    }
}

#[cfg(unix)]
struct UnixWatchdog {
    input: Option<std::process::ChildStdin>,
    child: std::process::Child,
}

#[cfg(unix)]
impl UnixWatchdog {
    fn spawn(process_group: u32) -> io::Result<Self> {
        use std::process::Stdio;

        let script = "IFS= read -r _ || true; \
                      kill -KILL -- \"-$1\" 2>/dev/null || true";
        let mut child = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(script)
            .arg("a3s-test-cua-watchdog")
            .arg(process_group.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        let Some(input) = child.stdin.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::other(
                "CUA process-group watchdog stdin is unavailable",
            ));
        };
        Ok(Self {
            input: Some(input),
            child,
        })
    }

    fn trigger(&mut self) -> io::Result<()> {
        self.input.take();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => return Ok(()),
                Ok(None) if std::time::Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Ok(None) => {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "CUA process-group watchdog did not exit",
                    ));
                }
                Err(error) => return Err(error),
            }
        }
    }
}

#[cfg(unix)]
impl Drop for UnixWatchdog {
    fn drop(&mut self) {
        let _ = self.trigger();
    }
}

pub(crate) async fn terminate_unattached_child(child: &mut Child, timeout: Duration) {
    let deadline = std::time::Instant::now() + timeout;

    #[cfg(any(unix, windows))]
    let process_id = child.id();

    #[cfg(unix)]
    if let Some(process_id) = process_id {
        terminate_process_group(process_id);
    }

    #[cfg(windows)]
    if let Some(process_id) = process_id {
        windows::terminate_process_tree(process_id, timeout / 2).await;
    }

    let _ = child.start_kill();
    let _ = tokio::time::timeout(remaining_until(deadline), child.wait()).await;

    #[cfg(unix)]
    if let Some(process_group) = process_id {
        let remaining = remaining_until(deadline);
        let _ =
            tokio::task::spawn_blocking(move || wait_for_process_group(process_group, remaining))
                .await;
    }
}

fn remaining_until(deadline: std::time::Instant) -> Duration {
    deadline.saturating_duration_since(std::time::Instant::now())
}

/// Immediately terminate CUA proxy process groups owned by this process.
///
/// The CLI invokes this before its emergency second-interrupt exit. Windows
/// proxies are assigned to kill-on-close Job Objects and therefore terminate
/// when the host process exits; Unix needs the explicit process-group signal.
pub fn terminate_active_cua_processes() {
    #[cfg(unix)]
    {
        let process_groups = active_process_groups()
            .lock()
            .map(|groups| groups.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        for process_group in process_groups {
            terminate_process_group(process_group);
        }
    }
}

#[cfg(unix)]
fn active_process_groups() -> &'static std::sync::Mutex<std::collections::HashSet<u32>> {
    static GROUPS: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<u32>>> =
        std::sync::OnceLock::new();
    GROUPS.get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()))
}

#[cfg(unix)]
fn register_process_group(process_group: u32) -> io::Result<()> {
    let mut groups = active_process_groups()
        .lock()
        .map_err(|_| io::Error::other("active CUA process registry is unavailable"))?;
    groups.insert(process_group);
    Ok(())
}

#[cfg(unix)]
fn unregister_process_group(process_group: u32) {
    if let Ok(mut groups) = active_process_groups().lock() {
        groups.remove(&process_group);
    }
}

#[cfg(unix)]
fn terminate_process_group(process_group: u32) {
    use nix::sys::signal::{killpg, Signal};
    use nix::unistd::{getpgrp, Pid};

    let Ok(process_group) = i32::try_from(process_group) else {
        return;
    };
    if process_group <= 1 || process_group == getpgrp().as_raw() {
        return;
    }
    let _ = killpg(Pid::from_raw(process_group), Signal::SIGKILL);
}

#[cfg(unix)]
fn wait_for_process_group(process_group: u32, timeout: Duration) -> io::Result<()> {
    use nix::errno::Errno;
    use nix::sys::signal::killpg;
    use nix::unistd::Pid;

    let process_group = i32::try_from(process_group)
        .map_err(|_| io::Error::other("CUA process-group identifier is too large"))?;
    let deadline = std::time::Instant::now() + timeout;
    loop {
        match killpg(Pid::from_raw(process_group), None) {
            Err(Errno::ESRCH) => return Ok(()),
            Ok(()) | Err(Errno::EPERM) => {}
            Err(error) => return Err(io::Error::from_raw_os_error(error as i32)),
        }
        if std::time::Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "CUA process group did not stop before the cleanup deadline",
            ));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(windows)]
mod windows {
    use std::ffi::c_void;
    use std::io;
    use std::mem::{size_of, MaybeUninit};
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    use tokio::process::{Child, Command};
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
    };
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectBasicAccountingInformation,
        JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
        TerminateJobObject, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    use windows_sys::Win32::System::Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME};

    pub(super) struct Job {
        handle: OwnedHandle,
    }

    impl Job {
        pub(super) fn attach(child: &Child) -> io::Result<Self> {
            let process_id = super::valid_process_id(child)?;
            let raw_handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            if raw_handle.is_null() {
                return Err(io::Error::last_os_error());
            }
            let handle = unsafe { OwnedHandle::from_raw_handle(raw_handle) };

            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let information_length =
                u32::try_from(size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>())
                    .map_err(|_| io::Error::other("Windows Job Object limits are too large"))?;
            let configured = unsafe {
                SetInformationJobObject(
                    handle.as_raw_handle(),
                    JobObjectExtendedLimitInformation,
                    (&raw const limits).cast::<c_void>(),
                    information_length,
                )
            };
            if configured == 0 {
                return Err(io::Error::last_os_error());
            }

            let child_handle = child.raw_handle().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "CUA MCP proxy exited before Job Object assignment",
                )
            })?;
            let assigned =
                unsafe { AssignProcessToJobObject(handle.as_raw_handle(), child_handle) };
            if assigned == 0 {
                return Err(io::Error::last_os_error());
            }
            let job = Self { handle };
            if let Err(error) = resume_process(process_id) {
                job.terminate();
                return Err(error);
            }
            Ok(job)
        }

        pub(super) fn terminate(&self) {
            let _ = unsafe { TerminateJobObject(self.handle.as_raw_handle(), 1) };
        }

        pub(super) fn wait_for_exit(&self, timeout: Duration) -> io::Result<()> {
            let deadline = Instant::now() + timeout;
            while self.active_process_count()? > 0 {
                if Instant::now() >= deadline {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "CUA Job Object did not empty before the cleanup deadline",
                    ));
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(())
        }

        fn active_process_count(&self) -> io::Result<u32> {
            let mut accounting = MaybeUninit::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>::uninit();
            let information_length =
                u32::try_from(size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>())
                    .map_err(|_| io::Error::other("Windows Job accounting is too large"))?;
            let queried = unsafe {
                QueryInformationJobObject(
                    self.handle.as_raw_handle(),
                    JobObjectBasicAccountingInformation,
                    accounting.as_mut_ptr().cast::<c_void>(),
                    information_length,
                    std::ptr::null_mut(),
                )
            };
            if queried == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(unsafe { accounting.assume_init() }.ActiveProcesses)
        }
    }

    fn resume_process(process_id: u32) -> io::Result<()> {
        let raw_snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
        if raw_snapshot == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        let snapshot = unsafe { OwnedHandle::from_raw_handle(raw_snapshot) };
        let mut entry = THREADENTRY32 {
            dwSize: u32::try_from(size_of::<THREADENTRY32>())
                .map_err(|_| io::Error::other("Windows thread entry is too large"))?,
            ..Default::default()
        };
        let mut found = false;
        let mut available = unsafe { Thread32First(snapshot.as_raw_handle(), &mut entry) } != 0;
        while available {
            if entry.th32OwnerProcessID == process_id {
                let raw_thread =
                    unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) };
                if raw_thread.is_null() {
                    return Err(io::Error::last_os_error());
                }
                let thread = unsafe { OwnedHandle::from_raw_handle(raw_thread) };
                let previous_count = unsafe { ResumeThread(thread.as_raw_handle()) };
                if previous_count == u32::MAX {
                    return Err(io::Error::last_os_error());
                }
                found = true;
            }
            available = unsafe { Thread32Next(snapshot.as_raw_handle(), &mut entry) } != 0;
        }
        if !found {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "suspended CUA MCP proxy thread was not found",
            ));
        }
        Ok(())
    }

    pub(super) async fn terminate_process_tree(process_id: u32, timeout: Duration) {
        if process_id <= 1 || process_id == std::process::id() {
            return;
        }
        let Some(taskkill) = system_executable("taskkill.exe") else {
            return;
        };
        let mut command = Command::new(taskkill);
        command
            .args(["/PID", &process_id.to_string(), "/T", "/F"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        use std::os::windows::process::CommandExt as _;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.as_std_mut().creation_flags(CREATE_NO_WINDOW);
        let _ = tokio::time::timeout(timeout, command.status()).await;
    }

    fn system_executable(relative: &str) -> Option<PathBuf> {
        let root = PathBuf::from(std::env::var_os("SystemRoot")?);
        if !root.is_absolute() {
            return None;
        }
        let executable = root.join("System32").join(relative);
        executable.is_file().then_some(executable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::process::Stdio;

    const DESCENDANT_FIXTURE_TEST: &str = "process::tests::owned_process_tree_descendant_fixture";
    #[cfg(unix)]
    const WATCHDOG_HOST_FIXTURE_TEST: &str =
        "process::tests::owned_process_tree_watchdog_host_fixture";
    const DESCENDANT_MODE_ENV: &str = "A3S_TEST_CUA_DESCENDANT_MODE";
    const DESCENDANT_GATE_ENV: &str = "A3S_TEST_CUA_DESCENDANT_GATE";
    const DESCENDANT_PID_ENV: &str = "A3S_TEST_CUA_DESCENDANT_PID_FILE";
    #[cfg(unix)]
    const WATCHDOG_PROXY_PID_ENV: &str = "A3S_TEST_CUA_WATCHDOG_PROXY_PID_FILE";

    #[tokio::test]
    async fn owned_process_tree_terminates_a_late_spawned_descendant() {
        let _test_guard = process_tree_test_lock().lock().await;
        assert_descendant_is_terminated(OwnedProcessTree::terminate).await;
    }

    #[tokio::test]
    async fn dropping_owned_process_tree_terminates_a_late_spawned_descendant() {
        let _test_guard = process_tree_test_lock().lock().await;
        assert_descendant_is_terminated(drop).await;
    }

    #[tokio::test]
    async fn owned_process_tree_contains_a_descendant_that_spawns_before_attachment() {
        let _test_guard = process_tree_test_lock().lock().await;
        let temp = tempfile::tempdir().expect("temp dir");
        let gate = temp.path().join("unused-gate");
        let pid_file = temp.path().join("immediate-child.pid");
        let mut command = descendant_fixture_command(&gate, &pid_file);
        command.env(DESCENDANT_MODE_ENV, "parent-immediate");
        configure_owned_process(&mut command);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let mut child = command.spawn().expect("spawn immediate proxy fixture");
        tokio::time::sleep(Duration::from_millis(250)).await;
        let process_tree = OwnedProcessTree::attach(&child).expect("bind owned process tree");
        let descendant = wait_for_pid(&pid_file).await;

        process_tree.terminate();
        let _ = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
        let stopped = wait_until_stopped(descendant).await;
        if !stopped {
            terminate_fixture(descendant).await;
        }
        assert!(
            stopped,
            "a descendant spawned before Job attachment escaped containment"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn host_sigkill_triggers_watchdog_for_proxy_and_descendant() {
        let _test_guard = process_tree_test_lock().lock().await;
        let temp = tempfile::tempdir().expect("temp dir");
        let proxy_pid_file = temp.path().join("proxy.pid");
        let descendant_pid_file = temp.path().join("descendant.pid");
        let mut host = std::process::Command::new(
            std::env::current_exe().expect("current watchdog host executable"),
        )
        .args([WATCHDOG_HOST_FIXTURE_TEST, "--ignored", "--exact"])
        .env(WATCHDOG_PROXY_PID_ENV, &proxy_pid_file)
        .env(DESCENDANT_PID_ENV, &descendant_pid_file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn watchdog host fixture");

        let proxy = wait_for_pid(&proxy_pid_file).await;
        let descendant = wait_for_pid(&descendant_pid_file).await;
        assert!(process_is_running(proxy), "proxy fixture never started");
        assert!(
            process_is_running(descendant),
            "proxy descendant fixture never started"
        );

        let host_pid = i32::try_from(host.id()).expect("watchdog host PID");
        nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(host_pid),
            nix::sys::signal::Signal::SIGKILL,
        )
        .expect("kill watchdog host fixture");
        wait_for_host_exit(&mut host).await;

        let proxy_stopped = wait_until_stopped(proxy).await;
        let descendant_stopped = wait_until_stopped(descendant).await;
        if !proxy_stopped {
            terminate_fixture(proxy).await;
        }
        if !descendant_stopped {
            terminate_fixture(descendant).await;
        }
        assert!(
            proxy_stopped,
            "CUA proxy {proxy} survived its host's SIGKILL"
        );
        assert!(
            descendant_stopped,
            "CUA proxy descendant {descendant} survived its host's SIGKILL"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn emergency_registry_terminates_a_late_spawned_descendant() {
        let _test_guard = process_tree_test_lock().lock().await;
        assert_descendant_is_terminated(|mut process_tree| {
            terminate_active_cua_processes();
            let _ = process_tree.disarm();
        })
        .await;
    }

    fn process_tree_test_lock() -> &'static tokio::sync::Mutex<()> {
        static LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
    }

    async fn assert_descendant_is_terminated(terminate: impl FnOnce(OwnedProcessTree)) {
        let temp = tempfile::tempdir().expect("temp dir");
        let gate = temp.path().join("spawn-child");
        let pid_file = temp.path().join("child.pid");
        let mut command = descendant_fixture_command(&gate, &pid_file);
        configure_owned_process(&mut command);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let mut child = command.spawn().expect("spawn owned proxy fixture");
        let process_tree = OwnedProcessTree::attach(&child).expect("bind owned process tree");
        tokio::fs::write(&gate, b"spawn")
            .await
            .expect("release child spawn gate");
        let descendant = wait_for_pid(&pid_file).await;
        assert!(process_is_running(descendant), "descendant never started");

        terminate(process_tree);
        let status = tokio::time::timeout(Duration::from_secs(5), child.wait())
            .await
            .expect("owned proxy termination deadline")
            .expect("wait for owned proxy");
        assert!(!status.success());
        assert!(
            wait_until_stopped(descendant).await,
            "descendant process {descendant} survived owned-tree termination"
        );
    }

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
                "descendant PID was not published"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

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
    async fn wait_for_host_exit(child: &mut std::process::Child) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            if child.try_wait().expect("poll watchdog host").is_some() {
                return;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "watchdog host did not stop after SIGKILL"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    fn descendant_fixture_command(gate: &Path, pid_file: &Path) -> Command {
        let mut command = Command::new(std::env::current_exe().expect("current test executable"));
        command
            .args([DESCENDANT_FIXTURE_TEST, "--ignored", "--exact"])
            .env(DESCENDANT_MODE_ENV, "parent")
            .env(DESCENDANT_GATE_ENV, gate)
            .env(DESCENDANT_PID_ENV, pid_file);
        command
    }

    #[test]
    #[ignore = "helper process for owned process-tree lifecycle tests"]
    fn owned_process_tree_descendant_fixture() {
        let mode = std::env::var(DESCENDANT_MODE_ENV).expect("descendant fixture mode");
        if mode == "leaf" {
            std::thread::sleep(Duration::from_secs(30));
            return;
        }
        assert!(matches!(mode.as_str(), "parent" | "parent-immediate"));

        let gate = std::env::var_os(DESCENDANT_GATE_ENV)
            .map(std::path::PathBuf::from)
            .expect("descendant fixture gate");
        let pid_file = std::env::var_os(DESCENDANT_PID_ENV)
            .map(std::path::PathBuf::from)
            .expect("descendant fixture PID file");
        if mode == "parent" {
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while !gate.is_file() {
                assert!(
                    std::time::Instant::now() < deadline,
                    "descendant fixture gate was not released"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
        }

        let mut child = std::process::Command::new(
            std::env::current_exe().expect("current descendant fixture executable"),
        )
        .args([DESCENDANT_FIXTURE_TEST, "--ignored", "--exact"])
        .env(DESCENDANT_MODE_ENV, "leaf")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn descendant fixture leaf");
        std::fs::write(pid_file, child.id().to_string()).expect("publish descendant PID");
        let _ = child.wait();
    }

    #[cfg(unix)]
    #[tokio::test]
    #[ignore = "helper process for watchdog host-death lifecycle test"]
    async fn owned_process_tree_watchdog_host_fixture() {
        let proxy_pid_file = std::env::var_os(WATCHDOG_PROXY_PID_ENV)
            .map(std::path::PathBuf::from)
            .expect("watchdog proxy PID file");
        let descendant_pid_file = std::env::var_os(DESCENDANT_PID_ENV)
            .map(std::path::PathBuf::from)
            .expect("watchdog descendant PID file");
        let unused_gate = proxy_pid_file.with_extension("gate");
        let mut command = descendant_fixture_command(&unused_gate, &descendant_pid_file);
        command.env(DESCENDANT_MODE_ENV, "parent-immediate");
        configure_owned_process(&mut command);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let child = command.spawn().expect("spawn watchdog-owned CUA proxy");
        let process_tree = OwnedProcessTree::attach(&child).expect("attach watchdog-owned proxy");
        std::fs::write(
            &proxy_pid_file,
            child.id().expect("watchdog-owned proxy PID").to_string(),
        )
        .expect("publish watchdog-owned proxy PID");

        let _owned_child = child;
        let _owned_process_tree = process_tree;
        std::future::pending::<()>().await;
    }

    #[cfg(unix)]
    async fn terminate_fixture(process_id: u32) {
        let Ok(process_id) = i32::try_from(process_id) else {
            return;
        };
        let _ = nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(process_id),
            nix::sys::signal::Signal::SIGKILL,
        );
    }

    #[cfg(windows)]
    async fn terminate_fixture(process_id: u32) {
        windows::terminate_process_tree(process_id, Duration::from_secs(5)).await;
    }

    #[cfg(not(any(unix, windows)))]
    async fn terminate_fixture(_process_id: u32) {}

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

    #[cfg(windows)]
    fn process_is_running(process_id: u32) -> bool {
        use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
        use windows_sys::Win32::Foundation::STILL_ACTIVE;
        use windows_sys::Win32::System::Threading::{
            GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };

        let raw_handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id) };
        if raw_handle.is_null() {
            return false;
        }
        let handle = unsafe { OwnedHandle::from_raw_handle(raw_handle) };
        let mut exit_code = 0;
        let queried = unsafe { GetExitCodeProcess(handle.as_raw_handle(), &mut exit_code) != 0 };
        queried && exit_code == STILL_ACTIVE as u32
    }

    #[cfg(not(any(unix, windows)))]
    fn process_is_running(_process_id: u32) -> bool {
        false
    }
}
