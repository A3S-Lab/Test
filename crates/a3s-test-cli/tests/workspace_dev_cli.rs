#![cfg(unix)]

use std::fs;
use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_a3s-test"))
}

fn process_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[test]
fn dev_uses_an_existing_server_without_starting_or_stopping_it() {
    let _guard = process_test_lock().lock().unwrap();
    let temp = tempfile::tempdir().expect("tempdir");
    let server = HttpFixture::start();
    let dev_server = write_never_start_server(temp.path());
    let browser = write_browser_fixture(temp.path());
    write_project(temp.path(), &server.url(), &dev_server, &browser, 250, true);

    let variables = [(
        "A3S_TEST_DEV_INVOKED",
        temp.path().join("dev-server-invoked").display().to_string(),
    )];
    let mut dev = spawn_dev(temp.path(), &variables);
    let ready = dev.next_event("ready");
    assert_eq!(ready["protocol"], "a3s.test.dev/1");
    assert_eq!(ready["event"], "ready");
    assert_eq!(ready["server"], "existing");
    assert_eq!(ready["session"], "dev");
    assert_eq!(ready["testkit"]["protocol"], "a3s.test.testkit-handshake/1");
    assert_eq!(ready["testkit"]["sdk_version"], "0.4.2");
    assert_eq!(ready["testkit"]["review_overlay_mounted"], true);

    send_sigint(dev.child.id());
    let stopped = dev.next_event("stopped");
    assert_eq!(stopped["reason"], "interrupt");
    let status = dev.wait();

    assert_eq!(status.code(), Some(130), "{}", dev.stderr());
    assert!(TcpStream::connect(server.address()).is_ok());
    assert!(!temp.path().join("dev-server-invoked").exists());
    assert_browser_opened_and_closed(temp.path());
    assert_session_aborted(temp.path(), "dev");
}

#[test]
fn dev_starts_waits_for_and_cleans_its_owned_server_tree() {
    let _guard = process_test_lock().lock().unwrap();
    let temp = tempfile::tempdir().expect("tempdir");
    let port = available_port();
    let url = format!("http://127.0.0.1:{port}/");
    let dev_server = write_owned_server(temp.path(), true);
    let browser = write_browser_fixture(temp.path());
    write_project(temp.path(), &url, &dev_server, &browser, 150, true);

    let variables = [
        ("A3S_TEST_DEV_FIXTURE", "serve".to_string()),
        ("A3S_TEST_DEV_PORT", port.to_string()),
        (
            "A3S_TEST_FIXTURE_BIN",
            std::env::current_exe()
                .expect("current test binary")
                .display()
                .to_string(),
        ),
        (
            "A3S_TEST_DEV_PID",
            temp.path().join("dev-server.pid").display().to_string(),
        ),
        (
            "A3S_TEST_LATE_PID",
            temp.path()
                .join("late-descendant.pid")
                .display()
                .to_string(),
        ),
    ];
    let mut dev = spawn_dev(temp.path(), &variables);
    let ready = dev.next_event("ready");
    assert_eq!(ready["server"], "started");
    let owned_pid = wait_for_pid(&temp.path().join("dev-server.pid"));

    send_sigint(dev.child.id());
    let stopped = dev.next_event("stopped");
    assert_eq!(stopped["reason"], "interrupt");
    let status = dev.wait();
    let late_pid = wait_for_pid(&temp.path().join("late-descendant.pid"));

    assert_eq!(status.code(), Some(130), "{}", dev.stderr());
    assert_process_stopped(&owned_pid);
    assert_process_stopped(&late_pid);
    assert_browser_opened_and_closed(temp.path());
    assert_session_aborted(temp.path(), "dev");
}

#[test]
fn dev_aborts_the_browser_when_the_owned_server_exits() {
    let _guard = process_test_lock().lock().unwrap();
    let temp = tempfile::tempdir().expect("tempdir");
    let port = available_port();
    let url = format!("http://127.0.0.1:{port}/");
    let dev_server = write_owned_server(temp.path(), false);
    let browser = write_browser_fixture(temp.path());
    write_project(temp.path(), &url, &dev_server, &browser, 250, true);
    let trigger = temp.path().join("stop-server");

    let variables = [
        ("A3S_TEST_DEV_FIXTURE", "exit_on_trigger".to_string()),
        ("A3S_TEST_DEV_PORT", port.to_string()),
        (
            "A3S_TEST_FIXTURE_BIN",
            std::env::current_exe()
                .expect("current test binary")
                .display()
                .to_string(),
        ),
        (
            "A3S_TEST_DEV_PID",
            temp.path().join("dev-server.pid").display().to_string(),
        ),
        (
            "A3S_TEST_BROWSER_OPEN_TRIGGER",
            trigger.display().to_string(),
        ),
    ];
    let mut dev = spawn_dev(temp.path(), &variables);
    let ready = dev.next_event("ready");
    assert_eq!(ready["server"], "started");
    let stopped = dev.next_event("stopped");
    assert_eq!(stopped["reason"], "server_exit");
    assert_eq!(stopped["server_exit_code"], 23);
    let status = dev.wait();

    assert_eq!(status.code(), Some(1), "{}", dev.stderr());
    assert_browser_opened_and_closed(temp.path());
    assert_session_aborted(temp.path(), "dev");
}

#[test]
fn dev_watchdog_reaps_the_owned_server_after_host_sigkill() {
    let _guard = process_test_lock().lock().unwrap();
    let temp = tempfile::tempdir().expect("tempdir");
    let port = available_port();
    let url = format!("http://127.0.0.1:{port}/");
    let dev_server = write_owned_server(temp.path(), false);
    let browser = write_browser_fixture(temp.path());
    write_project(temp.path(), &url, &dev_server, &browser, 250, true);

    let variables = [
        ("A3S_TEST_DEV_FIXTURE", "serve".to_string()),
        ("A3S_TEST_DEV_PORT", port.to_string()),
        (
            "A3S_TEST_FIXTURE_BIN",
            std::env::current_exe()
                .expect("current test binary")
                .display()
                .to_string(),
        ),
        (
            "A3S_TEST_DEV_PID",
            temp.path().join("dev-server.pid").display().to_string(),
        ),
    ];
    let mut dev = spawn_dev(temp.path(), &variables);
    let ready = dev.next_event("ready");
    assert_eq!(ready["server"], "started");
    let owned_pid = wait_for_pid(&temp.path().join("dev-server.pid"));

    let killed = Command::new("kill")
        .args(["-KILL", &dev.child.id().to_string()])
        .status()
        .expect("SIGKILL dev host");
    assert!(killed.success());
    let status = dev.wait();

    assert!(!status.success());
    assert_process_stopped(&owned_pid);

    let abort = Command::new(binary())
        .args(["agent", "abort", "--session", "dev", "--json"])
        .current_dir(temp.path())
        .env("A3S_TEST_BROWSER_LOG", temp.path().join("browser.log"))
        .output()
        .expect("recover interrupted browser session");
    assert!(abort.status.success(), "{abort:?}");
}

#[test]
fn dev_server_fixture() {
    let mode = match std::env::var("A3S_TEST_DEV_FIXTURE") {
        Ok(mode) => mode,
        Err(_) => return,
    };
    let port = std::env::var("A3S_TEST_DEV_PORT")
        .expect("fixture port")
        .parse::<u16>()
        .expect("numeric fixture port");
    let listener = TcpListener::bind(("127.0.0.1", port)).expect("bind fixture server");
    listener.set_nonblocking(true).expect("nonblocking server");
    loop {
        if mode == "exit_on_trigger"
            && std::env::var_os("A3S_TEST_BROWSER_OPEN_TRIGGER")
                .is_some_and(|path| Path::new(&path).exists())
        {
            std::process::exit(23);
        }
        match listener.accept() {
            Ok((mut stream, _)) => respond(&mut stream),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("fixture server failed: {error}"),
        }
    }
}

#[test]
fn required_testkit_missing_aborts_the_browser_without_stopping_an_existing_server() {
    let _guard = process_test_lock().lock().unwrap();
    let temp = tempfile::tempdir().expect("tempdir");
    let server = HttpFixture::start();
    let dev_server = write_never_start_server(temp.path());
    let browser = write_browser_fixture(temp.path());
    write_project(temp.path(), &server.url(), &dev_server, &browser, 250, true);

    let variables = [("A3S_TEST_TESTKIT_MODE", "absent".to_string())];
    let mut dev = spawn_dev(temp.path(), &variables);
    let status = dev.wait();

    assert_eq!(status.code(), Some(2), "{}", dev.stderr());
    assert!(TcpStream::connect(server.address()).is_ok());
    assert_browser_opened_and_closed(temp.path());
    assert_session_aborted(temp.path(), "dev");
    assert!(
        dev.stderr()
            .contains("test.driver.web.testkit_bridge_missing"),
        "{}",
        dev.stderr()
    );
    assert!(
        dev.stderr()
            .contains("npm install --save-dev @a3s-lab/testkit@^0.4.0"),
        "{}",
        dev.stderr()
    );
}

#[test]
fn optional_testkit_allows_an_absent_bridge_and_reports_null() {
    let _guard = process_test_lock().lock().unwrap();
    let temp = tempfile::tempdir().expect("tempdir");
    let server = HttpFixture::start();
    let dev_server = write_never_start_server(temp.path());
    let browser = write_browser_fixture(temp.path());
    write_project(
        temp.path(),
        &server.url(),
        &dev_server,
        &browser,
        250,
        false,
    );

    let variables = [("A3S_TEST_TESTKIT_MODE", "absent".to_string())];
    let mut dev = spawn_dev(temp.path(), &variables);
    let ready = dev.next_event("ready");
    assert_eq!(ready["testkit"], serde_json::Value::Null);

    send_sigint(dev.child.id());
    let stopped = dev.next_event("stopped");
    assert_eq!(stopped["cleanup"], "complete");
    let status = dev.wait();

    assert_eq!(status.code(), Some(130), "{}", dev.stderr());
    assert!(TcpStream::connect(server.address()).is_ok());
    assert_browser_opened_and_closed(temp.path());
    assert_session_aborted(temp.path(), "dev");
}

#[test]
fn incompatible_live_testkit_boundaries_fail_with_exact_repairs() {
    let _guard = process_test_lock().lock().unwrap();
    let cases = [
        (
            "incompatible_version",
            "test.driver.web.testkit_sdk_version_unsupported",
            "npm install --save-dev @a3s-lab/testkit@^0.4.0",
        ),
        (
            "missing_capability",
            "test.driver.web.testkit_capability_missing",
            "npm install --save-dev @a3s-lab/testkit@^0.4.0",
        ),
        (
            "overlay_missing",
            "test.driver.web.testkit_review_overlay_missing",
            "render <A3SReviewOverlay />",
        ),
    ];

    for (mode, code, repair) in cases {
        let temp = tempfile::tempdir().expect("tempdir");
        let server = HttpFixture::start();
        let dev_server = write_never_start_server(temp.path());
        let browser = write_browser_fixture(temp.path());
        write_project(temp.path(), &server.url(), &dev_server, &browser, 250, true);
        let variables = [("A3S_TEST_TESTKIT_MODE", mode.to_string())];
        let mut dev = spawn_dev(temp.path(), &variables);

        let status = dev.wait();

        assert_eq!(status.code(), Some(2), "{mode}: {}", dev.stderr());
        assert!(TcpStream::connect(server.address()).is_ok());
        assert_browser_opened_and_closed(temp.path());
        assert_session_aborted(temp.path(), "dev");
        assert!(dev.stderr().contains(code), "{mode}: {}", dev.stderr());
        assert!(dev.stderr().contains(repair), "{mode}: {}", dev.stderr());
    }
}

#[test]
fn handshake_failure_cleans_an_owned_server_tree() {
    let _guard = process_test_lock().lock().unwrap();
    let temp = tempfile::tempdir().expect("tempdir");
    let port = available_port();
    let url = format!("http://127.0.0.1:{port}/");
    let dev_server = write_owned_server(temp.path(), false);
    let browser = write_browser_fixture(temp.path());
    write_project(temp.path(), &url, &dev_server, &browser, 250, true);

    let variables = [
        ("A3S_TEST_DEV_FIXTURE", "serve".to_string()),
        ("A3S_TEST_DEV_PORT", port.to_string()),
        (
            "A3S_TEST_FIXTURE_BIN",
            std::env::current_exe()
                .expect("current test binary")
                .display()
                .to_string(),
        ),
        (
            "A3S_TEST_DEV_PID",
            temp.path().join("dev-server.pid").display().to_string(),
        ),
        ("A3S_TEST_TESTKIT_MODE", "absent".to_string()),
    ];
    let mut dev = spawn_dev(temp.path(), &variables);
    let status = dev.wait();
    let owned_pid = wait_for_pid(&temp.path().join("dev-server.pid"));

    assert_eq!(status.code(), Some(2), "{}", dev.stderr());
    assert_process_stopped(&owned_pid);
    assert_browser_opened_and_closed(temp.path());
    assert_session_aborted(temp.path(), "dev");
}

struct DevProcess {
    child: Child,
    events: mpsc::Receiver<String>,
    stdout_thread: Option<thread::JoinHandle<()>>,
    stderr_thread: Option<thread::JoinHandle<String>>,
    captured_stderr: Option<String>,
}

impl DevProcess {
    fn next_event(&mut self, expected: &str) -> serde_json::Value {
        let line = match self.events.recv_timeout(Duration::from_secs(10)) {
            Ok(line) => line,
            Err(error) => {
                let status = self.wait();
                panic!(
                    "timed out waiting for {expected} event ({error}); status {status}; stderr: {}",
                    self.stderr()
                );
            }
        };
        let event: serde_json::Value = serde_json::from_str(&line)
            .unwrap_or_else(|error| panic!("invalid dev event {line:?}: {error}"));
        assert_eq!(event["event"], expected, "{event}");
        event
    }

    fn wait(&mut self) -> ExitStatus {
        let deadline = Instant::now() + Duration::from_secs(10);
        let status = loop {
            match self.child.try_wait().expect("poll dev host") {
                Some(status) => break status,
                None if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
                None => {
                    let _ = self.child.kill();
                    panic!("dev host did not exit; stderr: {}", self.stderr());
                }
            }
        };
        if let Some(reader) = self.stdout_thread.take() {
            reader.join().expect("join dev stdout reader");
        }
        if let Some(reader) = self.stderr_thread.take() {
            self.captured_stderr = Some(reader.join().expect("join dev stderr reader"));
        }
        status
    }

    fn stderr(&self) -> &str {
        self.captured_stderr.as_deref().unwrap_or("")
    }
}

impl Drop for DevProcess {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn spawn_dev(root: &Path, variables: &[(&'static str, String)]) -> DevProcess {
    let mut command = Command::new(binary());
    command
        .args([
            "dev",
            "--root",
            root.to_str().expect("UTF-8 root"),
            "--json",
        ])
        .current_dir(root)
        .env("A3S_TEST_BROWSER_LOG", root.join("browser.log"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (name, value) in variables {
        command.env(name, value);
    }
    let mut child = command.spawn().expect("spawn a3s-test dev");
    let stdout = child.stdout.take().expect("dev stdout");
    let stderr = child.stderr.take().expect("dev stderr");
    let (sender, events) = mpsc::channel();
    let stdout_thread = thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            sender.send(line.expect("read dev event")).ok();
        }
    });
    let stderr_thread = thread::spawn(move || {
        let mut output = String::new();
        BufReader::new(stderr)
            .read_to_string(&mut output)
            .expect("read dev stderr");
        output
    });
    DevProcess {
        child,
        events,
        stdout_thread: Some(stdout_thread),
        stderr_thread: Some(stderr_thread),
        captured_stderr: None,
    }
}

struct HttpFixture {
    address: std::net::SocketAddr,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}

impl HttpFixture {
    fn start() -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind external server");
        let address = listener.local_addr().expect("external server address");
        listener.set_nonblocking(true).expect("nonblocking server");
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker = thread::spawn(move || {
            while !worker_stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut stream, _)) => respond(&mut stream),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("external server failed: {error}"),
                }
            }
        });
        Self {
            address,
            stop,
            worker: Some(worker),
        }
    }

    fn address(&self) -> std::net::SocketAddr {
        self.address
    }

    fn url(&self) -> String {
        format!("http://{}/", self.address)
    }
}

impl Drop for HttpFixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker.join().expect("join external server");
        }
    }
}

fn respond(stream: &mut TcpStream) {
    let mut request = [0_u8; 1024];
    let _ = stream.read(&mut request);
    stream
        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
        .expect("write HTTP response");
}

fn write_project(
    root: &Path,
    url: &str,
    dev_server: &Path,
    browser: &Path,
    cleanup_ms: u64,
    testkit_required: bool,
) {
    fs::write(
        root.join("package.json"),
        r#"{
  "name": "dev-fixture",
  "scripts": { "dev": "fixture" },
  "devDependencies": { "@a3s-lab/testkit": "0.4.0" }
}
"#,
    )
    .expect("package.json");
    let testkit = root.join("node_modules/@a3s-lab/testkit");
    fs::create_dir_all(&testkit).expect("Test Kit package directory");
    fs::write(
        testkit.join("package.json"),
        "{\"name\":\"@a3s-lab/testkit\",\"version\":\"0.4.0\"}\n",
    )
    .expect("Test Kit package metadata");
    let config_dir = root.join(".a3s-test");
    fs::create_dir(&config_dir).expect("profile directory");
    fs::write(
        config_dir.join("project.acl"),
        format!(
            r#"project "dev-fixture" {{
  version = 1
  root = ".."

  dev_server {{
    executable = "{}"
    args = ["fixture"]
    working_directory = "."
    url = "{}"
    startup_timeout_ms = 5000
    cleanup_timeout_ms = {}
  }}

  browser {{
    driver = "standalone"
    executable = "{}"
    session = "dev"
    headed = true
    command_timeout_ms = 2000
    idle_timeout_ms = 60000
  }}

  testkit {{
    required = {}
  }}
}}
"#,
            acl_escape(&dev_server.display().to_string()),
            acl_escape(url),
            cleanup_ms,
            acl_escape(&browser.display().to_string()),
            testkit_required,
        ),
    )
    .expect("project profile");
}

fn write_browser_fixture(root: &Path) -> PathBuf {
    let path = root.join("fake-agent-browser");
    fs::write(
        &path,
        r#"#!/bin/sh
case " $* " in
  *" --version "*)
    printf 'agent-browser 0.26.0\n'
    exit 0
    ;;
esac
printf '%s\n' "$*" >> "$A3S_TEST_BROWSER_LOG"
case " $* " in
  *" open "*)
    if [ -n "${A3S_TEST_BROWSER_OPEN_TRIGGER-}" ]; then
      : > "$A3S_TEST_BROWSER_OPEN_TRIGGER"
    fi
    ;;
  *" eval "*)
    case "${A3S_TEST_TESTKIT_MODE-compatible}" in
      absent)
        printf '%s\n' '{"success":true,"data":{"result":{"state":"absent"}}}'
        ;;
      incompatible_version)
        printf '%s\n' '{"success":true,"data":{"result":{"state":"present","handshake":{"protocol":"a3s.test.testkit-handshake/1","packageName":"@a3s-lab/testkit","sdkVersion":"0.5.0","pageContextProtocol":"a3s.test.page-context/1","capabilities":["bounded_snapshot","component_boundaries","design_references","geometry","repair_queue","revision_wait","scoped_inspection"]},"reviewOverlayMounted":true}}}'
        ;;
      missing_capability)
        printf '%s\n' '{"success":true,"data":{"result":{"state":"present","handshake":{"protocol":"a3s.test.testkit-handshake/1","packageName":"@a3s-lab/testkit","sdkVersion":"0.4.2","pageContextProtocol":"a3s.test.page-context/1","capabilities":["bounded_snapshot","component_boundaries","design_references","geometry","repair_queue","revision_wait"]},"reviewOverlayMounted":true}}}'
        ;;
      overlay_missing)
        printf '%s\n' '{"success":true,"data":{"result":{"state":"present","handshake":{"protocol":"a3s.test.testkit-handshake/1","packageName":"@a3s-lab/testkit","sdkVersion":"0.4.2","pageContextProtocol":"a3s.test.page-context/1","capabilities":["bounded_snapshot","component_boundaries","design_references","geometry","repair_queue","revision_wait","scoped_inspection"]},"reviewOverlayMounted":false}}}'
        ;;
      *)
        printf '%s\n' '{"success":true,"data":{"result":{"state":"present","handshake":{"protocol":"a3s.test.testkit-handshake/1","packageName":"@a3s-lab/testkit","sdkVersion":"0.4.2","pageContextProtocol":"a3s.test.page-context/1","capabilities":["bounded_snapshot","component_boundaries","design_references","geometry","repair_queue","revision_wait","scoped_inspection"]},"reviewOverlayMounted":true}}}'
        ;;
    esac
    exit 0
    ;;
esac
printf '{"success":true}\n'
"#,
    )
    .expect("browser fixture");
    make_executable(&path);
    path
}

fn write_never_start_server(root: &Path) -> PathBuf {
    let path = root.join("never-start-server");
    fs::write(
        &path,
        r#"#!/bin/sh
: > "$A3S_TEST_DEV_INVOKED"
exit 97
"#,
    )
    .expect("never-start server fixture");
    make_executable(&path);
    path
}

fn write_owned_server(root: &Path, late_descendant: bool) -> PathBuf {
    let path = root.join("owned-dev-server");
    let trap = if late_descendant {
        r#"trap 'sleep 30 & echo "$!" > "$A3S_TEST_LATE_PID"; wait' TERM
"#
    } else {
        ""
    };
    fs::write(
        &path,
        format!(
            r#"#!/bin/sh
printf '%s\n' "$$" > "$A3S_TEST_DEV_PID"
{trap}"$A3S_TEST_FIXTURE_BIN" --exact dev_server_fixture --nocapture &
server=$!
wait "$server"
"#
        ),
    )
    .expect("owned server fixture");
    make_executable(&path);
    path
}

fn make_executable(path: &Path) {
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("fixture permissions");
}

fn acl_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn available_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .expect("reserve port")
        .local_addr()
        .expect("reserved address")
        .port()
}

fn send_sigint(process_id: u32) {
    let status = Command::new("kill")
        .args(["-INT", &process_id.to_string()])
        .status()
        .expect("SIGINT dev host");
    assert!(status.success());
}

fn wait_for_pid(path: &Path) -> String {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(pid) = fs::read_to_string(path) {
            return pid.trim().to_string();
        }
        assert!(
            Instant::now() < deadline,
            "PID file {} was not written",
            path.display()
        );
        thread::sleep(Duration::from_millis(10));
    }
}

fn assert_process_stopped(process_id: &str) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if !Command::new("kill")
            .args(["-0", process_id])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
        {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    let _ = Command::new("kill").args(["-KILL", process_id]).status();
    panic!("owned process {process_id} survived cleanup");
}

fn assert_browser_opened_and_closed(root: &Path) {
    let log = fs::read_to_string(root.join("browser.log")).expect("browser log");
    assert!(log.lines().any(|line| line.contains(" open ")), "{log}");
    assert!(log.lines().any(|line| line.contains(" close")), "{log}");
}

fn assert_session_aborted(root: &Path, session: &str) {
    let state: serde_json::Value = serde_json::from_slice(
        &fs::read(
            root.join(".a3s-test/agent-sessions")
                .join(session)
                .join("session.json"),
        )
        .expect("agent session state"),
    )
    .expect("agent session JSON");
    assert_eq!(state["status"], "aborted", "{state}");
}
