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

        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.as_std_mut().creation_flags(CREATE_NO_WINDOW);
    }
}

pub(crate) struct OwnedProcessTree {
    #[cfg(unix)]
    process_group: u32,
    #[cfg(windows)]
    job: windows::Job,
    armed: bool,
}

impl OwnedProcessTree {
    pub(crate) fn attach(child: &Child) -> io::Result<Self> {
        #[cfg(unix)]
        {
            let process_id = valid_process_id(child)?;
            register_process_group(process_id)?;
            Ok(Self {
                process_group: process_id,
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

    pub(crate) fn terminate(mut self) {
        self.terminate_inner();
        self.disarm();
    }

    fn terminate_inner(&self) {
        #[cfg(unix)]
        terminate_process_group(self.process_group);

        #[cfg(windows)]
        self.job.terminate();
    }

    fn disarm(&mut self) {
        if !self.armed {
            return;
        }
        self.armed = false;

        #[cfg(unix)]
        unregister_process_group(self.process_group);
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
            self.disarm();
        }
    }
}

pub(crate) async fn terminate_unattached_child(child: &mut Child, timeout: Duration) {
    #[cfg(any(unix, windows))]
    let process_id = child.id();

    #[cfg(unix)]
    if let Some(process_id) = process_id {
        terminate_process_group(process_id);
    }

    #[cfg(windows)]
    if let Some(process_id) = process_id {
        windows::terminate_process_tree(process_id, timeout).await;
    }

    let _ = child.start_kill();
    let _ = tokio::time::timeout(timeout, child.wait()).await;
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

#[cfg(windows)]
mod windows {
    use std::ffi::c_void;
    use std::io;
    use std::mem::size_of;
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use std::path::PathBuf;
    use std::time::Duration;

    use tokio::process::{Child, Command};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    pub(super) struct Job {
        handle: OwnedHandle,
    }

    impl Job {
        pub(super) fn attach(child: &Child) -> io::Result<Self> {
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
            Ok(Self { handle })
        }

        pub(super) fn terminate(&self) {
            let _ = unsafe { TerminateJobObject(self.handle.as_raw_handle(), 1) };
        }
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
    const DESCENDANT_MODE_ENV: &str = "A3S_TEST_CUA_DESCENDANT_MODE";
    const DESCENDANT_GATE_ENV: &str = "A3S_TEST_CUA_DESCENDANT_GATE";
    const DESCENDANT_PID_ENV: &str = "A3S_TEST_CUA_DESCENDANT_PID_FILE";

    #[tokio::test]
    async fn owned_process_tree_terminates_a_late_spawned_descendant() {
        assert_descendant_is_terminated(OwnedProcessTree::terminate).await;
    }

    #[tokio::test]
    async fn dropping_owned_process_tree_terminates_a_late_spawned_descendant() {
        assert_descendant_is_terminated(drop).await;
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn emergency_registry_terminates_a_late_spawned_descendant() {
        assert_descendant_is_terminated(|mut process_tree| {
            terminate_active_cua_processes();
            process_tree.disarm();
        })
        .await;
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
        wait_until_stopped(descendant).await;
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

    async fn wait_until_stopped(process_id: u32) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while process_is_running(process_id) {
            assert!(
                tokio::time::Instant::now() < deadline,
                "descendant process {process_id} survived owned-tree termination"
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
        assert_eq!(mode, "parent");

        let gate = std::env::var_os(DESCENDANT_GATE_ENV)
            .map(std::path::PathBuf::from)
            .expect("descendant fixture gate");
        let pid_file = std::env::var_os(DESCENDANT_PID_ENV)
            .map(std::path::PathBuf::from)
            .expect("descendant fixture PID file");
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !gate.is_file() {
            assert!(
                std::time::Instant::now() < deadline,
                "descendant fixture gate was not released"
            );
            std::thread::sleep(Duration::from_millis(10));
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
