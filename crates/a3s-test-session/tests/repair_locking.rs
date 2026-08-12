use std::sync::Arc;
use std::time::Duration;

use a3s_test_core::{
    RepairFinding, RepairIntent, RepairSeverity, RepairStatus, RepairTarget, RepairTargetKind,
    Surface,
};
use a3s_test_session::{AgentSessionManager, SessionManagerOptions, StartSessionRequest};
use serde_json::json;
use tokio::sync::{Mutex, Notify};

mod support;
use support::session_fixture::{FakeDriver, FakeState};

fn repair_finding() -> RepairFinding {
    RepairFinding {
        id: "finding-1".to_string(),
        batch_id: "batch-1".to_string(),
        instruction: "Fix the broken button".to_string(),
        success_criteria: Some("The button works".to_string()),
        intent: RepairIntent::Fix,
        severity: RepairSeverity::Important,
        relations: Vec::new(),
        target: RepairTarget {
            kind: RepairTargetKind::Node,
            node_ids: vec!["n1".to_string()],
            selected_text: None,
            region: None,
            drawing: None,
        },
        created_at: "2026-08-12T00:00:00Z".to_string(),
        page_id: "checkout".to_string(),
        url: "http://127.0.0.1/checkout".to_string(),
        context_revision: 3,
        context: json!({ "untrusted": true }),
        status: RepairStatus::Queued,
        submitted_at: "2026-08-12T00:00:01Z".to_string(),
    }
}

async fn start_manager(
    temp: &tempfile::TempDir,
    session: &str,
    state: Arc<Mutex<FakeState>>,
) -> Arc<AgentSessionManager> {
    let manager = Arc::new(
        AgentSessionManager::new(
            vec![Arc::new(FakeDriver { state })],
            SessionManagerOptions {
                artifacts_root: temp.path().to_path_buf(),
                cleanup_timeout: Duration::from_secs(1),
                max_sessions: 1,
            },
        )
        .expect("manager"),
    );
    manager
        .start(StartSessionRequest {
            session: session.to_string(),
            surface: Surface::Gui,
            goal: "Exercise repair locking".to_string(),
            success_criteria: vec!["Repair operation completes".to_string()],
            auto_resolve_repairs: false,
        })
        .await
        .expect("start");
    manager
}

async fn assert_workspace_lock_available(temp: &tempfile::TempDir, message: &str) {
    let workspace = a3s_test_session::RepairWorkspace::from_artifacts_root(temp.path());
    let lock = tokio::time::timeout(Duration::from_millis(100), workspace.acquire())
        .await
        .expect(message)
        .expect("workspace lock");
    drop(lock);
}

#[tokio::test]
async fn repair_ingest_releases_lock_during_before_evidence_capture() {
    let evidence_started = Arc::new(Notify::new());
    let evidence_release = Arc::new(Notify::new());
    let temp = tempfile::tempdir().expect("tempdir");
    let state = Arc::new(Mutex::new(FakeState {
        repairs: vec![repair_finding()],
        before_evidence_started: Some(Arc::clone(&evidence_started)),
        before_evidence_release: Some(Arc::clone(&evidence_release)),
        ..FakeState::default()
    }));
    let manager = start_manager(&temp, "short-ingest-lock", state).await;
    let ingest = tokio::spawn({
        let manager = Arc::clone(&manager);
        async move { manager.ingest_repairs("short-ingest-lock", 10).await }
    });

    tokio::time::timeout(Duration::from_secs(1), evidence_started.notified())
        .await
        .expect("before evidence started");
    assert_workspace_lock_available(&temp, "lock held during before evidence capture").await;
    evidence_release.notify_one();

    let repairs = tokio::time::timeout(Duration::from_secs(1), ingest)
        .await
        .expect("ingest deadline")
        .expect("ingest task")
        .expect("ingest result");
    assert!(repairs[0].before_evidence.is_some());
    manager.abort("short-ingest-lock").await.expect("abort");
}

#[tokio::test]
async fn repair_watch_releases_lock_while_waiting_for_page_findings() {
    let wait_started = Arc::new(Notify::new());
    let wait_release = Arc::new(Notify::new());
    let temp = tempfile::tempdir().expect("tempdir");
    let state = Arc::new(Mutex::new(FakeState {
        repair_wait_started: Some(Arc::clone(&wait_started)),
        repair_wait_release: Some(Arc::clone(&wait_release)),
        ..FakeState::default()
    }));
    let manager = start_manager(&temp, "short-watch-lock", state).await;
    let watch = tokio::spawn({
        let manager = Arc::clone(&manager);
        async move {
            manager
                .watch_repairs("short-watch-lock", 10, 1_000, 100)
                .await
        }
    });

    tokio::time::timeout(Duration::from_secs(1), wait_started.notified())
        .await
        .expect("repair wait started");
    assert_workspace_lock_available(&temp, "lock held while waiting for page findings").await;
    wait_release.notify_one();

    let repairs = tokio::time::timeout(Duration::from_secs(1), watch)
        .await
        .expect("watch deadline")
        .expect("watch task")
        .expect("watch result");
    assert!(repairs.is_empty());
    manager.abort("short-watch-lock").await.expect("abort");
}
