use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use a3s_test_core::{
    Action, DriverError, DriverSession, PageContextInspectRequest, PageContextInspectScope,
    RepairActor, RepairCheckResult, RepairCheckStatus, RepairFinding, RepairHumanAction,
    RepairHumanActionKind, RepairIntent, RepairSeverity, RepairStatus, RepairTarget,
    RepairTargetKind, ScenarioContext, StepOutput, Surface, SurfaceDriver, SurfaceObservation,
    Target, TestStep,
};
use a3s_test_session::{
    ActSessionRequest, AgentSessionManager, FinishSessionRequest, RepairRecord, RepairTransition,
    RepairVerifyRequest, SessionFinishStatus, SessionManagerOptions, StartSessionRequest,
};
use async_trait::async_trait;
use serde_json::json;
use tokio::sync::{Barrier, Mutex, Notify};

mod support;
use support::session_fixture::{ready_page_context, FakeDriver, FakeSession, FakeState};

fn test_repair_finding() -> RepairFinding {
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

fn manager(state: Arc<Mutex<FakeState>>) -> AgentSessionManager {
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

async fn verify_repair_with_policy(
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

async fn prepare_verifying_repair(
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
            })
            .await
            .expect("repair transition");
    }
    (temp, manager)
}

fn repair_verify_request(session: &str) -> RepairVerifyRequest {
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

#[tokio::test]
async fn projects_surface_neutral_observe_act_finish_turns() {
    let state = Arc::new(Mutex::new(FakeState::default()));
    let manager = manager(Arc::clone(&state));
    manager
        .start(StartSessionRequest {
            session: "gui_editor".to_string(),
            surface: Surface::Gui,
            goal: "Save the document".to_string(),
            success_criteria: vec!["Save completes".to_string()],
            auto_resolve_repairs: false,
        })
        .await
        .expect("start");
    let observation = manager.observe("gui_editor").await.expect("observe");
    let action = Action::Click {
        target: Target::Ref {
            value: "@g1.1".to_string(),
        },
    };
    manager
        .act(ActSessionRequest {
            session: "gui_editor".to_string(),
            observation_id: Some(observation.observation_id),
            action: action.clone(),
        })
        .await
        .expect("act");
    let finished = manager
        .finish(FinishSessionRequest {
            session: "gui_editor".to_string(),
            status: SessionFinishStatus::Passed,
            summary: "Saved".to_string(),
        })
        .await
        .expect("finish");

    assert_eq!(finished.status, SessionFinishStatus::Passed);
    let state = state.lock().await;
    assert_eq!(state.opened, 1);
    assert_eq!(state.actions, [action]);
    assert_eq!(state.closed, 1);
}

#[tokio::test]
async fn rejects_refs_not_bound_to_the_latest_observation() {
    let state = Arc::new(Mutex::new(FakeState::default()));
    let manager = manager(state);
    manager
        .start(StartSessionRequest {
            session: "stale".to_string(),
            surface: Surface::Gui,
            goal: "Click".to_string(),
            success_criteria: vec!["Clicked".to_string()],
            auto_resolve_repairs: false,
        })
        .await
        .expect("start");
    manager.observe("stale").await.expect("observe");

    let error = manager
        .act(ActSessionRequest {
            session: "stale".to_string(),
            observation_id: Some(99),
            action: Action::Click {
                target: Target::Ref {
                    value: "@g1.1".to_string(),
                },
            },
        })
        .await
        .expect_err("stale observation");
    assert_eq!(error.code(), "test.session.stale_observation");
    manager.abort("stale").await.expect("abort");
}

#[tokio::test]
async fn visual_points_require_the_latest_observation_id() {
    let state = Arc::new(Mutex::new(FakeState::default()));
    let manager = manager(state);
    manager
        .start(StartSessionRequest {
            session: "visual".to_string(),
            surface: Surface::Gui,
            goal: "Click the grounded point".to_string(),
            success_criteria: vec!["Point clicked".to_string()],
            auto_resolve_repairs: false,
        })
        .await
        .expect("start");
    let observed = manager.observe("visual").await.expect("observe");

    let error = manager
        .act(ActSessionRequest {
            session: "visual".to_string(),
            observation_id: None,
            action: Action::Click {
                target: Target::VisualPoint {
                    snapshot: "@v1".to_string(),
                    x: 20,
                    y: 30,
                },
            },
        })
        .await
        .expect_err("visual point without observation id");
    assert_eq!(error.code(), "test.session.stale_observation");

    manager
        .act(ActSessionRequest {
            session: "visual".to_string(),
            observation_id: Some(observed.observation_id),
            action: Action::Click {
                target: Target::VisualPoint {
                    snapshot: "@v1".to_string(),
                    x: 20,
                    y: 30,
                },
            },
        })
        .await
        .expect("current visual action");
    manager.abort("visual").await.expect("abort");
}

#[tokio::test]
async fn binds_private_page_nodes_to_observation_scoped_context_refs() {
    let state = Arc::new(Mutex::new(FakeState {
        page_context: true,
        ..FakeState::default()
    }));
    let manager = manager(Arc::clone(&state));
    manager
        .start(StartSessionRequest {
            session: "context-ref".to_string(),
            surface: Surface::Gui,
            goal: "Click the Test Kit target".to_string(),
            success_criteria: vec!["Target clicked".to_string()],
            auto_resolve_repairs: false,
        })
        .await
        .expect("start");

    let observed = manager.observe("context-ref").await.expect("observe");
    let snapshot = observed
        .observation
        .page_context
        .as_ref()
        .and_then(|context| context.snapshot.as_ref())
        .expect("snapshot");
    assert_eq!(snapshot.nodes[0].r#ref.as_deref(), Some("@c1"));
    assert!(snapshot.nodes[0].id.is_empty());
    assert!(snapshot.nodes[0].parent_id.is_none());
    assert!(snapshot.removed_node_ids.is_empty());

    manager
        .act(ActSessionRequest {
            session: "context-ref".to_string(),
            observation_id: Some(observed.observation_id),
            action: Action::Click {
                target: Target::Ref {
                    value: "@c1".to_string(),
                },
            },
        })
        .await
        .expect("context action");
    assert_eq!(
        state.lock().await.actions,
        [Action::Click {
            target: Target::TestId {
                value: "pay".to_string(),
            },
        }]
    );
    manager.abort("context-ref").await.expect("abort");
}

#[tokio::test]
async fn scoped_inspection_returns_fresh_context_refs() {
    let state = Arc::new(Mutex::new(FakeState::default()));
    let manager = manager(state);
    manager
        .start(StartSessionRequest {
            session: "inspect-context".to_string(),
            surface: Surface::Gui,
            goal: "Inspect the checkout component".to_string(),
            success_criteria: vec!["The component context is available".to_string()],
            auto_resolve_repairs: false,
        })
        .await
        .expect("start");

    let inspected = manager
        .inspect_page_context(
            "inspect-context",
            PageContextInspectRequest {
                detail: "scoped".to_string(),
                scope: PageContextInspectScope::Component("checkout".to_string()),
                cursor: None,
                limit: 100,
            },
        )
        .await
        .expect("inspect context");
    let snapshot = inspected
        .observation
        .page_context
        .and_then(|context| context.snapshot)
        .expect("context snapshot");
    assert_eq!(snapshot.nodes[0].r#ref.as_deref(), Some("@c1"));
    assert!(snapshot.nodes[0].id.is_empty());
    manager.abort("inspect-context").await.expect("abort");
}

#[tokio::test]
async fn retries_page_projection_without_duplicating_the_durable_transition() {
    let temp = tempfile::tempdir().expect("tempdir");
    let state = Arc::new(Mutex::new(FakeState {
        repairs: vec![test_repair_finding()],
        fail_repair_projection_once: true,
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
            session: "repair-projection".to_string(),
            surface: Surface::Gui,
            goal: "Repair the button".to_string(),
            success_criteria: vec!["Button works".to_string()],
            auto_resolve_repairs: false,
        })
        .await
        .expect("start");
    manager
        .ingest_repairs("repair-projection", 10)
        .await
        .expect("ingest");
    let transition = RepairTransition {
        session: "repair-projection".to_string(),
        finding_id: "finding-1".to_string(),
        request_id: "request-1".to_string(),
        status: RepairStatus::Claimed,
        actor: RepairActor::Agent,
        attempt_id: Some("attempt-1".to_string()),
        lease_expires_at_ms: Some(
            u64::try_from(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system clock")
                    .as_millis(),
            )
            .unwrap_or(u64::MAX)
            .saturating_add(300_000),
        ),
        summary: Some("claimed".to_string()),
        message: None,
        verification: None,
    };
    let error = manager
        .transition_repair(transition.clone())
        .await
        .expect_err("first projection must fail");
    assert_eq!(error.code(), "test.session.repair_projection_failed");
    assert!(error.retryable());

    let repaired = manager
        .transition_repair(transition)
        .await
        .expect("retry projection");
    assert_eq!(repaired.status, RepairStatus::Claimed);
    assert_eq!(repaired.sequence, 1);
    let state = state.lock().await;
    assert_eq!(state.repair_events.len(), 1);
    assert_eq!(state.repair_events[0].sequence, 1);
    drop(state);
    let ledger = tokio::fs::read_to_string(temp.path().join("repair-projection/repairs.jsonl"))
        .await
        .expect("ledger");
    assert_eq!(ledger.lines().count(), 3);
    manager.abort("repair-projection").await.expect("abort");
}

#[tokio::test]
async fn verifies_a_new_ready_revision_and_generates_an_acl_candidate() {
    let (_temp, state, manager, verified) =
        verify_repair_with_policy("verify-repair", false, true).await;
    assert_eq!(verified.status, RepairStatus::ReviewReady);
    let verification = verified.verification.expect("verification");
    assert!(verification.passed);
    assert!(verification
        .acl_proof
        .as_ref()
        .is_some_and(|proof| proof.passed));
    assert_eq!(verification.before_revision, 3);
    assert_eq!(verification.after_revision, 4);
    let acl = verification.acl_candidate.expect("ACL candidate");
    a3s_test_core::TestSuite::from_acl(&acl).expect("valid ACL candidate");
    assert!(acl.contains("testid(\"pay\")"));
    assert_eq!(
        state
            .lock()
            .await
            .repair_events
            .iter()
            .map(|event| event.status)
            .collect::<Vec<_>>(),
        [
            RepairStatus::Claimed,
            RepairStatus::Repairing,
            RepairStatus::Verifying,
            RepairStatus::ReviewReady,
        ]
    );
    manager.abort("verify-repair").await.expect("abort");
}

#[tokio::test]
async fn auto_resolves_only_after_auditing_review_ready() {
    let (temp, state, manager, verified) =
        verify_repair_with_policy("auto-resolve", true, true).await;
    assert_eq!(verified.status, RepairStatus::Resolved);
    assert!(verified
        .verification
        .as_ref()
        .is_some_and(|verification| verification.passed));
    let projected = state
        .lock()
        .await
        .repair_events
        .iter()
        .map(|event| (event.status, event.actor))
        .collect::<Vec<_>>();
    assert_eq!(
        &projected[projected.len() - 2..],
        [
            (RepairStatus::ReviewReady, RepairActor::A3sTest),
            (RepairStatus::Resolved, RepairActor::A3sTest),
        ]
    );
    let ledger = tokio::fs::read_to_string(temp.path().join("auto-resolve/repairs.jsonl"))
        .await
        .expect("ledger");
    let audited_statuses = ledger
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter_map(|event| {
            event
                .pointer("/event/status")
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
        .collect::<Vec<_>>();
    assert!(audited_statuses.ends_with(&["review_ready".to_string(), "resolved".to_string()]));
    manager.abort("auto-resolve").await.expect("abort");
}

#[tokio::test]
async fn failed_verification_never_auto_resolves() {
    let (_temp, state, manager, verified) =
        verify_repair_with_policy("failed-auto-resolve", true, false).await;
    assert_eq!(verified.status, RepairStatus::VerificationFailed);
    assert!(verified
        .verification
        .as_ref()
        .is_some_and(|verification| !verification.passed));
    let projected = state
        .lock()
        .await
        .repair_events
        .iter()
        .map(|event| event.status)
        .collect::<Vec<_>>();
    assert_eq!(projected.last(), Some(&RepairStatus::VerificationFailed));
    assert!(!projected.contains(&RepairStatus::ReviewReady));
    assert!(!projected.contains(&RepairStatus::Resolved));
    manager.abort("failed-auto-resolve").await.expect("abort");
}

#[tokio::test]
async fn verification_does_not_hold_the_workspace_lock_during_browser_work() {
    let evidence_started = Arc::new(Notify::new());
    let evidence_release = Arc::new(Notify::new());
    let acl_started = Arc::new(Notify::new());
    let acl_release = Arc::new(Notify::new());
    let mut finding = test_repair_finding();
    finding.context = json!({
        "untrusted": true,
        "nodes": [{ "locators": [{ "type": "test_id", "value": "pay" }] }]
    });
    let state = Arc::new(Mutex::new(FakeState {
        repairs: vec![finding],
        inspect_context: Some(ready_page_context(4)),
        evidence_started: Some(Arc::clone(&evidence_started)),
        evidence_release: Some(Arc::clone(&evidence_release)),
        acl_started: Some(Arc::clone(&acl_started)),
        acl_release: Some(Arc::clone(&acl_release)),
        ..FakeState::default()
    }));
    let (temp, manager) = prepare_verifying_repair("short-lock", state).await;
    let verification = tokio::spawn({
        let manager = Arc::clone(&manager);
        async move {
            manager
                .verify_repair(repair_verify_request("short-lock"))
                .await
        }
    });

    tokio::time::timeout(Duration::from_secs(1), evidence_started.notified())
        .await
        .expect("after evidence started");
    let workspace = a3s_test_session::RepairWorkspace::from_artifacts_root(temp.path());
    let lock = tokio::time::timeout(Duration::from_millis(100), workspace.acquire())
        .await
        .expect("workspace lock was held during evidence capture")
        .expect("workspace lock");
    drop(lock);
    evidence_release.notify_one();

    tokio::time::timeout(Duration::from_secs(1), acl_started.notified())
        .await
        .expect("ACL proof started");
    let lock = tokio::time::timeout(Duration::from_millis(100), workspace.acquire())
        .await
        .expect("workspace lock was held during ACL proof")
        .expect("workspace lock");
    drop(lock);
    acl_release.notify_one();

    let verified = tokio::time::timeout(Duration::from_secs(1), verification)
        .await
        .expect("verification deadline")
        .expect("verification task")
        .expect("verification result");
    assert_eq!(verified.status, RepairStatus::ReviewReady);
    manager.abort("short-lock").await.expect("abort");
}

#[tokio::test]
async fn ingests_human_clarification_and_projects_the_authoritative_transition() {
    let temp = tempfile::tempdir().expect("tempdir");
    let state = Arc::new(Mutex::new(FakeState {
        repairs: vec![test_repair_finding()],
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
            session: "human-reply".to_string(),
            surface: Surface::Gui,
            goal: "Repair with human clarification".to_string(),
            success_criteria: vec!["Repair is clarified".to_string()],
            auto_resolve_repairs: false,
        })
        .await
        .expect("start");
    manager
        .ingest_repairs("human-reply", 10)
        .await
        .expect("ingest");
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
        ("question", RepairStatus::NeedsInput),
    ] {
        manager
            .transition_repair(RepairTransition {
                session: "human-reply".to_string(),
                finding_id: "finding-1".to_string(),
                request_id: request_id.to_string(),
                status,
                actor: RepairActor::Agent,
                attempt_id: Some("attempt-1".to_string()),
                lease_expires_at_ms: (status == RepairStatus::Claimed).then_some(lease),
                summary: Some(request_id.to_string()),
                message: (status == RepairStatus::NeedsInput).then(|| "Which state?".to_string()),
                verification: None,
            })
            .await
            .expect("repair transition");
    }
    state.lock().await.human_actions.push(RepairHumanAction {
        request_id: "human-reply-1".to_string(),
        finding_id: "finding-1".to_string(),
        action: RepairHumanActionKind::Reply,
        timestamp: "2026-08-13T00:00:00Z".to_string(),
        message: Some("Use the enabled state".to_string()),
    });

    let queued = manager
        .watch_repairs("human-reply", 10, 0, 0)
        .await
        .expect("watch human reply");
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].status, RepairStatus::Queued);
    assert_eq!(queued[0].attempts[0].replies.len(), 2);
    let state_guard = state.lock().await;
    assert_eq!(
        state_guard.repair_events.last().expect("event").actor,
        RepairActor::Human
    );
    assert_eq!(
        state_guard.repair_events.last().expect("event").status,
        RepairStatus::Queued
    );
    drop(state_guard);
    manager.abort("human-reply").await.expect("abort");
}

struct CancellationDriver {
    attempts: Arc<AtomicUsize>,
    state: Arc<Mutex<FakeState>>,
}

#[async_trait]
impl SurfaceDriver for CancellationDriver {
    fn surface(&self) -> Surface {
        Surface::Gui
    }

    async fn open(
        &self,
        _context: &ScenarioContext,
    ) -> Result<Box<dyn DriverSession>, DriverError> {
        if self.attempts.fetch_add(1, Ordering::SeqCst) == 0 {
            return std::future::pending().await;
        }
        Ok(Box::new(FakeSession {
            state: Arc::clone(&self.state),
        }))
    }
}

#[tokio::test]
async fn cancelled_open_releases_the_session_reservation() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let state = Arc::new(Mutex::new(FakeState::default()));
    let manager = Arc::new(
        AgentSessionManager::new(
            vec![Arc::new(CancellationDriver {
                attempts: Arc::clone(&attempts),
                state,
            })],
            SessionManagerOptions {
                artifacts_root: std::env::temp_dir().join("a3s-test-session-cancellation"),
                cleanup_timeout: Duration::from_secs(1),
                max_sessions: 1,
            },
        )
        .expect("manager"),
    );
    let request = StartSessionRequest {
        session: "cancelled-open".to_string(),
        surface: Surface::Gui,
        goal: "Open safely".to_string(),
        success_criteria: vec!["Opened".to_string()],
        auto_resolve_repairs: false,
    };
    let first = tokio::spawn({
        let manager = Arc::clone(&manager);
        let request = request.clone();
        async move { manager.start(request).await }
    });
    while attempts.load(Ordering::SeqCst) == 0 {
        tokio::task::yield_now().await;
    }
    first.abort();
    let _ = first.await;

    manager
        .start(request)
        .await
        .expect("reservation released after cancellation");
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
    manager.abort("cancelled-open").await.expect("abort");
}

#[tokio::test]
async fn failed_observation_invalidates_the_previous_refs() {
    let state = Arc::new(Mutex::new(FakeState::default()));
    let manager = manager(Arc::clone(&state));
    manager
        .start(StartSessionRequest {
            session: "observe-failure".to_string(),
            surface: Surface::Gui,
            goal: "Act only on fresh state".to_string(),
            success_criteria: vec!["No stale action".to_string()],
            auto_resolve_repairs: false,
        })
        .await
        .expect("start");
    let observation = manager
        .observe("observe-failure")
        .await
        .expect("first observation");
    state.lock().await.fail_observation = true;
    let error = manager
        .observe("observe-failure")
        .await
        .expect_err("second observation failure");
    assert_eq!(error.code(), "test.driver.fake.observe_failed");

    let stale = manager
        .act(ActSessionRequest {
            session: "observe-failure".to_string(),
            observation_id: Some(observation.observation_id),
            action: Action::Click {
                target: Target::Ref {
                    value: "@g1.1".to_string(),
                },
            },
        })
        .await
        .expect_err("old ref after failed observation");
    assert_eq!(stale.code(), "test.session.observation_required");
    manager.abort("observe-failure").await.expect("abort");
}

struct BarrierCloseDriver {
    barrier: Arc<Barrier>,
    closed: Arc<AtomicUsize>,
}

#[async_trait]
impl SurfaceDriver for BarrierCloseDriver {
    fn surface(&self) -> Surface {
        Surface::Gui
    }

    async fn open(
        &self,
        _context: &ScenarioContext,
    ) -> Result<Box<dyn DriverSession>, DriverError> {
        Ok(Box::new(BarrierCloseSession {
            barrier: Arc::clone(&self.barrier),
            closed: Arc::clone(&self.closed),
        }))
    }
}

struct BarrierCloseSession {
    barrier: Arc<Barrier>,
    closed: Arc<AtomicUsize>,
}

#[async_trait]
impl DriverSession for BarrierCloseSession {
    async fn observe(&mut self) -> Result<SurfaceObservation, DriverError> {
        Ok(SurfaceObservation::new("barrier"))
    }

    async fn execute(&mut self, _step: &TestStep) -> Result<StepOutput, DriverError> {
        Ok(StepOutput::new("barrier"))
    }

    async fn close(&mut self) -> Result<(), DriverError> {
        self.barrier.wait().await;
        self.closed.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[tokio::test]
async fn close_all_cleans_independent_sessions_concurrently() {
    const SESSION_COUNT: usize = 3;

    let barrier = Arc::new(Barrier::new(SESSION_COUNT));
    let closed = Arc::new(AtomicUsize::new(0));
    let manager = AgentSessionManager::new(
        vec![Arc::new(BarrierCloseDriver {
            barrier,
            closed: Arc::clone(&closed),
        })],
        SessionManagerOptions {
            artifacts_root: std::env::temp_dir().join("a3s-test-session-close-all"),
            cleanup_timeout: Duration::from_millis(250),
            max_sessions: SESSION_COUNT,
        },
    )
    .expect("manager");
    for index in 0..SESSION_COUNT {
        manager
            .start(StartSessionRequest {
                session: format!("close-{index}"),
                surface: Surface::Gui,
                goal: "Close concurrently".to_string(),
                success_criteria: vec!["Closed".to_string()],
                auto_resolve_repairs: false,
            })
            .await
            .expect("start");
    }

    let results = manager.close_all().await;
    assert_eq!(results.len(), SESSION_COUNT);
    assert!(results.iter().all(|result| result.cleanup_error.is_none()));
    assert_eq!(closed.load(Ordering::SeqCst), SESSION_COUNT);
}
