use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use super::assertion_stability::assert_passed_stability;
use super::browser_process::{
    assert_no_new_private_runtime_directories, assert_process_success, private_runtime_directories,
};
use super::web_fixture::WebFixture;

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_a3s-test"))
}

#[test]
fn hermetic_control_state_acls_are_admitted() {
    let temp = tempfile::tempdir().expect("temporary control-state ACL workspace");
    for (name, source) in [
        (
            "control-state-pass.acl",
            state_suite("http://127.0.0.1:4173"),
        ),
        (
            "control-state-errors.acl",
            state_failure_suite("http://127.0.0.1:4173"),
        ),
    ] {
        let manifest = temp.path().join(name);
        std::fs::write(&manifest, source).expect("write control-state ACL");
        let output = Command::new(binary())
            .args([
                "check",
                manifest.to_str().expect("UTF-8 control-state ACL path"),
                "--json",
            ])
            .current_dir(temp.path())
            .output()
            .expect("check control-state ACL");
        assert_process_success(name, &output);
    }
}

#[test]
#[ignore = "requires the exact standalone agent-browser 0.26.x runtime"]
fn real_agent_browser_verifies_and_classifies_control_state() {
    let Some(browser) = std::env::var_os("A3S_TEST_AGENT_BROWSER").map(PathBuf::from) else {
        eprintln!("A3S_TEST_AGENT_BROWSER is not set; skipping control-state E2E");
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
        "control-state E2E requires the admitted 0.26.x protocol: {}",
        String::from_utf8_lossy(&version.stdout)
    );

    let fixture = WebFixture::start().expect("start control-state Web fixture");
    let fixture_address = fixture.address();
    let temp = tempfile::tempdir().expect("temporary control-state E2E workspace");
    let runtime_directories_before = private_runtime_directories();

    run_control_state_success(&browser, &fixture, temp.path());
    run_control_state_failures(&browser, &fixture, temp.path());
    assert_no_new_private_runtime_directories(&runtime_directories_before);

    drop(fixture);
    assert!(
        TcpStream::connect_timeout(&fixture_address, Duration::from_millis(250)).is_err(),
        "control-state fixture listener must be closed"
    );
}

fn run_control_state_success(browser: &Path, fixture: &WebFixture, workspace: &Path) {
    let manifest = workspace.join("control-state-pass.acl");
    std::fs::write(&manifest, state_suite(&fixture.origin()))
        .expect("write control-state success suite");
    let output = run_control_state_manifest(browser, &manifest, workspace);
    assert!(
        output.status.success(),
        "control-state success E2E failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("control-state success report JSON");
    assert_eq!(report["status"], "passed");
    assert_eq!(report["scenarios"][0]["status"], "passed");
    assert!(report["scenarios"][0]["cleanup_error"].is_null());
    assert_passed_stability(&report, 0, "display-name-value", 100, 25);

    let steps = report["scenarios"][0]["steps"]
        .as_array()
        .expect("control-state success steps");
    let step = |id: &str| {
        steps
            .iter()
            .find(|entry| entry["id"] == id)
            .unwrap_or_else(|| panic!("missing control-state success step {id}"))
    };
    for id in [
        "display-name-empty",
        "submit-enabled",
        "disabled-action",
        "terms-unchecked",
        "draft-selected",
        "review-unselected",
        "initial-status-values",
        "empty-status-values",
        "display-name-value",
        "terms-checked",
        "terms-unchecked-again",
        "draft-unselected",
        "review-selected",
        "published-selected",
        "selected-status-values",
    ] {
        assert_eq!(step(id)["status"], "passed", "state step {id} failed");
    }
    assert_eq!(step("display-name-empty")["output"]["data"]["actual"], "");
    assert_eq!(step("submit-enabled")["output"]["data"]["actual"], true);
    assert_eq!(step("disabled-action")["output"]["data"]["actual"], false);
    assert_eq!(step("terms-unchecked")["output"]["data"]["actual"], false);
    assert_eq!(
        step("initial-status-values")["output"]["data"]["actual"],
        serde_json::json!(["draft"])
    );
    assert_eq!(
        step("empty-status-values")["output"]["data"]["actual"],
        serde_json::json!([])
    );
    assert_eq!(
        step("selected-status-values")["output"]["data"]["actual"],
        serde_json::json!(["published", "review"])
    );
}

fn run_control_state_failures(browser: &Path, fixture: &WebFixture, workspace: &Path) {
    let manifest = workspace.join("control-state-errors.acl");
    std::fs::write(&manifest, state_failure_suite(&fixture.origin()))
        .expect("write control-state failure suite");
    let output = run_control_state_manifest(browser, &manifest, workspace);
    assert_eq!(
        output.status.code(),
        Some(1),
        "control-state failure suite returned the wrong exit status\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
            panic!(
            "control-state failure E2E returned invalid JSON: {error}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        });
    assert_eq!(report["status"], "failed");
    let scenarios = report["scenarios"]
        .as_array()
        .expect("control-state failure scenarios");
    assert_eq!(scenarios.len(), 4);
    for (scenario_id, step_id, error_code) in [
        (
            "missing-target",
            "missing-disabled",
            "test.driver.web.target_not_found",
        ),
        (
            "ambiguous-target",
            "ambiguous-checked",
            "test.driver.web.target_ambiguous",
        ),
        (
            "unsupported-state",
            "unsupported-checked",
            "test.driver.web.state_unsupported",
        ),
        (
            "product-mismatch",
            "enabled-is-not-disabled",
            "test.assert.disabled",
        ),
    ] {
        let scenario = scenarios
            .iter()
            .find(|entry| entry["id"] == scenario_id)
            .unwrap_or_else(|| panic!("missing control-state scenario {scenario_id}"));
        assert_eq!(scenario["status"], "failed");
        assert!(scenario["cleanup_error"].is_null());
        let failed_step = scenario["steps"]
            .as_array()
            .expect("control-state scenario steps")
            .iter()
            .find(|entry| entry["id"] == step_id)
            .unwrap_or_else(|| panic!("missing control-state step {step_id}"));
        assert_eq!(failed_step["status"], "failed");
        assert_eq!(failed_step["error"]["code"], error_code);
    }
}

fn run_control_state_manifest(
    browser: &Path,
    manifest: &Path,
    workspace: &Path,
) -> std::process::Output {
    Command::new(binary())
        .args([
            "run",
            manifest
                .to_str()
                .expect("UTF-8 control-state manifest path"),
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
        .current_dir(workspace)
        .output()
        .expect("run real browser control-state manifest")
}

fn state_suite(origin: &str) -> String {
    format!(
        r##"suite "web-control-state-e2e" {{
    version = 1

    scenario "control-state" {{
        name = "Verify live form state before and after actions"
        surface = "web"
        timeout_ms = 60000

        navigate "open" {{ url = "{origin}/" }}
        wait "loaded" {{ load = "domcontentloaded" }}

        expect "display-name-empty" {{
            target = label("Display name")
            value = ""
        }}
        expect "submit-enabled" {{ enabled = role("button", "Save profile") }}
        expect "disabled-action" {{ disabled = role("button", "Unavailable action") }}
        expect "terms-unchecked" {{ unchecked = label("Accept terms") }}
        expect "draft-selected" {{ selected = role("option", "Draft") }}
        expect "review-unselected" {{ unselected = css("#status option[value=review]") }}
        expect "initial-status-values" {{
            target = role("listbox", "Publication status")
            selected_values = ["draft"]
        }}
        expect "empty-status-values" {{
            target = css("#empty-status")
            selected_values = []
        }}

        fill "display-name" {{
            target = label("Display name")
            value = "Grace Lovelace"
        }}
        expect "display-name-value" {{
            target = css("#display-name")
            value = "Grace Lovelace"
            stable_for_ms = 100
            sample_interval_ms = 25
        }}

        check "accept-terms" {{ target = label("Accept terms") }}
        expect "terms-checked" {{ checked = label("Accept terms") }}
        uncheck "remove-terms" {{ target = css("#terms") }}
        expect "terms-unchecked-again" {{ unchecked = css("#terms") }}

        select "publish-status" {{
            target = css("#status")
            values = ["review", "published"]
        }}
        expect "draft-unselected" {{ unselected = css("#status option[value=draft]") }}
        expect "review-selected" {{ selected = role("option", "Review") }}
        expect "published-selected" {{ selected = css("#status option[value=published]") }}
        expect "selected-status-values" {{
            target = role("listbox", "Publication status")
            selected_values = ["published", "review"]
        }}
    }}
}}
"##
    )
}

fn state_failure_suite(origin: &str) -> String {
    format!(
        r##"suite "web-control-state-errors" {{
    version = 1

    scenario "missing-target" {{
        surface = "web"
        timeout_ms = 30000
        navigate "open" {{ url = "{origin}/" }}
        expect "missing-disabled" {{ disabled = css("#missing-control") }}
    }}

    scenario "ambiguous-target" {{
        surface = "web"
        timeout_ms = 30000
        navigate "open" {{ url = "{origin}/" }}
        expect "ambiguous-checked" {{ checked = css(".ambiguous-state") }}
    }}

    scenario "unsupported-state" {{
        surface = "web"
        timeout_ms = 30000
        navigate "open" {{ url = "{origin}/" }}
        expect "unsupported-checked" {{ checked = css("#unsupported-state") }}
    }}

    scenario "product-mismatch" {{
        surface = "web"
        timeout_ms = 30000
        navigate "open" {{ url = "{origin}/" }}
        expect "enabled-is-not-disabled" {{ disabled = role("button", "Save profile") }}
    }}
}}
"##
    )
}
