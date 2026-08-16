use std::path::{Path, PathBuf};

pub fn assert_nonempty_artifact(workspace: &Path, path: &Path) {
    let workspace = workspace.canonicalize().expect("canonical E2E workspace");
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace.join(path)
    }
    .canonicalize()
    .expect("canonical E2E evidence");
    assert!(
        path.starts_with(&workspace),
        "evidence escaped the E2E workspace: {path:?}"
    );
    assert!(
        path.metadata().expect("evidence metadata").len() > 0,
        "evidence must not be empty"
    );
}

pub fn failed_run_summary(report: &serde_json::Value) -> String {
    let mut lines = vec![format!(
        "run status: {}",
        report["status"].as_str().unwrap_or("unknown")
    )];
    let Some(scenarios) = report["scenarios"].as_array() else {
        lines.push("scenarios: missing".to_string());
        return lines.join("\n");
    };
    for scenario in scenarios {
        if scenario["status"] == "passed" {
            continue;
        }
        let scenario_id = scenario["id"].as_str().unwrap_or("unknown");
        let scenario_status = scenario["status"].as_str().unwrap_or("unknown");
        lines.push(format!("scenario {scenario_id}: {scenario_status}"));
        if let Some(cleanup) = concise_report_error(&scenario["cleanup_error"]) {
            lines.push(format!("  cleanup: {cleanup}"));
        }
        for step in scenario["steps"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|step| step["status"] != "passed")
        {
            let step_id = step["id"].as_str().unwrap_or("unknown");
            let step_status = step["status"].as_str().unwrap_or("unknown");
            let duration_ms = step["duration_ms"].as_u64().unwrap_or_default();
            lines.push(format!(
                "  step {step_id}: {step_status} after {duration_ms} ms"
            ));
            if let Some(error) = concise_report_error(&step["error"]) {
                lines.push(format!("    error: {error}"));
            }
        }
    }
    lines.join("\n")
}

pub fn assert_png_artifact(
    workspace: &Path,
    path: &Path,
    expected_width: u32,
    expected_height: u32,
) {
    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    const MAX_SCREENSHOT_BYTES: u64 = 32 * 1_024 * 1_024;

    let workspace = workspace.canonicalize().expect("canonical E2E workspace");
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace.join(path)
    }
    .canonicalize()
    .expect("canonical PNG evidence");
    assert!(
        path.starts_with(&workspace),
        "PNG evidence escaped the E2E workspace: {path:?}"
    );
    let bytes = std::fs::read(&path).expect("read PNG evidence");
    assert!(
        (24..=MAX_SCREENSHOT_BYTES as usize).contains(&bytes.len()),
        "PNG evidence must contain 24 to {MAX_SCREENSHOT_BYTES} bytes"
    );
    assert_eq!(&bytes[..8], PNG_SIGNATURE, "invalid PNG signature");
    assert_eq!(&bytes[12..16], b"IHDR", "PNG must start with IHDR");
    assert_eq!(
        u32::from_be_bytes(bytes[16..20].try_into().expect("PNG width")),
        expected_width,
        "PNG width did not match its viewport"
    );
    assert_eq!(
        u32::from_be_bytes(bytes[20..24].try_into().expect("PNG height")),
        expected_height,
        "PNG height did not match its viewport"
    );
}

pub fn assert_empty_browser_diagnostics(report: &serde_json::Value, workspace: &Path) {
    for scenario in report["scenarios"]
        .as_array()
        .expect("website E2E scenarios")
    {
        let scenario_id = scenario["id"].as_str().unwrap_or("unknown");
        for (step_suffix, pointer, label) in [
            ("console-evidence", "/data/messages", "console messages"),
            ("page-error-evidence", "/data/errors", "page errors"),
        ] {
            let step = scenario["steps"]
                .as_array()
                .expect("website E2E steps")
                .iter()
                .find(|entry| {
                    entry["id"]
                        .as_str()
                        .is_some_and(|id| id.ends_with(step_suffix))
                })
                .unwrap_or_else(|| {
                    panic!("website E2E scenario {scenario_id} omitted {step_suffix}")
                });
            let step_id = step["id"].as_str().unwrap_or(step_suffix);
            let evidence_path = step
                .pointer("/output/evidence/0/path")
                .and_then(serde_json::Value::as_str)
                .map(PathBuf::from)
                .unwrap_or_else(|| panic!("website E2E step {step_id} omitted its evidence path"));
            let evidence_path = if evidence_path.is_absolute() {
                evidence_path
            } else {
                workspace.join(evidence_path)
            };
            let evidence: serde_json::Value = serde_json::from_slice(
                &std::fs::read(&evidence_path)
                    .unwrap_or_else(|error| panic!("read {label} evidence: {error}")),
            )
            .unwrap_or_else(|error| panic!("parse {label} evidence: {error}"));
            assert_eq!(
                evidence
                    .pointer(pointer)
                    .and_then(serde_json::Value::as_array),
                Some(&Vec::new()),
                "built website scenario {scenario_id} emitted unexpected {label}: {evidence}"
            );
        }
    }
}

fn concise_report_error(error: &serde_json::Value) -> Option<String> {
    (!error.is_null()).then(|| {
        let code = error["code"].as_str().unwrap_or("unknown");
        let message = error["message"].as_str().unwrap_or("missing message");
        format!("{code}: {message}")
    })
}
