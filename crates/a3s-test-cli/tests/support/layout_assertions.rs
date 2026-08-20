use std::path::{Path, PathBuf};
use std::process::{Command, Output};

mod acl;

use super::assertion_stability::assert_passed_stability;
use super::browser_process::{
    assert_no_new_private_runtime_directories, assert_process_success, bounded_output,
    private_runtime_directories,
};
use super::web_fixture::WebFixture;
use acl::{layout_failure_suite, layout_success_suite};

pub(super) const RELATIONS: [(&str, &str); 17] = [
    ("above", "above"),
    ("below", "below"),
    ("left-of", "left_of"),
    ("right-of", "right_of"),
    ("contains", "contains"),
    ("inside", "inside"),
    ("overlaps", "overlaps"),
    ("not-overlapping", "not_overlapping"),
    ("aligned-left", "aligned_left"),
    ("aligned-right", "aligned_right"),
    ("aligned-top", "aligned_top"),
    ("aligned-bottom", "aligned_bottom"),
    ("aligned-center-x", "aligned_center_x"),
    ("aligned-center-y", "aligned_center_y"),
    ("same-width", "same_width"),
    ("same-height", "same_height"),
    ("same-size", "same_size"),
];

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_a3s-test"))
}

#[test]
fn hermetic_layout_assertion_acls_are_admitted() {
    let temp = tempfile::tempdir().expect("temporary layout assertion ACL workspace");
    for (name, source) in [
        (
            "layout-assertions-pass.acl",
            layout_success_suite("http://127.0.0.1:4173"),
        ),
        (
            "layout-assertions-errors.acl",
            layout_failure_suite("http://127.0.0.1:4173"),
        ),
    ] {
        let manifest = temp.path().join(name);
        std::fs::write(&manifest, source).expect("write layout assertion ACL");
        let mut command = Command::new(binary());
        command
            .args([
                "check",
                manifest.to_str().expect("UTF-8 layout ACL path"),
                "--json",
            ])
            .current_dir(temp.path());
        let output = bounded_output(&mut command, "check layout assertion ACL");
        assert_process_success(name, &output);
    }
}

#[test]
#[ignore = "requires the exact standalone agent-browser 0.26.x runtime"]
fn real_agent_browser_verifies_and_classifies_layout_assertions() {
    let Some(browser) = std::env::var_os("A3S_TEST_AGENT_BROWSER").map(PathBuf::from) else {
        eprintln!("A3S_TEST_AGENT_BROWSER is not set; skipping layout assertion E2E");
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
        "layout assertion E2E requires the admitted 0.26.x protocol: {}",
        String::from_utf8_lossy(&version.stdout)
    );

    let fixture = WebFixture::start().expect("start layout assertion Web fixture");
    let fixture_shutdown = fixture.shutdown_probe();
    let temp = tempfile::tempdir().expect("temporary layout assertion E2E workspace");
    let runtime_directories_before = private_runtime_directories();

    run_success(&browser, &fixture, temp.path());
    run_failures(&browser, &fixture, temp.path());
    assert_no_new_private_runtime_directories(&runtime_directories_before);

    drop(fixture);
    assert!(
        fixture_shutdown.is_closed(),
        "layout assertion fixture listener must be closed"
    );
}

fn run_success(browser: &Path, fixture: &WebFixture, workspace: &Path) {
    let manifest = workspace.join("layout-assertions-pass.acl");
    std::fs::write(&manifest, layout_success_suite(&fixture.origin()))
        .expect("write layout assertion success suite");
    let output = run_manifest(browser, &manifest, workspace);
    assert!(
        output.status.success(),
        "layout assertion success E2E failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("layout assertion success report JSON");
    assert_eq!(report["status"], "passed");
    let scenario = &report["scenarios"][0];
    assert_eq!(scenario["status"], "passed");
    assert!(scenario["cleanup_error"].is_null());

    for (fixture_name, relation) in RELATIONS {
        let current = step(scenario, &format!("relation-{fixture_name}"));
        assert_eq!(current["status"], "passed", "{relation}");
        assert_eq!(current["output"]["data"]["relation"], relation);
        assert_eq!(current["output"]["data"]["matched"], true);
        assert!(current["output"]["data"]["target_rect"].is_object());
        assert!(current["output"]["data"]["relative_rect"].is_object());
    }

    for id in [
        "role-locator",
        "label-locator",
        "placeholder-locator",
        "text-locator",
        "css-aria-hidden-locator",
        "shadow-semantic-locator",
        "tolerance-boundary",
    ] {
        assert_eq!(step(scenario, id)["status"], "passed", "{id}");
        assert_eq!(step(scenario, id)["output"]["data"]["matched"], true);
    }

    assert_eq!(
        step(scenario, "tolerance-boundary")["output"]["data"]["tolerance_px"],
        1
    );
    assert_passed_stability(&report, 0, "stable-above", 100, 25);
    let stable = &step(scenario, "stable-above")["output"]["data"];
    for sample in ["first", "last"] {
        assert_eq!(stable["assertion"][sample]["relation"], "above");
        assert_eq!(stable["assertion"][sample]["matched"], true);
        assert!(stable["assertion"][sample]["target_rect"].is_object());
        assert!(stable["assertion"][sample]["relative_rect"].is_object());
    }
}

fn run_failures(browser: &Path, fixture: &WebFixture, workspace: &Path) {
    let manifest = workspace.join("layout-assertions-errors.acl");
    std::fs::write(&manifest, layout_failure_suite(&fixture.origin()))
        .expect("write layout assertion failure suite");
    let output = run_manifest(browser, &manifest, workspace);
    assert_eq!(
        output.status.code(),
        Some(1),
        "layout assertion failure suite returned the wrong exit status\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
            panic!(
            "layout assertion failure E2E returned invalid JSON: {error}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        });
    assert_eq!(report["status"], "failed");
    let scenarios = report["scenarios"]
        .as_array()
        .expect("layout assertion failure scenarios");
    assert_eq!(scenarios.len(), 15);
    for (scenario_id, step_id, error_code) in [
        ("wrong-direction", "wrong-direction", "test.assert.layout"),
        (
            "wrong-containment",
            "wrong-containment",
            "test.assert.layout",
        ),
        ("wrong-overlap", "wrong-overlap", "test.assert.layout"),
        ("wrong-alignment", "wrong-alignment", "test.assert.layout"),
        ("wrong-size", "wrong-size", "test.assert.layout"),
        (
            "missing-target",
            "missing-target",
            "test.driver.web.target_not_found",
        ),
        (
            "missing-relative",
            "missing-relative",
            "test.driver.web.target_not_found",
        ),
        (
            "ambiguous-target",
            "ambiguous-target",
            "test.driver.web.target_ambiguous",
        ),
        (
            "ambiguous-relative",
            "ambiguous-relative",
            "test.driver.web.target_ambiguous",
        ),
        (
            "invalid-target",
            "invalid-target",
            "test.driver.web.target_invalid",
        ),
        (
            "invalid-relative",
            "invalid-relative",
            "test.driver.web.target_invalid",
        ),
        (
            "transient-layout",
            "transient-layout",
            "test.assert.unstable",
        ),
        (
            "semantic-hidden",
            "semantic-hidden",
            "test.driver.web.target_not_found",
        ),
        (
            "shadow-css",
            "shadow-css",
            "test.driver.web.target_not_found",
        ),
        (
            "invalid-geometry",
            "invalid-geometry",
            "test.driver.web.output_invalid",
        ),
    ] {
        let scenario = scenarios
            .iter()
            .find(|entry| entry["id"] == scenario_id)
            .unwrap_or_else(|| panic!("missing layout assertion scenario {scenario_id}"));
        assert_eq!(scenario["status"], "failed");
        assert!(scenario["cleanup_error"].is_null());
        let failed_step = step(scenario, step_id);
        assert_eq!(failed_step["status"], "failed");
        assert_eq!(failed_step["error"]["code"], error_code);
        if error_code == "test.assert.unstable" {
            assert_eq!(
                failed_step["output"]["data"]["stability"]["outcome"],
                "unstable"
            );
            assert_eq!(
                failed_step["output"]["data"]["assertion"]["first"]["matched"],
                true
            );
            assert!(failed_step["attempts"].as_u64().unwrap_or_default() >= 2);
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
                .expect("UTF-8 layout assertion manifest path"),
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
    bounded_output(&mut command, "run real browser layout assertion manifest")
}

fn step<'a>(scenario: &'a serde_json::Value, id: &str) -> &'a serde_json::Value {
    scenario["steps"]
        .as_array()
        .expect("layout assertion scenario steps")
        .iter()
        .find(|entry| entry["id"] == id)
        .unwrap_or_else(|| panic!("missing layout assertion step {id}"))
}
