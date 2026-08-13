mod support;

use std::collections::BTreeSet;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use support::web_fixture::{get, start_testkit_fixture, WebFixture};

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
fn local_web_fixture_handles_repeated_short_lived_connections() {
    let fixture = WebFixture::start().expect("start Web fixture");
    let origin = fixture.origin();

    for request in 0..128 {
        let health = get(&origin, "/health")
            .unwrap_or_else(|error| panic!("fixture health request {request} failed: {error}"));
        assert_eq!(health.status, 200);
        assert_eq!(health.body, b"ready");
    }

    assert_eq!(fixture.primary_requests().len(), 128);
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

#[test]
#[ignore = "requires Node esbuild and the exact standalone agent-browser 0.26.x runtime"]
fn real_agent_browser_runs_the_embedded_testkit_suite() {
    let Some(browser) = std::env::var_os("A3S_TEST_AGENT_BROWSER").map(PathBuf::from) else {
        eprintln!("A3S_TEST_AGENT_BROWSER is not set; skipping TestKit browser E2E");
        return;
    };
    assert!(
        browser.is_file(),
        "browser executable does not exist: {browser:?}"
    );
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate workspace root")
        .to_path_buf();
    let esbuild = crate_root.join("packages/testkit/node_modules/.bin/esbuild");
    assert!(
        esbuild.is_file(),
        "run `npm install` in packages/testkit before this E2E"
    );
    let temp = tempfile::tempdir().expect("temporary TestKit browser fixture");
    let bundle_path = temp.path().join("testkit.js");
    let bundle = Command::new(&esbuild)
        .args([
            crate_root
                .join("packages/testkit/src/browser-fixture.tsx")
                .to_str()
                .expect("UTF-8 entry"),
            "--bundle",
            "--format=esm",
            "--platform=browser",
            "--target=es2022",
            &format!("--outfile={}", bundle_path.display()),
        ])
        .output()
        .expect("bundle TestKit fixture");
    assert!(
        bundle.status.success(),
        "TestKit fixture bundle failed: {}",
        String::from_utf8_lossy(&bundle.stderr)
    );
    let fixture = start_testkit_fixture(std::fs::read(&bundle_path).expect("read TestKit bundle"))
        .expect("start TestKit fixture");
    let session = format!("a3s-testkit-e2e-{}", std::process::id());
    let mut cleanup = StandaloneBrowserSessionCleanup::new(&browser, &session);
    let command = |arguments: &[&str]| {
        Command::new(&browser)
            .arg("--session")
            .arg(&session)
            .args(arguments)
            .output()
            .expect("run standalone browser command")
    };
    let opened = command(&["open", &fixture.origin()]);
    cleanup.arm();
    assert_process_success("open TestKit fixture", &opened);
    let context = command(&[
        "eval",
        "JSON.stringify(window[Symbol.for('a3s.test.page-context')].snapshot({detail:'forensic'}))",
    ]);
    assert_process_success("capture TestKit context", &context);
    let stdout = String::from_utf8_lossy(&context.stdout);
    assert!(
        stdout.contains("app-shell"),
        "component context missing: {stdout}"
    );
    assert!(
        stdout.contains("repair-target"),
        "semantic locator missing: {stdout}"
    );
    assert!(
        stdout.contains("Shadow action"),
        "open Shadow DOM missing: {stdout}"
    );
    assert!(
        stdout.contains("Confirm dialog"),
        "dialog context missing: {stdout}"
    );
    assert!(
        stdout.contains("sticky"),
        "sticky geometry missing: {stdout}"
    );

    let accessibility = command(&["snapshot"]);
    assert_process_success("capture TestKit accessibility tree", &accessibility);
    let accessibility = String::from_utf8_lossy(&accessibility.stdout);
    for expected in [
        "dialog \"Review & repair\"",
        "button \"Pause page animations\"",
        "button \"Turn auto-send on\"",
        "button \"Change overlay theme; current theme is system\"",
        "button \"Close review overlay\"",
    ] {
        assert!(
            accessibility.contains(expected),
            "TestKit accessibility tree missing {expected:?}: {accessibility}"
        );
    }

    let focus_round_trip = command(&[
        "eval",
        "(async()=>{let host=null;for(let frame=0;frame<120;frame+=1){const candidate=document.querySelector('[data-a3s-testkit-overlay]');if(candidate?.isConnected&&candidate.shadowRoot?.querySelector('[aria-label=\"Close review overlay\"]')){await new Promise(resolve=>requestAnimationFrame(resolve));if(candidate.isConnected&&candidate.shadowRoot?.querySelector('[aria-label=\"Close review overlay\"]')){host=candidate;break}}await new Promise(resolve=>requestAnimationFrame(resolve))}if(!host)throw new Error('stable TestKit overlay host not found');const shadow=host.shadowRoot;shadow.querySelector('[aria-label=\"Close review overlay\"]').click();await new Promise(resolve=>requestAnimationFrame(resolve));const closeFocus=shadow.activeElement?.classList.contains('a3s-launch')===true;shadow.querySelector('.a3s-launch').click();await new Promise(resolve=>requestAnimationFrame(resolve));const openFocus=shadow.activeElement?.classList.contains('a3s-panel')===true;return JSON.stringify({closeFocus,openFocus})})()",
    ]);
    assert_process_success(
        "exercise TestKit overlay focus round trip",
        &focus_round_trip,
    );
    let focus_round_trip: String = serde_json::from_slice(&focus_round_trip.stdout)
        .expect("TestKit overlay focus round trip JSON string");
    let focus_round_trip: serde_json::Value =
        serde_json::from_str(&focus_round_trip).expect("TestKit overlay focus round trip JSON");
    assert!(
        focus_round_trip["closeFocus"] == true && focus_round_trip["openFocus"] == true,
        "TestKit overlay focus round trip failed: {focus_round_trip}"
    );

    assert_process_success(
        "set TestKit browser viewport and DPR",
        &command(&["set", "viewport", "1280", "720", "2"]),
    );
    let before_zoom = capture_testkit_zoom_geometry(&command, "before browser zoom");
    assert_approx(
        before_zoom.pointer("/page/viewport/width"),
        1280.0,
        0.01,
        "layout viewport width before zoom",
    );
    assert_approx(
        before_zoom.pointer("/page/viewport/dpr"),
        2.0,
        0.01,
        "DPR before zoom",
    );
    assert_approx(
        before_zoom.pointer("/page/viewport/visual/scale"),
        1.0,
        0.01,
        "visual scale before zoom",
    );
    assert_approx(
        before_zoom.pointer("/target/geometry/visibleRatio"),
        1.0,
        0.01,
        "edge target visibility before zoom",
    );

    let cdp_url = command(&["get", "cdp-url"]);
    assert_process_success("read TestKit browser CDP URL", &cdp_url);
    set_browser_page_scale(
        String::from_utf8_lossy(&cdp_url.stdout).trim(),
        1.5,
        &command,
    );
    let after_zoom = capture_testkit_zoom_geometry(&command, "after browser zoom");
    assert_approx(
        after_zoom.pointer("/page/viewport/width"),
        1280.0,
        0.01,
        "layout viewport width after zoom",
    );
    assert_approx(
        after_zoom.pointer("/page/viewport/dpr"),
        2.0,
        0.01,
        "DPR after zoom",
    );
    assert_approx(
        after_zoom.pointer("/page/viewport/visual/width"),
        853.333,
        0.1,
        "visual viewport width after zoom",
    );
    assert_approx(
        after_zoom.pointer("/page/viewport/visual/scale"),
        1.5,
        0.01,
        "visual scale after zoom",
    );
    assert_approx(
        after_zoom.pointer("/target/geometry/viewport/x"),
        1000.0,
        0.01,
        "CSS-pixel target position after zoom",
    );
    assert_approx(
        after_zoom.pointer("/target/geometry/viewport/width"),
        180.0,
        0.01,
        "CSS-pixel target width after zoom",
    );
    assert_approx(
        after_zoom.pointer("/target/geometry/visibleRatio"),
        0.0,
        0.01,
        "edge target visibility after zoom",
    );
    assert_approx(
        after_zoom.pointer("/target/geometry/normalized/x"),
        1.171875,
        0.001,
        "visual-viewport normalized target position after zoom",
    );
    set_browser_page_scale(
        String::from_utf8_lossy(&cdp_url.stdout).trim(),
        1.0,
        &command,
    );

    let select_keyboard_marking = command(&[
        "eval",
        "(()=>{const host=document.querySelector('[data-a3s-testkit-overlay]'); [...host.shadowRoot.querySelectorAll('button')].find(button=>button.textContent==='Element').click(); return true})()",
    ]);
    assert_process_success(
        "select TestKit keyboard marking mode",
        &select_keyboard_marking,
    );
    let marking_ready = command(&[
        "wait",
        "--fn",
        "Boolean(document.querySelector('[data-a3s-testkit-overlay]')?.shadowRoot?.querySelector('.a3s-hint'))",
    ]);
    assert_process_success("wait for TestKit keyboard marking mode", &marking_ready);
    let keyboard_mark = command(&[
        "eval",
        "(()=>{const target=document.querySelector('#sticky'); target.focus(); target.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',bubbles:true})); return true})()",
    ]);
    assert_process_success("mark TestKit target by keyboard", &keyboard_mark);
    let editor_ready = command(&[
        "wait",
        "--fn",
        "Boolean(document.querySelector('[data-a3s-testkit-overlay]')?.shadowRoot?.querySelector('.a3s-editor'))",
    ]);
    assert_process_success("wait for TestKit finding editor", &editor_ready);
    let fill_instruction = command(&[
        "eval",
        "(()=>{const host=document.querySelector('[data-a3s-testkit-overlay]'); const textarea=host.shadowRoot.querySelector('.a3s-editor textarea'); const setter=Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype,'value').set; setter.call(textarea,'Repair the broken action'); textarea.dispatchEvent(new Event('input',{bubbles:true,composed:true})); return true})()",
    ]);
    assert_process_success("fill TestKit repair instruction", &fill_instruction);
    let submission_ready = command(&[
        "wait",
        "--fn",
        "[...document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelectorAll('button')].some(button=>button.textContent==='Send and auto-fix'&&!button.disabled)",
    ]);
    assert_process_success("wait for TestKit finding submission", &submission_ready);
    let submit = command(&[
        "eval",
        "(()=>{const host=document.querySelector('[data-a3s-testkit-overlay]'); [...host.shadowRoot.querySelectorAll('button')].find(button=>button.textContent==='Send and auto-fix'&&!button.disabled).click(); return true})()",
    ]);
    assert_process_success("submit TestKit keyboard finding", &submit);
    let submitted = command(&[
        "wait",
        "--fn",
        "window[Symbol.for('a3s.test.page-context')].listRepairs().length===1",
    ]);
    assert_process_success("wait for TestKit keyboard finding", &submitted);

    let changed = command(&[
        "eval",
        "window.testkitFixture.route(); window.testkitFixture.virtualize(); true",
    ]);
    assert_process_success("mutate TestKit fixture", &changed);
    let virtualized = command(&[
        "wait",
        "--fn",
        "document.querySelector('#virtual-row')?.textContent==='Virtual row 50'",
    ]);
    assert_process_success("wait for TestKit virtual window update", &virtualized);
    let changed = command(&[
        "eval",
        "JSON.stringify(window[Symbol.for('a3s.test.page-context')].snapshot({detail:'forensic'}))",
    ]);
    assert_process_success("capture mutated TestKit context", &changed);
    let changed = String::from_utf8_lossy(&changed.stdout);
    assert!(
        changed.contains("/routed?view=2"),
        "route context missing: {changed}"
    );
    assert!(
        changed.contains("Virtual row 50"),
        "virtual window update missing: {changed}"
    );

    let teardown = command(&[
        "eval",
        "window.testkitFixture.teardown(); window[Symbol.for('a3s.test.page-context')] === undefined",
    ]);
    assert_process_success("teardown TestKit fixture", &teardown);
    assert!(String::from_utf8_lossy(&teardown.stdout).contains("true"));
    let closed = cleanup.close();
    assert_process_success("close TestKit browser session", &closed);
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
    assert!(
        observe.status.success(),
        "observe contained agent session failed\nstart stdout:\n{}\nobserve stdout:\n{}\nobserve stderr:\n{}",
        String::from_utf8_lossy(&start.stdout),
        String::from_utf8_lossy(&observe.stdout),
        String::from_utf8_lossy(&observe.stderr)
    );
    assert!(
        String::from_utf8_lossy(&observe.stdout).contains("Browser containment fixture ready"),
        "contained observation did not remain on the allowed page: {}",
        String::from_utf8_lossy(&observe.stdout)
    );
    std::thread::sleep(Duration::from_millis(250));
    let load_requests = fixture.blocked_requests();
    assert!(
        load_requests.is_empty(),
        "browser domain policy allowed a script, image, fetch, iframe, or redirect while loading the containment page: {load_requests:?}"
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
    let blocked_requests = fixture.blocked_requests();
    assert!(
        blocked_requests.is_empty(),
        "browser domain policy allowed a link, script, image, fetch, or redirect request: {blocked_requests:?}"
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

struct StandaloneBrowserSessionCleanup {
    browser: PathBuf,
    session: String,
    armed: bool,
}

impl StandaloneBrowserSessionCleanup {
    fn new(browser: &Path, session: &str) -> Self {
        Self {
            browser: browser.to_path_buf(),
            session: session.to_string(),
            armed: false,
        }
    }

    fn arm(&mut self) {
        self.armed = true;
    }

    fn close(&mut self) -> std::process::Output {
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

fn close_standalone_browser_session(browser: &Path, session: &str) -> std::process::Output {
    Command::new(browser)
        .args(["--session", session, "close"])
        .output()
        .expect("close standalone browser session")
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

fn capture_testkit_zoom_geometry(
    command: &impl Fn(&[&str]) -> std::process::Output,
    context: &str,
) -> serde_json::Value {
    let output = command(&[
        "eval",
        "(()=>{const snapshot=window[Symbol.for('a3s.test.page-context')].snapshot({detail:'forensic'});const target=snapshot.nodes.find(node=>node.testId==='zoom-edge');return JSON.stringify({page:snapshot.page,target});})()",
    ]);
    assert_process_success(context, &output);
    let encoded: String = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("{context} did not return an encoded JSON string: {error}"));
    serde_json::from_str(&encoded)
        .unwrap_or_else(|error| panic!("{context} returned invalid TestKit JSON: {error}"))
}

fn set_browser_page_scale(
    cdp_url: &str,
    factor: f64,
    command: &impl Fn(&[&str]) -> std::process::Output,
) {
    let port = cdp_url
        .split_once("://")
        .and_then(|(_, rest)| rest.split('/').next())
        .and_then(|authority| authority.rsplit_once(':'))
        .map(|(_, port)| port)
        .unwrap_or_else(|| panic!("browser returned an invalid CDP URL: {cdp_url}"));
    let output = Command::new("node")
        .args([
            "--input-type=module",
            "-e",
            CDP_PAGE_SCALE_SCRIPT,
            port,
            &factor.to_string(),
        ])
        .output()
        .expect("run bounded CDP page-scale helper");
    assert_process_success("set browser page scale", &output);
    let expected = format!("visualViewport.scale==={factor}");
    let wait = command(&["wait", "--fn", &expected]);
    assert_process_success("wait for browser page scale", &wait);
}

fn assert_approx(actual: Option<&serde_json::Value>, expected: f64, epsilon: f64, label: &str) {
    let actual = actual
        .and_then(serde_json::Value::as_f64)
        .unwrap_or_else(|| panic!("{label} was not a number"));
    assert!(
        (actual - expected).abs() <= epsilon,
        "{label}: expected {expected} ± {epsilon}, got {actual}"
    );
}

const CDP_PAGE_SCALE_SCRIPT: &str = r#"
const port = process.argv[1];
const factor = Number(process.argv[2]);
if (!/^\d{1,5}$/.test(port) || !Number.isFinite(factor) || factor < 0.25 || factor > 5) {
  throw new Error("invalid bounded page-scale arguments");
}
const response = await fetch(`http://127.0.0.1:${port}/json/list`);
if (!response.ok) throw new Error(`CDP target discovery returned ${response.status}`);
const targets = await response.json();
const page = targets.find((target) => target.type === "page");
if (!page?.webSocketDebuggerUrl) throw new Error("CDP page target missing");
await new Promise((resolve, reject) => {
  const socket = new WebSocket(page.webSocketDebuggerUrl);
  const timer = setTimeout(() => reject(new Error("CDP page-scale command timed out")), 5_000);
  socket.onopen = () => socket.send(JSON.stringify({
    id: 1,
    method: "Emulation.setPageScaleFactor",
    params: { pageScaleFactor: factor },
  }));
  socket.onmessage = (event) => {
    const message = JSON.parse(event.data);
    if (message.id !== 1) return;
    clearTimeout(timer);
    socket.close();
    if (message.error) reject(new Error(JSON.stringify(message.error)));
    else resolve();
  };
  socket.onerror = () => reject(new Error("CDP WebSocket failed"));
});
"#;

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
