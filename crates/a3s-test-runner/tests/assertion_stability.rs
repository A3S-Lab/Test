use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use a3s_test_core::{
    Action, AssertionStability, DriverError, DriverSession, Expectation, ScenarioContext,
    StepOutput, Surface, SurfaceDriver, Target, TestScenario, TestStep, TestSuite,
};
use a3s_test_runner::{RetryPolicy, RunStatus, Runner, RunnerOptions};
use async_trait::async_trait;
use serde_json::json;
use tokio_util::sync::CancellationToken;

const DATASET_SIZE: usize = 100;

#[derive(Clone)]
struct TransientDriver {
    executions: Arc<AtomicUsize>,
    remains_visible: bool,
}

struct TransientSession {
    executions: Arc<AtomicUsize>,
    remains_visible: bool,
    sample: usize,
}

#[async_trait]
impl SurfaceDriver for TransientDriver {
    fn surface(&self) -> Surface {
        Surface::Web
    }

    async fn open(
        &self,
        _context: &ScenarioContext,
    ) -> Result<Box<dyn DriverSession>, DriverError> {
        Ok(Box::new(TransientSession {
            executions: Arc::clone(&self.executions),
            remains_visible: self.remains_visible,
            sample: 0,
        }))
    }
}

#[async_trait]
impl DriverSession for TransientSession {
    async fn execute(&mut self, step: &TestStep) -> Result<StepOutput, DriverError> {
        assert!(matches!(step.action, Action::Assert { .. }));
        self.executions.fetch_add(1, Ordering::SeqCst);
        self.sample += 1;
        if self.sample == 1 || self.remains_visible {
            return Ok(
                StepOutput::new("scripted assertion matched").with_data(json!({ "visible": true }))
            );
        }
        Err(DriverError::new(
            "test.assert.visible",
            "scripted target disappeared after the first sample",
        ))
    }

    async fn close(&mut self) -> Result<(), DriverError> {
        Ok(())
    }
}

#[tokio::test]
async fn immediate_assertions_accept_the_first_sample_of_100_scripted_transients() {
    let executions = Arc::new(AtomicUsize::new(0));
    let runner = scripted_runner(Arc::clone(&executions), false);

    let result = runner
        .run(&transient_suite(None), CancellationToken::new())
        .await;

    assert_eq!(result.status, RunStatus::Passed);
    assert_eq!(
        result
            .scenarios
            .iter()
            .filter(|scenario| scenario.status == RunStatus::Passed)
            .count(),
        DATASET_SIZE
    );
    assert_eq!(executions.load(Ordering::SeqCst), DATASET_SIZE);
}

#[tokio::test]
async fn stable_assertions_reject_100_of_100_scripted_transients() {
    let executions = Arc::new(AtomicUsize::new(0));
    let runner = scripted_runner(Arc::clone(&executions), false);
    let stability = AssertionStability {
        stable_for_ms: 20,
        sample_interval_ms: 5,
    };

    let result = runner
        .run(&transient_suite(Some(stability)), CancellationToken::new())
        .await;

    assert_eq!(result.status, RunStatus::Failed);
    assert_eq!(
        result
            .scenarios
            .iter()
            .filter(|scenario| scenario.status == RunStatus::Failed)
            .count(),
        DATASET_SIZE
    );
    for scenario in &result.scenarios {
        let step = &scenario.steps[0];
        assert_eq!(step.attempts, 2);
        assert_eq!(
            step.error.as_ref().map(|error| error.code.as_str()),
            Some("test.assert.unstable")
        );
        assert_eq!(
            step.output.as_ref().expect("stability evidence").data["stability"]["samples"],
            2
        );
    }
    assert_eq!(executions.load(Ordering::SeqCst), DATASET_SIZE * 2);
}

#[tokio::test]
async fn stable_assertions_accept_100_of_100_consistent_states_with_bounded_metrics() {
    let executions = Arc::new(AtomicUsize::new(0));
    let runner = scripted_runner(Arc::clone(&executions), true);
    let stability = AssertionStability {
        stable_for_ms: 20,
        sample_interval_ms: 5,
    };

    let result = runner
        .run(&transient_suite(Some(stability)), CancellationToken::new())
        .await;

    assert_eq!(result.status, RunStatus::Passed);
    let mut measured_executions = 0_usize;
    for scenario in &result.scenarios {
        let step = &scenario.steps[0];
        let metrics = &step.output.as_ref().expect("stability evidence").data["stability"];
        let samples = metrics["samples"].as_u64().expect("sample count");
        assert_eq!(scenario.status, RunStatus::Passed);
        assert_eq!(metrics["outcome"], "passed");
        assert_eq!(metrics["required_ms"], 20);
        assert_eq!(metrics["sample_interval_ms"], 5);
        assert!(metrics["observed_ms"].as_u64().expect("observed time") >= 20);
        assert!((2..=stability.planned_samples()).contains(&samples));
        assert_eq!(u64::from(step.attempts), samples);
        measured_executions += usize::try_from(samples).expect("bounded samples");
    }
    assert_eq!(executions.load(Ordering::SeqCst), measured_executions);
    assert!((DATASET_SIZE * 2..=DATASET_SIZE * 5).contains(&measured_executions));
}

fn scripted_runner(executions: Arc<AtomicUsize>, remains_visible: bool) -> Runner {
    Runner::new(
        vec![Arc::new(TransientDriver {
            executions,
            remains_visible,
        })],
        RunnerOptions {
            cleanup_timeout: Duration::from_secs(1),
            retry_policy: RetryPolicy {
                max_retries: 0,
                backoff: Duration::ZERO,
            },
            max_parallel_scenarios: 64,
            ..RunnerOptions::default()
        },
    )
    .expect("transient runner")
}

fn transient_suite(stability: Option<AssertionStability>) -> TestSuite {
    TestSuite {
        name: "assertion-stability-dataset".to_string(),
        version: 1,
        scenarios: (0..DATASET_SIZE)
            .map(|index| TestScenario {
                id: format!("transient-{index}"),
                name: format!("Transient {index}"),
                surface: Surface::Web,
                timeout_ms: 1_000,
                steps: vec![TestStep {
                    id: "assert-visible".to_string(),
                    action: Action::Assert {
                        expectation: Expectation::Visible(Target::Css {
                            selector: "#transient".to_string(),
                        }),
                    },
                    stability,
                    assertion_mode: Default::default(),
                    wait_mode: Default::default(),
                }],
            })
            .collect(),
    }
}
