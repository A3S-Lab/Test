use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

#[derive(Clone)]
struct ActiveSession {
    runtime_dir: PathBuf,
    namespace: String,
    session: String,
    process_markers: Vec<String>,
}

pub(crate) struct SessionRegistration {
    runtime_dir: PathBuf,
}

impl SessionRegistration {
    pub(crate) fn new(
        runtime_dir: PathBuf,
        namespace: String,
        session: String,
        process_markers: Vec<String>,
    ) -> Self {
        let active = ActiveSession {
            runtime_dir: runtime_dir.clone(),
            namespace,
            session,
            process_markers,
        };
        if let Ok(mut sessions) = active_sessions().lock() {
            sessions.insert(runtime_dir.clone(), active);
        }
        Self { runtime_dir }
    }
}

impl Drop for SessionRegistration {
    fn drop(&mut self) {
        if let Ok(mut sessions) = active_sessions().lock() {
            sessions.remove(&self.runtime_dir);
        }
    }
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
    runtime_dir: &Path,
    namespace: &str,
    session: &str,
    process_markers: &[String],
) -> bool {
    let runtime_dirs = [
        runtime_dir.to_path_buf(),
        runtime_dir.join("namespaces").join(namespace).join("run"),
    ];
    let mut terminated = false;
    for directory in runtime_dirs {
        let pid_path = directory.join(format!("{session}.pid"));
        let Ok(pid_source) = std::fs::read_to_string(&pid_path) else {
            continue;
        };
        let Ok(process_id) = pid_source.trim().parse::<u32>() else {
            cleanup_session_sidecars(&directory, session);
            continue;
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
        terminate_owned_session(
            &active.runtime_dir,
            &active.namespace,
            &active.session,
            &active.process_markers,
        );
        let _ = std::fs::remove_dir_all(&active.runtime_dir);
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
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &process_id.to_string(), "/T", "/F"])
            .status();
    }
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

    let Ok(output) = std::process::Command::new("ps")
        .args(["-axo", "pid=,ppid=,pgid=,command="])
        .output()
    else {
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

#[cfg(windows)]
fn terminate_process_tree(process_id: u32, _process_markers: &[String]) -> bool {
    if process_id <= 1 {
        return false;
    }
    std::process::Command::new("taskkill")
        .args(["/PID", &process_id.to_string(), "/T", "/F"])
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(not(any(unix, windows)))]
fn terminate_process_tree(_process_id: u32, _process_markers: &[String]) -> bool {
    false
}
