use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use a3s_test_core::{
    Action, AssertionStability, DriverError, DriverSession, ElementState, Expectation,
    LayoutRelation, ScenarioContext, StepOutput, Surface, SurfaceDriver, Target, TestScenario,
    TestStep, TestSuite, ViewportCoverageComparison,
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
            Expectation::State {
                state, expected, ..
            } => {
                let (name, mismatch_code) = match (state, expected) {
                    (ElementState::Enabled, true) => ("enabled", "test.assert.enabled"),
                    (ElementState::Enabled, false) => ("enabled", "test.assert.disabled"),
                    (ElementState::Checked, true) => ("checked", "test.assert.checked"),
                    (ElementState::Checked, false) => ("checked", "test.assert.unchecked"),
                    (ElementState::Selected, true) => ("selected", "test.assert.selected"),
                    (ElementState::Selected, false) => ("selected", "test.assert.unselected"),
                    (ElementState::Focused, true) => ("focused", "test.assert.focused"),
                    (ElementState::Focused, false) => ("focused", "test.assert.unfocused"),
                    (ElementState::FocusWithin, true) => {
                        ("focus_within", "test.assert.focus_within")
                    }
                    (ElementState::FocusWithin, false) => {
                        ("focus_within", "test.assert.focus_outside")
                    }
                    (ElementState::Expanded, true) => ("expanded", "test.assert.expanded"),
                    (ElementState::Expanded, false) => ("expanded", "test.assert.collapsed"),
                    (ElementState::Pressed, true) => ("pressed", "test.assert.pressed"),
                    (ElementState::Pressed, false) => ("pressed", "test.assert.unpressed"),
                    (ElementState::ReadOnly, true) => ("readonly", "test.assert.readonly"),
                    (ElementState::ReadOnly, false) => ("readonly", "test.assert.writable"),
                    (ElementState::Required, true) => ("required", "test.assert.required"),
                    (ElementState::Required, false) => ("required", "test.assert.optional"),
                    (ElementState::Invalid, true) => ("invalid", "test.assert.invalid"),
                    (ElementState::Invalid, false) => ("invalid", "test.assert.valid"),
                };
                (
                    json!({ "state": name, "expected": expected, "actual": expected }),
                    mismatch_code,
                )
            }
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
            Expectation::InViewport(target) => {
                let offset = self.sample as f64;
                (
                    json!({
                        "target": target,
                        "target_rect": {
                            "x": 950.0 + offset,
                            "y": 100.0,
                            "width": 100.0,
                            "height": 50.0,
                        },
                        "viewport_rect": {
                            "x": 0.0,
                            "y": 0.0,
                            "width": 1000.0,
                            "height": 800.0,
                        },
                        "intersection_ratio": 0.5,
                        "in_viewport": true,
                    }),
                    "test.assert.in_viewport",
                )
            }
            Expectation::ViewportCoverage {
                target,
                comparison,
                percent,
            } => {
                let offset = self.sample as f64;
                let mismatch_code = match comparison {
                    ViewportCoverageComparison::AtLeast => "test.assert.viewport_coverage_at_least",
                    ViewportCoverageComparison::AtMost => "test.assert.viewport_coverage_at_most",
                };
                (
                    json!({
                        "target": target,
                        "target_rect": {
                            "x": offset,
                            "y": 0.0,
                            "width": 100.0,
                            "height": 100.0,
                        },
                        "viewport_rect": {
                            "x": 0.0,
                            "y": 0.0,
                            "width": 1000.0,
                            "height": 800.0,
                        },
                        "intersection_ratio": f64::from(*percent) / 100.0,
                        "actual_percent": percent,
                        "comparison": comparison,
                        "threshold_percent": percent,
                        "matched": true,
                    }),
                    mismatch_code,
                )
            }
            Expectation::PointerReachable(target) => {
                let offset = self.sample as f64;
                (
                    json!({
                        "target": target,
                        "target_rect": {
                            "x": 100.0 + offset,
                            "y": 100.0,
                            "width": 90.0,
                            "height": 90.0,
                        },
                        "viewport_rect": {
                            "x": 0.0,
                            "y": 0.0,
                            "width": 1000.0,
                            "height": 800.0,
                        },
                        "intersection_ratio": 1.0,
                        "pointer_reachable": true,
                        "sample_count": 9,
                        "reachable_samples": 1,
                        "samples": [{
                            "x": 115.0 + offset,
                            "y": 115.0,
                            "reachable": true,
                        }],
                    }),
                    "test.assert.pointer_reachable",
                )
            }
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
async fn stable_semantic_state_assertions_reject_100_of_100_transient_states() {
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
async fn stable_semantic_state_assertions_accept_100_of_100_consistent_states() {
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
async fn stable_focus_assertions_reject_200_of_200_transient_ownership_states() {
    let executions = Arc::new(AtomicUsize::new(0));
    let runner = scripted_runner(Arc::clone(&executions), false);
    let stability = AssertionStability {
        stable_for_ms: 20,
        sample_interval_ms: 5,
    };

    let result = runner
        .run(&focus_suite(stability), CancellationToken::new())
        .await;

    assert_eq!(result.status, RunStatus::Failed);
    assert_eq!(result.scenarios.len(), DATASET_SIZE * 2);
    for scenario in &result.scenarios {
        let step = &scenario.steps[0];
        let data = &step.output.as_ref().expect("focus stability evidence").data;
        assert_eq!(step.attempts, 2);
        assert_eq!(
            step.error.as_ref().map(|error| error.code.as_str()),
            Some("test.assert.unstable")
        );
        assert_eq!(data["assertion"]["first"]["actual"], true);
        assert_eq!(data["stability"]["outcome"], "unstable");
        assert_eq!(data["stability"]["samples"], 2);
    }
    assert_eq!(executions.load(Ordering::SeqCst), DATASET_SIZE * 4);
}

#[tokio::test]
async fn stable_focus_assertions_accept_200_of_200_sustained_ownership_states() {
    let executions = Arc::new(AtomicUsize::new(0));
    let runner = scripted_runner(Arc::clone(&executions), true);
    let stability = AssertionStability {
        stable_for_ms: 20,
        sample_interval_ms: 5,
    };

    let result = runner
        .run(&focus_suite(stability), CancellationToken::new())
        .await;

    assert_eq!(result.status, RunStatus::Passed);
    assert_eq!(result.scenarios.len(), DATASET_SIZE * 2);
    let mut measured_executions = 0_usize;
    for scenario in &result.scenarios {
        let data = &scenario.steps[0]
            .output
            .as_ref()
            .expect("focus stability evidence")
            .data;
        assert_eq!(data["assertion"]["first"]["actual"], true);
        assert_eq!(data["assertion"]["last"]["actual"], true);
        assert_eq!(data["stability"]["outcome"], "passed");
        let samples = data["stability"]["samples"]
            .as_u64()
            .expect("focus sample count");
        assert!((2..=stability.planned_samples()).contains(&samples));
        measured_executions += usize::try_from(samples).expect("bounded focus samples");
    }
    assert_eq!(executions.load(Ordering::SeqCst), measured_executions);
    assert!((DATASET_SIZE * 4..=DATASET_SIZE * 10).contains(&measured_executions));
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
async fn stable_interactability_assertions_reject_200_of_200_transient_signals() {
    let executions = Arc::new(AtomicUsize::new(0));
    let runner = scripted_runner(Arc::clone(&executions), false);
    let stability = AssertionStability {
        stable_for_ms: 20,
        sample_interval_ms: 5,
    };

    let result = runner
        .run(&interactability_suite(stability), CancellationToken::new())
        .await;

    assert_eq!(result.status, RunStatus::Failed);
    assert_eq!(result.scenarios.len(), DATASET_SIZE * 2);
    for scenario in &result.scenarios {
        let step = &scenario.steps[0];
        let data = &step
            .output
            .as_ref()
            .expect("interactability stability evidence")
            .data;
        assert_eq!(scenario.status, RunStatus::Failed);
        assert_eq!(step.attempts, 2);
        assert_eq!(
            step.error.as_ref().map(|error| error.code.as_str()),
            Some("test.assert.unstable")
        );
        assert!(data["assertion"]["first"]["target_rect"].is_object());
        assert!(data["assertion"]["first"]["viewport_rect"].is_object());
        assert_eq!(data["stability"]["outcome"], "unstable");
        assert_eq!(data["stability"]["samples"], 2);
    }
    assert_eq!(executions.load(Ordering::SeqCst), DATASET_SIZE * 4);
}

#[tokio::test]
async fn stable_interactability_assertions_accept_200_of_200_sustained_signals() {
    let executions = Arc::new(AtomicUsize::new(0));
    let runner = scripted_runner(Arc::clone(&executions), true);
    let stability = AssertionStability {
        stable_for_ms: 20,
        sample_interval_ms: 5,
    };

    let result = runner
        .run(&interactability_suite(stability), CancellationToken::new())
        .await;

    assert_eq!(result.status, RunStatus::Passed);
    assert_eq!(result.scenarios.len(), DATASET_SIZE * 2);
    let mut measured_executions = 0_usize;
    for scenario in &result.scenarios {
        let step = &scenario.steps[0];
        let data = &step
            .output
            .as_ref()
            .expect("interactability stability evidence")
            .data;
        let first = &data["assertion"]["first"];
        let last = &data["assertion"]["last"];
        let samples = data["stability"]["samples"]
            .as_u64()
            .expect("interactability sample count");
        assert_eq!(scenario.status, RunStatus::Passed);
        assert!(first["in_viewport"] == true || first["pointer_reachable"] == true);
        assert!(last["in_viewport"] == true || last["pointer_reachable"] == true);
        assert!(first["target_rect"].is_object());
        assert!(last["target_rect"].is_object());
        assert_ne!(first["target_rect"]["x"], last["target_rect"]["x"]);
        assert_eq!(data["stability"]["outcome"], "passed");
        assert!((2..=stability.planned_samples()).contains(&samples));
        measured_executions += usize::try_from(samples).expect("bounded samples");
    }
    assert_eq!(executions.load(Ordering::SeqCst), measured_executions);
    assert!((DATASET_SIZE * 4..=DATASET_SIZE * 10).contains(&measured_executions));
}

#[tokio::test]
async fn stable_viewport_coverage_assertions_reject_100_of_100_transient_signals() {
    let executions = Arc::new(AtomicUsize::new(0));
    let runner = scripted_runner(Arc::clone(&executions), false);
    let stability = AssertionStability {
        stable_for_ms: 20,
        sample_interval_ms: 5,
    };

    let result = runner
        .run(
            &viewport_coverage_suite(stability),
            CancellationToken::new(),
        )
        .await;

    assert_eq!(result.status, RunStatus::Failed);
    assert_eq!(result.scenarios.len(), DATASET_SIZE);
    for scenario in &result.scenarios {
        let step = &scenario.steps[0];
        let data = &step
            .output
            .as_ref()
            .expect("viewport coverage stability evidence")
            .data;
        assert_eq!(step.attempts, 2);
        assert_eq!(
            step.error.as_ref().map(|error| error.code.as_str()),
            Some("test.assert.unstable")
        );
        assert_eq!(data["assertion"]["first"]["matched"], true);
        assert_eq!(data["stability"]["outcome"], "unstable");
        assert_eq!(data["stability"]["samples"], 2);
    }
    assert_eq!(executions.load(Ordering::SeqCst), DATASET_SIZE * 2);
}

#[tokio::test]
async fn stable_viewport_coverage_assertions_accept_100_of_100_sustained_signals() {
    let executions = Arc::new(AtomicUsize::new(0));
    let runner = scripted_runner(Arc::clone(&executions), true);
    let stability = AssertionStability {
        stable_for_ms: 20,
        sample_interval_ms: 5,
    };

    let result = runner
        .run(
            &viewport_coverage_suite(stability),
            CancellationToken::new(),
        )
        .await;

    assert_eq!(result.status, RunStatus::Passed);
    assert_eq!(result.scenarios.len(), DATASET_SIZE);
    let mut measured_executions = 0_usize;
    for scenario in &result.scenarios {
        let data = &scenario.steps[0]
            .output
            .as_ref()
            .expect("viewport coverage stability evidence")
            .data;
        let first = &data["assertion"]["first"];
        let last = &data["assertion"]["last"];
        let samples = data["stability"]["samples"]
            .as_u64()
            .expect("viewport coverage samples");
        assert_eq!(first["matched"], true);
        assert_eq!(last["matched"], true);
        assert!(first["target_rect"].is_object());
        assert!(last["target_rect"].is_object());
        assert_ne!(first["target_rect"]["x"], last["target_rect"]["x"]);
        assert_eq!(data["stability"]["outcome"], "passed");
        assert!((2..=stability.planned_samples()).contains(&samples));
        measured_executions += usize::try_from(samples).expect("bounded coverage samples");
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
    let states = [
        ElementState::Expanded,
        ElementState::Pressed,
        ElementState::ReadOnly,
        ElementState::Required,
        ElementState::Invalid,
    ];
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
                    id: "assert-semantic-state".to_string(),
                    action: Action::Assert {
                        expectation: Expectation::State {
                            target: Target::Css {
                                selector: format!("#semantic-state-{index}"),
                            },
                            state: states[index % states.len()],
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

fn focus_suite(stability: AssertionStability) -> TestSuite {
    let focused = (0..DATASET_SIZE).map(|index| TestScenario {
        id: format!("focused-{index}"),
        name: format!("Focused {index}"),
        surface: Surface::Web,
        timeout_ms: 1_000,
        steps: vec![TestStep {
            id: "assert-focused".to_string(),
            action: Action::Assert {
                expectation: Expectation::State {
                    target: Target::TestId {
                        value: format!("focused-{index}"),
                    },
                    state: ElementState::Focused,
                    expected: true,
                },
            },
            stability: Some(stability),
            assertion_mode: Default::default(),
            wait_mode: Default::default(),
        }],
    });
    let focus_within = (0..DATASET_SIZE).map(|index| TestScenario {
        id: format!("focus-within-{index}"),
        name: format!("Focus within {index}"),
        surface: Surface::Web,
        timeout_ms: 1_000,
        steps: vec![TestStep {
            id: "assert-focus-within".to_string(),
            action: Action::Assert {
                expectation: Expectation::State {
                    target: Target::TestId {
                        value: format!("focus-scope-{index}"),
                    },
                    state: ElementState::FocusWithin,
                    expected: true,
                },
            },
            stability: Some(stability),
            assertion_mode: Default::default(),
            wait_mode: Default::default(),
        }],
    });

    TestSuite {
        name: "focus-stability-dataset".to_string(),
        version: 1,
        scenarios: focused.chain(focus_within).collect(),
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

fn interactability_suite(stability: AssertionStability) -> TestSuite {
    let viewport = (0..DATASET_SIZE).map(|index| TestScenario {
        id: format!("in-viewport-{index}"),
        name: format!("In viewport {index}"),
        surface: Surface::Web,
        timeout_ms: 1_000,
        steps: vec![TestStep {
            id: "assert-in-viewport".to_string(),
            action: Action::Assert {
                expectation: Expectation::InViewport(Target::TestId {
                    value: format!("viewport-{index}"),
                }),
            },
            stability: Some(stability),
            assertion_mode: Default::default(),
            wait_mode: Default::default(),
        }],
    });
    let pointer = (0..DATASET_SIZE).map(|index| TestScenario {
        id: format!("pointer-reachable-{index}"),
        name: format!("Pointer reachable {index}"),
        surface: Surface::Web,
        timeout_ms: 1_000,
        steps: vec![TestStep {
            id: "assert-pointer-reachable".to_string(),
            action: Action::Assert {
                expectation: Expectation::PointerReachable(Target::TestId {
                    value: format!("pointer-{index}"),
                }),
            },
            stability: Some(stability),
            assertion_mode: Default::default(),
            wait_mode: Default::default(),
        }],
    });

    TestSuite {
        name: "interactability-stability-dataset".to_string(),
        version: 1,
        scenarios: viewport.chain(pointer).collect(),
    }
}

fn viewport_coverage_suite(stability: AssertionStability) -> TestSuite {
    TestSuite {
        name: "viewport-coverage-stability-dataset".to_string(),
        version: 1,
        scenarios: (0..DATASET_SIZE)
            .map(|index| {
                let (comparison, percent) = if index % 2 == 0 {
                    (ViewportCoverageComparison::AtLeast, 80)
                } else {
                    (ViewportCoverageComparison::AtMost, 20)
                };
                TestScenario {
                    id: format!("viewport-coverage-{index}"),
                    name: format!("Viewport coverage {index}"),
                    surface: Surface::Web,
                    timeout_ms: 1_000,
                    steps: vec![TestStep {
                        id: "assert-viewport-coverage".to_string(),
                        action: Action::Assert {
                            expectation: Expectation::ViewportCoverage {
                                target: Target::TestId {
                                    value: format!("coverage-{index}"),
                                },
                                comparison,
                                percent,
                            },
                        },
                        stability: Some(stability),
                        assertion_mode: Default::default(),
                        wait_mode: Default::default(),
                    }],
                }
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
