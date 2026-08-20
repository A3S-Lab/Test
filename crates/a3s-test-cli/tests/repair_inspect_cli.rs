use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;

use a3s_test_core::{
    RepairFinding, RepairIntent, RepairSeverity, RepairStatus, RepairTarget, RepairTargetKind,
};
use a3s_test_session::RepairLedger;
use serde_json::{json, Value};

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_a3s-test"))
}

#[tokio::test]
async fn repair_inspect_reads_a_closed_session_without_a_browser_connection() {
    let temp = tempfile::tempdir().expect("workspace");
    let workspace = temp.path().canonicalize().expect("canonical workspace");
    let session = "inspect-closed";
    let root = workspace
        .join(".a3s-test")
        .join("agent-sessions")
        .join(session);
    std::fs::create_dir_all(root.join("artifacts")).expect("session store");
    write_closed_state(&workspace, &root, session);

    let mut ledger = RepairLedger::load(root.join("repairs.jsonl"))
        .await
        .expect("load ledger");
    ledger
        .ingest(session, vec![finding()], 100)
        .await
        .expect("ingest finding");

    let output = Command::new(binary())
        .args([
            "agent",
            "repair-inspect",
            "finding-closed",
            "--session",
            session,
            "--json",
        ])
        .current_dir(&workspace)
        .env("PATH", "")
        .output()
        .expect("run repair inspect");
    assert!(
        output.status.success(),
        "repair inspect failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).expect("repair loop JSON");
    assert_eq!(value["protocol"], "a3s.test.repair-loop-record/1");
    assert_eq!(value["session"], session);
    assert_eq!(value["finding_id"], "finding-closed");
    assert_eq!(
        value["intent"]["instruction"],
        "Keep the closed session inspectable"
    );
    assert_eq!(value["resume"]["action"], "claim");
}

fn write_closed_state(workspace: &Path, root: &Path, session: &str) {
    let runtime_suffix = workspace
        .to_string_lossy()
        .bytes()
        .fold(0_u64, |value, byte| {
            value.wrapping_mul(31).wrapping_add(u64::from(byte))
        });
    #[cfg(unix)]
    let runtime_base = PathBuf::from("/tmp");
    #[cfg(not(unix))]
    let runtime_base = std::env::temp_dir();
    let runtime_dir = runtime_base.join(format!("a3st-i-{runtime_suffix:016x}"));
    assert!(
        !runtime_dir.exists(),
        "synthetic closed runtime already exists"
    );
    let state = json!({
        "schema_version": 1,
        "session": session,
        "workspace": workspace,
        "surface": "web",
        "status": "aborted",
        "goal": "Preserve a repair loop",
        "success_criteria": ["A later agent can inspect it"],
        "auto_resolve_repairs": false,
        "allowed_origins": ["http://127.0.0.1:3000"],
        "browser_containment": "hostname_v1",
        "browser_allowed_origins": ["http://127.0.0.1:3000"],
        "browser_allowed_domains": [],
        "browser": {
            "driver": "standalone",
            "executable": "/missing/browser-must-not-be-opened",
            "headed": false,
            "command_timeout_ms": 25000,
            "idle_timeout_ms": 300000,
            "microphone": "disabled"
        },
        "namespace": session_namespace(workspace, session),
        "driver_session": format!("agent-{session}"),
        "runtime_dir": runtime_dir,
        "artifacts_dir": root.join("artifacts"),
        "active_video_path": null,
        "next_sequence": 1,
        "next_observation_id": 1,
        "latest_observation": null,
        "started_at_ms": 1,
        "updated_at_ms": 2,
        "summary": "Closed after preserving the ledger"
    });
    std::fs::write(
        root.join("session.json"),
        serde_json::to_vec_pretty(&state).expect("state JSON"),
    )
    .expect("write state");
}

fn session_namespace(workspace: &Path, session: &str) -> String {
    let mut hasher = DefaultHasher::new();
    workspace.hash(&mut hasher);
    session.hash(&mut hasher);
    format!("a3st-{:016x}", hasher.finish())
}

fn finding() -> RepairFinding {
    RepairFinding {
        id: "finding-closed".to_string(),
        batch_id: "batch-closed".to_string(),
        instruction: "Keep the closed session inspectable".to_string(),
        success_criteria: Some("The durable loop record is available".to_string()),
        intent: RepairIntent::Fix,
        severity: RepairSeverity::Important,
        relations: Vec::new(),
        design_reference: None,
        target: RepairTarget {
            kind: RepairTargetKind::Node,
            node_ids: vec!["closed-node".to_string()],
            selected_text: None,
            region: None,
            drawing: None,
            layout: None,
        },
        created_at: "2026-08-20T00:00:00Z".to_string(),
        page_id: "closed-page".to_string(),
        url: "http://127.0.0.1:3000/closed".to_string(),
        context_revision: 1,
        context: json!({ "nodes": [], "untrusted": true }),
        status: RepairStatus::Queued,
        submitted_at: "2026-08-20T00:00:01Z".to_string(),
    }
}
