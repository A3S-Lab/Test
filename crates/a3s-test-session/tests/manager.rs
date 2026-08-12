use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use a3s_test_core::{
    Action, DriverError, DriverSession, PageContextInspectRequest, PageContextInspectScope,
    PageContextLocator, PageContextNode, PageContextNodeState, PageContextObservation,
    PageContextPage, PageContextPoint, PageContextSize, PageContextSnapshot, PageContextTheme,
    PageContextViewport, RepairAclProof, RepairActor, RepairCheckResult, RepairCheckStatus,
    RepairEvidenceBundle, RepairEvidenceRequest, RepairFinding, RepairHumanAction,
    RepairHumanActionKind, RepairIntent, RepairSeverity, RepairStatus, RepairStatusEvent,
    RepairTarget, RepairTargetKind, ScenarioContext, StepOutput, Surface, SurfaceDriver,
    SurfaceObservation, Target, TestStep,
};
use a3s_test_session::{
    ActSessionRequest, AgentSessionManager, FinishSessionRequest, RepairTransition,
    RepairVerifyRequest, SessionFinishStatus, SessionManagerOptions, StartSessionRequest,
};
use async_trait::async_trait;
use serde_json::json;
use tokio::sync::{Barrier, Mutex};

struct FakeDriver {
    state: Arc<Mutex<FakeState>>,
}

#[derive(Default)]
struct FakeState {
    opened: usize,
    actions: Vec<Action>,
    closed: usize,
    fail_observation: bool,
    page_context: bool,
    repairs: Vec<RepairFinding>,
    repair_events: Vec<RepairStatusEvent>,
    human_actions: Vec<RepairHumanAction>,
    fail_repair_projection_once: bool,
    inspect_context: Option<PageContextObservation>,
    console_errors: u32,
    page_errors: u32,
}

#[async_trait]
impl SurfaceDriver for FakeDriver {
    fn surface(&self) -> Surface {
        Surface::Gui
    }

    async fn open(
        &self,
        _context: &ScenarioContext,
    ) -> Result<Box<dyn DriverSession>, DriverError> {
        self.state.lock().await.opened += 1;
        Ok(Box::new(FakeSession {
            state: Arc::clone(&self.state),
        }))
    }
}

struct FakeSession {
    state: Arc<Mutex<FakeState>>,
}

#[async_trait]
impl DriverSession for FakeSession {
    async fn observe(&mut self) -> Result<SurfaceObservation, DriverError> {
        let mut state = self.state.lock().await;
        if state.fail_observation {
            state.fail_observation = false;
            return Err(DriverError::new(
                "test.driver.fake.observe_failed",
                "fake observation failed",
            ));
        }
        let page_context = state.page_context;
        drop(state);
        let observation = SurfaceObservation::new("fake GUI").with_data(json!({
            "elements": [{ "ref": "@g1.1", "role": "AXButton", "name": "Save" }]
        }));
        Ok(if page_context {
            observation.with_page_context(test_page_context())
        } else {
            observation
        })
    }

    async fn execute(&mut self, step: &TestStep) -> Result<StepOutput, DriverError> {
        self.state.lock().await.actions.push(step.action.clone());
        Ok(StepOutput::new("acted"))
    }

    async fn validate_page_context_revision(
        &mut self,
        expected_revision: u64,
    ) -> Result<(), DriverError> {
        if expected_revision == 3 {
            Ok(())
        } else {
            Err(DriverError::new(
                "test.driver.fake.page_context_stale",
                "fake page context revision changed",
            ))
        }
    }

    async fn inspect_page_context(
        &mut self,
        request: &PageContextInspectRequest,
    ) -> Result<PageContextObservation, DriverError> {
        if let Some(context) = self.state.lock().await.inspect_context.clone() {
            return Ok(context);
        }
        if request.scope == PageContextInspectScope::Component("checkout".to_string()) {
            Ok(test_page_context())
        } else {
            Err(DriverError::new(
                "test.driver.fake.inspect_scope_invalid",
                "unexpected fake inspect scope",
            ))
        }
    }

    async fn page_console_error_count(&mut self) -> Result<u32, DriverError> {
        Ok(self.state.lock().await.console_errors)
    }

    async fn page_error_count(&mut self) -> Result<u32, DriverError> {
        Ok(self.state.lock().await.page_errors)
    }

    async fn take_repairs(&mut self, _limit: usize) -> Result<Vec<RepairFinding>, DriverError> {
        Ok(std::mem::take(&mut self.state.lock().await.repairs))
    }

    async fn apply_repair_event(&mut self, event: &RepairStatusEvent) -> Result<(), DriverError> {
        let mut state = self.state.lock().await;
        if state.fail_repair_projection_once {
            state.fail_repair_projection_once = false;
            return Err(DriverError::new(
                "test.driver.fake.repair_projection_failed",
                "fake page projection failed",
            ));
        }
        state.repair_events.push(event.clone());
        Ok(())
    }

    async fn take_repair_actions(
        &mut self,
        _limit: usize,
    ) -> Result<Vec<RepairHumanAction>, DriverError> {
        Ok(std::mem::take(&mut self.state.lock().await.human_actions))
    }

    async fn capture_repair_evidence(
        &mut self,
        request: &RepairEvidenceRequest,
    ) -> Result<RepairEvidenceBundle, DriverError> {
        let state = self.state.lock().await;
        let context = match request.phase {
            a3s_test_core::RepairEvidencePhase::Before => ready_page_context(3),
            a3s_test_core::RepairEvidencePhase::After => state
                .inspect_context
                .clone()
                .unwrap_or_else(test_page_context),
        }
        .snapshot
        .ok_or_else(|| DriverError::new("test.driver.fake.context_missing", "context missing"))?;
        let revision = context.revision.unwrap_or(3);
        Ok(RepairEvidenceBundle {
            captured_at_ms: revision,
            context_revision: revision,
            context_sha256: "a".repeat(64),
            context,
            console_errors: state.console_errors,
            page_errors: state.page_errors,
            screenshot: a3s_test_core::Evidence {
                name: format!("{:?}", request.phase),
                path: format!("repairs/{}/evidence.png", request.finding_id),
                media_type: "image/png".to_string(),
            },
            screenshot_sha256: "b".repeat(64),
        })
    }

    async fn prove_repair_acl(
        &mut self,
        finding_id: &str,
        attempt_id: &str,
        finding_url: &str,
        candidate: &str,
    ) -> Result<RepairAclProof, DriverError> {
        a3s_test_core::TestSuite::from_repair_acl(candidate, finding_url).map_err(|error| {
            DriverError::new("test.driver.fake.acl_invalid", error.message().to_string())
        })?;
        Ok(RepairAclProof {
            path: format!("repairs/{finding_id}/{attempt_id}/regression.acl"),
            passed: true,
            summary: "fake fresh-session ACL proof passed".to_string(),
        })
    }

    async fn close(&mut self) -> Result<(), DriverError> {
        self.state.lock().await.closed += 1;
        Ok(())
    }
}

fn test_page_context() -> PageContextObservation {
    PageContextObservation::from_snapshot(PageContextSnapshot {
        protocol: Some("a3s.test.page-context/1".to_string()),
        sdk_version: Some("0.1.0".to_string()),
        revision: Some(3),
        page: None,
        components: Vec::new(),
        nodes: vec![PageContextNode {
            id: "private-n1".to_string(),
            r#ref: None,
            parent_id: Some("private-parent".to_string()),
            component_id: None,
            tag: "button".to_string(),
            role: Some("button".to_string()),
            name: Some("Pay".to_string()),
            text: Some("Pay".to_string()),
            description: None,
            test_id: Some("pay".to_string()),
            geometry: None,
            state: PageContextNodeState {
                visible: true,
                disabled: None,
                checked: None,
                selected: None,
                expanded: None,
                focused: Some(false),
                readonly: None,
                required: None,
                invalid: None,
            },
            locators: vec![PageContextLocator::TestId {
                value: "pay".to_string(),
            }],
            classes: None,
            attributes: None,
            computed_styles: None,
        }],
        facts: serde_json::Map::new(),
        removed_node_ids: vec!["private-removed".to_string()],
        truncated: false,
        next_cursor: None,
    })
}

fn ready_page_context(revision: u64) -> PageContextObservation {
    let mut context = test_page_context();
    let snapshot = context.snapshot.as_mut().expect("snapshot");
    snapshot.revision = Some(revision);
    snapshot.page = Some(PageContextPage {
        id: "checkout".to_string(),
        url: "http://127.0.0.1/checkout".to_string(),
        route: "/checkout".to_string(),
        title: "Checkout".to_string(),
        ready: true,
        viewport: PageContextViewport {
            width: 1280.0,
            height: 720.0,
            dpr: 1.0,
        },
        document: PageContextSize {
            width: 1280.0,
            height: 720.0,
        },
        scroll: PageContextPoint { x: 0.0, y: 0.0 },
        language: "en".to_string(),
        theme: PageContextTheme::Light,
    });
    context.revision = Some(revision);
    context
}

fn test_repair_finding() -> RepairFinding {
    RepairFinding {
        id: "finding-1".to_string(),
        batch_id: "batch-1".to_string(),
        instruction: "Fix the broken button".to_string(),
        success_criteria: Some("The button works".to_string()),
        intent: RepairIntent::Fix,
        severity: RepairSeverity::Important,
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
            session: "verify-repair".to_string(),
            surface: Surface::Gui,
            goal: "Repair the checkout action".to_string(),
            success_criteria: vec!["The button works".to_string()],
            auto_resolve_repairs: false,
        })
        .await
        .expect("start");
    manager
        .ingest_repairs("verify-repair", 10)
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
        ("progress", RepairStatus::Repairing),
        ("complete", RepairStatus::Verifying),
    ] {
        manager
            .transition_repair(RepairTransition {
                session: "verify-repair".to_string(),
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
            session: "verify-repair".to_string(),
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
        })
        .await
        .expect("verify repair");
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
    manager.abort("verify-repair").await.expect("abort");
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
