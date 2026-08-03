use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use a3s_test_core::{
    Action, DriverError, DriverSession, ScenarioContext, StepOutput, Surface, SurfaceDriver,
    SurfaceObservation, Target, TestStep,
};
use a3s_test_session::{
    ActSessionRequest, AgentSessionManager, FinishSessionRequest, SessionFinishStatus,
    SessionManagerOptions, StartSessionRequest,
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
        drop(state);
        Ok(SurfaceObservation::new("fake GUI").with_data(json!({
            "elements": [{ "ref": "@g1.1", "role": "AXButton", "name": "Save" }]
        })))
    }

    async fn execute(&mut self, step: &TestStep) -> Result<StepOutput, DriverError> {
        self.state.lock().await.actions.push(step.action.clone());
        Ok(StepOutput::new("acted"))
    }

    async fn close(&mut self) -> Result<(), DriverError> {
        self.state.lock().await.closed += 1;
        Ok(())
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
            })
            .await
            .expect("start");
    }

    let results = manager.close_all().await;
    assert_eq!(results.len(), SESSION_COUNT);
    assert!(results.iter().all(|result| result.cleanup_error.is_none()));
    assert_eq!(closed.load(Ordering::SeqCst), SESSION_COUNT);
}
