use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::process::Child;

const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
const SHUTDOWN_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Platform containment for a browser command or a complete deterministic
/// session, including descendants that outlive short-lived launchers.
pub(crate) struct BrowserProcessTree {
    platform: platform::ProcessTree,
    attached: AtomicBool,
}

impl BrowserProcessTree {
    pub(crate) fn new() -> io::Result<Self> {
        Ok(Self {
            platform: platform::ProcessTree::new()?,
            attached: AtomicBool::new(false),
        })
    }

    pub(crate) fn attach(&self, child: &Child) -> io::Result<()> {
        self.platform.attach(child)?;
        self.attached.store(true, Ordering::Release);
        Ok(())
    }

    pub(crate) fn terminate_and_wait(&self) -> io::Result<()> {
        self.terminate()?;
        self.wait_for_exit()
    }

    pub(crate) fn terminate(&self) -> io::Result<()> {
        self.platform.terminate()
    }

    pub(crate) fn wait_for_exit(&self) -> io::Result<()> {
        self.platform.wait_for_exit(SHUTDOWN_TIMEOUT)
    }

    pub(crate) fn disarm(&self) -> io::Result<()> {
        self.platform.disarm()
    }

    pub(crate) fn prune_exited(&self) -> io::Result<()> {
        self.platform.prune_exited()
    }

    pub(crate) fn has_attached_processes(&self) -> bool {
        self.attached.load(Ordering::Acquire)
    }
}

pub(crate) fn resume_process(process_id: u32) -> io::Result<()> {
    platform::resume_process(process_id)
}

fn valid_process_id(child: &Child) -> io::Result<u32> {
    child
        .id()
        .filter(|process_id| *process_id > 1)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "browser command did not expose a valid process identifier",
            )
        })
}

#[cfg(unix)]
mod platform {
    use std::collections::HashSet;
    use std::io::{self, Write as _};
    use std::process::{ChildStdin, Command, Stdio};
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    use nix::errno::Errno;
    use nix::sys::signal::{killpg, Signal};
    use nix::unistd::{getpgrp, Pid};
    use tokio::process::Child;

    use super::{valid_process_id, SHUTDOWN_POLL_INTERVAL};

    pub(super) struct ProcessTree {
        state: Mutex<ProcessState>,
    }

    struct ProcessState {
        process_groups: HashSet<u32>,
        watchdog: Option<Watchdog>,
    }

    struct Watchdog {
        input: Option<ChildStdin>,
        child: std::process::Child,
    }

    impl ProcessTree {
        pub(super) fn new() -> io::Result<Self> {
            Ok(Self {
                state: Mutex::new(ProcessState {
                    process_groups: HashSet::new(),
                    watchdog: Some(Watchdog::spawn()?),
                }),
            })
        }

        pub(super) fn attach(&self, child: &Child) -> io::Result<()> {
            let process_group = valid_process_id(child)?;
            self.prune_exited()?;
            let mut state = self
                .state
                .lock()
                .map_err(|_| io::Error::other("browser process-group registry is unavailable"))?;
            if state.process_groups.contains(&process_group) {
                return Ok(());
            }
            state
                .watchdog
                .as_mut()
                .ok_or_else(|| io::Error::other("browser watchdog is unavailable"))?
                .add(process_group)?;
            state.process_groups.insert(process_group);
            Ok(())
        }

        pub(super) fn prune_exited(&self) -> io::Result<()> {
            let mut state = self
                .state
                .lock()
                .map_err(|_| io::Error::other("browser process-group registry is unavailable"))?;
            let exited = state
                .process_groups
                .iter()
                .copied()
                .filter(|process_group| !process_group_exists(*process_group))
                .collect::<Vec<_>>();
            for process_group in exited {
                state
                    .watchdog
                    .as_mut()
                    .ok_or_else(|| io::Error::other("browser watchdog is unavailable"))?
                    .remove(process_group)?;
                state.process_groups.remove(&process_group);
            }
            Ok(())
        }

        pub(super) fn terminate(&self) -> io::Result<()> {
            let groups = self
                .state
                .lock()
                .map_err(|_| io::Error::other("browser process-group registry is unavailable"))?
                .process_groups
                .iter()
                .copied()
                .collect::<Vec<_>>();
            terminate_groups(&groups)
        }

        pub(super) fn wait_for_exit(&self, timeout: Duration) -> io::Result<()> {
            let groups = self
                .state
                .lock()
                .map_err(|_| io::Error::other("browser process-group registry is unavailable"))?
                .process_groups
                .iter()
                .copied()
                .collect::<Vec<_>>();
            let deadline = Instant::now() + timeout;
            while groups.iter().any(|group| process_group_exists(*group)) {
                if Instant::now() >= deadline {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "browser process groups did not stop before the cleanup deadline",
                    ));
                }
                std::thread::sleep(SHUTDOWN_POLL_INTERVAL);
            }
            Ok(())
        }

        pub(super) fn disarm(&self) -> io::Result<()> {
            let (watchdog, groups_empty) = {
                let mut state = self.state.lock().map_err(|_| {
                    io::Error::other("browser process-group registry is unavailable")
                })?;
                (state.watchdog.take(), state.process_groups.is_empty())
            };
            let Some(mut watchdog) = watchdog else {
                return if groups_empty {
                    Ok(())
                } else {
                    Err(io::Error::other(
                        "browser watchdog closed before process groups were released",
                    ))
                };
            };
            watchdog.disarm()?;
            self.state
                .lock()
                .map_err(|_| io::Error::other("browser process-group registry is unavailable"))?
                .process_groups
                .clear();
            Ok(())
        }
    }

    impl Drop for ProcessTree {
        fn drop(&mut self) {
            let state = self
                .state
                .get_mut()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let groups = state.process_groups.drain().collect::<Vec<_>>();
            let watchdog = state.watchdog.take();
            let _ = terminate_groups(&groups);
            if let Some(mut watchdog) = watchdog {
                let _ = watchdog.trigger();
            }
        }
    }

    impl Watchdog {
        fn spawn() -> io::Result<Self> {
            let script = "groups=''; \
                          while IFS= read -r control; do \
                            case \"$control\" in \
                              disarm) exit 0 ;; \
                              'add '*) groups=\"$groups ${control#add }\" ;; \
                              'remove '*) \
                                target=${control#remove }; remaining=''; \
                                for group in $groups; do \
                                  [ \"$group\" = \"$target\" ] || remaining=\"$remaining $group\"; \
                                done; \
                                groups=$remaining ;; \
                            esac; \
                          done; \
                          for group in $groups; do \
                            /bin/kill -KILL -- \"-$group\" 2>/dev/null || true; \
                          done";
            let mut child = Command::new("/bin/sh")
                .args(["-c", script, "a3s-test-browser-watchdog"])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?;
            let Some(input) = child.stdin.take() else {
                let _ = child.kill();
                let _ = child.wait();
                return Err(io::Error::other(
                    "browser process-group watchdog stdin is unavailable",
                ));
            };
            Ok(Self {
                input: Some(input),
                child,
            })
        }

        fn add(&mut self, process_group: u32) -> io::Result<()> {
            let input = self
                .input
                .as_mut()
                .ok_or_else(|| io::Error::other("browser watchdog is already closed"))?;
            writeln!(input, "add {process_group}")
        }

        fn remove(&mut self, process_group: u32) -> io::Result<()> {
            let input = self
                .input
                .as_mut()
                .ok_or_else(|| io::Error::other("browser watchdog is already closed"))?;
            writeln!(input, "remove {process_group}")
        }

        fn disarm(&mut self) -> io::Result<()> {
            let write_result = self
                .input
                .as_mut()
                .ok_or_else(|| io::Error::other("browser watchdog is already closed"))?
                .write_all(b"disarm\n");
            self.input.take();
            let wait_result = self.wait();
            write_result.and(wait_result)
        }

        fn trigger(&mut self) -> io::Result<()> {
            self.input.take();
            self.wait()
        }

        fn wait(&mut self) -> io::Result<()> {
            let deadline = Instant::now() + super::SHUTDOWN_TIMEOUT;
            loop {
                match self.child.try_wait() {
                    Ok(Some(_)) => return Ok(()),
                    Ok(None) if Instant::now() < deadline => {
                        std::thread::sleep(SHUTDOWN_POLL_INTERVAL);
                    }
                    Ok(None) => {
                        let _ = self.child.kill();
                        let _ = self.child.wait();
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "browser process-group watchdog did not exit",
                        ));
                    }
                    Err(error) => return Err(error),
                }
            }
        }
    }

    impl Drop for Watchdog {
        fn drop(&mut self) {
            let _ = self.trigger();
        }
    }

    pub(super) fn resume_process(_process_id: u32) -> io::Result<()> {
        Ok(())
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

    fn terminate_groups(groups: &[u32]) -> io::Result<()> {
        let current_group = u32::try_from(getpgrp().as_raw()).unwrap_or_default();
        let mut first_error = None;
        for group in groups
            .iter()
            .copied()
            .filter(|group| *group > 1 && *group != current_group)
        {
            let Ok(group) = i32::try_from(group) else {
                continue;
            };
            if let Err(error) = killpg(Pid::from_raw(group), Signal::SIGKILL) {
                if error != Errno::ESRCH && first_error.is_none() {
                    first_error = Some(io::Error::from_raw_os_error(error as i32));
                }
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    #[cfg(test)]
    mod tests {
        use std::os::unix::process::CommandExt as _;

        use super::*;

        #[test]
        fn removed_group_is_not_killed_when_watchdog_control_closes() {
            let mut command = std::process::Command::new("sleep");
            command
                .arg("30")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .process_group(0);
            let mut child = command.spawn().expect("spawn watchdog fixture group");
            let process_group = child.id();
            let mut watchdog = Watchdog::spawn().expect("spawn browser watchdog");
            watchdog.add(process_group).expect("register fixture group");
            watchdog
                .remove(process_group)
                .expect("remove fixture group");
            watchdog.trigger().expect("stop browser watchdog");

            let remained_running = child.try_wait().expect("inspect fixture group").is_none();
            terminate_groups(&[process_group]).expect("terminate fixture group");
            let _ = child.wait();
            assert!(
                remained_running,
                "watchdog killed a process group after ownership was released"
            );
        }
    }
}

#[cfg(windows)]
mod platform {
    use std::ffi::c_void;
    use std::io;
    use std::mem::{size_of, MaybeUninit};
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use std::time::{Duration, Instant};

    use tokio::process::Child;
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

    use super::{valid_process_id, SHUTDOWN_POLL_INTERVAL};

    pub(super) struct ProcessTree {
        job: OwnedHandle,
    }

    impl ProcessTree {
        pub(super) fn new() -> io::Result<Self> {
            let raw_handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            if raw_handle.is_null() {
                return Err(io::Error::last_os_error());
            }
            let job = unsafe { OwnedHandle::from_raw_handle(raw_handle) };

            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let information_length =
                u32::try_from(size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>())
                    .map_err(|_| io::Error::other("Windows Job Object limits are too large"))?;
            let configured = unsafe {
                SetInformationJobObject(
                    job.as_raw_handle(),
                    JobObjectExtendedLimitInformation,
                    (&raw const limits).cast::<c_void>(),
                    information_length,
                )
            };
            if configured == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(Self { job })
        }

        pub(super) fn attach(&self, child: &Child) -> io::Result<()> {
            valid_process_id(child)?;
            let child_handle = child.raw_handle().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "browser command exited before Job Object assignment",
                )
            })?;
            let assigned =
                unsafe { AssignProcessToJobObject(self.job.as_raw_handle(), child_handle) };
            if assigned == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        }

        pub(super) fn terminate(&self) -> io::Result<()> {
            let terminated = unsafe { TerminateJobObject(self.job.as_raw_handle(), 1) };
            if terminated == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        }

        pub(super) fn wait_for_exit(&self, timeout: Duration) -> io::Result<()> {
            let deadline = Instant::now() + timeout;
            while self.active_process_count()? > 0 {
                if Instant::now() >= deadline {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "browser Job Object did not empty before the cleanup deadline",
                    ));
                }
                std::thread::sleep(SHUTDOWN_POLL_INTERVAL);
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
                    self.job.as_raw_handle(),
                    JobObjectBasicAccountingInformation,
                    accounting.as_mut_ptr().cast::<c_void>(),
                    information_length,
                    std::ptr::null_mut(),
                )
            };
            if queried == 0 {
                return Err(io::Error::last_os_error());
            }
            let accounting = unsafe { accounting.assume_init() };
            Ok(accounting.ActiveProcesses)
        }

        pub(super) fn disarm(&self) -> io::Result<()> {
            let limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            let information_length =
                u32::try_from(size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>())
                    .map_err(|_| io::Error::other("Windows Job Object limits are too large"))?;
            let configured = unsafe {
                SetInformationJobObject(
                    self.job.as_raw_handle(),
                    JobObjectExtendedLimitInformation,
                    (&raw const limits).cast::<c_void>(),
                    information_length,
                )
            };
            if configured == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        }

        pub(super) fn prune_exited(&self) -> io::Result<()> {
            Ok(())
        }
    }

    pub(super) fn resume_process(process_id: u32) -> io::Result<()> {
        if process_id <= 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "browser command process identifier is invalid",
            ));
        }
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
                "suspended browser command thread was not found",
            ));
        }
        Ok(())
    }
}

#[cfg(not(any(unix, windows)))]
mod platform {
    use std::io;
    use std::time::Duration;

    use tokio::process::Child;

    use super::valid_process_id;

    pub(super) struct ProcessTree;

    impl ProcessTree {
        pub(super) fn new() -> io::Result<Self> {
            Ok(Self)
        }

        pub(super) fn attach(&self, child: &Child) -> io::Result<()> {
            valid_process_id(child).map(|_| ())
        }

        pub(super) fn terminate(&self) -> io::Result<()> {
            Ok(())
        }

        pub(super) fn wait_for_exit(&self, _timeout: Duration) -> io::Result<()> {
            Ok(())
        }

        pub(super) fn disarm(&self) -> io::Result<()> {
            Ok(())
        }

        pub(super) fn prune_exited(&self) -> io::Result<()> {
            Ok(())
        }
    }

    pub(super) fn resume_process(_process_id: u32) -> io::Result<()> {
        Ok(())
    }
}
