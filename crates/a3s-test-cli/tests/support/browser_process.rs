use std::collections::BTreeSet;
use std::fs::File;
use std::io::{Read as _, Seek as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const MAX_OUTPUT_BYTES: u64 = 8 * 1024 * 1024;

pub fn bounded_output(command: &mut Command, context: &str) -> Output {
    let mut stdout = tempfile::tempfile()
        .unwrap_or_else(|error| panic!("{context}: prepare stdout capture: {error}"));
    let mut stderr = tempfile::tempfile()
        .unwrap_or_else(|error| panic!("{context}: prepare stderr capture: {error}"));
    command
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout.try_clone().unwrap_or_else(|error| {
            panic!("{context}: clone stdout capture: {error}")
        })))
        .stderr(Stdio::from(stderr.try_clone().unwrap_or_else(|error| {
            panic!("{context}: clone stderr capture: {error}")
        })));
    let status = command
        .status()
        .unwrap_or_else(|error| panic!("{context}: execute command: {error}"));
    Output {
        status,
        stdout: read_output(&mut stdout, context, "stdout"),
        stderr: read_output(&mut stderr, context, "stderr"),
    }
}

pub fn assert_process_success(context: &str, output: &Output) {
    assert!(
        output.status.success(),
        "{context} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

pub struct StandaloneBrowserSessionCleanup {
    browser: PathBuf,
    session: String,
    armed: bool,
}

impl StandaloneBrowserSessionCleanup {
    pub fn new(browser: &Path, session: &str) -> Self {
        Self {
            browser: browser.to_path_buf(),
            session: session.to_string(),
            armed: false,
        }
    }

    pub fn arm(&mut self) {
        self.armed = true;
    }

    pub fn close(&mut self) -> Output {
        let output = close_standalone_browser_session(&self.browser, &self.session);
        if output.status.success() {
            self.armed = false;
        }
        output
    }
}

impl Drop for StandaloneBrowserSessionCleanup {
    fn drop(&mut self) {
        if self.armed {
            let _ = close_standalone_browser_session(&self.browser, &self.session);
        }
    }
}

fn close_standalone_browser_session(browser: &Path, session: &str) -> Output {
    let mut command = Command::new(browser);
    command.args(["--session", session, "close"]);
    bounded_output(&mut command, "close standalone browser session")
}

pub fn private_runtime_directories() -> BTreeSet<PathBuf> {
    #[cfg(unix)]
    let root = Path::new("/tmp");
    #[cfg(not(unix))]
    let root = std::env::temp_dir();

    std::fs::read_dir(root)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("a3st-"))
        })
        .collect()
}

pub fn assert_no_new_private_runtime_directories(before: &BTreeSet<PathBuf>) {
    for _ in 0..20 {
        let current = private_runtime_directories();
        let leaked = current.difference(before).cloned().collect::<Vec<_>>();
        if leaked.is_empty() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let current = private_runtime_directories();
    let leaked = current.difference(before).collect::<Vec<_>>();
    panic!("browser runtime directories leaked after E2E cleanup: {leaked:?}");
}

fn read_output(file: &mut File, context: &str, stream: &str) -> Vec<u8> {
    file.rewind()
        .unwrap_or_else(|error| panic!("{context}: rewind {stream}: {error}"));
    let mut output = Vec::new();
    file.take(MAX_OUTPUT_BYTES + 1)
        .read_to_end(&mut output)
        .unwrap_or_else(|error| panic!("{context}: read {stream}: {error}"));
    assert!(
        output.len() as u64 <= MAX_OUTPUT_BYTES,
        "{context}: {stream} exceeded {MAX_OUTPUT_BYTES} bytes"
    );
    output
}
