use std::collections::VecDeque;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use a3s_test_core::{Action, Expectation, Target};
use a3s_test_driver_web::{
    AgentBrowserConfig, AgentBrowserConnectionConfig, AgentBrowserDriver, BrowserCommand,
    BrowserNetworkPolicy, CommandError, CommandExecutor, CommandInvocation, CommandOutput,
};
use async_trait::async_trait;
use serde_json::{json, Value};

const DATASET_SIZE: usize = 500;
const POINTER_SAMPLE_COUNT: usize = 9;

struct QueueExecutor {
    outputs: Mutex<VecDeque<CommandOutput>>,
    invocations: Mutex<Vec<CommandInvocation>>,
}

impl QueueExecutor {
    fn new(outputs: impl IntoIterator<Item = CommandOutput>) -> Self {
        Self {
            outputs: Mutex::new(outputs.into_iter().collect()),
            invocations: Mutex::new(Vec::new()),
        }
    }

    fn actions(&self) -> Vec<Vec<String>> {
        self.invocations
            .lock()
            .expect("invocations")
            .iter()
            .filter(|invocation| {
                invocation
                    .args
                    .last()
                    .is_none_or(|argument| argument != "--version")
            })
            .map(|invocation| {
                browser_action(&invocation.args)
                    .iter()
                    .map(|value| value.to_string_lossy().into_owned())
                    .collect()
            })
            .collect()
    }
}

#[async_trait]
impl CommandExecutor for QueueExecutor {
    async fn run(&self, invocation: CommandInvocation) -> Result<CommandOutput, CommandError> {
        let version = invocation
            .args
            .last()
            .is_some_and(|argument| argument == "--version");
        let close = browser_action(&invocation.args)
            .first()
            .is_some_and(|argument| argument == "close");
        self.invocations
            .lock()
            .expect("invocations")
            .push(invocation);
        if version {
            return Ok(output("agent-browser 0.26.0"));
        }
        if close {
            return Ok(output(r#"{"success":true,"data":{"closed":true}}"#));
        }
        self.outputs
            .lock()
            .expect("outputs")
            .pop_front()
            .ok_or_else(|| CommandError::output("unexpected browser command"))
    }
}

#[tokio::test]
async fn interactability_probes_are_atomic_and_retain_geometry_and_hit_evidence() {
    let executor = Arc::new(QueueExecutor::new([
        geometry_probe(rect(100.0, 100.0, 90.0, 90.0), viewport()),
        pointer_probe(
            rect(100.0, 100.0, 90.0, 90.0),
            viewport(),
            [true, true, false, false, false, false, false, false, false],
        ),
    ]));
    let (_temp, mut session) = connected(Arc::clone(&executor)).await;
    let target = Target::TestId {
        value: "checkout".to_string(),
    };

    let viewport_output = session
        .execute_action(
            "in-viewport",
            Action::Assert {
                expectation: Expectation::InViewport(target.clone()),
            },
        )
        .await
        .expect("viewport intersection");
    assert_eq!(viewport_output.data["in_viewport"], true);
    assert_eq!(viewport_output.data["intersection_ratio"], 1.0);
    assert_eq!(viewport_output.data["target_rect"]["width"], 90.0);
    assert_eq!(viewport_output.data["viewport_rect"]["height"], 800.0);

    let pointer_output = session
        .execute_action(
            "pointer-reachable",
            Action::Assert {
                expectation: Expectation::PointerReachable(target),
            },
        )
        .await
        .expect("pointer reachability");
    assert_eq!(pointer_output.data["pointer_reachable"], true);
    assert_eq!(pointer_output.data["sample_count"], POINTER_SAMPLE_COUNT);
    assert_eq!(pointer_output.data["reachable_samples"], 2);
    assert_eq!(
        pointer_output.data["samples"]
            .as_array()
            .expect("hit samples")
            .len(),
        POINTER_SAMPLE_COUNT
    );

    session.close_surface().await.expect("close");
    let actions = executor.actions();
    assert_eq!(actions.len(), 3);
    for action in &actions[..2] {
        assert_eq!(action[0], "eval");
        assert!(action[1].contains("A3S_INTERACTABILITY_PROBE"));
        assert!(action[1].contains("getBoundingClientRect"));
        assert!(action[1].contains("visualViewport"));
    }
    assert!(actions[1][1].contains("elementFromPoint"));
    assert!(actions[1][1].contains("ShadowRoot"));
    assert!(!actions[1][1].contains(".click("));
    assert!(!actions[1][1].contains(".focus("));
    assert!(!actions[1][1].contains("scrollIntoView"));
}

#[tokio::test]
async fn deterministic_interactability_dataset_classifies_2000_of_2000_cases() {
    let mut outputs = Vec::with_capacity(DATASET_SIZE * 4);
    for index in 0..DATASET_SIZE {
        outputs.push(geometry_probe(partial_rect(index), viewport()));
    }
    for index in 0..DATASET_SIZE {
        outputs.push(geometry_probe(outside_rect(index), viewport()));
    }
    for index in 0..DATASET_SIZE {
        let mut hits = [false; POINTER_SAMPLE_COUNT];
        hits[index % POINTER_SAMPLE_COUNT] = true;
        outputs.push(pointer_probe(partial_rect(index), viewport(), hits));
    }
    for index in 0..DATASET_SIZE {
        outputs.push(pointer_probe(
            partial_rect(index),
            viewport(),
            [false; POINTER_SAMPLE_COUNT],
        ));
    }
    let executor = Arc::new(QueueExecutor::new(outputs));
    let (_temp, mut session) = connected(Arc::clone(&executor)).await;

    for index in 0..DATASET_SIZE {
        let output = session
            .execute_action(format!("viewport-match-{index}"), in_viewport_action(index))
            .await
            .expect("intersecting target");
        assert_eq!(output.data["in_viewport"], true);
    }
    for index in 0..DATASET_SIZE {
        let error = session
            .execute_action(
                format!("viewport-mismatch-{index}"),
                in_viewport_action(index),
            )
            .await
            .expect_err("offscreen target");
        assert_eq!(error.code(), "test.assert.in_viewport");
    }
    for index in 0..DATASET_SIZE {
        let output = session
            .execute_action(format!("pointer-match-{index}"), pointer_action(index))
            .await
            .expect("reachable target");
        assert_eq!(output.data["pointer_reachable"], true);
        assert_eq!(output.data["reachable_samples"], 1);
    }
    for index in 0..DATASET_SIZE {
        let error = session
            .execute_action(format!("pointer-mismatch-{index}"), pointer_action(index))
            .await
            .expect_err("occluded target");
        assert_eq!(error.code(), "test.assert.pointer_reachable");
    }

    session.close_surface().await.expect("close");
    let actions = executor.actions();
    assert_eq!(actions.len(), DATASET_SIZE * 4 + 1);
    assert!(actions[..actions.len() - 1]
        .iter()
        .all(|action| action[0] == "eval" && action[1].contains("A3S_INTERACTABILITY_PROBE")));
}

#[tokio::test]
async fn interactability_probe_preserves_resolution_and_untrusted_evidence_errors() {
    let malformed_samples = json!([
        { "x": 115.0, "y": 115.0, "reachable": "yes" }
    ]);
    let executor = Arc::new(QueueExecutor::new([
        probe(json!({ "status": "not_found", "count": 0 })),
        probe(json!({ "status": "ambiguous", "count": 2 })),
        probe(json!({ "status": "invalid_target", "message": "invalid selector" })),
        probe(json!({
            "status": "ok",
            "target_rect": { "x": 0.0, "y": 0.0, "width": 0.0, "height": 10.0 },
            "viewport_rect": rect_json(viewport())
        })),
        probe(json!({
            "status": "ok",
            "target_rect": { "x": "zero", "y": 0.0, "width": 10.0, "height": 10.0 },
            "viewport_rect": rect_json(viewport())
        })),
        probe(json!({
            "status": "ok",
            "target_rect": rect_json(rect(100.0, 100.0, 90.0, 90.0)),
            "viewport_rect": rect_json(viewport()),
            "samples": malformed_samples
        })),
        out_of_intersection_probe(),
    ]));
    let (_temp, mut session) = connected(executor).await;

    for expected_code in [
        "test.driver.web.target_not_found",
        "test.driver.web.target_ambiguous",
        "test.driver.web.target_invalid",
        "test.driver.web.output_invalid",
        "test.driver.web.output_invalid",
    ] {
        let error = session
            .execute_action("invalid-viewport", in_viewport_action(0))
            .await
            .expect_err("invalid viewport evidence");
        assert_eq!(error.code(), expected_code);
    }
    let malformed = session
        .execute_action("malformed-hit-samples", pointer_action(0))
        .await
        .expect_err("malformed hit samples");
    assert_eq!(malformed.code(), "test.driver.web.output_invalid");

    let off_intersection = session
        .execute_action(
            "out-of-intersection-sample",
            Action::Assert {
                expectation: Expectation::PointerReachable(Target::Css {
                    selector: "#target".to_string(),
                }),
            },
        )
        .await
        .expect_err("sample outside target intersection");
    assert_eq!(off_intersection.code(), "test.driver.web.output_invalid");
    session.close_surface().await.expect("close");
}

#[tokio::test]
async fn interactability_programmatic_boundary_rejects_unstable_and_non_web_targets() {
    let executor = Arc::new(QueueExecutor::new([]));
    let (_temp, mut session) = connected(Arc::clone(&executor)).await;
    for target in [
        Target::Ref {
            value: "@e1".to_string(),
        },
        Target::VisualPoint {
            snapshot: "@v1".to_string(),
            x: 10,
            y: 20,
        },
        Target::AutomationId {
            value: "checkout".to_string(),
        },
    ] {
        for expectation in [
            Expectation::InViewport(target.clone()),
            Expectation::PointerReachable(target.clone()),
        ] {
            let error = session
                .execute_action(
                    "unsupported-interactability-target",
                    Action::Assert { expectation },
                )
                .await
                .expect_err("unsupported interactability target");
            assert_eq!(error.code(), "test.driver.web.target_unsupported");
        }
    }
    session.close_surface().await.expect("close");
    assert_eq!(executor.actions(), [vec!["close".to_string()]]);
}

fn in_viewport_action(index: usize) -> Action {
    Action::Assert {
        expectation: Expectation::InViewport(Target::Css {
            selector: format!("[data-viewport='{index}']"),
        }),
    }
}

fn pointer_action(index: usize) -> Action {
    Action::Assert {
        expectation: Expectation::PointerReachable(Target::TestId {
            value: format!("pointer-{index}"),
        }),
    }
}

fn viewport() -> Rect {
    rect(0.0, 0.0, 1_000.0, 800.0)
}

fn partial_rect(index: usize) -> Rect {
    let overlap = 1.0 + (index % 89) as f64;
    rect(1_000.0 - overlap, 100.0 + (index % 200) as f64, 90.0, 90.0)
}

fn outside_rect(index: usize) -> Rect {
    rect(1_000.0 + (index % 100) as f64, 100.0, 90.0, 90.0)
}

#[derive(Clone, Copy)]
struct Rect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

fn geometry_probe(target_rect: Rect, viewport_rect: Rect) -> CommandOutput {
    probe(json!({
        "status": "ok",
        "target_rect": rect_json(target_rect),
        "viewport_rect": rect_json(viewport_rect),
    }))
}

fn pointer_probe(
    target_rect: Rect,
    viewport_rect: Rect,
    hits: [bool; POINTER_SAMPLE_COUNT],
) -> CommandOutput {
    let left = target_rect.x.max(viewport_rect.x);
    let top = target_rect.y.max(viewport_rect.y);
    let right = (target_rect.x + target_rect.width).min(viewport_rect.x + viewport_rect.width);
    let bottom = (target_rect.y + target_rect.height).min(viewport_rect.y + viewport_rect.height);
    let fractions = [1.0 / 6.0, 0.5, 5.0 / 6.0];
    let mut samples = Vec::with_capacity(POINTER_SAMPLE_COUNT);
    for (index, reachable) in hits.into_iter().enumerate() {
        let column = index % 3;
        let row = index / 3;
        samples.push(json!({
            "x": left + (right - left) * fractions[column],
            "y": top + (bottom - top) * fractions[row],
            "reachable": reachable,
        }));
    }
    probe(json!({
        "status": "ok",
        "target_rect": rect_json(target_rect),
        "viewport_rect": rect_json(viewport_rect),
        "samples": samples,
    }))
}

fn out_of_intersection_probe() -> CommandOutput {
    let samples = (0..POINTER_SAMPLE_COUNT)
        .map(|index| {
            json!({
                "x": if index == 0 { 999.0 } else { 115.0 + (index % 3) as f64 * 30.0 },
                "y": 115.0 + (index / 3) as f64 * 30.0,
                "reachable": true,
            })
        })
        .collect::<Vec<_>>();
    probe(json!({
        "status": "ok",
        "target_rect": rect_json(rect(100.0, 100.0, 90.0, 90.0)),
        "viewport_rect": rect_json(viewport()),
        "samples": samples,
    }))
}

fn rect_json(rect: Rect) -> Value {
    json!({
        "x": rect.x,
        "y": rect.y,
        "width": rect.width,
        "height": rect.height,
    })
}

async fn connected(
    executor: Arc<QueueExecutor>,
) -> (tempfile::TempDir, a3s_test_driver_web::AgentBrowserSession) {
    let temp = tempfile::tempdir().expect("tempdir");
    let driver = AgentBrowserDriver::with_executor(
        AgentBrowserConfig {
            command: BrowserCommand::Standalone {
                executable: PathBuf::from("fixture-agent-browser"),
            },
            namespace: "interactability-assertions".to_string(),
            headed: false,
            command_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(30),
            microphone: Default::default(),
            network_policy: BrowserNetworkPolicy::default(),
        },
        executor,
    );
    let session = driver
        .connect(AgentBrowserConnectionConfig {
            namespace: "interactability-assertions".to_string(),
            session: "fixture".to_string(),
            runtime_dir: temp.path().join("runtime"),
            artifacts_dir: temp.path().join("artifacts"),
            active_video_path: None,
        })
        .await
        .expect("connect");
    (temp, session)
}

fn probe(result: Value) -> CommandOutput {
    output(
        &json!({
            "success": true,
            "data": { "result": result },
            "error": null
        })
        .to_string(),
    )
}

fn output(stdout: &str) -> CommandOutput {
    CommandOutput {
        exit_code: 0,
        stdout: stdout.to_string(),
        stderr: String::new(),
    }
}

fn browser_action(args: &[OsString]) -> &[OsString] {
    let index = args
        .iter()
        .position(|value| value == "--headed")
        .map_or(0, |index| index + 2);
    &args[index..]
}
