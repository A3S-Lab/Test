use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use a3s_test_core::{
    DriverError, DriverSession, ScenarioContext, StepOutput, Surface, SurfaceDriver,
    SurfaceObservation, TestStep,
};
use a3s_test_session::{
    AgentSessionManager, FinishSessionRequest, SessionFinishStatus, SessionManagerOptions,
    StartSessionRequest,
};
use async_trait::async_trait;
use tokio::sync::Notify;

struct CloseDriver {
    attempts: Arc<AtomicUsize>,
    behavior: CloseBehavior,
}

#[derive(Clone)]
enum CloseBehavior {
    FailOnce,
    ControlledOnce {
        control: Arc<CloseControl>,
        fail: bool,
    },
}

#[derive(Default)]
struct CloseControl {
    started: Notify,
    release: Notify,
}

#[async_trait]
impl SurfaceDriver for CloseDriver {
    fn surface(&self) -> Surface {
        Surface::Gui
    }

    async fn open(
        &self,
        _context: &ScenarioContext,
    ) -> Result<Box<dyn DriverSession>, DriverError> {
        Ok(Box::new(CloseSession {
            attempts: Arc::clone(&self.attempts),
            behavior: self.behavior.clone(),
        }))
    }
}

struct CloseSession {
    attempts: Arc<AtomicUsize>,
    behavior: CloseBehavior,
}

#[async_trait]
impl DriverSession for CloseSession {
    async fn observe(&mut self) -> Result<SurfaceObservation, DriverError> {
        Ok(SurfaceObservation::new("cleanup retry fixture"))
    }

    async fn execute(&mut self, _step: &TestStep) -> Result<StepOutput, DriverError> {
        Ok(StepOutput::new("cleanup retry fixture"))
    }

    async fn close(&mut self) -> Result<(), DriverError> {
        let attempt = self.attempts.fetch_add(1, Ordering::SeqCst);
        if attempt > 0 {
            return Ok(());
        }
        match &self.behavior {
            CloseBehavior::FailOnce => Err(DriverError::new(
                "test.driver.fake.cleanup_failed",
                "transient cleanup failure",
            )
            .with_retryable(true)),
            CloseBehavior::ControlledOnce { control, fail } => {
                control.started.notify_one();
                control.release.notified().await;
                if *fail {
                    Err(DriverError::new(
                        "test.driver.fake.cleanup_failed",
                        "delayed transient cleanup failure",
                    )
                    .with_retryable(true))
                } else {
                    Ok(())
                }
            }
        }
    }
}

#[tokio::test]
async fn retryable_cleanup_failure_preserves_only_terminal_operations() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let manager = manager(
        Arc::clone(&attempts),
        CloseBehavior::FailOnce,
        Duration::from_secs(1),
    );
    start(&manager, "retryable-cleanup").await;

    let first = manager
        .finish(finish_request("retryable-cleanup"))
        .await
        .expect("first finish result");
    assert_eq!(first.status, SessionFinishStatus::Failed);
    let cleanup = first.cleanup_error.expect("cleanup failure");
    assert_eq!(cleanup.code, "test.driver.fake.cleanup_failed");
    assert!(cleanup.retryable);

    let turn_error = manager
        .observe("retryable-cleanup")
        .await
        .expect_err("turn after cleanup failure");
    assert_eq!(turn_error.code(), "test.session.cleanup_required");

    let duplicate = manager
        .start(start_request("retryable-cleanup"))
        .await
        .expect_err("replacement before cleanup");
    assert_eq!(duplicate.code(), "test.session.already_exists");

    let retried = manager
        .abort("retryable-cleanup")
        .await
        .expect("retry cleanup");
    assert_eq!(retried.status, SessionFinishStatus::Aborted);
    assert!(retried.cleanup_error.is_none());
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn cleanup_timeout_does_not_cancel_eventual_success() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let control = Arc::new(CloseControl::default());
    let manager = manager(
        Arc::clone(&attempts),
        CloseBehavior::ControlledOnce {
            control: Arc::clone(&control),
            fail: false,
        },
        Duration::from_millis(10),
    );
    start(&manager, "cleanup-timeout").await;

    let timed_out = manager
        .finish(finish_request("cleanup-timeout"))
        .await
        .expect("timed-out finish result");
    assert_eq!(timed_out.status, SessionFinishStatus::Failed);
    let cleanup = timed_out.cleanup_error.expect("cleanup timeout");
    assert_eq!(cleanup.code, "test.session.cleanup_timeout");
    assert!(cleanup.retryable);

    let turn_error = manager
        .observe("cleanup-timeout")
        .await
        .expect_err("turn while background cleanup runs");
    assert_eq!(turn_error.code(), "test.session.cleanup_in_progress");
    assert!(turn_error.retryable());

    control.release.notify_one();
    wait_for_session_error(&manager, "cleanup-timeout", "test.session.not_found").await;
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn delayed_retryable_failure_becomes_cleanup_required() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let control = Arc::new(CloseControl::default());
    let manager = manager(
        Arc::clone(&attempts),
        CloseBehavior::ControlledOnce {
            control: Arc::clone(&control),
            fail: true,
        },
        Duration::from_millis(10),
    );
    start(&manager, "delayed-cleanup-failure").await;

    let timed_out = manager
        .finish(finish_request("delayed-cleanup-failure"))
        .await
        .expect("timed-out finish result");
    assert_eq!(timed_out.status, SessionFinishStatus::Failed);
    assert_eq!(
        timed_out.cleanup_error.expect("cleanup timeout").code,
        "test.session.cleanup_timeout"
    );
    let in_progress = manager
        .observe("delayed-cleanup-failure")
        .await
        .expect_err("cleanup still running");
    assert_eq!(in_progress.code(), "test.session.cleanup_in_progress");
    assert!(in_progress.retryable());

    control.release.notify_one();
    wait_for_session_error(
        &manager,
        "delayed-cleanup-failure",
        "test.session.cleanup_required",
    )
    .await;
    let retried = manager
        .abort("delayed-cleanup-failure")
        .await
        .expect("retry delayed cleanup failure");
    assert_eq!(retried.status, SessionFinishStatus::Aborted);
    assert!(retried.cleanup_error.is_none());
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn cancelling_the_finish_caller_does_not_cancel_cleanup() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let control = Arc::new(CloseControl::default());
    let manager = Arc::new(manager(
        Arc::clone(&attempts),
        CloseBehavior::ControlledOnce {
            control: Arc::clone(&control),
            fail: false,
        },
        Duration::from_secs(1),
    ));
    start(&manager, "cancelled-finish").await;

    let finish = tokio::spawn({
        let manager = Arc::clone(&manager);
        async move { manager.finish(finish_request("cancelled-finish")).await }
    });
    control.started.notified().await;
    finish.abort();
    let _ = finish.await;

    let in_progress = manager
        .observe("cancelled-finish")
        .await
        .expect_err("cleanup continues after caller cancellation");
    assert_eq!(in_progress.code(), "test.session.cleanup_in_progress");
    assert!(in_progress.retryable());
    control.release.notify_one();
    wait_for_session_error(&manager, "cancelled-finish", "test.session.not_found").await;
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn close_all_waits_for_a_cleanup_that_outlived_its_caller_deadline() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let control = Arc::new(CloseControl::default());
    let manager = Arc::new(manager(
        Arc::clone(&attempts),
        CloseBehavior::ControlledOnce {
            control: Arc::clone(&control),
            fail: false,
        },
        Duration::from_millis(50),
    ));
    start(&manager, "close-all-drain").await;
    let timed_out = manager
        .finish(finish_request("close-all-drain"))
        .await
        .expect("timed-out finish result");
    assert_eq!(
        timed_out.cleanup_error.expect("cleanup timeout").code,
        "test.session.cleanup_timeout"
    );

    let close_all = tokio::spawn({
        let manager = Arc::clone(&manager);
        async move { manager.close_all().await }
    });
    tokio::task::yield_now().await;
    assert!(!close_all.is_finished());
    control.release.notify_one();
    let results = tokio::time::timeout(Duration::from_secs(1), close_all)
        .await
        .expect("close-all deadline")
        .expect("close-all task");
    assert!(results.is_empty());
    let completed = manager
        .abort("close-all-drain")
        .await
        .expect_err("background cleanup completed");
    assert_eq!(completed.code(), "test.session.not_found");
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
}

fn manager(
    attempts: Arc<AtomicUsize>,
    behavior: CloseBehavior,
    cleanup_timeout: Duration,
) -> AgentSessionManager {
    AgentSessionManager::new(
        vec![Arc::new(CloseDriver { attempts, behavior })],
        SessionManagerOptions {
            artifacts_root: std::env::temp_dir().join("a3s-test-session-cleanup-retry"),
            cleanup_timeout,
            max_sessions: 1,
        },
    )
    .expect("session manager")
}

async fn start(manager: &AgentSessionManager, session: &str) {
    manager
        .start(start_request(session))
        .await
        .expect("start session");
}

fn start_request(session: &str) -> StartSessionRequest {
    StartSessionRequest {
        session: session.to_string(),
        surface: Surface::Gui,
        goal: "Close the owned GUI safely".to_string(),
        success_criteria: vec!["No owned resource survives cleanup".to_string()],
    }
}

fn finish_request(session: &str) -> FinishSessionRequest {
    FinishSessionRequest {
        session: session.to_string(),
        status: SessionFinishStatus::Passed,
        summary: "Product behavior passed".to_string(),
    }
}

async fn wait_for_session_error(manager: &AgentSessionManager, session: &str, expected: &str) {
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let error = manager
                .observe(session)
                .await
                .expect_err("session must not admit another turn");
            if error.code() == expected {
                break;
            }
            assert_eq!(error.code(), "test.session.cleanup_in_progress");
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("cleanup state transition deadline");
}
