mod support;

use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use support::assertion_stability::{assert_passed_stability, run_transient_stability_e2e};
use support::browser_process::{
    assert_no_new_private_runtime_directories, assert_process_success, bounded_output,
    private_runtime_directories, StandaloneBrowserSessionCleanup,
};
use support::browser_zoom::{
    assert_approx, capture_testkit_zoom_geometry, json_number, set_browser_page_scale,
};
use support::testkit_accessibility::{
    exercise_repair_status_accessibility, exercise_review_candidate_accessibility,
    verify_audit_fixture_reset,
};
use support::testkit_browser::{
    assert_wcag_accessibility, assert_wcag_accessibility_across_themes, click_accessible,
    run_review_workflow, verify_hide_until_restart_focus,
};
use support::testkit_bundle::bundle_browser_fixture;
use support::ui_understanding::verify_testkit_ui_understanding_through_driver;
use support::web_evidence::{
    assert_empty_browser_diagnostics, assert_nonempty_artifact, assert_png_artifact,
    failed_run_summary,
};
use support::web_fixture::{
    assert_blocked_sentinel_reachable, get, start_static_site_fixture, start_testkit_fixture,
    WebFixture,
};
use support::website::build_website;

const WEBSITE_TESTKIT_SUITE: &str = include_str!("../../../tests/e2e/website-testkit.acl");

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

    let focus = get(&origin, "/focus.html").expect("focus ownership fixture");
    assert_eq!(focus.status, 200);
    assert!(String::from_utf8(focus.body)
        .expect("UTF-8 focus ownership fixture")
        .contains("data-testid=\"shadow-focus-scope\""));

    let transient = get(&origin, "/transient.html").expect("transient assertion fixture");
    assert_eq!(transient.status, 200);
    assert!(String::from_utf8(transient.body)
        .expect("UTF-8 transient assertion fixture")
        .contains("data-testid=\"transient-state\""));

    let rendered = get(&origin, "/rendered.html").expect("rendered assertion fixture");
    assert_eq!(rendered.status, 200);
    assert!(String::from_utf8(rendered.body)
        .expect("UTF-8 rendered assertion fixture")
        .contains("data-testid=\"total-copy\""));

    let layout = get(&origin, "/layout.html").expect("layout assertion fixture");
    assert_eq!(layout.status, 200);
    assert!(String::from_utf8(layout.body)
        .expect("UTF-8 layout assertion fixture")
        .contains("A3S Test layout assertion fixture"));

    let interactability =
        get(&origin, "/interactability.html").expect("interactability assertion fixture");
    assert_eq!(interactability.status, 200);
    assert!(String::from_utf8(interactability.body)
        .expect("UTF-8 interactability assertion fixture")
        .contains("A3S Test interactability assertion fixture"));

    let semantic_state =
        get(&origin, "/semantic-state.html").expect("semantic state assertion fixture");
    assert_eq!(semantic_state.status, 200);
    assert!(String::from_utf8(semantic_state.body)
        .expect("UTF-8 semantic state assertion fixture")
        .contains("data-testid=\"mixed-pressed\""));

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
    assert_eq!(fixture.primary_requests().len(), 13);

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
fn website_testkit_acl_is_admitted() {
    let temp = tempfile::tempdir().expect("temporary website ACL workspace");
    let manifest = temp.path().join("website-testkit.acl");
    std::fs::write(&manifest, WEBSITE_TESTKIT_SUITE).expect("write website Test Kit ACL");

    let output = Command::new(binary())
        .args([
            "check",
            manifest.to_str().expect("UTF-8 website ACL path"),
            "--json",
        ])
        .current_dir(temp.path())
        .output()
        .expect("check website Test Kit ACL");

    assert_process_success("admit website Test Kit ACL", &output);
}

#[test]
fn website_failure_summary_keeps_errors_without_dumping_passed_page_context() {
    let report = serde_json::json!({
        "status": "timed_out",
        "scenarios": [{
            "id": "desktop",
            "status": "timed_out",
            "cleanup_error": null,
            "steps": [
                {
                    "id": "observe",
                    "status": "passed",
                    "duration_ms": 12,
                    "output": { "page_context": "large-page-context-payload" },
                    "error": null
                },
                {
                    "id": "screenshot",
                    "status": "timed_out",
                    "duration_ms": 10_000,
                    "error": {
                        "code": "test.driver.web.command_unavailable",
                        "message": "browser command exceeded 10000 ms"
                    }
                }
            ]
        }]
    });

    let summary = failed_run_summary(&report);

    assert!(summary.contains("scenario desktop: timed_out"));
    assert!(summary.contains("step screenshot: timed_out after 10000 ms"));
    assert!(summary.contains("browser command exceeded 10000 ms"));
    assert!(!summary.contains("large-page-context-payload"));
}

#[test]
#[ignore = "requires website dependencies and the exact standalone agent-browser 0.26.x runtime"]
fn real_agent_browser_runs_the_website_testkit_suite() {
    let Some(browser) = std::env::var_os("A3S_TEST_AGENT_BROWSER").map(PathBuf::from) else {
        eprintln!("A3S_TEST_AGENT_BROWSER is not set; skipping website Test Kit E2E");
        return;
    };
    assert!(
        browser.is_file(),
        "browser executable does not exist: {browser:?}"
    );

    let website = build_website("build website Test Kit fixture");
    let fixture =
        start_static_site_fixture(&website, "/Test/").expect("start built website fixture");
    let fixture_address = fixture.address();
    let homepage = get(&fixture.origin(), "/Test/").expect("built website homepage");
    assert_eq!(homepage.status, 200);
    assert!(
        String::from_utf8(homepage.body)
            .expect("UTF-8 built website homepage")
            .contains("data-testid=\"a3s-experience-submit\""),
        "built homepage must contain the embedded Test Kit experience"
    );

    let temp = tempfile::tempdir().expect("temporary website E2E workspace");
    let manifest = temp.path().join("website-testkit.acl");
    let suite = WEBSITE_TESTKIT_SUITE.replace("http://127.0.0.1:4173", &fixture.origin());
    std::fs::write(&manifest, suite).expect("write dynamic website Test Kit ACL");
    let runtime_directories_before = private_runtime_directories();

    let output = Command::new(binary())
        .args([
            "run",
            manifest.to_str().expect("UTF-8 website manifest path"),
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
            "--max-parallel-scenarios",
            "1",
            "--json",
        ])
        .current_dir(temp.path())
        .output()
        .expect("run website Test Kit E2E");

    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
            panic!(
                "website Test Kit E2E returned invalid JSON: {error}\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )
        });
    assert!(
        output.status.success(),
        "website Test Kit E2E failed\n{}\nstderr:\n{}",
        failed_run_summary(&report),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(report["status"], "passed");
    let scenarios = report["scenarios"]
        .as_array()
        .expect("website E2E scenarios");
    assert_eq!(scenarios.len(), 2);
    for scenario in scenarios {
        assert_eq!(scenario["status"], "passed");
        assert!(scenario["cleanup_error"].is_null());
    }

    for (scenario_index, step) in [
        (0, "review-screenshot-evidence"),
        (0, "review-evidence"),
        (0, "semantic-evidence"),
        (0, "console-evidence"),
        (0, "page-error-evidence"),
        (1, "mobile-layout-screenshot-evidence"),
        (1, "mobile-layout-evidence"),
        (1, "mobile-semantic-evidence"),
        (1, "mobile-console-evidence"),
        (1, "mobile-page-error-evidence"),
    ] {
        let evidence_path = report["scenarios"][scenario_index]["steps"]
            .as_array()
            .expect("website E2E steps")
            .iter()
            .find(|entry| entry["id"] == step)
            .and_then(|entry| entry.pointer("/output/evidence/0/path"))
            .and_then(serde_json::Value::as_str)
            .map(PathBuf::from)
            .unwrap_or_else(|| panic!("website E2E step {step} omitted its evidence path"));
        assert_nonempty_artifact(temp.path(), &evidence_path);
    }

    for (scenario_index, step, width, height) in [
        (0, "review-screenshot-evidence", 1440, 900),
        (1, "mobile-layout-screenshot-evidence", 390, 844),
    ] {
        let evidence = report["scenarios"][scenario_index]["steps"]
            .as_array()
            .expect("website E2E steps")
            .iter()
            .find(|entry| entry["id"] == step)
            .and_then(|entry| entry.pointer("/output/evidence/0"))
            .unwrap_or_else(|| panic!("website E2E step {step} omitted its evidence"));
        assert_eq!(evidence["media_type"], "image/png");
        let evidence_path = evidence["path"]
            .as_str()
            .map(PathBuf::from)
            .unwrap_or_else(|| panic!("website E2E step {step} omitted its evidence path"));
        assert_png_artifact(temp.path(), &evidence_path, width, height);
    }

    assert_empty_browser_diagnostics(&report, temp.path());
    assert_no_new_private_runtime_directories(&runtime_directories_before);

    drop(fixture);
    assert!(
        TcpStream::connect_timeout(&fixture_address, Duration::from_millis(250)).is_err(),
        "website fixture listener must be closed on drop"
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
    assert_passed_stability(&report, 0, "submitted", 100, 25);

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
    run_transient_stability_e2e(&binary(), &browser, &fixture.origin(), temp.path());
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
    let (_bundle_workspace, bundle) = bundle_browser_fixture("bundle TestKit fixture");
    let fixture = start_testkit_fixture(bundle).expect("start TestKit fixture");
    verify_testkit_ui_understanding_through_driver(&binary(), &browser, &fixture.origin());
    let workflow_manifest = get(&fixture.origin(), "/screen-reader-workflows.json")
        .expect("read screen-reader workflow manifest from shared fixture");
    assert_eq!(workflow_manifest.status, 200);
    assert_eq!(
        workflow_manifest
            .headers
            .get("content-type")
            .map(String::as_str),
        Some("application/json; charset=utf-8")
    );
    let workflow_manifest: serde_json::Value = serde_json::from_slice(&workflow_manifest.body)
        .expect("screen-reader workflow manifest JSON");
    assert_eq!(
        workflow_manifest["protocol"],
        "a3s.test.screen-reader-workflows/1"
    );
    assert_eq!(
        workflow_manifest["workflows"].as_array().map(Vec::len),
        Some(15)
    );
    let session = format!("a3s-testkit-e2e-{}", std::process::id());
    let mut cleanup = StandaloneBrowserSessionCleanup::new(&browser, &session);
    let command = |arguments: &[&str]| {
        let mut command = Command::new(&browser);
        command.arg("--session").arg(&session).args(arguments);
        bounded_output(&mut command, "run standalone browser command")
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
    for expected in [
        "a3s.test.ui-understanding/1",
        "observationId",
        "computed_style",
        "boxModel",
        "overflowMetrics",
        "responsiveConditions",
        "rangeStarts",
        "stateDiffs",
        "prefersReducedMotion",
    ] {
        assert!(
            stdout.contains(expected),
            "rendered UI understanding missing {expected:?}: {stdout}"
        );
    }

    let accessibility = command(&["snapshot"]);
    assert_process_success("capture TestKit accessibility tree", &accessibility);
    let accessibility = String::from_utf8_lossy(&accessibility.stdout);
    for expected in [
        "region \"Review\"",
        "tab \"New feedback\"",
        "tab \"Findings\"",
        "tab \"Review preferences\"",
        "button \"Mark element\"",
        "button \"Mark multi\"",
        "button \"Mark text\"",
        "button \"Layout\"",
        "button \"Close review overlay\"",
        "heading \"Screen-reader audit controls\"",
        "button \"Seed contract and design candidates\"",
        "combobox \"Repair state\"",
        "button \"Apply repair state\"",
        "button \"Reset fixture\"",
        "link \"Audit workflow manifest\"",
    ] {
        assert!(
            accessibility.contains(expected),
            "TestKit accessibility tree missing {expected:?}: {accessibility}"
        );
    }

    click_accessible(
        &command,
        "open the TestKit review preferences",
        "tab",
        "Review preferences",
    );
    let tool_accessibility = command(&["snapshot"]);
    assert_process_success(
        "capture the TestKit review preferences",
        &tool_accessibility,
    );
    let tool_accessibility = String::from_utf8_lossy(&tool_accessibility.stdout);
    for expected in [
        "button \"Pause page animations\"",
        "button \"Turn auto-send on\"",
        "combobox \"Overlay theme\"",
        "combobox \"Panel dock\"",
        "button \"Hide until tab restart\"",
    ] {
        assert!(
            tool_accessibility.contains(expected),
            "TestKit review preferences missing {expected:?}: {tool_accessibility}"
        );
    }

    assert_wcag_accessibility_across_themes(&command);

    let focus_round_trip = command(&[
        "eval",
        "(async()=>{let host=null;for(let frame=0;frame<120;frame+=1){const candidate=document.querySelector('[data-a3s-testkit-overlay]');if(candidate?.isConnected&&candidate.shadowRoot?.querySelector('[aria-label=\"Close review overlay\"]')){await new Promise(resolve=>requestAnimationFrame(resolve));if(candidate.isConnected&&candidate.shadowRoot?.querySelector('[aria-label=\"Close review overlay\"]')){host=candidate;break}}await new Promise(resolve=>requestAnimationFrame(resolve))}if(!host)throw new Error('stable TestKit overlay host not found');const shadow=host.shadowRoot;const waitForFocus=async className=>{for(let frame=0;frame<120;frame+=1){if(shadow.activeElement?.classList.contains(className))return frame;await new Promise(resolve=>requestAnimationFrame(resolve))}return -1};shadow.querySelector('[aria-label=\"Close review overlay\"]').click();const closeFrame=await waitForFocus('a3s-launch');shadow.querySelector('.a3s-launch').click();const openFrame=await waitForFocus('a3s-panel');return JSON.stringify({closeFocus:closeFrame>=0,openFocus:openFrame>=0,closeFrame,openFrame})})()",
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
    let before_visual_width = json_number(
        &before_zoom,
        "/page/viewport/visual/width",
        "visual viewport width before zoom",
    );
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
    let after_visual_width = json_number(
        &after_zoom,
        "/page/viewport/visual/width",
        "visual viewport width after zoom",
    );
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
        before_visual_width / 1.5,
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
        1000.0 / after_visual_width,
        0.001,
        "visual-viewport normalized target position after zoom",
    );
    set_browser_page_scale(
        String::from_utf8_lossy(&cdp_url.stdout).trim(),
        1.0,
        &command,
    );

    run_review_workflow(&command);
    exercise_review_candidate_accessibility(&command);

    click_accessible(
        &command,
        "return to new feedback for keyboard marking",
        "tab",
        "New feedback",
    );
    click_accessible(
        &command,
        "select TestKit keyboard marking mode",
        "button",
        "Mark element",
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
    assert_wcag_accessibility(&command, "audit the submitted repair state");
    exercise_repair_status_accessibility(&command);

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

    verify_hide_until_restart_focus(&command);
    verify_audit_fixture_reset(&command);

    let teardown = command(&[
        "eval",
        "window.testkitFixture.teardown(); window[Symbol.for('a3s.test.page-context')] === undefined",
    ]);
    assert_process_success("teardown TestKit fixture", &teardown);
    assert!(String::from_utf8_lossy(&teardown.stdout).contains("true"));
    let closed = cleanup.close();
    assert_process_success("close TestKit browser session", &closed);
}

#[test]
#[ignore = "requires Node esbuild and the exact standalone agent-browser 0.26.x runtime"]
fn real_agent_browser_attaches_testkit_design_reference() {
    let Some(browser) = std::env::var_os("A3S_TEST_AGENT_BROWSER").map(PathBuf::from) else {
        eprintln!("A3S_TEST_AGENT_BROWSER is not set; skipping TestKit design reference E2E");
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
    assert_process_success("probe standalone browser version", &version);
    assert!(
        String::from_utf8_lossy(&version.stdout).contains("0.26."),
        "real E2E requires the admitted 0.26.x protocol: {}",
        String::from_utf8_lossy(&version.stdout)
    );

    let (_bundle_workspace, bundle) = bundle_browser_fixture("bundle TestKit fixture");
    let fixture = start_testkit_fixture(bundle).expect("start TestKit fixture");
    let evidence_workspace = tempfile::tempdir().expect("temporary design reference evidence");
    let board_screenshot = evidence_workspace.path().join("design-reference-board.png");
    let session = format!("a3s-testkit-design-e2e-{}", std::process::id());
    let mut cleanup = StandaloneBrowserSessionCleanup::new(&browser, &session);
    let command = |arguments: &[&str]| {
        let mut command = Command::new(&browser);
        command.arg("--session").arg(&session).args(arguments);
        bounded_output(&mut command, "run standalone browser command")
    };

    let opened = command(&["open", &fixture.origin()]);
    cleanup.arm();
    assert_process_success("open TestKit fixture", &opened);
    assert_process_success(
        "set TestKit design reference viewport",
        &command(&["set", "viewport", "1280", "800", "2"]),
    );
    let overlay_ready = command(&[
        "wait",
        "--fn",
        "Boolean(window[Symbol.for('a3s.test.page-context')]&&document.querySelector('[data-a3s-testkit-overlay]')?.shadowRoot)",
    ]);
    assert_process_success("wait for TestKit bridge and overlay", &overlay_ready);

    let select_element = command(&[
        "eval",
        "(()=>{const shadow=document.querySelector('[data-a3s-testkit-overlay]').shadowRoot;[...shadow.querySelectorAll('button')].find(button=>button.textContent==='Element').click();return true})()",
    ]);
    assert_process_success("select TestKit element marking mode", &select_element);
    let marking_ready = command(&[
        "wait",
        "--fn",
        "Boolean(document.querySelector('[data-a3s-testkit-overlay]')?.shadowRoot?.querySelector('.a3s-hint'))",
    ]);
    assert_process_success("wait for TestKit element marking mode", &marking_ready);
    let keyboard_mark = command(&[
        "eval",
        "(()=>{const target=document.querySelector('#sticky');target.focus();target.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',bubbles:true}));return true})()",
    ]);
    assert_process_success("mark TestKit target by keyboard", &keyboard_mark);
    let editor_ready = command(&[
        "wait",
        "--fn",
        "Boolean(document.querySelector('[data-a3s-testkit-overlay]')?.shadowRoot?.querySelector('.a3s-editor'))",
    ]);
    assert_process_success("wait for TestKit finding editor", &editor_ready);

    let open_design_board = command(&[
        "eval",
        "(()=>{const shadow=document.querySelector('[data-a3s-testkit-overlay]').shadowRoot;[...shadow.querySelectorAll('button')].find(button=>button.textContent==='Open design board').click();return true})()",
    ]);
    assert_process_success("open TestKit design board", &open_design_board);
    let design_board_ready = command(&[
        "wait",
        "--fn",
        "document.querySelector('[data-a3s-testkit-overlay]')?.shadowRoot?.querySelector('[data-testid=\"design-canvas\"]')?.getAttribute('aria-label')==='Desired UI design canvas'",
    ]);
    assert_process_success("wait for TestKit design board", &design_board_ready);
    let design_accessibility = command(&["snapshot"]);
    assert_process_success(
        "capture TestKit design board accessibility tree",
        &design_accessibility,
    );
    let design_accessibility = String::from_utf8_lossy(&design_accessibility.stdout);
    for expected in [
        "dialog \"Design reference\"",
        "button \"Draw\"",
        "button \"Upload screenshot\"",
        "button \"Attach to finding\"",
    ] {
        assert!(
            design_accessibility.contains(expected),
            "TestKit design board accessibility tree missing {expected:?}: {design_accessibility}"
        );
    }

    let select_draw_tool = command(&[
        "eval",
        "(()=>{const shadow=document.querySelector('[data-a3s-testkit-overlay]').shadowRoot;const draw=shadow.querySelector('[data-testid=\"design-tool-draw\"]');if(!(draw instanceof HTMLButtonElement))throw new Error('design board Draw tool is unavailable');draw.click();return draw.getAttribute('aria-pressed')})()",
    ]);
    assert_process_success("select design board Pen tool", &select_draw_tool);
    let draw_design_reference = command(&[
        "eval",
        "(()=>{const canvas=document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelector('.a3s-design-canvas-surface');if(!(canvas instanceof SVGSVGElement))throw new Error('design canvas is unavailable');const rect=canvas.getBoundingClientRect();const dispatch=(type,x,y,buttons)=>canvas.dispatchEvent(new PointerEvent(type,{bubbles:true,composed:true,pointerId:1,pointerType:'mouse',isPrimary:true,button:0,buttons,clientX:rect.left+x,clientY:rect.top+y}));dispatch('pointerdown',180,150,1);dispatch('pointermove',340,220,1);dispatch('pointerup',460,300,0);return true})()",
    ]);
    assert_process_success("draw TestKit design reference", &draw_design_reference);
    let design_attach_ready = command(&[
        "wait",
        "--fn",
        "[...document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelectorAll('button')].some(button=>button.textContent==='Attach to finding'&&!button.disabled)",
    ]);
    assert_process_success(
        "wait for TestKit design reference attachment",
        &design_attach_ready,
    );
    let board_screenshot_path = board_screenshot
        .to_str()
        .expect("UTF-8 design reference screenshot path");
    let screenshot = command(&["screenshot", board_screenshot_path]);
    assert_process_success("capture TestKit design board screenshot", &screenshot);
    assert_nonempty_artifact(evidence_workspace.path(), &board_screenshot);

    let attach_design_reference = command(&[
        "eval",
        "(()=>{const shadow=document.querySelector('[data-a3s-testkit-overlay]').shadowRoot;[...shadow.querySelectorAll('button')].find(button=>button.textContent==='Attach to finding'&&!button.disabled).click();return true})()",
    ]);
    assert_process_success("attach TestKit design reference", &attach_design_reference);
    let design_reference_ready = command(&[
        "wait",
        "--fn",
        "document.querySelector('[data-a3s-testkit-overlay]')?.shadowRoot?.querySelector('.a3s-design-reference strong')?.textContent==='Sketch attached'",
    ]);
    assert_process_success(
        "wait for attached TestKit design reference",
        &design_reference_ready,
    );

    let fill_instruction = command(&[
        "eval",
        "(()=>{const textarea=document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelector('.a3s-editor textarea');const setter=Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype,'value').set;setter.call(textarea,'Match the attached design reference');textarea.dispatchEvent(new Event('input',{bubbles:true,composed:true}));return true})()",
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
        "(()=>{const shadow=document.querySelector('[data-a3s-testkit-overlay]').shadowRoot;[...shadow.querySelectorAll('button')].find(button=>button.textContent==='Send and auto-fix'&&!button.disabled).click();return true})()",
    ]);
    assert_process_success("submit TestKit design reference finding", &submit);
    let submitted = command(&[
        "wait",
        "--fn",
        "window[Symbol.for('a3s.test.page-context')].listRepairs().length===1",
    ]);
    assert_process_success("wait for TestKit design reference finding", &submitted);
    let submitted_reference = command(&[
        "eval",
        "JSON.stringify(window[Symbol.for('a3s.test.page-context')].listRepairs()[0].designReference)",
    ]);
    assert_process_success(
        "read submitted TestKit design reference",
        &submitted_reference,
    );
    let submitted_reference: String = serde_json::from_slice(&submitted_reference.stdout)
        .expect("submitted TestKit design reference JSON string");
    let submitted_reference: serde_json::Value = serde_json::from_str(&submitted_reference)
        .expect("submitted TestKit design reference JSON");
    assert_eq!(submitted_reference["kind"], "sketch");
    assert_eq!(submitted_reference["width"], 960);
    assert_eq!(submitted_reference["height"], 600);
    assert_eq!(submitted_reference["image"]["kind"], "inline");
    assert_eq!(submitted_reference["image"]["mediaType"], "image/png");
    assert!(submitted_reference["image"]["dataUrl"]
        .as_str()
        .is_some_and(
            |value| value.starts_with("data:image/png;base64,") && value.len() < 384 * 1_024
        ));

    let closed = cleanup.close();
    assert_process_success("close TestKit design reference browser session", &closed);
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
    assert_eq!(start_json["browser_containment"], "hostname_v1");
    assert_eq!(
        start_json["browser_allowed_origins"],
        serde_json::json!([fixture.origin()])
    );
    assert_eq!(start_json["browser_allowed_domains"], serde_json::json!([]));

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

        focus "display-name-selection" {{
            target = css("#display-name")
        }}

        press "selection-start" {{
            key = "Home"
        }}

        press "selection-first-character" {{
            key = "Shift+ArrowRight"
        }}

        press "selection-second-character" {{
            key = "Shift+ArrowRight"
        }}

        press "selection-third-character" {{
            key = "Shift+ArrowRight"
        }}

        insert_text "replace-current-selection" {{
            value = "Grace"
        }}

        click "submit" {{
            target = css("#submit")
        }}

        expect "submitted" {{
            text = "submitted: Grace Lovelace"
            stable_for_ms = 100
            sample_interval_ms = 25
        }}

        screenshot "form-evidence" {{
            path = "screenshots/form.png"
        }}

        click "offscreen-same-origin" {{
            target = css("#offscreen-same-origin")
        }}

        expect "next-page" {{
            text = "Same-origin navigation passed"
        }}
    }}
}}
"##
    )
}
