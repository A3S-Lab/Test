use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Duration;

mod acl;

use super::assertion_stability::assert_passed_stability;
use super::browser_process::{
    assert_no_new_private_runtime_directories, assert_process_success, bounded_output,
    private_runtime_directories,
};
use super::web_fixture::WebFixture;
use acl::{interactability_failure_suite, interactability_success_suite};

const VIEWPORT_STEPS: [&str; 8] = [
    "viewport-testid",
    "viewport-partial",
    "viewport-role",
    "viewport-label",
    "viewport-placeholder",
    "viewport-text",
    "viewport-css-aria-hidden",
    "viewport-shadow",
];
const POINTER_STEPS: [&str; 10] = [
    "pointer-testid",
    "pointer-role",
    "pointer-label",
    "pointer-placeholder",
    "pointer-text",
    "pointer-css-aria-hidden",
    "pointer-shadow",
    "pointer-child",
    "pointer-partial-cover",
    "pointer-pass-through",
];

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_a3s-test"))
}

#[test]
fn hermetic_interactability_assertion_acls_are_admitted() {
    let temp = tempfile::tempdir().expect("temporary interactability ACL workspace");
    for (name, source) in [
        (
            "interactability-assertions-pass.acl",
            interactability_success_suite("http://127.0.0.1:4173"),
        ),
        (
            "interactability-assertions-errors.acl",
            interactability_failure_suite("http://127.0.0.1:4173"),
        ),
    ] {
        let manifest = temp.path().join(name);
        std::fs::write(&manifest, source).expect("write interactability assertion ACL");
        let mut command = Command::new(binary());
        command
            .args([
                "check",
                manifest.to_str().expect("UTF-8 interactability ACL path"),
                "--json",
            ])
            .current_dir(temp.path());
        let output = bounded_output(&mut command, "check interactability assertion ACL");
        assert_process_success(name, &output);
    }
}

#[test]
#[ignore = "requires the exact standalone agent-browser 0.26.x runtime"]
fn real_agent_browser_verifies_and_classifies_interactability_assertions() {
    let Some(browser) = std::env::var_os("A3S_TEST_AGENT_BROWSER").map(PathBuf::from) else {
        eprintln!("A3S_TEST_AGENT_BROWSER is not set; skipping interactability assertion E2E");
        return;
    };
    assert!(
        browser.is_file(),
        "browser executable does not exist: {browser:?}"
    );
    let mut version = Command::new(&browser);
    version.arg("--version");
    let version = bounded_output(&mut version, "probe standalone browser version");
    assert_process_success("probe standalone browser version", &version);
    assert!(
        String::from_utf8_lossy(&version.stdout).contains("0.26."),
        "interactability E2E requires the admitted 0.26.x protocol: {}",
        String::from_utf8_lossy(&version.stdout)
    );

    let fixture = WebFixture::start().expect("start interactability Web fixture");
    let fixture_address = fixture.address();
    let temp = tempfile::tempdir().expect("temporary interactability E2E workspace");
    let runtime_directories_before = private_runtime_directories();

    run_success(&browser, &fixture, temp.path());
    run_failures(&browser, &fixture, temp.path());
    assert_no_new_private_runtime_directories(&runtime_directories_before);

    drop(fixture);
    assert!(
        TcpStream::connect_timeout(&fixture_address, Duration::from_millis(250)).is_err(),
        "interactability fixture listener must be closed"
    );
}

fn run_success(browser: &Path, fixture: &WebFixture, workspace: &Path) {
    let manifest = workspace.join("interactability-assertions-pass.acl");
    std::fs::write(&manifest, interactability_success_suite(&fixture.origin()))
        .expect("write interactability assertion success suite");
    let output = run_manifest(browser, &manifest, workspace);
    assert!(
        output.status.success(),
        "interactability success E2E failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("interactability assertion success report JSON");
    assert_eq!(report["status"], "passed");
    let scenario = &report["scenarios"][0];
    assert_eq!(scenario["status"], "passed");
    assert!(scenario["cleanup_error"].is_null());

    for id in VIEWPORT_STEPS {
        let data = &step(scenario, id)["output"]["data"];
        assert_eq!(step(scenario, id)["status"], "passed", "{id}");
        assert_eq!(data["in_viewport"], true, "{id}");
        assert!(data["target_rect"].is_object(), "{id}");
        assert!(data["viewport_rect"].is_object(), "{id}");
        assert!(data["intersection_ratio"].as_f64().unwrap_or_default() > 0.0);
    }
    let partial_ratio = step(scenario, "viewport-partial")["output"]["data"]["intersection_ratio"]
        .as_f64()
        .expect("partial viewport ratio");
    assert!(partial_ratio > 0.0 && partial_ratio < 1.0);

    for id in POINTER_STEPS {
        let data = &step(scenario, id)["output"]["data"];
        assert_eq!(step(scenario, id)["status"], "passed", "{id}");
        assert_eq!(data["pointer_reachable"], true, "{id}");
        assert_eq!(data["sample_count"], 9, "{id}");
        assert!(data["reachable_samples"].as_u64().unwrap_or_default() > 0);
        assert_eq!(data["samples"].as_array().map(Vec::len), Some(9));
    }
    assert_eq!(
        step(scenario, "pointer-partial-cover")["output"]["data"]["reachable_samples"],
        3
    );
    assert_eq!(
        step(scenario, "pointer-pass-through")["output"]["data"]["reachable_samples"],
        9
    );

    for id in ["stable-viewport", "stable-pointer"] {
        assert_passed_stability(&report, 0, id, 100, 25);
        let data = &step(scenario, id)["output"]["data"];
        assert!(data["assertion"]["first"]["target_rect"].is_object());
        assert!(data["assertion"]["last"]["target_rect"].is_object());
        assert!(data["assertion"]["first"]["viewport_rect"].is_object());
        assert!(data["assertion"]["last"]["viewport_rect"].is_object());
    }
}

fn run_failures(browser: &Path, fixture: &WebFixture, workspace: &Path) {
    let manifest = workspace.join("interactability-assertions-errors.acl");
    std::fs::write(&manifest, interactability_failure_suite(&fixture.origin()))
        .expect("write interactability assertion failure suite");
    let output = run_manifest(browser, &manifest, workspace);
    assert_eq!(
        output.status.code(),
        Some(1),
        "interactability failure suite returned the wrong exit status\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
            panic!(
            "interactability failure E2E returned invalid JSON: {error}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        });
    assert_eq!(report["status"], "failed");
    let scenarios = report["scenarios"]
        .as_array()
        .expect("interactability failure scenarios");
    assert_eq!(scenarios.len(), 15);
    for (scenario_id, error_code) in [
        ("offscreen-viewport", "test.assert.in_viewport"),
        ("offscreen-pointer", "test.assert.pointer_reachable"),
        ("covered-pointer", "test.assert.pointer_reachable"),
        ("transparent-cover-pointer", "test.assert.pointer_reachable"),
        (
            "pointer-events-none-target",
            "test.assert.pointer_reachable",
        ),
        ("missing-viewport", "test.driver.web.target_not_found"),
        ("missing-pointer", "test.driver.web.target_not_found"),
        ("ambiguous-viewport", "test.driver.web.target_ambiguous"),
        ("invalid-pointer-selector", "test.driver.web.target_invalid"),
        ("semantic-hidden", "test.driver.web.target_not_found"),
        ("shadow-css", "test.driver.web.target_not_found"),
        (
            "invalid-viewport-geometry",
            "test.driver.web.output_invalid",
        ),
        ("invalid-pointer-geometry", "test.driver.web.output_invalid"),
        ("transient-viewport", "test.assert.unstable"),
        ("transient-pointer", "test.assert.unstable"),
    ] {
        let scenario = scenarios
            .iter()
            .find(|entry| entry["id"] == scenario_id)
            .unwrap_or_else(|| panic!("missing interactability scenario {scenario_id}"));
        assert_eq!(scenario["status"], "failed");
        assert!(scenario["cleanup_error"].is_null());
        let failed_step = step(scenario, scenario_id);
        assert_eq!(failed_step["status"], "failed");
        assert_eq!(failed_step["error"]["code"], error_code);
        if error_code == "test.assert.unstable" {
            assert_eq!(
                failed_step["output"]["data"]["stability"]["outcome"],
                "unstable"
            );
            assert!(failed_step["attempts"].as_u64().unwrap_or_default() >= 2);
            assert!(failed_step["output"]["data"]["assertion"]["first"]["target_rect"].is_object());
        }
    }
}

fn run_manifest(browser: &Path, manifest: &Path, workspace: &Path) -> Output {
    let mut command = Command::new(binary());
    command
        .args([
            "run",
            manifest
                .to_str()
                .expect("UTF-8 interactability assertion manifest path"),
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
        .current_dir(workspace);
    bounded_output(
        &mut command,
        "run real browser interactability assertion manifest",
    )
}

fn step<'a>(scenario: &'a serde_json::Value, id: &str) -> &'a serde_json::Value {
    scenario["steps"]
        .as_array()
        .expect("interactability assertion scenario steps")
        .iter()
        .find(|entry| entry["id"] == id)
        .unwrap_or_else(|| panic!("missing interactability assertion step {id}"))
}
