use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use a3s_test_core::{
    Action, AssertionMode, AssertionStability, DriverError, DriverSession, Expectation,
    ScenarioContext, StepOutput, Surface, SurfaceDriver, Target, TestScenario, TestStep, TestSuite,
};
use a3s_test_runner::{RetryPolicy, RunStatus, Runner, RunnerOptions};
use async_trait::async_trait;
use serde_json::json;
use tokio_util::sync::CancellationToken;

const DATASET_SIZE: usize = 100;

#[derive(Clone, Copy)]
enum ScriptedState {
    Hidden,
    Visible,
    AppearsAfterFirstProbe,
    DriverFailure,
}

#[derive(Clone)]
struct HiddenDriver {
    executions: Arc<AtomicUsize>,
    state: ScriptedState,
}

struct HiddenSession {
    executions: Arc<AtomicUsize>,
    state: ScriptedState,
    sample: usize,
}

#[async_trait]
impl SurfaceDriver for HiddenDriver {
    fn surface(&self) -> Surface {
        Surface::Web
    }

    async fn open(
        &self,
        _context: &ScenarioContext,
    ) -> Result<Box<dyn DriverSession>, DriverError> {
        Ok(Box::new(HiddenSession {
            executions: Arc::clone(&self.executions),
            state: self.state,
            sample: 0,
        }))
    }
}

#[async_trait]
impl DriverSession for HiddenSession {
    async fn execute(&mut self, step: &TestStep) -> Result<StepOutput, DriverError> {
        assert!(matches!(step.action, Action::Assert { .. }));
        assert_eq!(step.assertion_mode, AssertionMode::Positive);
        self.executions.fetch_add(1, Ordering::SeqCst);
        self.sample += 1;

        match self.state {
            ScriptedState::Hidden => Err(not_visible()),
            ScriptedState::Visible => Ok(visible_output()),
            ScriptedState::AppearsAfterFirstProbe if self.sample == 1 => Err(not_visible()),
            ScriptedState::AppearsAfterFirstProbe => Ok(visible_output()),
            ScriptedState::DriverFailure => Err(DriverError::new(
                "test.driver.web.output_invalid",
                "scripted visibility response was malformed",
            )),
        }
    }

    async fn close(&mut self) -> Result<(), DriverError> {
        Ok(())
    }
}

#[tokio::test]
async fn hidden_assertions_accept_100_of_100_non_visible_targets() {
    let executions = Arc::new(AtomicUsize::new(0));
    let result = runner(Arc::clone(&executions), ScriptedState::Hidden)
        .run(&suite(None), CancellationToken::new())
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
    for scenario in &result.scenarios {
        let output = scenario.steps[0].output.as_ref().expect("hidden evidence");
        assert_eq!(output.data["expected"], "hidden");
        assert_eq!(output.data["visible"], false);
        assert_eq!(output.data["probe_error"]["code"], "test.assert.visible");
    }
    assert_eq!(executions.load(Ordering::SeqCst), DATASET_SIZE);
}

#[tokio::test]
async fn hidden_assertions_reject_100_of_100_visible_targets() {
    let executions = Arc::new(AtomicUsize::new(0));
    let result = runner(Arc::clone(&executions), ScriptedState::Visible)
        .run(&suite(None), CancellationToken::new())
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
        assert_eq!(
            step.error.as_ref().map(|error| error.code.as_str()),
            Some("test.assert.hidden")
        );
        let output = step.output.as_ref().expect("visible counter-evidence");
        assert_eq!(output.data["expected"], "hidden");
        assert_eq!(output.data["visible"], true);
        assert_eq!(output.data["probe"]["visible"], true);
    }
    assert_eq!(executions.load(Ordering::SeqCst), DATASET_SIZE);
}

#[tokio::test]
async fn stable_hidden_assertions_accept_100_of_100_consistently_hidden_targets() {
    let executions = Arc::new(AtomicUsize::new(0));
    let stability = AssertionStability {
        stable_for_ms: 20,
        sample_interval_ms: 5,
    };
    let result = runner(Arc::clone(&executions), ScriptedState::Hidden)
        .run(&suite(Some(stability)), CancellationToken::new())
        .await;

    assert_eq!(result.status, RunStatus::Passed);
    let mut measured_executions = 0_usize;
    for scenario in &result.scenarios {
        let step = &scenario.steps[0];
        let data = &step.output.as_ref().expect("stability evidence").data;
        let samples = data["stability"]["samples"]
            .as_u64()
            .expect("stability sample count");
        assert_eq!(scenario.status, RunStatus::Passed);
        assert_eq!(data["stability"]["outcome"], "passed");
        assert_eq!(data["assertion"]["first"]["visible"], false);
        assert_eq!(data["assertion"]["last"]["visible"], false);
        assert!((2..=stability.planned_samples()).contains(&samples));
        assert_eq!(u64::from(step.attempts), samples);
        measured_executions += usize::try_from(samples).expect("bounded samples");
    }
    assert_eq!(executions.load(Ordering::SeqCst), measured_executions);
    assert!((DATASET_SIZE * 2..=DATASET_SIZE * 5).contains(&measured_executions));
}

#[tokio::test]
async fn stable_hidden_assertions_reject_100_of_100_targets_that_reappear() {
    let executions = Arc::new(AtomicUsize::new(0));
    let stability = AssertionStability {
        stable_for_ms: 20,
        sample_interval_ms: 5,
    };
    let result = runner(
        Arc::clone(&executions),
        ScriptedState::AppearsAfterFirstProbe,
    )
    .run(&suite(Some(stability)), CancellationToken::new())
    .await;

    assert_eq!(result.status, RunStatus::Failed);
    for scenario in &result.scenarios {
        let step = &scenario.steps[0];
        assert_eq!(step.attempts, 2);
        assert_eq!(
            step.error.as_ref().map(|error| error.code.as_str()),
            Some("test.assert.unstable")
        );
        let data = &step.output.as_ref().expect("stability evidence").data;
        assert_eq!(data["stability"]["outcome"], "unstable");
        assert_eq!(data["stability"]["samples"], 2);
        assert_eq!(data["assertion"]["first"]["visible"], false);
        assert_eq!(data["assertion"]["last"]["visible"], true);
    }
    assert_eq!(executions.load(Ordering::SeqCst), DATASET_SIZE * 2);
}

#[tokio::test]
async fn hidden_assertions_preserve_driver_failures() {
    let executions = Arc::new(AtomicUsize::new(0));
    let result = runner(Arc::clone(&executions), ScriptedState::DriverFailure)
        .run(&suite(None), CancellationToken::new())
        .await;

    assert_eq!(result.status, RunStatus::Failed);
    for scenario in &result.scenarios {
        assert_eq!(
            scenario.steps[0]
                .error
                .as_ref()
                .map(|error| error.code.as_str()),
            Some("test.driver.web.output_invalid")
        );
    }
    assert_eq!(executions.load(Ordering::SeqCst), DATASET_SIZE);
}

#[tokio::test]
async fn programmatic_hidden_assertions_fail_closed_for_observation_bound_targets() {
    let executions = Arc::new(AtomicUsize::new(0));
    let runner = runner(Arc::clone(&executions), ScriptedState::Hidden);
    let mut invalid = suite(None);
    invalid.scenarios.truncate(1);
    invalid.scenarios[0].steps[0].action = Action::Assert {
        expectation: Expectation::Visible(Target::Ref {
            value: "@e4".to_string(),
        }),
    };

    let result = runner.run(&invalid, CancellationToken::new()).await;

    assert_eq!(result.status, RunStatus::Failed);
    assert_eq!(
        result.scenarios[0].steps[0]
            .error
            .as_ref()
            .map(|error| error.code.as_str()),
        Some("test.run.assertion_mode_invalid")
    );
    assert_eq!(executions.load(Ordering::SeqCst), 0);
}

fn not_visible() -> DriverError {
    DriverError::new("test.assert.visible", "scripted target is not visible")
}

fn visible_output() -> StepOutput {
    StepOutput::new("scripted target is visible").with_data(json!({ "visible": true }))
}

fn runner(executions: Arc<AtomicUsize>, state: ScriptedState) -> Runner {
    Runner::new(
        vec![Arc::new(HiddenDriver { executions, state })],
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
    .expect("hidden assertion runner")
}

fn suite(stability: Option<AssertionStability>) -> TestSuite {
    TestSuite {
        name: "hidden-assertion-dataset".to_string(),
        version: 1,
        scenarios: (0..DATASET_SIZE)
            .map(|index| TestScenario {
                id: format!("hidden-{index}"),
                name: format!("Hidden {index}"),
                surface: Surface::Web,
                timeout_ms: 1_000,
                steps: vec![TestStep {
                    id: "assert-hidden".to_string(),
                    action: Action::Assert {
                        expectation: Expectation::Visible(Target::Css {
                            selector: "#dialog".to_string(),
                        }),
                    },
                    stability,
                    assertion_mode: AssertionMode::Hidden,
                }],
            })
            .collect(),
    }
}
