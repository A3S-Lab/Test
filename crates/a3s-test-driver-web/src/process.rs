use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use tokio::process::Child;

use crate::path_security::is_link_like;
use crate::process_tree::BrowserProcessTree;
use crate::runtime::RuntimeDirectory;

const RUNTIME_ENVIRONMENTS: [&str; 2] = ["A3S_USE_BROWSER_SOCKET_DIR", "AGENT_BROWSER_SOCKET_DIR"];

#[derive(Clone)]
struct ActiveSession {
    runtime: RuntimeDirectory,
    namespace: String,
    session: String,
    process_markers: Vec<String>,
    process_tree: Arc<BrowserProcessTree>,
}

pub(crate) struct SessionRegistration {
    runtime_dir: PathBuf,
    process_tree: Arc<BrowserProcessTree>,
    armed: bool,
}

impl SessionRegistration {
    pub(crate) fn new(
        runtime: RuntimeDirectory,
        namespace: String,
        session: String,
        process_markers: Vec<String>,
    ) -> io::Result<Self> {
        let runtime_dir = runtime.path().to_path_buf();
        let process_tree = Arc::new(BrowserProcessTree::new()?);
        let active = ActiveSession {
            runtime,
            namespace,
            session,
            process_markers,
            process_tree: Arc::clone(&process_tree),
        };
        let mut sessions = active_sessions()
            .lock()
            .map_err(|_| io::Error::other("active browser session registry is unavailable"))?;
        if sessions.contains_key(&runtime_dir) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "browser runtime is already registered to this process",
            ));
        }
        sessions.insert(runtime_dir.clone(), active);
        drop(sessions);
        Ok(Self {
            runtime_dir,
            process_tree,
            armed: true,
        })
    }

    pub(crate) fn terminate(mut self) -> io::Result<bool> {
        let attached = self.has_attached_processes();
        let result = self.process_tree.terminate_and_wait();
        self.unregister();
        self.armed = false;
        result.map(|()| attached)
    }

    pub(crate) fn has_attached_processes(&self) -> bool {
        self.process_tree.has_attached_processes()
    }

    fn unregister(&self) {
        if let Ok(mut sessions) = active_sessions().lock() {
            sessions.remove(&self.runtime_dir);
        }
    }
}

impl Drop for SessionRegistration {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.process_tree.terminate_and_wait();
            self.unregister();
            self.armed = false;
        }
    }
}

pub(crate) fn attach_command_to_session(
    environment: &BTreeMap<OsString, OsString>,
    child: &Child,
) -> io::Result<Option<Arc<BrowserProcessTree>>> {
    let Some(runtime_dir) = browser_runtime_from_environment(environment) else {
        return Ok(None);
    };
    let process_tree = active_sessions()
        .lock()
        .map_err(|_| io::Error::other("active browser session registry is unavailable"))?
        .get(&runtime_dir)
        .map(|session| Arc::clone(&session.process_tree));
    match process_tree {
        Some(process_tree) => {
            process_tree.attach(child)?;
            Ok(Some(process_tree))
        }
        None => Ok(None),
    }
}

fn browser_runtime_from_environment(environment: &BTreeMap<OsString, OsString>) -> Option<PathBuf> {
    RUNTIME_ENVIRONMENTS.iter().find_map(|name| {
        environment
            .get(OsStr::new(name))
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    })
}

/// Immediately terminate command process groups still owned by this process.
///
/// The CLI invokes this on a second interrupt, after the first interrupt has
/// already requested graceful cancellation.
pub fn terminate_active_commands() {
    let process_ids = active_process_groups()
        .lock()
        .map(|groups| groups.iter().copied().collect::<Vec<_>>())
        .unwrap_or_default();
    for process_id in process_ids {
        terminate_process_group(process_id);
    }
    terminate_active_sessions();
}

pub(crate) fn register_process_group(process_id: u32) {
    if process_id > 1 {
        if let Ok(mut groups) = active_process_groups().lock() {
            groups.insert(process_id);
        }
    }
}

pub(crate) fn unregister_process_group(process_id: u32) {
    if let Ok(mut groups) = active_process_groups().lock() {
        groups.remove(&process_id);
    }
}

pub(crate) fn terminate_owned_session(
    runtime: &RuntimeDirectory,
    namespace: &str,
    session: &str,
    process_markers: &[String],
) -> bool {
    if runtime.verify_sync().is_err() {
        return false;
    }
    let mut terminated = false;
    for directory in session_runtime_directories(runtime, namespace) {
        if runtime.verify_sync().is_err() {
            break;
        }
        let process_id = match read_session_pid(&directory, session) {
            SessionPid::Missing => continue,
            SessionPid::Invalid => {
                cleanup_session_sidecars(&directory, session);
                continue;
            }
            SessionPid::Valid(process_id) => process_id,
        };
        if terminate_process_tree(process_id, process_markers) {
            terminated = true;
        }
        cleanup_session_sidecars(&directory, session);
    }
    terminated
}

#[cfg(unix)]
pub(crate) fn create_runtime_directory() -> std::io::Result<tempfile::TempDir> {
    tempfile::Builder::new().prefix("a3st-").tempdir_in("/tmp")
}

#[cfg(not(unix))]
pub(crate) fn create_runtime_directory() -> std::io::Result<tempfile::TempDir> {
    tempfile::Builder::new().prefix("a3st-").tempdir()
}

fn active_process_groups() -> &'static Mutex<HashSet<u32>> {
    static GROUPS: OnceLock<Mutex<HashSet<u32>>> = OnceLock::new();
    GROUPS.get_or_init(|| Mutex::new(HashSet::new()))
}

fn active_sessions() -> &'static Mutex<HashMap<PathBuf, ActiveSession>> {
    static SESSIONS: OnceLock<Mutex<HashMap<PathBuf, ActiveSession>>> = OnceLock::new();
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn terminate_active_sessions() {
    let sessions = active_sessions()
        .lock()
        .map(|sessions| sessions.values().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    for active in sessions {
        let _ = active.process_tree.terminate();
        terminate_owned_session(
            &active.runtime,
            &active.namespace,
            &active.session,
            &active.process_markers,
        );
        if active.runtime.verify_sync().is_ok() {
            let _ = std::fs::remove_dir_all(active.runtime.path());
        }
    }
}

fn cleanup_session_sidecars(directory: &Path, session: &str) {
    for extension in [
        "config",
        "engine",
        "extensions",
        "pid",
        "port",
        "provider",
        "sock",
        "stream",
        "version",
    ] {
        let _ = std::fs::remove_file(directory.join(format!("{session}.{extension}")));
    }
}

#[cfg(unix)]
pub(crate) fn terminate_process_group(process_id: u32) {
    use nix::sys::signal::{killpg, Signal};
    use nix::unistd::Pid;

    if let Ok(process_id) = i32::try_from(process_id) {
        if process_id > 1 {
            let _ = killpg(Pid::from_raw(process_id), Signal::SIGKILL);
        }
    }
}

#[cfg(windows)]
pub(crate) fn terminate_process_group(process_id: u32) {
    if process_id > 1 {
        let Some(taskkill) = windows_system_executable("taskkill.exe") else {
            return;
        };
        let mut command = hidden_windows_command(&taskkill);
        command.args(["/PID", &process_id.to_string(), "/T", "/F"]);
        let _ = bounded_windows_output(command, std::time::Duration::from_secs(5));
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SessionPid {
    Missing,
    Invalid,
    Valid(u32),
}

fn session_runtime_directories(runtime: &RuntimeDirectory, namespace: &str) -> Vec<PathBuf> {
    let mut directories = vec![runtime.path().to_path_buf()];
    let mut current = runtime.path().to_path_buf();
    for component in ["namespaces", namespace, "run"] {
        let candidate = current.join(component);
        let Ok(metadata) = std::fs::symlink_metadata(&candidate) else {
            return directories;
        };
        if is_link_like(&metadata) || !metadata.is_dir() {
            return directories;
        }
        current = candidate;
    }
    directories.push(current);
    directories
}

fn read_session_pid(directory: &Path, session: &str) -> SessionPid {
    let pid_path = directory.join(format!("{session}.pid"));
    let metadata = match std::fs::symlink_metadata(&pid_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return SessionPid::Missing,
        Err(_) => return SessionPid::Invalid,
    };
    if is_link_like(&metadata) || !metadata.is_file() {
        return SessionPid::Invalid;
    }
    std::fs::read_to_string(pid_path)
        .ok()
        .and_then(|source| source.trim().parse::<u32>().ok())
        .map_or(SessionPid::Invalid, SessionPid::Valid)
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn terminate_process_group(_process_id: u32) {}

#[cfg(unix)]
fn terminate_process_tree(process_id: u32, process_markers: &[String]) -> bool {
    use nix::sys::signal::{kill, killpg, Signal};
    use nix::unistd::{getpgrp, Pid};

    #[derive(Clone)]
    struct Process {
        parent: u32,
        group: u32,
        command: String,
    }

    let mut command = std::process::Command::new("ps");
    command.args(["-axo", "pid=,ppid=,pgid=,command="]);
    let Some(output) = bounded_unix_output(command, std::time::Duration::from_secs(5)) else {
        return false;
    };
    let mut processes = HashMap::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let mut fields = line.split_whitespace();
        let (Some(pid), Some(parent), Some(group)) = (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let (Ok(pid), Ok(parent), Ok(group)) = (
            pid.parse::<u32>(),
            parent.parse::<u32>(),
            group.parse::<u32>(),
        ) else {
            continue;
        };
        processes.insert(
            pid,
            Process {
                parent,
                group,
                command: fields.collect::<Vec<_>>().join(" "),
            },
        );
    }

    let Some(root) = processes.get(&process_id) else {
        return false;
    };
    if !process_markers
        .iter()
        .any(|marker| root.command.contains(marker))
    {
        return false;
    }

    let mut owned = HashSet::from([process_id]);
    loop {
        let before = owned.len();
        for (pid, process) in &processes {
            if owned.contains(&process.parent) {
                owned.insert(*pid);
            }
        }
        if owned.len() == before {
            break;
        }
    }

    let current_group = u32::try_from(getpgrp().as_raw()).unwrap_or_default();
    let mut groups = owned
        .iter()
        .filter_map(|pid| processes.get(pid).map(|process| process.group))
        .filter(|group| *group > 1 && *group != current_group)
        .collect::<HashSet<_>>();
    if process_id > 1 && process_id != current_group {
        groups.insert(process_id);
    }
    for group in groups {
        if let Ok(group) = i32::try_from(group) {
            let _ = killpg(Pid::from_raw(group), Signal::SIGKILL);
        }
    }
    for pid in owned {
        if pid == std::process::id() {
            continue;
        }
        if let Ok(pid) = i32::try_from(pid) {
            let _ = kill(Pid::from_raw(pid), Signal::SIGKILL);
        }
    }
    true
}

#[cfg(unix)]
fn bounded_unix_output(
    mut command: std::process::Command,
    timeout: std::time::Duration,
) -> Option<std::process::Output> {
    use std::io::{Read as _, Seek as _};
    use std::os::unix::process::CommandExt as _;

    const MAX_OUTPUT_BYTES: u64 = 8 * 1024 * 1024;

    let mut stdout = tempfile::tempfile().ok()?;
    command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::from(stdout.try_clone().ok()?))
        .stderr(std::process::Stdio::null())
        .process_group(0);
    let mut child = command.spawn().ok()?;
    let process_group = child.id();
    let deadline = std::time::Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Ok(None) | Err(_) => {
                terminate_unix_group(process_group);
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    terminate_unix_group(process_group);
    if stdout.metadata().ok()?.len() > MAX_OUTPUT_BYTES {
        return None;
    }
    stdout.rewind().ok()?;
    let mut bytes = Vec::new();
    stdout.read_to_end(&mut bytes).ok()?;
    status.map(|status| std::process::Output {
        status,
        stdout: bytes,
        stderr: Vec::new(),
    })
}

#[cfg(unix)]
fn terminate_unix_group(process_group: u32) {
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
fn terminate_process_tree(process_id: u32, process_markers: &[String]) -> bool {
    if process_id <= 1 || process_id == std::process::id() {
        return false;
    }
    let Some(command_line) = windows_process_command_line(process_id) else {
        return false;
    };
    if !windows_command_matches_process_markers(&command_line, process_markers) {
        return false;
    }
    let Some(taskkill) = windows_system_executable("taskkill.exe") else {
        return false;
    };
    let mut command = hidden_windows_command(&taskkill);
    command.args(["/PID", &process_id.to_string(), "/T", "/F"]);
    bounded_windows_output(command, std::time::Duration::from_secs(5))
        .is_some_and(|output| output.status.success())
}

#[cfg(windows)]
fn windows_process_command_line(process_id: u32) -> Option<String> {
    let powershell = windows_system_executable(r"WindowsPowerShell\v1.0\powershell.exe")?;
    let script = format!(
        "$process = Get-CimInstance Win32_Process -Filter 'ProcessId = {process_id}' \
         -ErrorAction Stop; if ($null -eq $process -or \
         [string]::IsNullOrEmpty($process.CommandLine)) {{ exit 3 }}; \
         [Console]::OutputEncoding = [System.Text.Encoding]::UTF8; \
         [Console]::Out.Write($process.CommandLine)"
    );
    let mut command = hidden_windows_command(&powershell);
    command
        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command"])
        .arg(script);
    let output = bounded_windows_output(command, std::time::Duration::from_secs(5))?;
    if !output.status.success() {
        return None;
    }
    let command_line = String::from_utf8(output.stdout).ok()?;
    let command_line = command_line.trim_matches(|character: char| {
        character.is_whitespace() || character == '\u{feff}' || character == '\0'
    });
    (!command_line.is_empty()).then(|| command_line.to_string())
}

#[cfg(windows)]
fn windows_command_matches_process_markers(command_line: &str, process_markers: &[String]) -> bool {
    let command_line = command_line.to_lowercase();
    process_markers
        .iter()
        .any(|marker| !marker.is_empty() && command_line.contains(marker.to_lowercase().as_str()))
}

#[cfg(windows)]
fn windows_system_executable(relative: &str) -> Option<PathBuf> {
    let root = PathBuf::from(std::env::var_os("SystemRoot")?);
    if !root.is_absolute() {
        return None;
    }
    let executable = root.join("System32").join(relative);
    executable.is_file().then_some(executable)
}

#[cfg(windows)]
fn hidden_windows_command(program: &Path) -> std::process::Command {
    use std::os::windows::process::CommandExt as _;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut command = std::process::Command::new(program);
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

#[cfg(windows)]
fn bounded_windows_output(
    mut command: std::process::Command,
    timeout: std::time::Duration,
) -> Option<std::process::Output> {
    use std::io::{Read as _, Seek as _};

    const MAX_OUTPUT_BYTES: u64 = 8 * 1024 * 1024;

    let mut stdout = tempfile::tempfile().ok()?;
    command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::from(stdout.try_clone().ok()?))
        .stderr(std::process::Stdio::null());
    let mut child = command.spawn().ok()?;
    let deadline = std::time::Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Ok(None) | Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    };
    if stdout.metadata().ok()?.len() > MAX_OUTPUT_BYTES {
        return None;
    }
    stdout.rewind().ok()?;
    let mut bytes = Vec::new();
    stdout.read_to_end(&mut bytes).ok()?;
    Some(std::process::Output {
        status,
        stdout: bytes,
        stderr: Vec::new(),
    })
}

#[cfg(not(any(unix, windows)))]
fn terminate_process_tree(_process_id: u32, _process_markers: &[String]) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    fn symlink_directory(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    #[cfg(windows)]
    fn symlink_directory(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_dir(target, link)
    }

    #[cfg(unix)]
    fn symlink_file(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    #[cfg(windows)]
    fn symlink_file(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_file(target, link)
    }

    fn unavailable_without_host_privilege(error: &std::io::Error) -> bool {
        cfg!(windows)
            && matches!(
                error.kind(),
                std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::Unsupported
            )
    }

    #[cfg(unix)]
    #[test]
    fn unix_process_snapshot_command_is_killed_and_reaped_on_timeout() {
        use nix::errno::Errno;
        use nix::sys::signal::kill;
        use nix::unistd::Pid;

        let temp = tempfile::tempdir().expect("tempdir");
        let descendant_pid = temp.path().join("descendant.pid");
        let mut command = std::process::Command::new("/bin/sh");
        command
            .args([
                "-c",
                "sleep 30 & echo $! > \"$1\"; wait",
                "snapshot-fixture",
            ])
            .arg(&descendant_pid);
        let started = std::time::Instant::now();

        let output = bounded_unix_output(command, std::time::Duration::from_millis(250));

        assert!(output.is_none());
        assert!(
            started.elapsed() < std::time::Duration::from_secs(2),
            "bounded cleanup helper exceeded its deadline"
        );
        let process_id = std::fs::read_to_string(descendant_pid)
            .expect("snapshot helper descendant PID")
            .trim()
            .parse::<i32>()
            .expect("numeric snapshot helper descendant PID");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            if matches!(kill(Pid::from_raw(process_id), None), Err(Errno::ESRCH)) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("snapshot helper descendant {process_id} survived timeout cleanup");
    }

    #[tokio::test]
    async fn namespaced_runtime_link_is_not_admitted_for_cleanup() {
        let temp = tempfile::tempdir().expect("temp dir");
        let root = temp.path().join("runtime");
        let namespaces = root.join("namespaces");
        let outside = temp.path().join("outside");
        std::fs::create_dir_all(&namespaces).expect("namespaces");
        std::fs::create_dir(&outside).expect("outside");
        let runtime = RuntimeDirectory::bind_existing(&root)
            .await
            .expect("bind runtime");
        if let Err(error) = symlink_directory(&outside, &namespaces.join("contained")) {
            if unavailable_without_host_privilege(&error) {
                return;
            }
            panic!("failed to create namespace link: {error}");
        }

        assert_eq!(
            session_runtime_directories(&runtime, "contained"),
            [runtime.path().to_path_buf()]
        );
    }

    #[tokio::test]
    async fn linked_pid_file_is_not_followed() {
        let temp = tempfile::tempdir().expect("temp dir");
        let root = temp.path().join("runtime");
        let target = temp.path().join("outside.pid");
        std::fs::create_dir(&root).expect("runtime");
        std::fs::write(&target, "4294967295").expect("outside pid");
        let runtime = RuntimeDirectory::bind_existing(&root)
            .await
            .expect("bind runtime");
        if let Err(error) = symlink_file(&target, &root.join("contained.pid")) {
            if unavailable_without_host_privilege(&error) {
                return;
            }
            panic!("failed to create pid link: {error}");
        }

        assert_eq!(
            read_session_pid(runtime.path(), "contained"),
            SessionPid::Invalid
        );
        assert_eq!(
            std::fs::read_to_string(target).expect("outside pid"),
            "4294967295"
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_process_query_reports_the_current_executable() {
        let command_line = windows_process_command_line(std::process::id())
            .expect("query current process command line");
        let executable = std::env::current_exe().expect("current executable");
        let marker = executable
            .file_name()
            .and_then(|name| name.to_str())
            .expect("executable name")
            .to_string();

        assert!(windows_command_matches_process_markers(
            &command_line,
            &[marker]
        ));
        assert!(!windows_command_matches_process_markers(
            &command_line,
            &["definitely-not-this-process".to_string()]
        ));
    }

    #[cfg(windows)]
    #[test]
    fn windows_emergency_cleanup_rejects_an_unmatched_owned_child() {
        let cmd = windows_system_executable("cmd.exe").expect("cmd.exe");
        let mut child = hidden_windows_command(&cmd)
            .args(["/D", "/S", "/C", "ping -n 30 127.0.0.1 >NUL"])
            .spawn()
            .expect("spawn child");

        let terminated = terminate_process_tree(
            child.id(),
            &["definitely-not-the-owned-browser".to_string()],
        );
        let remained_running = child.try_wait().expect("inspect child").is_none();
        let _ = child.kill();
        let _ = child.wait();

        assert!(!terminated);
        assert!(remained_running);
    }

    #[cfg(windows)]
    #[test]
    fn windows_emergency_cleanup_terminates_a_marker_matched_owned_child() {
        let cmd = windows_system_executable("cmd.exe").expect("cmd.exe");
        let mut child = hidden_windows_command(&cmd)
            .args(["/D", "/S", "/C", "ping -n 30 127.0.0.1 >NUL"])
            .spawn()
            .expect("spawn child");

        let terminated = terminate_process_tree(child.id(), &["cmd.exe".to_string()]);
        if !terminated {
            let _ = child.kill();
        }
        let status = child.wait().expect("reap child");

        assert!(terminated);
        assert!(!status.success());
    }
}
