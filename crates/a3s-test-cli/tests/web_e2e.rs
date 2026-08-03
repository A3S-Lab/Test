mod support;

use std::collections::BTreeSet;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use support::web_fixture::{get, WebFixture};

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_a3s-test"))
}

#[test]
fn local_web_fixture_has_deterministic_routes_and_owned_lifecycle() {
    let fixture = WebFixture::start().expect("start Web fixture");
    let origin = fixture.origin();
    let blocked_origin = fixture.blocked_origin();
    let address = fixture.address();

    assert_blocked_sentinel_reachable(&fixture);

    let health = get(&origin, "/health").expect("fixture health");
    assert_eq!(health.status, 200);
    assert_eq!(health.body, b"ready");
    assert_eq!(
        health.headers.get("cache-control").map(String::as_str),
        Some("no-store")
    );

    let home = get(&origin, "/?cache-bust=1").expect("fixture home");
    assert_eq!(home.status, 200);
    let home = String::from_utf8(home.body).expect("UTF-8 fixture home");
    assert!(home.contains("A3S Test hermetic Web E2E"));
    assert!(home.contains(&format!("{blocked_origin}/blocked")));

    let advanced = get(&origin, "/advanced.html").expect("advanced fixture");
    assert_eq!(advanced.status, 200);
    assert!(String::from_utf8(advanced.body)
        .expect("UTF-8 advanced fixture")
        .contains("A3S Test advanced interactions"));

    let containment = get(&origin, "/origin-policy.html").expect("containment fixture");
    assert_eq!(containment.status, 200);
    let containment = String::from_utf8(containment.body).expect("UTF-8 containment fixture");
    assert!(containment.contains(&format!("{blocked_origin}/blocked.js")));

    let same_redirect = get(&origin, "/redirect-same").expect("same-origin redirect");
    assert_eq!(same_redirect.status, 302);
    assert_eq!(
        same_redirect.headers.get("location").map(String::as_str),
        Some("/next")
    );

    let cross_redirect = get(&origin, "/redirect-cross").expect("cross-origin redirect");
    assert_eq!(cross_redirect.status, 302);
    let cross_location = format!("{blocked_origin}/blocked");
    assert_eq!(
        cross_redirect.headers.get("location").map(String::as_str),
        Some(cross_location.as_str())
    );

    let missing = get(&origin, "/missing").expect("missing route");
    assert_eq!(missing.status, 404);
    assert!(fixture.blocked_requests().is_empty());
    assert_eq!(fixture.primary_requests().len(), 7);

    drop(fixture);
    assert!(
        TcpStream::connect_timeout(&address, Duration::from_millis(250)).is_err(),
        "fixture listener must be closed on drop"
    );
}

#[test]
#[ignore = "requires the exact standalone agent-browser 0.26.x runtime"]
fn real_agent_browser_runs_the_hermetic_web_suite() {
    let Some(browser) = std::env::var_os("A3S_TEST_AGENT_BROWSER").map(PathBuf::from) else {
        eprintln!("A3S_TEST_AGENT_BROWSER is not set; skipping real browser E2E");
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
        "real E2E requires the admitted 0.26.x protocol: {}",
        String::from_utf8_lossy(&version.stdout)
    );

    let fixture = WebFixture::start().expect("start Web fixture");
    assert_blocked_sentinel_reachable(&fixture);
    let fixture_address = fixture.address();
    let temp = tempfile::tempdir().expect("temporary E2E workspace");
    let manifest = temp.path().join("hermetic-web-e2e.acl");
    std::fs::write(&manifest, suite(&fixture.origin())).expect("write E2E suite");
    let runtime_directories_before = private_runtime_directories();

    let output = Command::new(binary())
        .args([
            "run",
            manifest.to_str().expect("UTF-8 manifest path"),
            "--browser-driver",
            "standalone",
            "--browser-executable",
            browser.to_str().expect("UTF-8 browser path"),
            "--command-timeout-ms",
            "60000",
            "--idle-timeout-ms",
            "15000",
            "--cleanup-timeout-ms",
            "15000",
            "--infrastructure-retries",
            "0",
            "--json",
        ])
        .current_dir(temp.path())
        .output()
        .expect("run real browser E2E");

    assert!(
        output.status.success(),
        "real browser E2E failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("JSON run report");
    assert_eq!(report["status"], "passed");
    assert_eq!(report["scenarios"][0]["status"], "passed");
    assert!(report["scenarios"][0]["cleanup_error"].is_null());

    let evidence_path = report["scenarios"][0]["steps"]
        .as_array()
        .expect("steps")
        .iter()
        .find(|step| step["id"] == "form-evidence")
        .and_then(|step| step.pointer("/output/evidence/0/path"))
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from)
        .expect("screenshot evidence path");
    assert_nonempty_artifact(temp.path(), &evidence_path);
    run_agent_domain_containment(&browser, &fixture, temp.path());

    let primary_paths = fixture
        .primary_requests()
        .into_iter()
        .map(|request| request.path)
        .collect::<Vec<_>>();
    assert!(primary_paths.iter().any(|path| path == "/"));
    assert!(primary_paths.iter().any(|path| path == "/next"));
    assert!(
        primary_paths.iter().any(|path| path == "/redirect-cross"),
        "the browser never exercised the redirect containment route: {primary_paths:?}"
    );
    assert!(
        fixture.blocked_requests().is_empty(),
        "the allowed E2E path must not contact the cross-origin sentinel"
    );
    assert_no_new_private_runtime_directories(&runtime_directories_before);

    drop(fixture);
    assert!(
        TcpStream::connect_timeout(&fixture_address, Duration::from_millis(250)).is_err(),
        "real E2E fixture listener must be closed"
    );
}

fn run_agent_domain_containment(browser: &Path, fixture: &WebFixture, workspace: &Path) {
    let mut cleanup = AgentSessionCleanup::new(workspace, "domain-containment");
    let fixture_url = format!("{}/origin-policy.html", fixture.origin());
    let start = Command::new(binary())
        .args([
            "agent",
            "start",
            &fixture_url,
            "--session",
            "domain-containment",
            "--goal",
            "Verify browser-level cross-domain containment",
            "--success",
            "The allowed page loads without contacting the sentinel origin",
            "--browser-driver",
            "standalone",
            "--browser-executable",
            browser.to_str().expect("UTF-8 browser path"),
            "--command-timeout-ms",
            "60000",
            "--idle-timeout-ms",
            "15000",
            "--json",
        ])
        .current_dir(workspace)
        .output()
        .expect("start contained agent session");
    if start.status.success() {
        cleanup.arm();
    }
    assert_process_success("start contained agent session", &start);
    let start_json: serde_json::Value =
        serde_json::from_slice(&start.stdout).expect("contained start JSON");
    assert_eq!(
        start_json["browser_allowed_domains"],
        serde_json::json!(["127.0.0.1"])
    );

    let observe = Command::new(binary())
        .args([
            "agent",
            "observe",
            "--session",
            "domain-containment",
            "--json",
        ])
        .current_dir(workspace)
        .output()
        .expect("observe contained agent session");
    assert_process_success("observe contained agent session", &observe);
    assert!(
        String::from_utf8_lossy(&observe.stdout).contains("Browser containment fixture ready"),
        "contained observation did not remain on the allowed page: {}",
        String::from_utf8_lossy(&observe.stdout)
    );

    let click = Command::new(binary())
        .args([
            "agent",
            "click",
            "#cross-origin",
            "--session",
            "domain-containment",
            "--json",
        ])
        .current_dir(workspace)
        .output()
        .expect("dispatch contained cross-origin link");
    assert_process_success("dispatch contained cross-origin link", &click);
    let click_json: serde_json::Value =
        serde_json::from_slice(&click.stdout).expect("contained click JSON");
    assert_eq!(click_json["session"], "domain-containment");

    std::thread::sleep(Duration::from_millis(250));
    assert!(
        fixture.blocked_requests().is_empty(),
        "browser domain policy allowed a link, script, image, fetch, or redirect request"
    );

    let abort = cleanup.abort();
    assert_process_success("abort contained agent session", &abort);
    let abort_json: serde_json::Value =
        serde_json::from_slice(&abort.stdout).expect("contained abort JSON");
    assert!(abort_json["cleanup_error"].is_null());
}

fn assert_blocked_sentinel_reachable(fixture: &WebFixture) {
    let health = get(&fixture.blocked_origin(), "/health").expect("blocked sentinel health");
    assert_eq!(health.status, 200);
    assert_eq!(health.body, b"ready");
    assert_eq!(
        fixture.blocked_requests(),
        [support::web_fixture::RecordedRequest {
            method: "GET".to_string(),
            path: "/health".to_string(),
        }]
    );
    fixture.clear_blocked_requests();
}

struct AgentSessionCleanup {
    workspace: PathBuf,
    session: &'static str,
    armed: bool,
}

impl AgentSessionCleanup {
    fn new(workspace: &Path, session: &'static str) -> Self {
        Self {
            workspace: workspace.to_path_buf(),
            session,
            armed: false,
        }
    }

    fn arm(&mut self) {
        self.armed = true;
    }

    fn abort(&mut self) -> std::process::Output {
        let output = abort_agent_session(&self.workspace, self.session);
        if output.status.success() {
            self.armed = false;
        }
        output
    }
}

impl Drop for AgentSessionCleanup {
    fn drop(&mut self) {
        if self.armed {
            let _ = abort_agent_session(&self.workspace, self.session);
        }
    }
}

fn abort_agent_session(workspace: &Path, session: &str) -> std::process::Output {
    Command::new(binary())
        .args(["agent", "abort", "--session", session, "--json"])
        .current_dir(workspace)
        .output()
        .expect("abort agent session cleanup")
}

fn assert_process_success(context: &str, output: &std::process::Output) {
    assert!(
        output.status.success(),
        "{context} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn suite(origin: &str) -> String {
    format!(
        r##"suite "web-hermetic-e2e" {{
    version = 1

    scenario "semantic-form" {{
        name = "Drive a local semantic form and retain evidence"
        surface = "web"
        timeout_ms = 60000

        navigate "open" {{
            url = "{origin}/"
        }}

        wait "loaded" {{
            load = "domcontentloaded"
        }}

        fill "display-name" {{
            target = label("Display name")
            value = "Ada Lovelace"
        }}

        click "submit" {{
            target = css("#submit")
        }}

        expect "submitted" {{
            text = "submitted: Ada Lovelace"
        }}

        screenshot "form-evidence" {{
            path = "screenshots/form.png"
        }}

        click "same-origin" {{
            target = css("#same-origin")
        }}

        expect "next-page" {{
            text = "Same-origin navigation passed"
        }}
    }}
}}
"##
    )
}

fn assert_nonempty_artifact(workspace: &Path, path: &Path) {
    let workspace = workspace.canonicalize().expect("canonical E2E workspace");
    let path = path.canonicalize().expect("canonical screenshot evidence");
    assert!(
        path.starts_with(&workspace),
        "screenshot evidence escaped the E2E workspace: {path:?}"
    );
    assert!(
        path.metadata().expect("screenshot metadata").len() > 0,
        "screenshot evidence must not be empty"
    );
}

fn private_runtime_directories() -> BTreeSet<PathBuf> {
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

fn assert_no_new_private_runtime_directories(before: &BTreeSet<PathBuf>) {
    for _ in 0..20 {
        let current = private_runtime_directories();
        let leaked = current.difference(before).cloned().collect::<Vec<_>>();
        if leaked.is_empty() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let current = private_runtime_directories();
    let leaked = current.difference(before).collect::<Vec<_>>();
    panic!("browser runtime directories leaked after E2E cleanup: {leaked:?}");
}
