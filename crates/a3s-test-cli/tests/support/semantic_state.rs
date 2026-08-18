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
use acl::{semantic_state_failure_suite, semantic_state_success_suite};

const SUCCESS_STEPS: [(&str, &str, bool); 26] = [
    ("native-expanded", "expanded", true),
    ("native-collapsed", "expanded", false),
    ("aria-expanded", "expanded", true),
    ("aria-collapsed", "expanded", false),
    ("shadow-collapsed", "expanded", false),
    ("hidden-css-expanded", "expanded", true),
    ("pressed", "pressed", true),
    ("unpressed", "pressed", false),
    ("shadow-pressed", "pressed", true),
    ("readonly", "readonly", true),
    ("writable", "readonly", false),
    ("aria-readonly", "readonly", true),
    ("aria-writable", "readonly", false),
    ("disabled-writable", "readonly", false),
    ("shadow-readonly", "readonly", true),
    ("required", "required", true),
    ("optional", "required", false),
    ("aria-required", "required", true),
    ("aria-optional", "required", false),
    ("shadow-required", "required", true),
    ("invalid", "invalid", true),
    ("valid", "invalid", false),
    ("aria-invalid", "invalid", true),
    ("aria-valid", "invalid", false),
    ("grammar-invalid", "invalid", true),
    ("shadow-spelling-invalid", "invalid", true),
];

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_a3s-test"))
}

#[test]
fn hermetic_semantic_state_acls_are_admitted() {
    let temp = tempfile::tempdir().expect("temporary semantic state ACL workspace");
    for (name, source) in [
        (
            "semantic-state-pass.acl",
            semantic_state_success_suite("http://127.0.0.1:4173"),
        ),
        (
            "semantic-state-errors.acl",
            semantic_state_failure_suite("http://127.0.0.1:4173"),
        ),
    ] {
        let manifest = temp.path().join(name);
        std::fs::write(&manifest, source).expect("write semantic state ACL");
        let mut command = Command::new(binary());
        command
            .args([
                "check",
                manifest.to_str().expect("UTF-8 semantic state ACL path"),
                "--json",
            ])
            .current_dir(temp.path());
        let output = bounded_output(&mut command, "check semantic state ACL");
        assert_process_success(name, &output);
    }
}

#[test]
#[ignore = "requires the exact standalone agent-browser 0.26.x runtime"]
fn real_agent_browser_verifies_and_classifies_semantic_state() {
    let Some(browser) = std::env::var_os("A3S_TEST_AGENT_BROWSER").map(PathBuf::from) else {
        eprintln!("A3S_TEST_AGENT_BROWSER is not set; skipping semantic state E2E");
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
        "semantic state E2E requires the admitted 0.26.x protocol: {}",
        String::from_utf8_lossy(&version.stdout)
    );

    let fixture = WebFixture::start().expect("start semantic state Web fixture");
    let fixture_address = fixture.address();
    let temp = tempfile::tempdir().expect("temporary semantic state E2E workspace");
    let runtime_directories_before = private_runtime_directories();

    run_success(&browser, &fixture, temp.path());
    run_failures(&browser, &fixture, temp.path());
    assert_no_new_private_runtime_directories(&runtime_directories_before);

    drop(fixture);
    assert!(
        TcpStream::connect_timeout(&fixture_address, Duration::from_millis(250)).is_err(),
        "semantic state fixture listener must be closed"
    );
}

fn run_success(browser: &Path, fixture: &WebFixture, workspace: &Path) {
    let manifest = workspace.join("semantic-state-pass.acl");
    std::fs::write(&manifest, semantic_state_success_suite(&fixture.origin()))
        .expect("write semantic state success suite");
    let output = run_manifest(browser, &manifest, workspace);
    assert!(
        output.status.success(),
        "semantic state success E2E failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("semantic state success report JSON");
    assert_eq!(report["status"], "passed");
    let scenario = &report["scenarios"][0];
    assert_eq!(scenario["status"], "passed");
    assert!(scenario["cleanup_error"].is_null());

    for (id, state, actual) in SUCCESS_STEPS {
        let entry = step(scenario, id);
        assert_eq!(entry["status"], "passed", "semantic state step {id}");
        assert_eq!(entry["output"]["data"]["state"], state, "{id}");
        assert_eq!(entry["output"]["data"]["actual"], actual, "{id}");
        assert_eq!(entry["output"]["data"]["expected"], actual, "{id}");
    }

    assert_passed_stability(&report, 0, "stable-expanded", 100, 25);
    let stable = &step(scenario, "stable-expanded")["output"]["data"];
    assert_eq!(stable["assertion"]["first"]["state"], "expanded");
    assert_eq!(stable["assertion"]["first"]["actual"], true);
    assert_eq!(stable["assertion"]["last"]["actual"], true);
}

fn run_failures(browser: &Path, fixture: &WebFixture, workspace: &Path) {
    let manifest = workspace.join("semantic-state-errors.acl");
    std::fs::write(&manifest, semantic_state_failure_suite(&fixture.origin()))
        .expect("write semantic state failure suite");
    let output = run_manifest(browser, &manifest, workspace);
    assert_eq!(
        output.status.code(),
        Some(1),
        "semantic state failure suite returned the wrong exit status\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
            panic!(
                "semantic state failure E2E returned invalid JSON: {error}\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )
        });
    assert_eq!(report["status"], "failed");
    let scenarios = report["scenarios"]
        .as_array()
        .expect("semantic state failure scenarios");
    assert_eq!(scenarios.len(), 17);
    for (scenario_id, error_code) in [
        ("missing-collapsed", "test.driver.web.target_not_found"),
        ("ambiguous-expanded", "test.driver.web.target_ambiguous"),
        ("expanded-mismatch", "test.assert.expanded"),
        ("collapsed-mismatch", "test.assert.collapsed"),
        ("pressed-mismatch", "test.assert.pressed"),
        ("unpressed-mismatch", "test.assert.unpressed"),
        ("writable-mismatch", "test.assert.writable"),
        ("optional-mismatch", "test.assert.optional"),
        ("valid-mismatch", "test.assert.valid"),
        ("mixed-pressed", "test.driver.web.state_unsupported"),
        ("unsupported-expanded", "test.driver.web.state_unsupported"),
        ("unsupported-readonly", "test.driver.web.state_unsupported"),
        ("unsupported-invalid", "test.driver.web.state_unsupported"),
        ("invalid-aria-token", "test.driver.web.state_unsupported"),
        ("invalid-selector", "test.driver.web.target_invalid"),
        ("hidden-semantic", "test.driver.web.target_not_found"),
        ("transient-expanded", "test.assert.unstable"),
    ] {
        let scenario = scenarios
            .iter()
            .find(|entry| entry["id"] == scenario_id)
            .unwrap_or_else(|| panic!("missing semantic state scenario {scenario_id}"));
        assert_eq!(scenario["status"], "failed");
        assert!(scenario["cleanup_error"].is_null());
        let failed_step = step(scenario, scenario_id);
        assert_eq!(failed_step["status"], "failed");
        assert_eq!(failed_step["error"]["code"], error_code);
        if scenario_id == "transient-expanded" {
            assert_eq!(
                failed_step["output"]["data"]["stability"]["outcome"],
                "unstable"
            );
            assert_eq!(
                failed_step["output"]["data"]["assertion"]["first"]["actual"],
                true
            );
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
                .expect("UTF-8 semantic state manifest path"),
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
    bounded_output(&mut command, "run real browser semantic state manifest")
}

fn step<'a>(scenario: &'a serde_json::Value, id: &str) -> &'a serde_json::Value {
    scenario["steps"]
        .as_array()
        .expect("semantic state scenario steps")
        .iter()
        .find(|entry| entry["id"] == id)
        .unwrap_or_else(|| panic!("missing semantic state step {id}"))
}
