use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use a3s_test_core::{
    Action, AssertionStability, DriverError, DriverSession, ElementState, Expectation,
    LayoutRelation, ScenarioContext, StepOutput, Surface, SurfaceDriver, Target, TestScenario,
    TestStep, TestSuite,
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
        let Action::Assert { expectation } = &step.action else {
            panic!("scripted stability driver accepts assertions only");
        };
        let (data, mismatch_code) = match expectation {
            Expectation::State { .. } => (
                json!({ "state": "checked", "expected": true, "actual": true }),
                "test.assert.checked",
            ),
            Expectation::RenderedText { value, .. } => (
                json!({ "expected": value, "actual": value }),
                "test.assert.rendered_text",
            ),
            Expectation::RenderedTexts { values, .. } => (
                json!({ "expected": values, "actual": values }),
                "test.assert.rendered_texts",
            ),
            Expectation::VisibleCount { count, .. } => (
                json!({ "expected": count, "actual": count }),
                "test.assert.visible_count",
            ),
            Expectation::Layout {
                target,
                relative_to,
                relation,
                tolerance_px,
            } => {
                let offset = self.sample as f64;
                (
                    json!({
                        "target": target,
                        "relative_to": relative_to,
                        "relation": relation,
                        "tolerance_px": tolerance_px,
                        "target_rect": {
                            "x": 120.0 + offset,
                            "y": 40.0 + offset,
                            "width": 40.0,
                            "height": 50.0,
                        },
                        "relative_rect": {
                            "x": 100.0 + offset,
                            "y": 100.0 + offset,
                            "width": 100.0,
                            "height": 100.0,
                        },
                        "matched": true,
                    }),
                    "test.assert.layout",
                )
            }
            _ => (json!({ "visible": true }), "test.assert.visible"),
        };
        self.executions.fetch_add(1, Ordering::SeqCst);
        self.sample += 1;
        if self.sample == 1 || self.remains_visible {
            return Ok(StepOutput::new("scripted assertion matched").with_data(data));
        }
        Err(DriverError::new(
            mismatch_code,
            "scripted assertion became false after the first sample",
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

#[tokio::test]
async fn stable_control_state_assertions_reject_100_of_100_transient_states() {
    let executions = Arc::new(AtomicUsize::new(0));
    let runner = scripted_runner(Arc::clone(&executions), false);
    let stability = AssertionStability {
        stable_for_ms: 20,
        sample_interval_ms: 5,
    };

    let result = runner
        .run(&state_suite(stability), CancellationToken::new())
        .await;

    assert_eq!(result.status, RunStatus::Failed);
    for scenario in &result.scenarios {
        let step = &scenario.steps[0];
        assert_eq!(step.attempts, 2);
        assert_eq!(
            step.error.as_ref().map(|error| error.code.as_str()),
            Some("test.assert.unstable")
        );
        let data = &step.output.as_ref().expect("state stability evidence").data;
        assert_eq!(data["assertion"]["first"]["actual"], true);
        assert_eq!(data["stability"]["samples"], 2);
    }
    assert_eq!(executions.load(Ordering::SeqCst), DATASET_SIZE * 2);
}

#[tokio::test]
async fn stable_control_state_assertions_accept_100_of_100_consistent_states() {
    let executions = Arc::new(AtomicUsize::new(0));
    let runner = scripted_runner(Arc::clone(&executions), true);
    let stability = AssertionStability {
        stable_for_ms: 20,
        sample_interval_ms: 5,
    };

    let result = runner
        .run(&state_suite(stability), CancellationToken::new())
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
        let data = &scenario.steps[0]
            .output
            .as_ref()
            .expect("state stability evidence")
            .data;
        assert_eq!(data["assertion"]["first"]["actual"], true);
        assert_eq!(data["assertion"]["last"]["actual"], true);
        assert_eq!(data["stability"]["outcome"], "passed");
    }
}

#[tokio::test]
async fn stable_layout_assertions_reject_100_of_100_transient_relations() {
    let executions = Arc::new(AtomicUsize::new(0));
    let runner = scripted_runner(Arc::clone(&executions), false);
    let stability = AssertionStability {
        stable_for_ms: 20,
        sample_interval_ms: 5,
    };

    let result = runner
        .run(&layout_suite(stability), CancellationToken::new())
        .await;

    assert_eq!(result.status, RunStatus::Failed);
    assert_eq!(result.scenarios.len(), DATASET_SIZE);
    for scenario in &result.scenarios {
        let step = &scenario.steps[0];
        let data = &step
            .output
            .as_ref()
            .expect("layout stability evidence")
            .data;
        assert_eq!(scenario.status, RunStatus::Failed);
        assert_eq!(step.attempts, 2);
        assert_eq!(
            step.error.as_ref().map(|error| error.code.as_str()),
            Some("test.assert.unstable")
        );
        assert_eq!(data["assertion"]["first"]["relation"], "above");
        assert_eq!(data["assertion"]["first"]["matched"], true);
        assert!(data["assertion"]["first"]["target_rect"].is_object());
        assert!(data["assertion"]["first"]["relative_rect"].is_object());
        assert_eq!(data["stability"]["outcome"], "unstable");
        assert_eq!(data["stability"]["samples"], 2);
    }
    assert_eq!(executions.load(Ordering::SeqCst), DATASET_SIZE * 2);
}

#[tokio::test]
async fn stable_layout_assertions_accept_100_of_100_consistent_relations_with_dual_rect_evidence() {
    let executions = Arc::new(AtomicUsize::new(0));
    let runner = scripted_runner(Arc::clone(&executions), true);
    let stability = AssertionStability {
        stable_for_ms: 20,
        sample_interval_ms: 5,
    };

    let result = runner
        .run(&layout_suite(stability), CancellationToken::new())
        .await;

    assert_eq!(result.status, RunStatus::Passed);
    assert_eq!(result.scenarios.len(), DATASET_SIZE);
    let mut measured_executions = 0_usize;
    for scenario in &result.scenarios {
        let step = &scenario.steps[0];
        let data = &step
            .output
            .as_ref()
            .expect("layout stability evidence")
            .data;
        let first = &data["assertion"]["first"];
        let last = &data["assertion"]["last"];
        let samples = data["stability"]["samples"]
            .as_u64()
            .expect("layout sample count");
        assert_eq!(scenario.status, RunStatus::Passed);
        assert_eq!(first["relation"], "above");
        assert_eq!(last["relation"], "above");
        assert_eq!(first["matched"], true);
        assert_eq!(last["matched"], true);
        assert!(first["target_rect"].is_object());
        assert!(first["relative_rect"].is_object());
        assert!(last["target_rect"].is_object());
        assert!(last["relative_rect"].is_object());
        assert_ne!(first["target_rect"]["x"], last["target_rect"]["x"]);
        assert_ne!(first["relative_rect"]["x"], last["relative_rect"]["x"]);
        assert_eq!(data["stability"]["outcome"], "passed");
        assert!((2..=stability.planned_samples()).contains(&samples));
        measured_executions += usize::try_from(samples).expect("bounded layout samples");
    }
    assert_eq!(executions.load(Ordering::SeqCst), measured_executions);
    assert!((DATASET_SIZE * 2..=DATASET_SIZE * 5).contains(&measured_executions));
}

#[tokio::test]
async fn stable_rendered_assertions_reject_300_of_300_scalar_sequence_and_count_transients() {
    let executions = Arc::new(AtomicUsize::new(0));
    let runner = scripted_runner(Arc::clone(&executions), false);
    let stability = AssertionStability {
        stable_for_ms: 20,
        sample_interval_ms: 5,
    };

    let result = runner
        .run(&rendered_suite(stability), CancellationToken::new())
        .await;

    assert_eq!(result.status, RunStatus::Failed);
    assert_eq!(result.scenarios.len(), DATASET_SIZE * 3);
    assert_eq!(
        result
            .scenarios
            .iter()
            .filter(|scenario| scenario.id.starts_with("rendered-text-"))
            .count(),
        DATASET_SIZE
    );
    assert_eq!(
        result
            .scenarios
            .iter()
            .filter(|scenario| scenario.id.starts_with("rendered-texts-"))
            .count(),
        DATASET_SIZE
    );
    assert_eq!(
        result
            .scenarios
            .iter()
            .filter(|scenario| scenario.id.starts_with("visible-count-"))
            .count(),
        DATASET_SIZE
    );
    for scenario in &result.scenarios {
        let step = &scenario.steps[0];
        assert_eq!(scenario.status, RunStatus::Failed);
        assert_eq!(step.attempts, 2);
        assert_eq!(
            step.error.as_ref().map(|error| error.code.as_str()),
            Some("test.assert.unstable")
        );
        let data = &step
            .output
            .as_ref()
            .expect("rendered stability evidence")
            .data;
        assert_eq!(
            data["assertion"]["first"]["actual"],
            data["assertion"]["first"]["expected"]
        );
        assert_eq!(data["stability"]["outcome"], "unstable");
        assert_eq!(data["stability"]["samples"], 2);
    }
    assert_eq!(executions.load(Ordering::SeqCst), DATASET_SIZE * 6);
}

#[tokio::test]
async fn stable_rendered_assertions_accept_300_of_300_consistent_scalar_sequences_and_counts() {
    let executions = Arc::new(AtomicUsize::new(0));
    let runner = scripted_runner(Arc::clone(&executions), true);
    let stability = AssertionStability {
        stable_for_ms: 20,
        sample_interval_ms: 5,
    };

    let result = runner
        .run(&rendered_suite(stability), CancellationToken::new())
        .await;

    assert_eq!(result.status, RunStatus::Passed);
    assert_eq!(
        result
            .scenarios
            .iter()
            .filter(|scenario| scenario.status == RunStatus::Passed)
            .count(),
        DATASET_SIZE * 3
    );
    let mut measured_executions = 0_usize;
    for scenario in &result.scenarios {
        let step = &scenario.steps[0];
        let data = &step
            .output
            .as_ref()
            .expect("rendered stability evidence")
            .data;
        let samples = data["stability"]["samples"].as_u64().expect("sample count");
        assert_eq!(data["assertion"]["first"], data["assertion"]["last"]);
        assert_eq!(data["stability"]["outcome"], "passed");
        assert!(
            data["stability"]["observed_ms"]
                .as_u64()
                .expect("observed time")
                >= stability.stable_for_ms
        );
        assert!((2..=stability.planned_samples()).contains(&samples));
        measured_executions += usize::try_from(samples).expect("bounded samples");
    }
    assert_eq!(executions.load(Ordering::SeqCst), measured_executions);
    assert!((DATASET_SIZE * 6..=DATASET_SIZE * 15).contains(&measured_executions));
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

fn state_suite(stability: AssertionStability) -> TestSuite {
    TestSuite {
        name: "state-stability-dataset".to_string(),
        version: 1,
        scenarios: (0..DATASET_SIZE)
            .map(|index| TestScenario {
                id: format!("state-{index}"),
                name: format!("State {index}"),
                surface: Surface::Web,
                timeout_ms: 1_000,
                steps: vec![TestStep {
                    id: "assert-checked".to_string(),
                    action: Action::Assert {
                        expectation: Expectation::State {
                            target: Target::Css {
                                selector: "#terms".to_string(),
                            },
                            state: ElementState::Checked,
                            expected: true,
                        },
                    },
                    stability: Some(stability),
                    assertion_mode: Default::default(),
                    wait_mode: Default::default(),
                }],
            })
            .collect(),
    }
}

fn layout_suite(stability: AssertionStability) -> TestSuite {
    TestSuite {
        name: "layout-stability-dataset".to_string(),
        version: 1,
        scenarios: (0..DATASET_SIZE)
            .map(|index| TestScenario {
                id: format!("layout-{index}"),
                name: format!("Layout {index}"),
                surface: Surface::Web,
                timeout_ms: 1_000,
                steps: vec![TestStep {
                    id: "assert-layout".to_string(),
                    action: Action::Assert {
                        expectation: Expectation::Layout {
                            target: Target::TestId {
                                value: format!("subject-{index}"),
                            },
                            relative_to: Target::TestId {
                                value: format!("reference-{index}"),
                            },
                            relation: LayoutRelation::Above,
                            tolerance_px: 1,
                        },
                    },
                    stability: Some(stability),
                    assertion_mode: Default::default(),
                    wait_mode: Default::default(),
                }],
            })
            .collect(),
    }
}

fn rendered_suite(stability: AssertionStability) -> TestSuite {
    let rendered_text = (0..DATASET_SIZE).map(|index| TestScenario {
        id: format!("rendered-text-{index}"),
        name: format!("Rendered text {index}"),
        surface: Surface::Web,
        timeout_ms: 1_000,
        steps: vec![TestStep {
            id: "assert-rendered-text".to_string(),
            action: Action::Assert {
                expectation: Expectation::RenderedText {
                    target: Target::TestId {
                        value: format!("copy-{index}"),
                    },
                    value: format!("Ready {index}"),
                },
            },
            stability: Some(stability),
            assertion_mode: Default::default(),
            wait_mode: Default::default(),
        }],
    });
    let visible_count = (0..DATASET_SIZE).map(|index| TestScenario {
        id: format!("visible-count-{index}"),
        name: format!("Visible count {index}"),
        surface: Surface::Web,
        timeout_ms: 1_000,
        steps: vec![TestStep {
            id: "assert-visible-count".to_string(),
            action: Action::Assert {
                expectation: Expectation::VisibleCount {
                    target: Target::Css {
                        selector: format!("[data-row='{index}']"),
                    },
                    count: 3,
                },
            },
            stability: Some(stability),
            assertion_mode: Default::default(),
            wait_mode: Default::default(),
        }],
    });
    let rendered_texts = (0..DATASET_SIZE).map(|index| TestScenario {
        id: format!("rendered-texts-{index}"),
        name: format!("Rendered text sequence {index}"),
        surface: Surface::Web,
        timeout_ms: 1_000,
        steps: vec![TestStep {
            id: "assert-rendered-texts".to_string(),
            action: Action::Assert {
                expectation: Expectation::RenderedTexts {
                    target: Target::Css {
                        selector: format!("[data-line-item='{index}']"),
                    },
                    values: vec![
                        format!("Item {index}"),
                        "Shipping".to_string(),
                        "Shipping".to_string(),
                    ],
                },
            },
            stability: Some(stability),
            assertion_mode: Default::default(),
            wait_mode: Default::default(),
        }],
    });

    TestSuite {
        name: "rendered-stability-dataset".to_string(),
        version: 1,
        scenarios: rendered_text
            .chain(visible_count)
            .chain(rendered_texts)
            .collect(),
    }
}
