use std::io::{Read as _, Write as _};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use a3s_test_core::Action;
use a3s_test_driver_web::{
    AgentBrowserConfig, AgentBrowserConnectionConfig, AgentBrowserDriver, BrowserCommand,
};

const REQUIRED_CAPABILITIES: [&str; 7] = [
    "bounded_snapshot",
    "component_boundaries",
    "design_references",
    "geometry",
    "repair_queue",
    "revision_wait",
    "scoped_inspection",
];

#[tokio::test]
#[ignore = "requires the exact standalone agent-browser 0.26.x runtime"]
async fn real_standalone_browser_waits_for_a_delayed_live_testkit_handshake() {
    let Some(browser) = std::env::var_os("A3S_TEST_AGENT_BROWSER").map(PathBuf::from) else {
        eprintln!("A3S_TEST_AGENT_BROWSER is not set; skipping live Test Kit handshake E2E");
        return;
    };
    assert!(
        browser.is_file(),
        "browser executable does not exist: {browser:?}"
    );
    let version = Command::new(&browser)
        .arg("--version")
        .output()
        .expect("probe standalone browser version");
    assert!(version.status.success(), "browser version probe failed");
    assert!(
        String::from_utf8_lossy(&version.stdout).contains("0.26."),
        "live handshake E2E requires the admitted 0.26.x protocol: {}",
        String::from_utf8_lossy(&version.stdout)
    );

    let fixture = DelayedTestKitFixture::start();
    let temp = tempfile::tempdir().expect("temporary live handshake workspace");
    let namespace = format!("testkit-handshake-{}", std::process::id());
    let session_id = "delayed-hydration";
    let runtime_dir = temp.path().join("runtime");
    let driver = AgentBrowserDriver::new(AgentBrowserConfig {
        command: BrowserCommand::Standalone {
            executable: browser.clone(),
        },
        namespace: namespace.clone(),
        headed: std::env::var_os("AGENT_BROWSER_HEADED").is_some(),
        command_timeout: Duration::from_secs(10),
        idle_timeout: Duration::from_secs(30),
        microphone: Default::default(),
        network_policy: Default::default(),
    });
    let mut session = driver
        .connect(AgentBrowserConnectionConfig {
            namespace: namespace.clone(),
            session: session_id.to_string(),
            runtime_dir: runtime_dir.clone(),
            artifacts_dir: temp.path().join("artifacts"),
            active_video_path: None,
        })
        .await
        .expect("connect live handshake browser session");
    let mut cleanup = LiveBrowserCleanup::new(browser, namespace, session_id, runtime_dir);
    cleanup.arm();

    session
        .execute_action(
            "open-delayed-testkit",
            Action::Navigate { url: fixture.url() },
        )
        .await
        .expect("open delayed Test Kit fixture");
    let handshake = session
        .testkit_handshake(true)
        .await
        .expect("wait for live Test Kit handshake")
        .expect("delayed Test Kit bridge");

    assert_eq!(handshake.protocol, "a3s.test.testkit-handshake/1");
    assert_eq!(handshake.package_name, "@a3s-lab/testkit");
    assert_eq!(handshake.sdk_version, "0.4.2");
    assert_eq!(handshake.page_context_protocol, "a3s.test.page-context/1");
    assert_eq!(
        handshake.capabilities,
        REQUIRED_CAPABILITIES.map(str::to_string).to_vec()
    );
    assert!(handshake.review_overlay_mounted);

    session
        .close_surface()
        .await
        .expect("close live handshake browser session");
    cleanup.disarm();
}

const DELAYED_TESTKIT_HTML: &str = r##"<!doctype html>
<html lang="en">
  <head><meta charset="utf-8"><title>Delayed Test Kit handshake</title></head>
  <body>
    <p id="state">Hydrating Test Kit</p>
    <script>
      setTimeout(() => {
        const overlay = document.createElement("div");
        overlay.setAttribute("data-a3s-testkit-overlay", "");
        overlay.attachShadow({ mode: "open" }).innerHTML = "<span>Review ready</span>";
        document.body.append(overlay);
        window[Symbol.for("a3s.test.page-context")] = {
          handshake() {
            return {
              protocol: "a3s.test.testkit-handshake/1",
              packageName: "@a3s-lab/testkit",
              sdkVersion: "0.4.2",
              pageContextProtocol: "a3s.test.page-context/1",
              capabilities: [
                "bounded_snapshot",
                "component_boundaries",
                "design_references",
                "geometry",
                "repair_queue",
                "revision_wait",
                "scoped_inspection"
              ]
            };
          }
        };
        document.querySelector("#state").textContent = "Test Kit ready";
      }, 750);
    </script>
  </body>
</html>
"##;

struct DelayedTestKitFixture {
    address: SocketAddr,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl DelayedTestKitFixture {
    fn start() -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind handshake fixture");
        let address = listener.local_addr().expect("handshake fixture address");
        listener
            .set_nonblocking(true)
            .expect("make handshake fixture nonblocking");
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker = thread::spawn(move || {
            while !worker_stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut stream, _)) => respond_with_delayed_testkit(&mut stream),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("handshake fixture failed: {error}"),
                }
            }
        });
        Self {
            address,
            stop,
            worker: Some(worker),
        }
    }

    fn url(&self) -> String {
        format!("http://{}/", self.address)
    }
}

impl Drop for DelayedTestKitFixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = TcpStream::connect_timeout(&self.address, Duration::from_millis(250));
        if let Some(worker) = self.worker.take() {
            worker.join().expect("join handshake fixture");
        }
    }
}

fn respond_with_delayed_testkit(stream: &mut TcpStream) {
    let mut request = [0_u8; 2_048];
    let _ = stream.read(&mut request);
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        DELAYED_TESTKIT_HTML.len(),
        DELAYED_TESTKIT_HTML
    );
    stream
        .write_all(response.as_bytes())
        .expect("write delayed Test Kit fixture");
}

struct LiveBrowserCleanup {
    browser: PathBuf,
    namespace: String,
    session: String,
    runtime_dir: PathBuf,
    armed: bool,
}

impl LiveBrowserCleanup {
    fn new(browser: PathBuf, namespace: String, session: &str, runtime_dir: PathBuf) -> Self {
        Self {
            browser,
            namespace,
            session: session.to_string(),
            runtime_dir,
            armed: false,
        }
    }

    fn arm(&mut self) {
        self.armed = true;
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for LiveBrowserCleanup {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let _ = Command::new(&self.browser)
            .args(["--session", &self.session, "close"])
            .env("AGENT_BROWSER_NAMESPACE", &self.namespace)
            .env("AGENT_BROWSER_SOCKET_DIR", &self.runtime_dir)
            .output();
    }
}
