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
