use std::sync::Arc;
use std::time::Duration;

use a3s_test_core::{
    RepairActor, RepairCheckResult, RepairCheckStatus, RepairFinding, RepairIntent, RepairSeverity,
    RepairStatus, RepairTarget, RepairTargetKind, Surface,
};
use a3s_test_session::{
    AgentSessionManager, RepairRecord, RepairTransition, RepairVerifyRequest,
    SessionManagerOptions, StartSessionRequest,
};
use serde_json::json;
use tokio::sync::Mutex;

use super::support::session_fixture::{ready_page_context, FakeDriver, FakeState};

pub(crate) fn test_repair_finding() -> RepairFinding {
    RepairFinding {
        id: "finding-1".to_string(),
        batch_id: "batch-1".to_string(),
        instruction: "Fix the broken button".to_string(),
        success_criteria: Some("The button works".to_string()),
        intent: RepairIntent::Fix,
        severity: RepairSeverity::Important,
        relations: Vec::new(),
        design_reference: None,
        target: RepairTarget {
            kind: RepairTargetKind::Node,
            node_ids: vec!["n1".to_string()],
            selected_text: None,
            region: None,
            drawing: None,
            layout: None,
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

pub(crate) fn manager(state: Arc<Mutex<FakeState>>) -> AgentSessionManager {
    AgentSessionManager::new(
        vec![Arc::new(FakeDriver { state })],
        SessionManagerOptions {
            artifacts_root: std::env::temp_dir().join("a3s-test-session-tests"),
            cleanup_timeout: Duration::from_secs(1),
            max_sessions: 2,
        },
    )
    .expect("manager")
}

pub(crate) async fn verify_repair_with_policy(
    session: &str,
    auto_resolve_repairs: bool,
    success_criteria_passed: bool,
) -> (
    tempfile::TempDir,
    Arc<Mutex<FakeState>>,
    AgentSessionManager,
    RepairRecord,
) {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut finding = test_repair_finding();
    finding.context = json!({
        "untrusted": true,
        "nodes": [{ "locators": [{ "type": "test_id", "value": "pay" }] }]
    });
    let state = Arc::new(Mutex::new(FakeState {
        repairs: vec![finding],
        inspect_context: Some(ready_page_context(4)),
        ..FakeState::default()
    }));
    let manager = AgentSessionManager::new(
        vec![Arc::new(FakeDriver {
            state: Arc::clone(&state),
        })],
        SessionManagerOptions {
            artifacts_root: temp.path().to_path_buf(),
            cleanup_timeout: Duration::from_secs(1),
            max_sessions: 1,
        },
    )
    .expect("manager");
    manager
        .start(StartSessionRequest {
            session: session.to_string(),
            surface: Surface::Gui,
            goal: "Repair the checkout action".to_string(),
            success_criteria: vec!["The button works".to_string()],
            auto_resolve_repairs,
        })
        .await
        .expect("start");
    manager.ingest_repairs(session, 10).await.expect("ingest");
    let lease = u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_millis(),
    )
    .unwrap_or(u64::MAX)
    .saturating_add(300_000);
    for (request_id, status) in [
        ("claim", RepairStatus::Claimed),
        ("progress", RepairStatus::Repairing),
        ("complete", RepairStatus::Verifying),
    ] {
        manager
            .transition_repair(RepairTransition {
                session: session.to_string(),
                finding_id: "finding-1".to_string(),
                request_id: request_id.to_string(),
                status,
                actor: RepairActor::Agent,
                attempt_id: Some("attempt-1".to_string()),
                lease_expires_at_ms: (status == RepairStatus::Claimed).then_some(lease),
                summary: Some(request_id.to_string()),
                message: None,
                verification: None,
                changed_files: (status == RepairStatus::Verifying)
                    .then(|| vec!["src/Checkout.tsx".to_string()]),
            })
            .await
            .expect("repair transition");
    }
    let verified = manager
        .verify_repair(RepairVerifyRequest {
            session: session.to_string(),
            finding_id: "finding-1".to_string(),
            request_id: "verify".to_string(),
            success_criteria_passed: Some(success_criteria_passed),
            changed_files: vec!["src/Checkout.tsx".to_string()],
            checks: vec![RepairCheckResult {
                command: "npm test".to_string(),
                status: RepairCheckStatus::Passed,
                summary: "focused test passed".to_string(),
            }],
            acl_candidate: None,
            summary: "New ready revision passed browser verification".to_string(),
        })
        .await
        .expect("verify repair");
    (temp, state, manager, verified)
}

pub(crate) async fn prepare_verifying_repair(
    session: &str,
    state: Arc<Mutex<FakeState>>,
) -> (tempfile::TempDir, Arc<AgentSessionManager>) {
    let temp = tempfile::tempdir().expect("tempdir");
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
            goal: "Repair the checkout action".to_string(),
            success_criteria: vec!["The button works".to_string()],
            auto_resolve_repairs: false,
        })
        .await
        .expect("start");
    manager.ingest_repairs(session, 10).await.expect("ingest");
    let lease = u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_millis(),
    )
    .unwrap_or(u64::MAX)
    .saturating_add(300_000);
    for (request_id, status) in [
        ("claim", RepairStatus::Claimed),
        ("progress", RepairStatus::Repairing),
        ("complete", RepairStatus::Verifying),
    ] {
        manager
            .transition_repair(RepairTransition {
                session: session.to_string(),
                finding_id: "finding-1".to_string(),
                request_id: request_id.to_string(),
                status,
                actor: RepairActor::Agent,
                attempt_id: Some("attempt-1".to_string()),
                lease_expires_at_ms: (status == RepairStatus::Claimed).then_some(lease),
                summary: Some(request_id.to_string()),
                message: None,
                verification: None,
                changed_files: (status == RepairStatus::Verifying)
                    .then(|| vec!["src/Checkout.tsx".to_string()]),
            })
            .await
            .expect("repair transition");
    }
    (temp, manager)
}

pub(crate) fn repair_verify_request(session: &str) -> RepairVerifyRequest {
    RepairVerifyRequest {
        session: session.to_string(),
        finding_id: "finding-1".to_string(),
        request_id: "verify".to_string(),
        success_criteria_passed: Some(true),
        changed_files: vec!["src/Checkout.tsx".to_string()],
        checks: vec![RepairCheckResult {
            command: "npm test".to_string(),
            status: RepairCheckStatus::Passed,
            summary: "focused test passed".to_string(),
        }],
        acl_candidate: None,
        summary: "New ready revision passed browser verification".to_string(),
    }
}
