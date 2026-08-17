use std::path::Path;
use std::process::Command;

pub fn assert_passed_stability(
    report: &serde_json::Value,
    scenario_index: usize,
    step_id: &str,
    required_ms: u64,
    sample_interval_ms: u64,
) {
    let step = report["scenarios"][scenario_index]["steps"]
        .as_array()
        .expect("stability scenario steps")
        .iter()
        .find(|step| step["id"] == step_id)
        .unwrap_or_else(|| panic!("stability step {step_id} was missing"));
    let stability = step
        .pointer("/output/data/stability")
        .unwrap_or_else(|| panic!("stability step {step_id} omitted its metrics"));
    let samples = stability["samples"]
        .as_u64()
        .expect("stability sample count");
    let observed_ms = stability["observed_ms"]
        .as_u64()
        .expect("stability observation duration");

    assert_eq!(step["status"], "passed");
    assert_eq!(stability["outcome"], "passed");
    assert_eq!(stability["required_ms"], required_ms);
    assert_eq!(stability["sample_interval_ms"], sample_interval_ms);
    assert!((2..=required_ms.div_ceil(sample_interval_ms) + 1).contains(&samples));
    assert!(observed_ms >= required_ms);
    assert_eq!(step["attempts"], samples);
    assert!(!step["output"]["data"]["assertion"]["first"].is_null());
    assert!(!step["output"]["data"]["assertion"]["last"].is_null());
}

pub fn run_transient_stability_e2e(binary: &Path, browser: &Path, origin: &str, workspace: &Path) {
    let manifest = workspace.join("transient-stability-e2e.acl");
    std::fs::write(&manifest, transient_suite(origin)).expect("write transient stability suite");
    let output = Command::new(binary)
        .args([
            "run",
            manifest.to_str().expect("UTF-8 transient manifest path"),
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
        .current_dir(workspace)
        .output()
        .expect("run transient assertion stability E2E");
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "transient assertion stability E2E returned invalid JSON: {error}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    });

    assert_eq!(
        output.status.code(),
        Some(1),
        "transient assertion stability E2E must report a product failure\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(report["status"], "failed");
    assert_eq!(report["scenarios"][0]["status"], "failed");
    assert!(report["scenarios"][0]["cleanup_error"].is_null());

    let step = report["scenarios"][0]["steps"]
        .as_array()
        .expect("transient scenario steps")
        .iter()
        .find(|step| step["id"] == "transient-state")
        .expect("transient assertion step");
    let stability = step
        .pointer("/output/data/stability")
        .expect("transient assertion stability metrics");
    let samples = stability["samples"]
        .as_u64()
        .expect("transient stability sample count");

    assert_eq!(step["status"], "failed");
    assert_eq!(step["error"]["code"], "test.assert.unstable");
    assert_eq!(stability["outcome"], "unstable");
    assert_eq!(stability["required_ms"], 200);
    assert_eq!(stability["sample_interval_ms"], 25);
    assert!(samples >= 2);
    assert_eq!(step["attempts"], samples);
    assert!(!step["output"]["data"]["assertion"]["first"].is_null());
    assert!(step["output"]["data"]["assertion"]["last"].is_null());
}

pub fn run_hidden_visibility_e2e(binary: &Path, browser: &Path, origin: &str, workspace: &Path) {
    let manifest = workspace.join("hidden-visibility-e2e.acl");
    std::fs::write(&manifest, hidden_suite(origin)).expect("write hidden visibility suite");
    let output = Command::new(binary)
        .args([
            "run",
            manifest.to_str().expect("UTF-8 hidden manifest path"),
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
        .current_dir(workspace)
        .output()
        .expect("run hidden visibility E2E");
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
            panic!(
                "hidden visibility E2E returned invalid JSON: {error}\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )
        });

    assert_eq!(
        output.status.code(),
        Some(1),
        "hidden visibility E2E must retain its negative counterexamples\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(report["status"], "failed");

    let static_hidden = scenario(&report, "static-hidden");
    assert_eq!(static_hidden["status"], "passed");
    assert!(static_hidden["cleanup_error"].is_null());
    let static_output = step(static_hidden, "dialog-hidden");
    assert_eq!(static_output["status"], "passed");
    assert_eq!(static_output["output"]["data"]["visible"], false);

    let absent = scenario(&report, "absent-target");
    assert_eq!(absent["status"], "passed");
    assert!(absent["cleanup_error"].is_null());
    assert_eq!(
        step(absent, "dialog-absent")["output"]["data"]["visible"],
        false
    );

    let visible = scenario(&report, "visible-target");
    assert_eq!(visible["status"], "failed");
    assert!(visible["cleanup_error"].is_null());
    let visible_step = step(visible, "dialog-visible");
    assert_eq!(visible_step["error"]["code"], "test.assert.hidden");
    assert_eq!(visible_step["output"]["data"]["visible"], true);

    let reappearing = scenario(&report, "reappearing-target");
    assert_eq!(reappearing["status"], "failed");
    assert!(reappearing["cleanup_error"].is_null());
    let reappearing_step = step(reappearing, "dialog-stays-hidden");
    assert_eq!(reappearing_step["error"]["code"], "test.assert.unstable");
    assert_eq!(
        reappearing_step["output"]["data"]["stability"]["outcome"],
        "unstable"
    );
    assert_eq!(
        reappearing_step["output"]["data"]["assertion"]["first"]["visible"],
        false
    );
    assert_eq!(
        reappearing_step["output"]["data"]["assertion"]["last"]["visible"],
        true
    );

    let waiting = scenario(&report, "wait-until-hidden");
    assert_eq!(waiting["status"], "passed");
    assert!(waiting["cleanup_error"].is_null());
    let waiting_step = step(waiting, "dialog-closes");
    assert_eq!(waiting_step["status"], "passed");
    assert_eq!(waiting_step["attempts"], 3);
    assert_eq!(waiting_step["output"]["data"]["visible"], false);
    assert_eq!(waiting_step["output"]["data"]["wait"]["outcome"], "matched");
    assert_eq!(waiting_step["output"]["data"]["wait"]["probes"], 3);

    let already_hidden = scenario(&report, "wait-already-hidden");
    assert_eq!(already_hidden["status"], "passed");
    assert!(already_hidden["cleanup_error"].is_null());
    let already_hidden_step = step(already_hidden, "dialog-already-closed");
    assert_eq!(already_hidden_step["status"], "passed");
    assert_eq!(already_hidden_step["attempts"], 1);
    assert_eq!(already_hidden_step["output"]["data"]["wait"]["probes"], 1);
}

fn scenario<'a>(report: &'a serde_json::Value, id: &str) -> &'a serde_json::Value {
    report["scenarios"]
        .as_array()
        .expect("hidden visibility scenarios")
        .iter()
        .find(|scenario| scenario["id"] == id)
        .unwrap_or_else(|| panic!("hidden visibility scenario {id} was missing"))
}

fn step<'a>(scenario: &'a serde_json::Value, id: &str) -> &'a serde_json::Value {
    scenario["steps"]
        .as_array()
        .expect("hidden visibility steps")
        .iter()
        .find(|step| step["id"] == id)
        .unwrap_or_else(|| panic!("hidden visibility step {id} was missing"))
}

fn transient_suite(origin: &str) -> String {
    format!(
        r##"suite "transient-assertion-stability" {{
    version = 1

    scenario "transient-state" {{
        name = "Reject a state that disappears after its first visibility sample"
        surface = "web"
        timeout_ms = 30000

        navigate "open" {{
            url = "{origin}/transient.html"
        }}

        expect "transient-state" {{
            visible = testid("transient-state")
            stable_for_ms = 200
            sample_interval_ms = 25
        }}
    }}
}}
"##
    )
}

fn hidden_suite(origin: &str) -> String {
    format!(
        r##"suite "hidden-visibility" {{
    version = 1

    scenario "static-hidden" {{
        name = "Accept an existing target with no visible box"
        surface = "web"
        timeout_ms = 30000

        navigate "open" {{
            url = "{origin}/transient.html"
        }}

        expect "dialog-hidden" {{
            hidden = testid("hidden-static")
        }}
    }}

    scenario "absent-target" {{
        name = "Accept a stable locator with no matching target"
        surface = "web"
        timeout_ms = 30000

        navigate "open" {{
            url = "{origin}/transient.html"
        }}

        expect "dialog-absent" {{
            hidden = testid("missing-dialog")
        }}
    }}

    scenario "visible-target" {{
        name = "Reject a target with a visible box"
        surface = "web"
        timeout_ms = 30000

        navigate "open" {{
            url = "{origin}/transient.html"
        }}

        expect "dialog-visible" {{
            hidden = testid("visible-static")
        }}
    }}

    scenario "reappearing-target" {{
        name = "Reject a target that reappears after the first hidden sample"
        surface = "web"
        timeout_ms = 30000

        navigate "open" {{
            url = "{origin}/transient.html"
        }}

        expect "dialog-stays-hidden" {{
            hidden = testid("hidden-then-visible")
            stable_for_ms = 200
            sample_interval_ms = 25
        }}
    }}

    scenario "wait-until-hidden" {{
        name = "Wait for a visible target to lose its visible box"
        surface = "web"
        timeout_ms = 30000

        navigate "open" {{
            url = "{origin}/transient.html"
        }}

        wait "dialog-closes" {{
            hidden = testid("visible-then-hidden")
        }}
    }}

    scenario "wait-already-hidden" {{
        name = "Finish immediately when the stable target is already hidden"
        surface = "web"
        timeout_ms = 30000

        navigate "open" {{
            url = "{origin}/transient.html"
        }}

        wait "dialog-already-closed" {{
            hidden = testid("hidden-static")
        }}
    }}
}}
"##
    )
}
