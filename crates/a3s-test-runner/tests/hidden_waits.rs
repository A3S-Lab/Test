use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use a3s_test_core::{
    Action, AssertionMode, AssertionStability, DriverError, DriverSession, Expectation,
    ScenarioContext, StepOutput, Surface, SurfaceDriver, Target, TestScenario, TestStep, TestSuite,
    WaitCondition, WaitMode,
};
use a3s_test_runner::{
    RetryPolicy, RunStatus, Runner, RunnerOptions, HIDDEN_WAIT_POLL_INTERVAL_MS,
    MAX_HIDDEN_WAIT_PROBES,
};
use async_trait::async_trait;
use serde_json::json;
use tokio_util::sync::CancellationToken;

const DATASET_SIZE: usize = 100;

#[derive(Clone, Copy)]
enum ScriptedState {
    HiddenAfter { visible_probes: usize },
    AlwaysVisible,
    DriverFailureAfterFirstProbe,
}

#[derive(Clone)]
struct HiddenWaitDriver {
    closes: Arc<AtomicUsize>,
    executions: Arc<AtomicUsize>,
    state: ScriptedState,
}

struct HiddenWaitSession {
    closes: Arc<AtomicUsize>,
    executions: Arc<AtomicUsize>,
    state: ScriptedState,
    probe: usize,
}

#[async_trait]
impl SurfaceDriver for HiddenWaitDriver {
    fn surface(&self) -> Surface {
        Surface::Web
    }

    async fn open(
        &self,
        _context: &ScenarioContext,
    ) -> Result<Box<dyn DriverSession>, DriverError> {
        Ok(Box::new(HiddenWaitSession {
            closes: Arc::clone(&self.closes),
            executions: Arc::clone(&self.executions),
            state: self.state,
            probe: 0,
        }))
    }
}

#[async_trait]
impl DriverSession for HiddenWaitSession {
    async fn execute(&mut self, step: &TestStep) -> Result<StepOutput, DriverError> {
        assert!(matches!(
            step.action,
            Action::Assert {
                expectation: Expectation::Visible(_)
            }
        ));
        assert_eq!(step.assertion_mode, AssertionMode::Positive);
        assert_eq!(step.wait_mode, WaitMode::Positive);
        assert!(step.stability.is_none());

        self.executions.fetch_add(1, Ordering::SeqCst);
        self.probe += 1;
        match self.state {
            ScriptedState::HiddenAfter { visible_probes } if self.probe > visible_probes => {
                Err(not_visible())
            }
            ScriptedState::HiddenAfter { .. } | ScriptedState::AlwaysVisible => {
                Ok(visible_output(self.probe))
            }
            ScriptedState::DriverFailureAfterFirstProbe if self.probe == 1 => {
                Ok(visible_output(self.probe))
            }
            ScriptedState::DriverFailureAfterFirstProbe => Err(DriverError::new(
                "test.driver.web.output_invalid",
                "scripted visibility response was malformed",
            )),
        }
    }

    async fn close(&mut self) -> Result<(), DriverError> {
        self.closes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[tokio::test]
async fn hidden_waits_accept_100_of_100_initially_hidden_targets_without_sleeping() {
    let harness = Harness::new(ScriptedState::HiddenAfter { visible_probes: 0 });
    let result = harness
        .runner(64)
        .run(&suite(DATASET_SIZE, 1_000), CancellationToken::new())
        .await;

    assert_eq!(result.status, RunStatus::Passed);
    for scenario in &result.scenarios {
        assert_eq!(scenario.status, RunStatus::Passed);
        let step = &scenario.steps[0];
        assert_eq!(step.attempts, 1);
        let data = &step.output.as_ref().expect("hidden wait evidence").data;
        assert_eq!(data["visible"], false);
        assert_eq!(data["wait"]["condition"], "hidden");
        assert_eq!(data["wait"]["outcome"], "matched");
        assert_eq!(data["wait"]["probes"], 1);
        assert_eq!(data["probe_error"]["code"], "test.assert.visible");
        assert!(data["first_visible"].is_null());
    }
    assert_eq!(harness.executions(), DATASET_SIZE);
    assert_eq!(harness.closes(), DATASET_SIZE);
}

#[tokio::test]
async fn hidden_waits_accept_100_of_100_targets_after_two_visible_probes() {
    let harness = Harness::new(ScriptedState::HiddenAfter { visible_probes: 2 });
    let result = harness
        .runner(64)
        .run(&suite(DATASET_SIZE, 1_000), CancellationToken::new())
        .await;

    assert_eq!(result.status, RunStatus::Passed);
    for scenario in &result.scenarios {
        assert_eq!(scenario.status, RunStatus::Passed);
        let step = &scenario.steps[0];
        assert_eq!(step.attempts, 3);
        let data = &step.output.as_ref().expect("hidden wait evidence").data;
        assert_eq!(data["visible"], false);
        assert_eq!(data["wait"]["outcome"], "matched");
        assert_eq!(
            data["wait"]["poll_interval_ms"],
            HIDDEN_WAIT_POLL_INTERVAL_MS
        );
        assert_eq!(data["wait"]["probes"], 3);
        assert_eq!(data["first_visible"]["visible"], true);
        assert_eq!(data["first_visible"]["probe"], 1);
    }
    assert_eq!(harness.executions(), DATASET_SIZE * 3);
    assert_eq!(harness.closes(), DATASET_SIZE);
}

#[tokio::test]
async fn hidden_waits_preserve_100_of_100_driver_failures() {
    let harness = Harness::new(ScriptedState::DriverFailureAfterFirstProbe);
    let result = harness
        .runner(64)
        .run(&suite(DATASET_SIZE, 1_000), CancellationToken::new())
        .await;

    assert_eq!(result.status, RunStatus::Failed);
    for scenario in &result.scenarios {
        assert_eq!(scenario.status, RunStatus::Failed);
        let step = &scenario.steps[0];
        assert_eq!(step.attempts, 2);
        assert_eq!(
            step.error.as_ref().map(|error| error.code.as_str()),
            Some("test.driver.web.output_invalid")
        );
        let data = &step
            .output
            .as_ref()
            .expect("inconclusive wait evidence")
            .data;
        assert_eq!(data["wait"]["outcome"], "inconclusive");
        assert_eq!(data["wait"]["probes"], 2);
        assert_eq!(data["last_visible"]["visible"], true);
    }
    assert_eq!(harness.executions(), DATASET_SIZE * 2);
    assert_eq!(harness.closes(), DATASET_SIZE);
}

#[tokio::test]
async fn hidden_waits_time_out_100_of_100_targets_that_remain_visible() {
    let harness = Harness::new(ScriptedState::AlwaysVisible);
    let result = harness
        .runner(64)
        .run(&suite(DATASET_SIZE, 75), CancellationToken::new())
        .await;

    assert_eq!(result.status, RunStatus::TimedOut);
    for scenario in &result.scenarios {
        assert_eq!(scenario.status, RunStatus::TimedOut);
        let step = &scenario.steps[0];
        assert_eq!(step.status, RunStatus::TimedOut);
        assert_eq!(
            step.error.as_ref().map(|error| error.code.as_str()),
            Some("test.run.timeout")
        );
        let data = &step.output.as_ref().expect("timeout counter-evidence").data;
        assert_eq!(data["wait"]["outcome"], "timed_out");
        assert!(data["wait"]["probes"]
            .as_u64()
            .is_some_and(|value| value >= 1));
        assert_eq!(data["last_visible"]["visible"], true);
    }
    assert!(harness.executions() >= DATASET_SIZE);
    assert_eq!(harness.closes(), DATASET_SIZE);
}

#[tokio::test(start_paused = true)]
async fn hidden_waits_fail_closed_at_the_static_probe_limit() {
    let harness = Harness::new(ScriptedState::AlwaysVisible);
    let result = harness
        .runner(1)
        .run(&suite(1, 120_000), CancellationToken::new())
        .await;

    assert_eq!(result.status, RunStatus::Failed);
    let step = &result.scenarios[0].steps[0];
    assert_eq!(u64::from(step.attempts), MAX_HIDDEN_WAIT_PROBES);
    assert_eq!(
        step.error.as_ref().map(|error| error.code.as_str()),
        Some("test.run.hidden_wait_probe_limit")
    );
    let data = &step.output.as_ref().expect("probe limit evidence").data;
    assert_eq!(data["wait"]["outcome"], "probe_limit");
    assert_eq!(data["wait"]["probes"], MAX_HIDDEN_WAIT_PROBES);
    assert_eq!(data["wait"]["max_probes"], MAX_HIDDEN_WAIT_PROBES);
    assert_eq!(harness.executions(), MAX_HIDDEN_WAIT_PROBES as usize);
    assert_eq!(harness.closes(), 1);
}

#[tokio::test]
async fn hidden_wait_cancellation_preserves_counter_evidence_and_closes_the_session() {
    let harness = Harness::new(ScriptedState::AlwaysVisible);
    let runner = harness.runner(1);
    let cancellation = CancellationToken::new();
    let task_cancellation = cancellation.clone();
    let task = tokio::spawn(async move { runner.run(&suite(1, 5_000), task_cancellation).await });

    tokio::time::sleep(Duration::from_millis(10)).await;
    cancellation.cancel();
    let result = task.await.expect("cancelled hidden wait run");

    assert_eq!(result.status, RunStatus::Cancelled);
    let step = &result.scenarios[0].steps[0];
    assert_eq!(step.status, RunStatus::Cancelled);
    assert_eq!(
        step.error.as_ref().map(|error| error.code.as_str()),
        Some("test.run.cancelled")
    );
    let data = &step
        .output
        .as_ref()
        .expect("cancellation counter-evidence")
        .data;
    assert_eq!(data["wait"]["outcome"], "cancelled");
    assert_eq!(data["last_visible"]["visible"], true);
    assert_eq!(harness.closes(), 1);
}

#[tokio::test]
async fn programmatic_hidden_waits_reject_observation_bound_targets_before_dispatch() {
    let harness = Harness::new(ScriptedState::HiddenAfter { visible_probes: 0 });
    let mut invalid = suite(1, 1_000);
    invalid.scenarios[0].steps[0].action = Action::Wait {
        condition: WaitCondition::Visible(Target::Ref {
            value: "@e4".to_string(),
        }),
    };

    let result = harness
        .runner(1)
        .run(&invalid, CancellationToken::new())
        .await;

    assert_eq!(result.status, RunStatus::Failed);
    assert_eq!(
        result.scenarios[0].steps[0]
            .error
            .as_ref()
            .map(|error| error.code.as_str()),
        Some("test.run.wait_mode_invalid")
    );
    assert_eq!(harness.executions(), 0);
    assert_eq!(harness.closes(), 1);
}

#[tokio::test]
async fn programmatic_hidden_waits_reject_incompatible_step_policy_before_dispatch() {
    let mut wrong_action = suite(1, 1_000);
    wrong_action.scenarios[0].steps[0].action = Action::Assert {
        expectation: Expectation::Visible(Target::Css {
            selector: "#dialog".to_string(),
        }),
    };

    let mut assertion_policy = suite(1, 1_000);
    assertion_policy.scenarios[0].steps[0].assertion_mode = AssertionMode::Hidden;

    let mut stability_policy = suite(1, 1_000);
    stability_policy.scenarios[0].steps[0].stability = Some(AssertionStability {
        stable_for_ms: 100,
        sample_interval_ms: 25,
    });

    for invalid in [wrong_action, assertion_policy, stability_policy] {
        let harness = Harness::new(ScriptedState::HiddenAfter { visible_probes: 0 });
        let result = harness
            .runner(1)
            .run(&invalid, CancellationToken::new())
            .await;

        assert_eq!(result.status, RunStatus::Failed);
        assert_eq!(
            result.scenarios[0].steps[0]
                .error
                .as_ref()
                .map(|error| error.code.as_str()),
            Some("test.run.wait_mode_invalid")
        );
        assert_eq!(harness.executions(), 0);
        assert_eq!(harness.closes(), 1);
    }
}

fn not_visible() -> DriverError {
    DriverError::new("test.assert.visible", "scripted target is not visible")
}

fn visible_output(probe: usize) -> StepOutput {
    StepOutput::new("scripted target is visible")
        .with_data(json!({ "visible": true, "probe": probe }))
}

struct Harness {
    closes: Arc<AtomicUsize>,
    executions: Arc<AtomicUsize>,
    state: ScriptedState,
}

impl Harness {
    fn new(state: ScriptedState) -> Self {
        Self {
            closes: Arc::new(AtomicUsize::new(0)),
            executions: Arc::new(AtomicUsize::new(0)),
            state,
        }
    }

    fn runner(&self, max_parallel_scenarios: usize) -> Runner {
        Runner::new(
            vec![Arc::new(HiddenWaitDriver {
                closes: Arc::clone(&self.closes),
                executions: Arc::clone(&self.executions),
                state: self.state,
            })],
            RunnerOptions {
                cleanup_timeout: Duration::from_secs(1),
                retry_policy: RetryPolicy {
                    max_retries: 0,
                    backoff: Duration::ZERO,
                },
                max_parallel_scenarios,
                ..RunnerOptions::default()
            },
        )
        .expect("hidden wait runner")
    }

    fn closes(&self) -> usize {
        self.closes.load(Ordering::SeqCst)
    }

    fn executions(&self) -> usize {
        self.executions.load(Ordering::SeqCst)
    }
}

fn suite(scenarios: usize, timeout_ms: u64) -> TestSuite {
    TestSuite {
        name: "hidden-wait-dataset".to_string(),
        version: 1,
        scenarios: (0..scenarios)
            .map(|index| TestScenario {
                id: format!("hidden-wait-{index}"),
                name: format!("Hidden wait {index}"),
                surface: Surface::Web,
                timeout_ms,
                steps: vec![TestStep {
                    id: "wait-hidden".to_string(),
                    action: Action::Wait {
                        condition: WaitCondition::Visible(Target::Css {
                            selector: "#dialog".to_string(),
                        }),
                    },
                    stability: None,
                    assertion_mode: AssertionMode::Positive,
                    wait_mode: WaitMode::Hidden,
                }],
            })
            .collect(),
    }
}
