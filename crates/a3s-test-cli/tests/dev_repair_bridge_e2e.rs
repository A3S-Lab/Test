mod support;

use std::fs;
use std::io::{BufRead as _, BufReader, Read as _};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use serde_json::Value;
use support::repair_fixture::{admitted_browser, assert_process_success, binary, start_fixture};

#[test]
#[ignore = "requires Node esbuild and the exact standalone agent-browser 0.26.x runtime"]
fn dev_repair_bridge_delivers_real_browser_findings_without_manual_watch() {
    let Some(browser) = admitted_browser() else {
        eprintln!("A3S_TEST_AGENT_BROWSER is not set; skipping development repair bridge E2E");
        return;
    };
    let (_bundle_workspace, fixture) = start_fixture();
    let workspace = tempfile::tempdir().expect("temporary development bridge workspace");
    write_project(
        workspace.path(),
        &format!("{}#a3s-auto-repair=dev-bridge", fixture.origin()),
        &browser,
    );
    let mut dev = DevHost::spawn(workspace.path());

    let ready = dev.next_event("ready");
    assert_eq!(ready["protocol"], "a3s.test.dev/1");
    assert_eq!(ready["repair_bridge"]["state"], "watching");
    assert_eq!(
        ready["repair_bridge"]["protocol"],
        "a3s.test.local-repair-bridge/1"
    );
    let session = ready["session"].as_str().expect("development session");

    let batch = dev.next_event("repair_batch");
    assert_eq!(batch["protocol"], "a3s.test.local-repair-bridge/1");
    assert_eq!(batch["session"], session);
    assert_eq!(batch["repairs"][0]["finding"]["id"], "finding-dev-real");
    assert_eq!(batch["repairs"][0]["status"], "queued");
    assert_eq!(
        batch["repairs"][0]["finding"]["context"]["nodes"][0]["sourceMapping"]["protocol"],
        "a3s.test.source-mapping/1"
    );
    assert_eq!(
        batch["repairs"][0]["finding"]["context"]["nodes"][0]["sourceMapping"]["candidates"][0]
            ["span"]["file"],
        "src/Fixture.tsx"
    );
    assert!(batch["repairs"][0]["before_evidence"].is_object());
    assert_eq!(
        batch["batches"][0]["id"],
        batch["repairs"][0]["finding"]["batchId"]
    );
    let ledger = PathBuf::from(batch["ledger_path"].as_str().expect("repair ledger path"));
    assert!(
        ledger.is_file(),
        "repair ledger missing: {}",
        ledger.display()
    );
    let screenshot = PathBuf::from(
        batch["repairs"][0]["before_evidence"]["screenshot"]["path"]
            .as_str()
            .expect("before screenshot path"),
    );
    assert!(
        screenshot.is_file(),
        "repair evidence missing: {}",
        screenshot.display()
    );
    dev.expect_no_event(Duration::from_millis(1_500));

    dev.stop_and_abort(session);
}

struct DevHost {
    child: Child,
    events: mpsc::Receiver<String>,
    stdout_thread: Option<thread::JoinHandle<()>>,
    stderr_thread: Option<thread::JoinHandle<String>>,
    workspace: PathBuf,
    cleaned: bool,
}

impl DevHost {
    fn spawn(workspace: &Path) -> Self {
        let mut child = Command::new(binary())
            .args([
                "dev",
                "--root",
                workspace.to_str().expect("UTF-8 root"),
                "--json",
            ])
            .current_dir(workspace)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start development bridge host");
        let stdout = child.stdout.take().expect("development host stdout");
        let stderr = child.stderr.take().expect("development host stderr");
        let (sender, events) = mpsc::channel();
        let stdout_thread = thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                if sender.send(line.expect("development event line")).is_err() {
                    break;
                }
            }
        });
        let stderr_thread = thread::spawn(move || {
            let mut output = String::new();
            BufReader::new(stderr)
                .read_to_string(&mut output)
                .expect("development stderr");
            output
        });
        Self {
            child,
            events,
            stdout_thread: Some(stdout_thread),
            stderr_thread: Some(stderr_thread),
            workspace: workspace.to_path_buf(),
            cleaned: false,
        }
    }

    fn next_event(&self, expected: &str) -> Value {
        let line = self
            .events
            .recv_timeout(Duration::from_secs(30))
            .unwrap_or_else(|error| panic!("timed out waiting for {expected}: {error}"));
        let event: Value = serde_json::from_str(&line)
            .unwrap_or_else(|error| panic!("invalid development event {line:?}: {error}"));
        assert_eq!(event["event"], expected, "{event:#}");
        event
    }

    fn expect_no_event(&self, duration: Duration) {
        match self.events.recv_timeout(duration) {
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(error) => panic!("development event stream ended unexpectedly: {error}"),
            Ok(line) => panic!("unexpected duplicate development event: {line}"),
        }
    }

    fn stop_and_abort(&mut self, session: &str) {
        self.child.kill().expect("stop development bridge host");
        self.child.wait().expect("wait for development bridge host");
        let abort = Command::new(binary())
            .args(["agent", "abort", "--session", session, "--json"])
            .current_dir(&self.workspace)
            .output()
            .expect("abort development browser session");
        assert_process_success("abort development browser session", &abort);
        self.join_readers();
        self.cleaned = true;
    }

    fn join_readers(&mut self) {
        if let Some(reader) = self.stdout_thread.take() {
            reader.join().expect("join development stdout reader");
        }
        if let Some(reader) = self.stderr_thread.take() {
            let stderr = reader.join().expect("join development stderr reader");
            assert!(
                stderr.is_empty(),
                "development stderr was not empty:\n{stderr}"
            );
        }
    }
}

impl Drop for DevHost {
    fn drop(&mut self) {
        if self.cleaned {
            return;
        }
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        let _ = Command::new(binary())
            .args(["agent", "abort", "--session", "dev-real", "--json"])
            .current_dir(&self.workspace)
            .output();
        if let Some(reader) = self.stdout_thread.take() {
            let _ = reader.join();
        }
        if let Some(reader) = self.stderr_thread.take() {
            let _ = reader.join();
        }
    }
}

fn write_project(root: &Path, url: &str, browser: &Path) {
    fs::write(
        root.join("package.json"),
        r#"{
  "name": "dev-repair-bridge-e2e",
  "scripts": { "dev": "fixture" },
  "devDependencies": { "@a3s-lab/testkit": "0.5.0" }
}
"#,
    )
    .expect("development package metadata");
    let testkit = root.join("node_modules/@a3s-lab/testkit");
    fs::create_dir_all(&testkit).expect("Test Kit package directory");
    fs::write(
        testkit.join("package.json"),
        "{\"name\":\"@a3s-lab/testkit\",\"version\":\"0.5.0\"}\n",
    )
    .expect("Test Kit package metadata");
    let config = root.join(".a3s-test");
    fs::create_dir(&config).expect("project profile directory");
    let executable = acl_escape(&browser.display().to_string());
    fs::write(
        config.join("project.acl"),
        format!(
            r#"project "dev-repair-bridge" {{
  version = 1
  root = ".."

  dev_server {{
    executable = "{executable}"
    args = ["fixture"]
    working_directory = "."
    url = "{}"
    startup_timeout_ms = 5000
    cleanup_timeout_ms = 5000
  }}

  browser {{
    driver = "standalone"
    executable = "{executable}"
    session = "dev-real"
    headed = false
    command_timeout_ms = 60000
    idle_timeout_ms = 60000
  }}

  testkit {{
    required = true
  }}
}}
"#,
            acl_escape(url),
        ),
    )
    .expect("project profile");
}

fn acl_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
