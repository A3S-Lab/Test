use std::sync::Arc;

use a3s_test_core::{
    DriverError, DriverSession, PageContextInspectRequest, PageContextInspectScope,
    PageContextLocator, PageContextNode, PageContextNodeState, PageContextObservation,
    PageContextPage, PageContextPoint, PageContextSize, PageContextSnapshot, PageContextTheme,
    PageContextViewport, RepairAclProof, RepairEvidenceBundle, RepairEvidenceRequest,
    RepairFinding, RepairHumanAction, RepairStatusEvent, ScenarioContext, StepOutput, Surface,
    SurfaceDriver, SurfaceObservation, TestStep,
};
use async_trait::async_trait;
use serde_json::json;
use tokio::sync::{Mutex, Notify};

pub(crate) struct FakeDriver {
    pub(crate) state: Arc<Mutex<FakeState>>,
}

#[derive(Default)]
pub(crate) struct FakeState {
    pub(crate) opened: usize,
    pub(crate) actions: Vec<a3s_test_core::Action>,
    pub(crate) closed: usize,
    pub(crate) fail_observation: bool,
    pub(crate) page_context: bool,
    pub(crate) repairs: Vec<RepairFinding>,
    pub(crate) repair_events: Vec<RepairStatusEvent>,
    pub(crate) human_actions: Vec<RepairHumanAction>,
    pub(crate) fail_repair_projection_once: bool,
    pub(crate) inspect_context: Option<PageContextObservation>,
    pub(crate) console_errors: u32,
    pub(crate) page_errors: u32,
    pub(crate) evidence_started: Option<Arc<Notify>>,
    pub(crate) evidence_release: Option<Arc<Notify>>,
    pub(crate) before_evidence_started: Option<Arc<Notify>>,
    pub(crate) before_evidence_release: Option<Arc<Notify>>,
    pub(crate) repair_wait_started: Option<Arc<Notify>>,
    pub(crate) repair_wait_release: Option<Arc<Notify>>,
    pub(crate) acl_started: Option<Arc<Notify>>,
    pub(crate) acl_release: Option<Arc<Notify>>,
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

pub(crate) struct FakeSession {
    pub(crate) state: Arc<Mutex<FakeState>>,
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

    async fn wait_for_repairs(
        &mut self,
        _limit: usize,
        _timeout_ms: u64,
        _batch_window_ms: u64,
    ) -> Result<Vec<RepairFinding>, DriverError> {
        let (started, release) = {
            let state = self.state.lock().await;
            (
                state.repair_wait_started.clone(),
                state.repair_wait_release.clone(),
            )
        };
        if let Some(started) = started {
            started.notify_one();
        }
        if let Some(release) = release {
            release.notified().await;
        }
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
        let (started, release) = {
            let state = self.state.lock().await;
            match request.phase {
                a3s_test_core::RepairEvidencePhase::After => (
                    state.evidence_started.clone(),
                    state.evidence_release.clone(),
                ),
                a3s_test_core::RepairEvidencePhase::Before => (
                    state.before_evidence_started.clone(),
                    state.before_evidence_release.clone(),
                ),
            }
        };
        if let Some(started) = started {
            started.notify_one();
        }
        if let Some(release) = release {
            release.notified().await;
        }
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
        let (started, release) = {
            let state = self.state.lock().await;
            (state.acl_started.clone(), state.acl_release.clone())
        };
        if let Some(started) = started {
            started.notify_one();
        }
        if let Some(release) = release {
            release.notified().await;
        }
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

pub(crate) fn test_page_context() -> PageContextObservation {
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

pub(crate) fn ready_page_context(revision: u64) -> PageContextObservation {
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
            visual: None,
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
