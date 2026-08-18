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
fn hermetic_focus_ownership_acls_are_admitted() {
    let temp = tempfile::tempdir().expect("temporary focus ownership ACL workspace");
    for (name, source) in [
        (
            "focus-ownership-pass.acl",
            focus_suite("http://127.0.0.1:4173"),
        ),
        (
            "focus-ownership-errors.acl",
            focus_failure_suite("http://127.0.0.1:4173"),
        ),
    ] {
        let manifest = temp.path().join(name);
        std::fs::write(&manifest, source).expect("write focus ownership ACL");
        let output = Command::new(binary())
            .args([
                "check",
                manifest.to_str().expect("UTF-8 focus ownership ACL path"),
                "--json",
            ])
            .current_dir(temp.path())
            .output()
            .expect("check focus ownership ACL");
        assert_process_success(name, &output);
    }
}

#[test]
#[ignore = "requires the exact standalone agent-browser 0.26.x runtime"]
fn real_agent_browser_verifies_and_classifies_focus_ownership() {
    let Some(browser) = std::env::var_os("A3S_TEST_AGENT_BROWSER").map(PathBuf::from) else {
        eprintln!("A3S_TEST_AGENT_BROWSER is not set; skipping focus ownership E2E");
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
        "focus ownership E2E requires the admitted 0.26.x protocol: {}",
        String::from_utf8_lossy(&version.stdout)
    );

    let fixture = WebFixture::start().expect("start focus ownership Web fixture");
    let fixture_address = fixture.address();
    let temp = tempfile::tempdir().expect("temporary focus ownership E2E workspace");
    let runtime_directories_before = private_runtime_directories();

    run_focus_success(&browser, &fixture, temp.path());
    run_focus_failures(&browser, &fixture, temp.path());
    assert_no_new_private_runtime_directories(&runtime_directories_before);

    drop(fixture);
    assert!(
        TcpStream::connect_timeout(&fixture_address, Duration::from_millis(250)).is_err(),
        "focus ownership fixture listener must be closed"
    );
}

fn run_focus_success(browser: &Path, fixture: &WebFixture, workspace: &Path) {
    let manifest = workspace.join("focus-ownership-pass.acl");
    std::fs::write(&manifest, focus_suite(&fixture.origin()))
        .expect("write focus ownership success suite");
    let output = run_focus_manifest(browser, &manifest, workspace);
    assert!(
        output.status.success(),
        "focus ownership success E2E failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("focus ownership success report JSON");
    assert_eq!(report["status"], "passed");
    assert_eq!(report["scenarios"][0]["status"], "passed");
    assert!(report["scenarios"][0]["cleanup_error"].is_null());
    assert_passed_stability(&report, 0, "shadow-scope-within", 100, 25);

    let steps = report["scenarios"][0]["steps"]
        .as_array()
        .expect("focus ownership success steps");
    let step = |id: &str| {
        steps
            .iter()
            .find(|entry| entry["id"] == id)
            .unwrap_or_else(|| panic!("missing focus ownership success step {id}"))
    };
    for id in [
        "before-focused",
        "after-unfocused",
        "lane-within-before",
        "profile-outside-before",
        "shadow-focused",
        "shadow-scope-within",
        "before-unfocused-after-tab",
        "lane-within-shadow",
        "after-focused",
        "shadow-scope-outside",
        "lane-within-after",
        "shadow-focused-reverse",
        "before-focused-reverse",
        "outside-focused",
        "lane-outside-final",
        "slotted-focused",
        "slotted-shadow-scope-within",
    ] {
        assert_eq!(step(id)["status"], "passed", "focus step {id} failed");
    }
    assert_eq!(step("before-focused")["output"]["data"]["state"], "focused");
    assert_eq!(
        step("shadow-scope-within")["output"]["data"]["assertion"]["first"]["state"],
        "focus_within"
    );
    assert_eq!(
        step("lane-outside-final")["output"]["data"]["actual"],
        false
    );
}

fn run_focus_failures(browser: &Path, fixture: &WebFixture, workspace: &Path) {
    let manifest = workspace.join("focus-ownership-errors.acl");
    std::fs::write(&manifest, focus_failure_suite(&fixture.origin()))
        .expect("write focus ownership failure suite");
    let output = run_focus_manifest(browser, &manifest, workspace);
    assert_eq!(
        output.status.code(),
        Some(1),
        "focus ownership failure suite returned the wrong exit status\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
            panic!(
                "focus ownership failure E2E returned invalid JSON: {error}\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )
        });
    assert_eq!(report["status"], "failed");
    let scenarios = report["scenarios"]
        .as_array()
        .expect("focus ownership failure scenarios");
    assert_eq!(scenarios.len(), 11);
    for (scenario_id, step_id, error_code) in [
        (
            "missing-target",
            "missing-unfocused",
            "test.driver.web.target_not_found",
        ),
        (
            "ambiguous-target",
            "ambiguous-focused",
            "test.driver.web.target_ambiguous",
        ),
        (
            "exact-focus-mismatch",
            "before-is-not-focused",
            "test.assert.focused",
        ),
        (
            "negative-focus-mismatch",
            "after-is-not-unfocused",
            "test.assert.unfocused",
        ),
        (
            "within-mismatch",
            "lane-does-not-own-outside",
            "test.assert.focus_within",
        ),
        (
            "outside-mismatch",
            "lane-is-not-outside",
            "test.assert.focus_outside",
        ),
        (
            "invalid-target",
            "invalid-focused-selector",
            "test.driver.web.target_invalid",
        ),
        (
            "transient-focus",
            "transient-focus-window",
            "test.assert.unstable",
        ),
        (
            "hidden-semantic-target",
            "hidden-semantic-focused",
            "test.driver.web.target_not_found",
        ),
        (
            "hidden-slotted-semantic-target",
            "hidden-slotted-semantic-focused",
            "test.driver.web.target_not_found",
        ),
        (
            "shadow-exactness",
            "shadow-host-is-not-exactly-focused",
            "test.assert.focused",
        ),
    ] {
        let scenario = scenarios
            .iter()
            .find(|entry| entry["id"] == scenario_id)
            .unwrap_or_else(|| panic!("missing focus ownership scenario {scenario_id}"));
        assert_eq!(scenario["status"], "failed");
        assert!(scenario["cleanup_error"].is_null());
        let failed_step = scenario["steps"]
            .as_array()
            .expect("focus ownership scenario steps")
            .iter()
            .find(|entry| entry["id"] == step_id)
            .unwrap_or_else(|| panic!("missing focus ownership step {step_id}"));
        assert_eq!(failed_step["status"], "failed");
        assert_eq!(failed_step["error"]["code"], error_code);
        if scenario_id == "transient-focus" {
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

fn run_focus_manifest(browser: &Path, manifest: &Path, workspace: &Path) -> std::process::Output {
    Command::new(binary())
        .args([
            "run",
            manifest
                .to_str()
                .expect("UTF-8 focus ownership manifest path"),
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
        .expect("run real browser focus ownership manifest")
}

fn focus_suite(origin: &str) -> String {
    format!(
        r##"suite "web-focus-ownership-e2e" {{
    version = 1

    scenario "focus-ownership" {{
        name = "Verify exact and composed keyboard focus ownership"
        surface = "web"
        timeout_ms = 60000

        navigate "open" {{ url = "{origin}/focus.html" }}
        wait "loaded" {{ load = "domcontentloaded" }}

        focus "focus-before" {{ target = css("#focus-before") }}
        expect "before-focused" {{ focused = role("button", "Before shadow") }}
        expect "after-unfocused" {{ unfocused = css("#focus-after") }}
        expect "lane-within-before" {{ focus_within = testid("focus-lane") }}
        expect "profile-outside-before" {{ focus_outside = css("#profile-panel") }}

        press "tab-into-shadow" {{ key = "Tab" }}
        expect "shadow-focused" {{ focused = role("button", "Shadow focus target") }}
        expect "shadow-scope-within" {{
            focus_within = testid("shadow-focus-scope")
            stable_for_ms = 100
            sample_interval_ms = 25
        }}
        expect "before-unfocused-after-tab" {{ unfocused = css("#focus-before") }}
        expect "lane-within-shadow" {{ focus_within = testid("focus-lane") }}

        press "tab-after-shadow" {{ key = "Tab" }}
        expect "after-focused" {{ focused = css("#focus-after") }}
        expect "shadow-scope-outside" {{ focus_outside = testid("shadow-focus-scope") }}
        expect "lane-within-after" {{ focus_within = css("#focus-lane") }}

        press "reverse-into-shadow" {{ key = "Shift+Tab" }}
        expect "shadow-focused-reverse" {{ focused = role("button", "Shadow focus target") }}
        press "reverse-before-shadow" {{ key = "Shift+Tab" }}
        expect "before-focused-reverse" {{ focused = css("#focus-before") }}

        focus "focus-outside" {{ target = css("#outside-action") }}
        expect "outside-focused" {{ focused = role("button", "Outside action") }}
        expect "lane-outside-final" {{ focus_outside = testid("focus-lane") }}

        focus "focus-slotted" {{ target = css("#slotted-focus") }}
        expect "slotted-focused" {{ focused = role("button", "Slotted focus target") }}
        expect "slotted-shadow-scope-within" {{
            focus_within = testid("slotted-shadow-scope")
        }}
    }}
}}
"##
    )
}

fn focus_failure_suite(origin: &str) -> String {
    format!(
        r##"suite "web-focus-ownership-errors" {{
    version = 1

    scenario "missing-target" {{
        surface = "web"
        timeout_ms = 30000
        navigate "open" {{ url = "{origin}/focus.html" }}
        expect "missing-unfocused" {{ unfocused = css("#missing-focus") }}
    }}

    scenario "ambiguous-target" {{
        surface = "web"
        timeout_ms = 30000
        navigate "open" {{ url = "{origin}/focus.html" }}
        expect "ambiguous-focused" {{ focused = css(".focus-duplicate") }}
    }}

    scenario "exact-focus-mismatch" {{
        surface = "web"
        timeout_ms = 30000
        navigate "open" {{ url = "{origin}/focus.html" }}
        focus "focus-after" {{ target = css("#focus-after") }}
        expect "before-is-not-focused" {{ focused = css("#focus-before") }}
    }}

    scenario "negative-focus-mismatch" {{
        surface = "web"
        timeout_ms = 30000
        navigate "open" {{ url = "{origin}/focus.html" }}
        focus "focus-after" {{ target = css("#focus-after") }}
        expect "after-is-not-unfocused" {{ unfocused = css("#focus-after") }}
    }}

    scenario "within-mismatch" {{
        surface = "web"
        timeout_ms = 30000
        navigate "open" {{ url = "{origin}/focus.html" }}
        focus "focus-outside" {{ target = css("#outside-action") }}
        expect "lane-does-not-own-outside" {{ focus_within = testid("focus-lane") }}
    }}

    scenario "outside-mismatch" {{
        surface = "web"
        timeout_ms = 30000
        navigate "open" {{ url = "{origin}/focus.html" }}
        focus "focus-before" {{ target = css("#focus-before") }}
        expect "lane-is-not-outside" {{ focus_outside = testid("focus-lane") }}
    }}

    scenario "invalid-target" {{
        surface = "web"
        timeout_ms = 30000
        navigate "open" {{ url = "{origin}/focus.html" }}
        expect "invalid-focused-selector" {{ focused = css("[") }}
    }}

    scenario "transient-focus" {{
        surface = "web"
        timeout_ms = 30000
        navigate "open" {{ url = "{origin}/focus.html" }}
        focus "focus-transient" {{ target = css("#transient-focus") }}
        click "schedule-focus-move" {{ target = css("#transient-focus") }}
        expect "transient-focus-window" {{
            focused = css("#transient-focus")
            stable_for_ms = 2500
            sample_interval_ms = 100
        }}
    }}

    scenario "hidden-semantic-target" {{
        surface = "web"
        timeout_ms = 30000
        navigate "open" {{ url = "{origin}/focus.html" }}
        focus "focus-hidden-semantic" {{ target = css("#aria-hidden-focus") }}
        expect "hidden-semantic-focused" {{ focused = role("button", "Hidden semantic focus") }}
    }}

    scenario "hidden-slotted-semantic-target" {{
        surface = "web"
        timeout_ms = 30000
        navigate "open" {{ url = "{origin}/focus.html" }}
        focus "focus-hidden-slotted" {{ target = css("#hidden-slotted-focus") }}
        expect "hidden-slotted-semantic-focused" {{ focused = role("button", "Hidden slotted focus") }}
    }}

    scenario "shadow-exactness" {{
        surface = "web"
        timeout_ms = 30000
        navigate "open" {{ url = "{origin}/focus.html" }}
        focus "focus-before" {{ target = css("#focus-before") }}
        press "tab-into-shadow" {{ key = "Tab" }}
        expect "shadow-host-is-not-exactly-focused" {{ focused = css("a3s-focus-host") }}
    }}
}}
"##
    )
}
