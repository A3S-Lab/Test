use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use a3s_test_core::{
    Action, DriverError, DriverSession, ScenarioContext, StepOutput, Surface, SurfaceDriver,
    TestScenario, TestStep, TestSuite,
};
use a3s_test_runner::{RunStatus, Runner, RunnerOptions};
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
struct FakeDriver {
    closes: Arc<AtomicUsize>,
    behavior: Behavior,
    close_behavior: CloseBehavior,
}

#[derive(Clone, Copy)]
enum Behavior {
    Pass,
    Fail,
    Hang,
}

#[derive(Clone, Copy)]
enum CloseBehavior {
    Pass,
    Fail,
    Hang,
}

struct FakeSession {
    closes: Arc<AtomicUsize>,
    behavior: Behavior,
    close_behavior: CloseBehavior,
}

#[async_trait]
impl SurfaceDriver for FakeDriver {
    fn surface(&self) -> Surface {
        Surface::Web
    }

    async fn open(
        &self,
        _context: &ScenarioContext,
    ) -> Result<Box<dyn DriverSession>, DriverError> {
        Ok(Box::new(FakeSession {
            closes: Arc::clone(&self.closes),
            behavior: self.behavior,
            close_behavior: self.close_behavior,
        }))
    }
}

#[async_trait]
impl DriverSession for FakeSession {
    async fn execute(&mut self, _step: &TestStep) -> Result<StepOutput, DriverError> {
        match self.behavior {
            Behavior::Pass => Ok(StepOutput::new("ok")),
            Behavior::Fail => Err(DriverError::new("fake.failure", "planned failure")),
            Behavior::Hang => std::future::pending().await,
        }
    }

    async fn close(&mut self) -> Result<(), DriverError> {
        self.closes.fetch_add(1, Ordering::SeqCst);
        match self.close_behavior {
            CloseBehavior::Pass => Ok(()),
            CloseBehavior::Fail => Err(DriverError::new("fake.close", "planned close failure")),
            CloseBehavior::Hang => std::future::pending().await,
        }
    }
}

fn suite(timeout_ms: u64) -> TestSuite {
    TestSuite {
        name: "lifecycle".to_string(),
        version: 1,
        scenarios: vec![TestScenario {
            id: "scenario".to_string(),
            name: "Scenario".to_string(),
            surface: Surface::Web,
            timeout_ms,
            steps: vec![TestStep {
                id: "step".to_string(),
                action: Action::Snapshot { interactive: true },
            }],
        }],
    }
}

fn runner(behavior: Behavior, closes: Arc<AtomicUsize>) -> Runner {
    runner_with_cleanup(
        behavior,
        CloseBehavior::Pass,
        closes,
        Duration::from_secs(1),
    )
}

fn runner_with_cleanup(
    behavior: Behavior,
    close_behavior: CloseBehavior,
    closes: Arc<AtomicUsize>,
    cleanup_timeout: Duration,
) -> Runner {
    Runner::new(
        vec![Arc::new(FakeDriver {
            closes,
            behavior,
            close_behavior,
        })],
        RunnerOptions { cleanup_timeout },
    )
    .expect("runner")
}

#[tokio::test]
async fn closes_surface_after_success() {
    let closes = Arc::new(AtomicUsize::new(0));
    let result = runner(Behavior::Pass, Arc::clone(&closes))
        .run(&suite(1_000), CancellationToken::new())
        .await;

    assert_eq!(result.status, RunStatus::Passed);
    assert_eq!(closes.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn closes_surface_after_step_failure() {
    let closes = Arc::new(AtomicUsize::new(0));
    let result = runner(Behavior::Fail, Arc::clone(&closes))
        .run(&suite(1_000), CancellationToken::new())
        .await;

    assert_eq!(result.status, RunStatus::Failed);
    assert_eq!(closes.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn closes_surface_after_scenario_timeout() {
    let closes = Arc::new(AtomicUsize::new(0));
    let result = runner(Behavior::Hang, Arc::clone(&closes))
        .run(&suite(20), CancellationToken::new())
        .await;

    assert_eq!(result.status, RunStatus::TimedOut);
    assert_eq!(closes.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn closes_surface_after_cancellation() {
    let closes = Arc::new(AtomicUsize::new(0));
    let cancellation = CancellationToken::new();
    let trigger = cancellation.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(20)).await;
        trigger.cancel();
    });

    let result = runner(Behavior::Hang, Arc::clone(&closes))
        .run(&suite(1_000), cancellation)
        .await;

    assert_eq!(result.status, RunStatus::Cancelled);
    assert_eq!(closes.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn reports_close_failure_as_a_failed_run() {
    let closes = Arc::new(AtomicUsize::new(0));
    let result = runner_with_cleanup(
        Behavior::Pass,
        CloseBehavior::Fail,
        Arc::clone(&closes),
        Duration::from_secs(1),
    )
    .run(&suite(1_000), CancellationToken::new())
    .await;

    assert_eq!(result.status, RunStatus::Failed);
    assert_eq!(
        result.scenarios[0]
            .cleanup_error
            .as_ref()
            .map(|error| error.code.as_str()),
        Some("fake.close")
    );
    assert_eq!(closes.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn bounds_a_hung_close_operation() {
    let closes = Arc::new(AtomicUsize::new(0));
    let result = runner_with_cleanup(
        Behavior::Pass,
        CloseBehavior::Hang,
        Arc::clone(&closes),
        Duration::from_millis(20),
    )
    .run(&suite(1_000), CancellationToken::new())
    .await;

    assert_eq!(result.status, RunStatus::Failed);
    assert_eq!(
        result.scenarios[0]
            .cleanup_error
            .as_ref()
            .map(|error| error.code.as_str()),
        Some("test.run.cleanup_timeout")
    );
    assert_eq!(closes.load(Ordering::SeqCst), 1);
}
