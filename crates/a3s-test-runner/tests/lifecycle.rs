use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use a3s_test_core::{
    Action, DriverError, DriverSession, ScenarioContext, StepOutput, Surface, SurfaceDriver,
    TestScenario, TestStep, TestSuite,
};
use a3s_test_runner::{RetryPolicy, RunStatus, Runner, RunnerOptions};
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
        RunnerOptions {
            cleanup_timeout,
            ..RunnerOptions::default()
        },
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

#[derive(Clone)]
struct RetryDriver {
    executions: Arc<AtomicUsize>,
    retryable: bool,
}

struct RetrySession {
    executions: Arc<AtomicUsize>,
    retryable: bool,
}

#[async_trait]
impl SurfaceDriver for RetryDriver {
    fn surface(&self) -> Surface {
        Surface::Web
    }

    async fn open(
        &self,
        _context: &ScenarioContext,
    ) -> Result<Box<dyn DriverSession>, DriverError> {
        Ok(Box::new(RetrySession {
            executions: Arc::clone(&self.executions),
            retryable: self.retryable,
        }))
    }
}

#[async_trait]
impl DriverSession for RetrySession {
    async fn execute(&mut self, _step: &TestStep) -> Result<StepOutput, DriverError> {
        let attempt = self.executions.fetch_add(1, Ordering::SeqCst);
        if attempt == 0 {
            return Err(
                DriverError::new("fake.infrastructure", "driver was not started")
                    .with_retryable(self.retryable),
            );
        }
        Ok(StepOutput::new("recovered"))
    }

    async fn close(&mut self) -> Result<(), DriverError> {
        Ok(())
    }
}

#[tokio::test]
async fn retries_only_explicitly_retryable_infrastructure_failures() {
    let executions = Arc::new(AtomicUsize::new(0));
    let runner = Runner::new(
        vec![Arc::new(RetryDriver {
            executions: Arc::clone(&executions),
            retryable: true,
        })],
        RunnerOptions {
            retry_policy: RetryPolicy {
                max_retries: 1,
                backoff: Duration::ZERO,
            },
            ..RunnerOptions::default()
        },
    )
    .expect("runner");

    let result = runner.run(&suite(1_000), CancellationToken::new()).await;

    assert_eq!(result.status, RunStatus::Passed);
    assert_eq!(result.scenarios[0].steps[0].attempts, 2);
    assert_eq!(executions.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn never_retries_a_non_retryable_product_failure() {
    let executions = Arc::new(AtomicUsize::new(0));
    let runner = Runner::new(
        vec![Arc::new(RetryDriver {
            executions: Arc::clone(&executions),
            retryable: false,
        })],
        RunnerOptions {
            retry_policy: RetryPolicy {
                max_retries: 3,
                backoff: Duration::ZERO,
            },
            ..RunnerOptions::default()
        },
    )
    .expect("runner");

    let result = runner.run(&suite(1_000), CancellationToken::new()).await;

    assert_eq!(result.status, RunStatus::Failed);
    assert_eq!(result.scenarios[0].steps[0].attempts, 1);
    assert_eq!(executions.load(Ordering::SeqCst), 1);
}

#[derive(Clone)]
struct ConcurrencyDriver {
    active: Arc<AtomicUsize>,
    maximum: Arc<AtomicUsize>,
}

struct ConcurrencySession {
    active: Arc<AtomicUsize>,
    maximum: Arc<AtomicUsize>,
}

#[async_trait]
impl SurfaceDriver for ConcurrencyDriver {
    fn surface(&self) -> Surface {
        Surface::Web
    }

    async fn open(
        &self,
        _context: &ScenarioContext,
    ) -> Result<Box<dyn DriverSession>, DriverError> {
        Ok(Box::new(ConcurrencySession {
            active: Arc::clone(&self.active),
            maximum: Arc::clone(&self.maximum),
        }))
    }
}

#[async_trait]
impl DriverSession for ConcurrencySession {
    async fn execute(&mut self, _step: &TestStep) -> Result<StepOutput, DriverError> {
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.maximum.fetch_max(active, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(30)).await;
        self.active.fetch_sub(1, Ordering::SeqCst);
        Ok(StepOutput::new("ok"))
    }

    async fn close(&mut self) -> Result<(), DriverError> {
        Ok(())
    }
}

#[tokio::test]
async fn bounds_parallel_scenarios_and_preserves_manifest_order() {
    let active = Arc::new(AtomicUsize::new(0));
    let maximum = Arc::new(AtomicUsize::new(0));
    let driver = ConcurrencyDriver {
        active,
        maximum: Arc::clone(&maximum),
    };
    let mut parallel_suite = suite(1_000);
    parallel_suite.scenarios = (0..5)
        .map(|index| {
            let mut scenario = parallel_suite.scenarios[0].clone();
            scenario.id = format!("scenario-{index}");
            scenario.name = format!("Scenario {index}");
            scenario
        })
        .collect();
    let runner = Runner::new(
        vec![Arc::new(driver)],
        RunnerOptions {
            max_parallel_scenarios: 2,
            ..RunnerOptions::default()
        },
    )
    .expect("runner");

    let result = runner.run(&parallel_suite, CancellationToken::new()).await;

    assert_eq!(result.status, RunStatus::Passed);
    assert_eq!(maximum.load(Ordering::SeqCst), 2);
    assert_eq!(
        result
            .scenarios
            .iter()
            .map(|scenario| scenario.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "scenario-0",
            "scenario-1",
            "scenario-2",
            "scenario-3",
            "scenario-4",
        ]
    );
}
