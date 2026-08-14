use std::path::PathBuf;
use std::process::Command;

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_a3s-test"))
}

#[test]
fn worker_schema_exposes_scheduling_evidence_without_trust_authority() {
    let output = Command::new(binary())
        .args(["worker", "schema", "--compact"])
        .output()
        .expect("run worker schema");

    assert!(output.status.success(), "{output:?}");
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("worker schema JSON");
    assert_eq!(value["protocol"], "a3s.test.worker-capabilities/1");
    assert_eq!(value["authority"], "scheduling_evidence");
    assert_eq!(value["invariants"]["self_reported"], true);
    assert_eq!(value["invariants"]["authenticated"], false);
    assert_eq!(value["invariants"]["authorizes_execution"], false);
    assert_eq!(
        value["invariants"]["external_image_identity_required"],
        true
    );
    assert_eq!(value["inventory_schema"]["additionalProperties"], false);
}

#[test]
fn worker_inventory_reports_the_compiled_tui_surface_by_default() {
    let output = Command::new(binary())
        .args([
            "worker",
            "inventory",
            "--max-parallel-scenarios",
            "4",
            "--compact",
        ])
        .output()
        .expect("run worker inventory");

    assert!(output.status.success(), "{output:?}");
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("worker inventory JSON");
    assert_eq!(value["protocol"], "a3s.test.worker-capabilities/1");
    assert_eq!(value["max_parallel_scenarios"], 4);
    assert_eq!(value["surfaces"].as_array().map(Vec::len), Some(1));
    assert_eq!(value["surfaces"][0]["surface"], "tui");
    assert!(value["surfaces"][0]["terminal"]["backend"].is_string());
}

#[test]
fn worker_inventory_rejects_an_unbounded_parallelism_claim() {
    let output = Command::new(binary())
        .args([
            "worker",
            "inventory",
            "--max-parallel-scenarios",
            "65",
            "--compact",
        ])
        .output()
        .expect("run worker inventory");

    assert!(!output.status.success(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("between 1 and 64"),
        "{output:?}"
    );
}

#[cfg(unix)]
#[test]
fn worker_inventory_adds_web_only_after_an_explicit_successful_probe() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().expect("tempdir");
    let driver = temp.path().join("fake-agent-browser");
    fs::write(&driver, "#!/bin/sh\nprintf 'agent-browser 0.26.0\\n'\n").expect("driver");
    fs::set_permissions(&driver, fs::Permissions::from_mode(0o755)).expect("permissions");

    let output = Command::new(binary())
        .args([
            "worker",
            "inventory",
            "--browser-driver",
            "standalone",
            "--browser-executable",
            driver.to_str().expect("driver path"),
            "--compact",
        ])
        .output()
        .expect("run worker inventory");

    assert!(output.status.success(), "{output:?}");
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("worker inventory JSON");
    assert_eq!(value["surfaces"].as_array().map(Vec::len), Some(2));
    assert_eq!(value["surfaces"][0]["surface"], "web");
    assert_eq!(value["surfaces"][0]["execution"], "headless");
    assert_eq!(value["surfaces"][0]["browser"]["integration"], "standalone");
    assert_eq!(value["surfaces"][1]["surface"], "tui");
}

#[cfg(unix)]
#[test]
fn worker_inventory_fails_closed_when_the_requested_web_probe_fails() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().expect("tempdir");
    let driver = temp.path().join("broken-agent-browser");
    fs::write(
        &driver,
        "#!/bin/sh\nprintf 'probe unavailable\\n' >&2\nexit 7\n",
    )
    .expect("driver");
    fs::set_permissions(&driver, fs::Permissions::from_mode(0o755)).expect("permissions");

    let output = Command::new(binary())
        .args([
            "worker",
            "inventory",
            "--browser-driver",
            "standalone",
            "--browser-executable",
            driver.to_str().expect("driver path"),
            "--compact",
        ])
        .output()
        .expect("run worker inventory");

    assert!(!output.status.success(), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("browser version probe failed"),
        "{output:?}"
    );
}

#[test]
fn worker_inventory_does_not_infer_a_browser_backend_from_an_executable() {
    let output = Command::new(binary())
        .args([
            "worker",
            "inventory",
            "--browser-executable",
            "agent-browser",
            "--compact",
        ])
        .output()
        .expect("run worker inventory");

    assert!(!output.status.success(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("--browser-driver"),
        "{output:?}"
    );
}
