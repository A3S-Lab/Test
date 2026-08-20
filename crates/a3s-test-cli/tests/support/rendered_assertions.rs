use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use super::assertion_stability::assert_passed_stability;
use super::browser_process::{
    assert_no_new_private_runtime_directories, assert_process_success, private_runtime_directories,
};
use super::web_fixture::WebFixture;

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_a3s-test"))
}

#[test]
fn hermetic_rendered_assertion_acls_are_admitted() {
    let temp = tempfile::tempdir().expect("temporary rendered assertion ACL workspace");
    for (name, source) in [
        (
            "rendered-assertions-pass.acl",
            rendered_success_suite("http://127.0.0.1:4173"),
        ),
        (
            "rendered-assertions-errors.acl",
            rendered_failure_suite("http://127.0.0.1:4173"),
        ),
    ] {
        let manifest = temp.path().join(name);
        std::fs::write(&manifest, source).expect("write rendered assertion ACL");
        let output = Command::new(binary())
            .args([
                "check",
                manifest.to_str().expect("UTF-8 rendered ACL path"),
                "--json",
            ])
            .current_dir(temp.path())
            .output()
            .expect("check rendered assertion ACL");
        assert_process_success(name, &output);
    }
}

#[test]
#[ignore = "requires the exact standalone agent-browser 0.26.x runtime"]
fn real_agent_browser_verifies_and_classifies_rendered_assertions() {
    let Some(browser) = std::env::var_os("A3S_TEST_AGENT_BROWSER").map(PathBuf::from) else {
        eprintln!("A3S_TEST_AGENT_BROWSER is not set; skipping rendered assertion E2E");
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
        "rendered assertion E2E requires the admitted 0.26.x protocol: {}",
        String::from_utf8_lossy(&version.stdout)
    );

    let fixture = WebFixture::start().expect("start rendered assertion Web fixture");
    let fixture_shutdown = fixture.shutdown_probe();
    let temp = tempfile::tempdir().expect("temporary rendered assertion E2E workspace");
    let runtime_directories_before = private_runtime_directories();

    run_success(&browser, &fixture, temp.path());
    run_failures(&browser, &fixture, temp.path());
    assert_no_new_private_runtime_directories(&runtime_directories_before);

    drop(fixture);
    assert!(
        fixture_shutdown.is_closed(),
        "rendered assertion fixture listener must be closed"
    );
}

fn run_success(browser: &Path, fixture: &WebFixture, workspace: &Path) {
    let manifest = workspace.join("rendered-assertions-pass.acl");
    std::fs::write(&manifest, rendered_success_suite(&fixture.origin()))
        .expect("write rendered assertion success suite");
    let output = run_manifest(browser, &manifest, workspace);
    assert!(
        output.status.success(),
        "rendered assertion success E2E failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("rendered assertion success report JSON");
    assert_eq!(report["status"], "passed");
    assert_eq!(report["scenarios"][0]["status"], "passed");
    assert!(report["scenarios"][0]["cleanup_error"].is_null());
    assert_passed_stability(&report, 0, "total-copy", 100, 25);
    assert_passed_stability(&report, 0, "line-items", 100, 25);
    assert_passed_stability(&report, 0, "visible-catalog-rows", 100, 25);

    let scenario = &report["scenarios"][0];
    assert_eq!(
        step(scenario, "total-copy")["output"]["data"]["assertion"]["last"]["actual"],
        "Total $42.00"
    );
    assert_eq!(
        step(scenario, "line-items")["output"]["data"]["assertion"]["last"]["actual"],
        serde_json::json!(["Keyboard × 1", "Mouse × 2", "Shipping", "Shipping"])
    );
    assert_eq!(
        step(scenario, "no-line-items")["output"]["data"]["actual"],
        serde_json::json!([])
    );
    assert_eq!(
        step(scenario, "visible-catalog-rows")["output"]["data"]["assertion"]["last"]["actual"],
        4
    );
    assert_eq!(
        step(scenario, "css-catalog-copy")["output"]["data"]["actual"],
        serde_json::json!(["Alpha", "Beta", "Gamma", "Decorative visual row"])
    );
    assert_eq!(
        step(scenario, "semantic-hidden-copy")["output"]["data"]["actual"],
        serde_json::json!([])
    );
    assert_eq!(
        step(scenario, "hidden-css-rows")["output"]["data"]["actual"],
        0
    );
    assert_eq!(
        step(scenario, "aria-hidden-semantic-row")["output"]["data"]["actual"],
        0
    );
    assert_eq!(
        step(scenario, "shadow-total")["output"]["data"]["actual"],
        "Shadow total $7.00"
    );
    assert_eq!(
        step(scenario, "shadow-button-count")["output"]["data"]["actual"],
        1
    );
    assert_eq!(
        step(scenario, "shadow-lines")["output"]["data"]["actual"],
        serde_json::json!(["Shadow A", "Shadow B"])
    );
    assert_eq!(
        step(scenario, "no-visible-errors")["output"]["data"]["actual"],
        0
    );
}

fn run_failures(browser: &Path, fixture: &WebFixture, workspace: &Path) {
    let manifest = workspace.join("rendered-assertions-errors.acl");
    std::fs::write(&manifest, rendered_failure_suite(&fixture.origin()))
        .expect("write rendered assertion failure suite");
    let output = run_manifest(browser, &manifest, workspace);
    assert_eq!(
        output.status.code(),
        Some(1),
        "rendered assertion failure suite returned the wrong exit status\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
            panic!(
                "rendered assertion failure E2E returned invalid JSON: {error}\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )
        });
    assert_eq!(report["status"], "failed");
    let scenarios = report["scenarios"]
        .as_array()
        .expect("rendered assertion failure scenarios");
    assert_eq!(scenarios.len(), 12);
    for (scenario_id, step_id, error_code) in [
        (
            "rendered-mismatch",
            "wrong-total",
            "test.assert.rendered_text",
        ),
        (
            "missing-target",
            "missing-copy",
            "test.driver.web.target_not_found",
        ),
        (
            "ambiguous-target",
            "duplicate-copy",
            "test.driver.web.target_ambiguous",
        ),
        (
            "count-mismatch",
            "wrong-row-count",
            "test.assert.visible_count",
        ),
        (
            "sequence-reordered",
            "wrong-line-order",
            "test.assert.rendered_texts",
        ),
        (
            "sequence-duplicate-mismatch",
            "missing-shipping-duplicate",
            "test.assert.rendered_texts",
        ),
        (
            "sequence-empty-mismatch",
            "missing-lines-expected",
            "test.assert.rendered_texts",
        ),
        (
            "sequence-invalid-selector",
            "invalid-sequence-target",
            "test.driver.web.target_invalid",
        ),
        (
            "invalid-selector",
            "invalid-count-target",
            "test.driver.web.target_invalid",
        ),
        (
            "transient-text",
            "stable-transient-copy",
            "test.assert.unstable",
        ),
        (
            "transient-count",
            "stable-transient-count",
            "test.assert.unstable",
        ),
        (
            "transient-sequence",
            "stable-transient-sequence",
            "test.assert.unstable",
        ),
    ] {
        let scenario = scenarios
            .iter()
            .find(|entry| entry["id"] == scenario_id)
            .unwrap_or_else(|| panic!("missing rendered assertion scenario {scenario_id}"));
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
            assert!(failed_step["attempts"].as_u64().unwrap_or_default() >= 2);
        }
    }
}

fn run_manifest(browser: &Path, manifest: &Path, workspace: &Path) -> Output {
    Command::new(binary())
        .args([
            "run",
            manifest
                .to_str()
                .expect("UTF-8 rendered assertion manifest path"),
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
        .expect("run rendered assertion manifest")
}

fn step<'a>(scenario: &'a serde_json::Value, id: &str) -> &'a serde_json::Value {
    scenario["steps"]
        .as_array()
        .expect("rendered assertion scenario steps")
        .iter()
        .find(|entry| entry["id"] == id)
        .unwrap_or_else(|| panic!("missing rendered assertion step {id}"))
}

fn rendered_success_suite(origin: &str) -> String {
    format!(
        r##"suite "web-rendered-assertions" {{
    version = 1

    scenario "rendered-state" {{
        name = "Verify rendered copy, ordered collections, and visible locator cardinality"
        surface = "web"
        timeout_ms = 60000

        navigate "open" {{ url = "{origin}/rendered.html" }}
        wait "loaded" {{ load = "domcontentloaded" }}

        expect "total-copy" {{
            target = testid("total-copy")
            rendered_text = "Total $42.00"
            stable_for_ms = 100
            sample_interval_ms = 25
        }}
        expect "line-items" {{
            target = css("[data-line-item]")
            rendered_texts = ["Keyboard × 1", "Mouse × 2", "Shipping", "Shipping"]
            stable_for_ms = 100
            sample_interval_ms = 25
        }}
        expect "no-line-items" {{
            target = css("[data-missing-line-item]")
            rendered_texts = []
        }}
        expect "visible-catalog-rows" {{
            target = css("[data-row]")
            visible_count = 4
            stable_for_ms = 100
            sample_interval_ms = 25
        }}
        expect "css-catalog-copy" {{
            target = css("[data-row]")
            rendered_texts = ["Alpha", "Beta", "Gamma", "Decorative visual row"]
        }}
        expect "semantic-hidden-copy" {{
            target = testid("decorative-row")
            rendered_texts = []
        }}
        expect "hidden-css-rows" {{
            target = css("[data-row]:not([data-row=alpha]):not([data-row=beta]):not([data-row=gamma]):not([data-row=decorative])")
            visible_count = 0
        }}
        expect "aria-hidden-semantic-row" {{
            target = testid("decorative-row")
            visible_count = 0
        }}
        expect "shadow-total" {{
            target = testid("shadow-total")
            rendered_text = "Shadow total $7.00"
        }}
        expect "shadow-button-count" {{
            target = role("button", "Shadow checkout")
            visible_count = 1
        }}
        expect "shadow-lines" {{
            target = testid("shadow-line-item")
            rendered_texts = ["Shadow A", "Shadow B"]
        }}
        expect "no-visible-errors" {{
            target = role("alert", "Checkout error")
            visible_count = 0
        }}
    }}
}}
"##
    )
}

fn rendered_failure_suite(origin: &str) -> String {
    format!(
        r##"suite "web-rendered-assertion-errors" {{
    version = 1

    scenario "rendered-mismatch" {{
        surface = "web"
        timeout_ms = 30000
        navigate "open" {{ url = "{origin}/rendered.html" }}
        expect "wrong-total" {{ target = testid("total-copy") rendered_text = "Total $41.00" }}
    }}

    scenario "missing-target" {{
        surface = "web"
        timeout_ms = 30000
        navigate "open" {{ url = "{origin}/rendered.html" }}
        expect "missing-copy" {{ target = testid("missing-copy") rendered_text = "Missing" }}
    }}

    scenario "ambiguous-target" {{
        surface = "web"
        timeout_ms = 30000
        navigate "open" {{ url = "{origin}/rendered.html" }}
        expect "duplicate-copy" {{ target = css(".duplicate-copy") rendered_text = "Duplicate copy" }}
    }}

    scenario "count-mismatch" {{
        surface = "web"
        timeout_ms = 30000
        navigate "open" {{ url = "{origin}/rendered.html" }}
        expect "wrong-row-count" {{ target = css("[data-row]") visible_count = 3 }}
    }}

    scenario "sequence-reordered" {{
        surface = "web"
        timeout_ms = 30000
        navigate "open" {{ url = "{origin}/rendered.html" }}
        expect "wrong-line-order" {{
            target = css("[data-line-item]")
            rendered_texts = ["Mouse × 2", "Keyboard × 1", "Shipping", "Shipping"]
        }}
    }}

    scenario "sequence-duplicate-mismatch" {{
        surface = "web"
        timeout_ms = 30000
        navigate "open" {{ url = "{origin}/rendered.html" }}
        expect "missing-shipping-duplicate" {{
            target = css("[data-line-item]")
            rendered_texts = ["Keyboard × 1", "Mouse × 2", "Shipping"]
        }}
    }}

    scenario "sequence-empty-mismatch" {{
        surface = "web"
        timeout_ms = 30000
        navigate "open" {{ url = "{origin}/rendered.html" }}
        expect "missing-lines-expected" {{
            target = css("[data-missing-line-item]")
            rendered_texts = ["Expected"]
        }}
    }}

    scenario "sequence-invalid-selector" {{
        surface = "web"
        timeout_ms = 30000
        navigate "open" {{ url = "{origin}/rendered.html" }}
        expect "invalid-sequence-target" {{ target = css(":not(") rendered_texts = [] }}
    }}

    scenario "invalid-selector" {{
        surface = "web"
        timeout_ms = 30000
        navigate "open" {{ url = "{origin}/rendered.html" }}
        expect "invalid-count-target" {{ target = css(":not(") visible_count = 0 }}
    }}

    scenario "transient-text" {{
        surface = "web"
        timeout_ms = 30000
        navigate "open" {{ url = "{origin}/rendered.html" }}
        expect "stable-transient-copy" {{
            target = testid("transient-copy")
            rendered_text = "Ready"
            stable_for_ms = 100
            sample_interval_ms = 25
        }}
    }}

    scenario "transient-count" {{
        surface = "web"
        timeout_ms = 30000
        navigate "open" {{ url = "{origin}/rendered.html" }}
        expect "stable-transient-count" {{
            target = css("[data-transient-row]")
            visible_count = 2
            stable_for_ms = 100
            sample_interval_ms = 25
        }}
    }}

    scenario "transient-sequence" {{
        surface = "web"
        timeout_ms = 30000
        navigate "open" {{ url = "{origin}/rendered.html" }}
        expect "stable-transient-sequence" {{
            target = css("[data-transient-line-item]")
            rendered_texts = ["Queued A", "Queued B"]
            stable_for_ms = 100
            sample_interval_ms = 25
        }}
    }}
}}
"##
    )
}
