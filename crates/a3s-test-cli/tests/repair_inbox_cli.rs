use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;

use a3s_test_core::{
    RepairActor, RepairFinding, RepairIntent, RepairSeverity, RepairStatus, RepairTarget,
    RepairTargetKind,
};
use a3s_test_session::{RepairLedger, RepairTransition};
use serde_json::{json, Value};

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_a3s-test"))
}

#[tokio::test]
async fn repair_inbox_discovers_closed_sessions_without_a_browser_connection() {
    let temp = tempfile::tempdir().expect("workspace");
    let workspace = temp.path().canonicalize().expect("canonical workspace");
    write_session(&workspace, "session-newer", "finding-newer", 200).await;
    write_session(&workspace, "session-older", "finding-older", 100).await;

    let output = Command::new(binary())
        .args(["agent", "repair-inbox", "--json"])
        .current_dir(&workspace)
        .env("PATH", "")
        .output()
        .expect("run repair inbox");
    assert_success(&output, "workspace repair inbox");
    let value: Value = serde_json::from_slice(&output.stdout).expect("repair inbox JSON");
    assert_eq!(value["protocol"], "a3s.test.repair-inbox/1");
    assert_eq!(value["scope"], "workspace");
    assert!(value["session"].is_null());
    assert_eq!(value["sessions_scanned"], 2);
    assert_eq!(value["total"], 2);
    assert_eq!(value["items"][0]["session"], "session-older");
    assert_eq!(value["items"][0]["finding_id"], "finding-older");
    assert_eq!(value["items"][0]["next"]["action"], "claim");

    let scoped = Command::new(binary())
        .args([
            "agent",
            "repair-inbox",
            "--session",
            "session-newer",
            "--json",
        ])
        .current_dir(&workspace)
        .env("PATH", "")
        .output()
        .expect("run scoped repair inbox");
    assert_success(&scoped, "scoped repair inbox");
    let scoped: Value = serde_json::from_slice(&scoped.stdout).expect("scoped inbox JSON");
    assert_eq!(scoped["scope"], "session");
    assert_eq!(scoped["session"], "session-newer");
    assert_eq!(scoped["sessions_scanned"], 1);
    assert_eq!(scoped["items"].as_array().expect("items").len(), 1);

    let invalid = run_inbox(&workspace, &["--limit", "0"]);
    assert!(
        !invalid.status.success(),
        "zero inbox limit unexpectedly passed"
    );
    assert!(
        String::from_utf8_lossy(&invalid.stderr).contains("repair inbox limit must be between"),
        "{}",
        String::from_utf8_lossy(&invalid.stderr)
    );
}

#[tokio::test]
async fn repair_inbox_hides_terminal_findings_unless_requested() {
    let temp = tempfile::tempdir().expect("workspace");
    let workspace = temp.path().canonicalize().expect("canonical workspace");
    let (root, mut ledger) = create_session(&workspace, "session-terminal").await;
    ledger
        .ingest("session-terminal", vec![finding("finding-terminal")], 100)
        .await
        .expect("terminal finding");
    ledger
        .transition(
            RepairTransition {
                session: "session-terminal".to_string(),
                finding_id: "finding-terminal".to_string(),
                request_id: "cancel-terminal".to_string(),
                status: RepairStatus::Cancelled,
                actor: RepairActor::Agent,
                attempt_id: None,
                lease_expires_at_ms: None,
                summary: Some("No longer needed".to_string()),
                message: None,
                verification: None,
                changed_files: None,
            },
            200,
        )
        .await
        .expect("cancel finding");
    assert!(root.join("repairs.jsonl").is_file());

    let hidden = run_inbox(&workspace, &["--session", "session-terminal"]);
    assert_success(&hidden, "terminal-hidden inbox");
    let hidden: Value = serde_json::from_slice(&hidden.stdout).expect("hidden inbox JSON");
    assert_eq!(hidden["total"], 0);

    let included = run_inbox(
        &workspace,
        &["--session", "session-terminal", "--include-terminal"],
    );
    assert_success(&included, "terminal-inclusive inbox");
    let included: Value = serde_json::from_slice(&included.stdout).expect("included inbox JSON");
    assert_eq!(included["total"], 1);
    assert_eq!(included["items"][0]["status"], "cancelled");
    assert_eq!(included["items"][0]["next"]["action"], "complete");
}

#[cfg(unix)]
#[tokio::test]
async fn repair_inbox_rejects_a_linked_ledger() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().expect("workspace");
    let workspace = temp.path().canonicalize().expect("canonical workspace");
    let (root, _) = create_session(&workspace, "session-linked").await;
    let external = temp.path().join("external-repairs.jsonl");
    let mut external_ledger = RepairLedger::load(external.clone())
        .await
        .expect("external ledger");
    external_ledger
        .ingest("session-linked", vec![finding("finding-linked")], 100)
        .await
        .expect("external finding");
    symlink(&external, root.join("repairs.jsonl")).expect("linked ledger");

    let output = run_inbox(&workspace, &["--session", "session-linked"]);
    assert!(
        !output.status.success(),
        "linked ledger unexpectedly passed"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("repair ledger must be a regular file"),
        "{stderr}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn workspace_repair_inbox_rejects_a_linked_discovery_root() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().expect("workspace");
    let workspace = temp.path().canonicalize().expect("canonical workspace");
    let state_root = workspace.join(".a3s-test");
    let external = workspace.join("external-agent-sessions");
    std::fs::create_dir_all(&state_root).expect("state root");
    std::fs::create_dir_all(&external).expect("external sessions root");
    symlink(&external, state_root.join("agent-sessions")).expect("linked discovery root");

    let output = run_inbox(&workspace, &[]);
    assert!(
        !output.status.success(),
        "linked discovery root unexpectedly passed"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("discovery root must be a regular directory"),
        "{stderr}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn scoped_repair_inbox_rejects_linked_session_metadata() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().expect("workspace");
    let workspace = temp.path().canonicalize().expect("canonical workspace");
    let (root, _) = create_session(&workspace, "session-linked-metadata").await;
    let external = workspace.join("external-session.json");
    std::fs::rename(root.join("session.json"), &external).expect("move session metadata");
    symlink(&external, root.join("session.json")).expect("linked session metadata");

    let output = run_inbox(&workspace, &["--session", "session-linked-metadata"]);
    assert!(
        !output.status.success(),
        "linked session metadata unexpectedly passed"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("session metadata must be a regular file"),
        "{stderr}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn scoped_repair_inbox_rejects_a_linked_session_directory() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().expect("workspace");
    let workspace = temp.path().canonicalize().expect("canonical workspace");
    let sessions = workspace.join(".a3s-test").join("agent-sessions");
    std::fs::create_dir_all(&sessions).expect("sessions root");
    let external = workspace.join("external-session");
    std::fs::create_dir_all(external.join("artifacts")).expect("external session");
    let linked = sessions.join("session-linked-root");
    symlink(&external, &linked).expect("linked session root");
    write_closed_state(&workspace, &linked, "session-linked-root");
    let mut ledger = RepairLedger::load(linked.join("repairs.jsonl"))
        .await
        .expect("linked-root ledger");
    ledger
        .ingest(
            "session-linked-root",
            vec![finding("finding-linked-root")],
            100,
        )
        .await
        .expect("linked-root finding");

    let output = run_inbox(&workspace, &["--session", "session-linked-root"]);
    assert!(
        !output.status.success(),
        "linked session root unexpectedly passed"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("session directory must not be a link"),
        "{stderr}"
    );
}

async fn write_session(workspace: &Path, session: &str, finding_id: &str, now_ms: u64) {
    let (_, mut ledger) = create_session(workspace, session).await;
    ledger
        .ingest(session, vec![finding(finding_id)], now_ms)
        .await
        .expect("ingest finding");
}

async fn create_session(workspace: &Path, session: &str) -> (PathBuf, RepairLedger) {
    let root = workspace
        .join(".a3s-test")
        .join("agent-sessions")
        .join(session);
    std::fs::create_dir_all(root.join("artifacts")).expect("session store");
    write_closed_state(workspace, &root, session);
    let ledger = RepairLedger::load(root.join("repairs.jsonl"))
        .await
        .expect("load ledger");
    (root, ledger)
}

fn run_inbox(workspace: &Path, extra: &[&str]) -> std::process::Output {
    let mut command = Command::new(binary());
    command.args(["agent", "repair-inbox"]);
    command.args(extra);
    command
        .arg("--json")
        .current_dir(workspace)
        .env("PATH", "")
        .output()
        .expect("run repair inbox")
}

fn assert_success(output: &std::process::Output, name: &str) {
    assert!(
        output.status.success(),
        "{name} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn write_closed_state(workspace: &Path, root: &Path, session: &str) {
    #[cfg(unix)]
    let runtime_base = PathBuf::from("/tmp");
    #[cfg(not(unix))]
    let runtime_base = std::env::temp_dir();
    let runtime_name = session_namespace(workspace, session).replacen("a3st-", "a3st-i-", 1);
    let runtime_dir = runtime_base.join(runtime_name);
    assert!(!runtime_dir.exists(), "synthetic runtime already exists");
    let state = json!({
        "schema_version": 1,
        "session": session,
        "workspace": workspace,
        "surface": "web",
        "status": "aborted",
        "goal": "Preserve workspace repair intent",
        "success_criteria": ["A later agent discovers the repair"],
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

fn finding(id: &str) -> RepairFinding {
    RepairFinding {
        id: id.to_string(),
        batch_id: format!("batch-{id}"),
        instruction: format!("Resume {id} without chat history"),
        success_criteria: Some("The repair is discoverable".to_string()),
        intent: RepairIntent::Fix,
        severity: RepairSeverity::Important,
        relations: Vec::new(),
        design_reference: None,
        target: RepairTarget {
            kind: RepairTargetKind::Node,
            node_ids: vec![format!("node-{id}")],
            selected_text: None,
            region: None,
            drawing: None,
            layout: None,
        },
        created_at: "2026-08-20T00:00:00Z".to_string(),
        page_id: "inbox-page".to_string(),
        url: "http://127.0.0.1:3000/inbox".to_string(),
        context_revision: 1,
        context: json!({ "nodes": [], "untrusted": true }),
        status: RepairStatus::Queued,
        submitted_at: "2026-08-20T00:00:01Z".to_string(),
    }
}
