use std::collections::VecDeque;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use a3s_test_core::{
    Action, Expectation, LayoutRect, LayoutRelation, Target, MAX_LAYOUT_COORDINATE_ABS,
    MAX_LAYOUT_TOLERANCE_PX,
};
use a3s_test_driver_web::{
    AgentBrowserConfig, AgentBrowserConnectionConfig, AgentBrowserDriver, BrowserCommand,
    BrowserNetworkPolicy, CommandError, CommandExecutor, CommandInvocation, CommandOutput,
};
use async_trait::async_trait;
use serde_json::json;

const DATASET_SIZE: usize = 100;

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
async fn layout_probe_is_atomic_and_preserves_both_rectangles_as_evidence() {
    let executor = Arc::new(QueueExecutor::new([layout_probe(
        rect(120.0, 40.0, 40.0, 50.0),
        rect(100.0, 100.0, 100.0, 100.0),
    )]));
    let (_temp, mut session) = connected(Arc::clone(&executor)).await;
    let action = layout_action(
        Target::Css {
            selector: "#subject".to_string(),
        },
        Target::TestId {
            value: "reference".to_string(),
        },
        LayoutRelation::Above,
        1,
    );

    let result = session
        .execute_action("layout", action)
        .await
        .expect("matching layout relation");
    assert_eq!(result.data["relation"], "above");
    assert_eq!(result.data["tolerance_px"], 1);
    assert_eq!(result.data["target_rect"]["x"], 120.0);
    assert_eq!(result.data["relative_rect"]["width"], 100.0);
    assert_eq!(result.data["matched"], true);

    session.close_surface().await.expect("close");
    let actions = executor.actions();
    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0][0], "eval");
    assert!(actions[0][1].contains("const target ="));
    assert!(actions[0][1].contains("const relativeTo ="));
    assert!(actions[0][1].contains("getBoundingClientRect"));
    assert!(actions[0][1].contains("A3S_LAYOUT_PROBE"));
}

#[tokio::test]
async fn deterministic_layout_dataset_classifies_3400_of_3400_relation_cases() {
    let cases = relation_cases();
    let mut outputs = Vec::with_capacity(cases.len() * DATASET_SIZE * 2);
    for case in &cases {
        for index in 0..DATASET_SIZE {
            let offset = index as f64 * 3.0;
            outputs.push(layout_probe(
                translate(case.matching, offset),
                translate(case.reference, offset),
            ));
        }
    }
    for case in &cases {
        for index in 0..DATASET_SIZE {
            let offset = index as f64 * 3.0;
            outputs.push(layout_probe(
                translate(case.violating, offset),
                translate(case.reference, offset),
            ));
        }
    }
    let executor = Arc::new(QueueExecutor::new(outputs));
    let (_temp, mut session) = connected(Arc::clone(&executor)).await;

    for case in &cases {
        for index in 0..DATASET_SIZE {
            let output = session
                .execute_action(
                    format!("{:?}-match-{index}", case.relation),
                    dataset_action(case.relation, index),
                )
                .await
                .unwrap_or_else(|error| {
                    panic!("{:?} matching case {index} failed: {error}", case.relation)
                });
            assert_eq!(output.data["matched"], true);
        }
    }
    for case in &cases {
        for index in 0..DATASET_SIZE {
            let error = session
                .execute_action(
                    format!("{:?}-mismatch-{index}", case.relation),
                    dataset_action(case.relation, index),
                )
                .await
                .expect_err("violating layout relation must fail");
            assert_eq!(error.code(), "test.assert.layout");
        }
    }

    session.close_surface().await.expect("close");
    let actions = executor.actions();
    assert_eq!(actions.len(), cases.len() * DATASET_SIZE * 2 + 1);
    assert!(actions[..actions.len() - 1]
        .iter()
        .all(|action| action[0] == "eval" && action[1].contains("A3S_LAYOUT_PROBE")));
}

#[tokio::test]
async fn layout_probe_preserves_resolution_and_untrusted_geometry_errors() {
    let executor = Arc::new(QueueExecutor::new([
        probe(json!({ "status": "not_found", "subject": "target", "count": 0 })),
        probe(json!({ "status": "ambiguous", "subject": "relative_to", "count": 2 })),
        probe(json!({
            "status": "invalid_target",
            "subject": "relative_to",
            "message": "invalid selector"
        })),
        probe(json!({
            "status": "ok",
            "target_rect": { "x": 0.0, "y": 0.0, "width": 0.0, "height": 10.0 },
            "relative_rect": { "x": 0.0, "y": 20.0, "width": 10.0, "height": 10.0 }
        })),
        probe(json!({
            "status": "ok",
            "target_rect": {
                "x": MAX_LAYOUT_COORDINATE_ABS,
                "y": 0.0,
                "width": 10.0,
                "height": 10.0
            },
            "relative_rect": { "x": 0.0, "y": 20.0, "width": 10.0, "height": 10.0 }
        })),
        probe(json!({
            "status": "ok",
            "target_rect": { "x": "zero", "y": 0.0, "width": 10.0, "height": 10.0 },
            "relative_rect": { "x": 0.0, "y": 20.0, "width": 10.0, "height": 10.0 }
        })),
    ]));
    let (_temp, mut session) = connected(executor).await;
    let action = || dataset_action(LayoutRelation::Above, 0);

    for expected_code in [
        "test.driver.web.target_not_found",
        "test.driver.web.target_ambiguous",
        "test.driver.web.target_invalid",
        "test.driver.web.output_invalid",
        "test.driver.web.output_invalid",
        "test.driver.web.output_invalid",
    ] {
        let error = session
            .execute_action("invalid-layout-observation", action())
            .await
            .expect_err("invalid layout observation");
        assert_eq!(error.code(), expected_code);
    }
    session.close_surface().await.expect("close");
}

#[tokio::test]
async fn layout_programmatic_boundary_rejects_unstable_targets_and_excessive_tolerance() {
    let executor = Arc::new(QueueExecutor::new([]));
    let (_temp, mut session) = connected(Arc::clone(&executor)).await;
    let stable = Target::Css {
        selector: "#reference".to_string(),
    };
    for action in [
        layout_action(
            Target::Ref {
                value: "@e1".to_string(),
            },
            stable.clone(),
            LayoutRelation::Above,
            0,
        ),
        layout_action(
            stable.clone(),
            Target::VisualPoint {
                snapshot: "@v1".to_string(),
                x: 10,
                y: 10,
            },
            LayoutRelation::Above,
            0,
        ),
    ] {
        let error = session
            .execute_action("unstable-layout-target", action)
            .await
            .expect_err("unstable layout targets must fail");
        assert_eq!(error.code(), "test.driver.web.target_unsupported");
    }
    let error = session
        .execute_action(
            "excessive-layout-tolerance",
            layout_action(
                stable.clone(),
                stable,
                LayoutRelation::Above,
                MAX_LAYOUT_TOLERANCE_PX + 1,
            ),
        )
        .await
        .expect_err("excessive tolerance must fail");
    assert_eq!(error.code(), "test.driver.web.expectation_invalid");

    session.close_surface().await.expect("close");
    assert_eq!(executor.actions(), [vec!["close".to_string()]]);
}

#[derive(Clone, Copy)]
struct RelationCase {
    relation: LayoutRelation,
    matching: LayoutRect,
    violating: LayoutRect,
    reference: LayoutRect,
}

fn relation_cases() -> Vec<RelationCase> {
    let reference = rect(100.0, 100.0, 100.0, 100.0);
    [
        (
            LayoutRelation::Above,
            rect(120.0, 40.0, 40.0, 50.0),
            rect(120.0, 110.0, 40.0, 50.0),
        ),
        (
            LayoutRelation::Below,
            rect(120.0, 210.0, 40.0, 50.0),
            rect(120.0, 140.0, 40.0, 50.0),
        ),
        (
            LayoutRelation::LeftOf,
            rect(40.0, 120.0, 50.0, 40.0),
            rect(110.0, 120.0, 50.0, 40.0),
        ),
        (
            LayoutRelation::RightOf,
            rect(210.0, 120.0, 50.0, 40.0),
            rect(140.0, 120.0, 50.0, 40.0),
        ),
        (
            LayoutRelation::Contains,
            rect(90.0, 90.0, 120.0, 120.0),
            rect(110.0, 90.0, 120.0, 120.0),
        ),
        (
            LayoutRelation::Inside,
            rect(120.0, 120.0, 40.0, 40.0),
            rect(80.0, 120.0, 40.0, 40.0),
        ),
        (
            LayoutRelation::Overlaps,
            rect(180.0, 180.0, 40.0, 40.0),
            rect(210.0, 210.0, 40.0, 40.0),
        ),
        (
            LayoutRelation::NotOverlapping,
            rect(210.0, 210.0, 40.0, 40.0),
            rect(180.0, 180.0, 40.0, 40.0),
        ),
        (
            LayoutRelation::AlignedLeft,
            rect(100.0, 230.0, 40.0, 40.0),
            rect(102.0, 230.0, 40.0, 40.0),
        ),
        (
            LayoutRelation::AlignedRight,
            rect(140.0, 230.0, 60.0, 40.0),
            rect(139.0, 230.0, 60.0, 40.0),
        ),
        (
            LayoutRelation::AlignedTop,
            rect(230.0, 100.0, 40.0, 40.0),
            rect(230.0, 102.0, 40.0, 40.0),
        ),
        (
            LayoutRelation::AlignedBottom,
            rect(230.0, 140.0, 40.0, 60.0),
            rect(230.0, 139.0, 40.0, 60.0),
        ),
        (
            LayoutRelation::AlignedCenterX,
            rect(130.0, 230.0, 40.0, 40.0),
            rect(132.0, 230.0, 40.0, 40.0),
        ),
        (
            LayoutRelation::AlignedCenterY,
            rect(230.0, 130.0, 40.0, 40.0),
            rect(230.0, 132.0, 40.0, 40.0),
        ),
        (
            LayoutRelation::SameWidth,
            rect(230.0, 230.0, 100.0, 40.0),
            rect(230.0, 230.0, 99.0, 40.0),
        ),
        (
            LayoutRelation::SameHeight,
            rect(230.0, 230.0, 40.0, 100.0),
            rect(230.0, 230.0, 40.0, 99.0),
        ),
        (
            LayoutRelation::SameSize,
            rect(230.0, 230.0, 100.0, 100.0),
            rect(230.0, 230.0, 100.0, 99.0),
        ),
    ]
    .into_iter()
    .map(|(relation, matching, violating)| RelationCase {
        relation,
        matching,
        violating,
        reference,
    })
    .collect()
}

fn dataset_action(relation: LayoutRelation, index: usize) -> Action {
    layout_action(
        Target::Css {
            selector: format!("[data-subject='{index}']"),
        },
        Target::TestId {
            value: format!("reference-{index}"),
        },
        relation,
        0,
    )
}

fn layout_action(
    target: Target,
    relative_to: Target,
    relation: LayoutRelation,
    tolerance_px: u32,
) -> Action {
    Action::Assert {
        expectation: Expectation::Layout {
            target,
            relative_to,
            relation,
            tolerance_px,
        },
    }
}

fn rect(x: f64, y: f64, width: f64, height: f64) -> LayoutRect {
    LayoutRect {
        x,
        y,
        width,
        height,
    }
}

fn translate(rect: LayoutRect, offset: f64) -> LayoutRect {
    LayoutRect {
        x: rect.x + offset,
        y: rect.y + offset,
        ..rect
    }
}

fn layout_probe(target_rect: LayoutRect, relative_rect: LayoutRect) -> CommandOutput {
    probe(json!({
        "status": "ok",
        "target_rect": rect_json(target_rect),
        "relative_rect": rect_json(relative_rect),
    }))
}

fn rect_json(rect: LayoutRect) -> serde_json::Value {
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
            namespace: "layout-assertions".to_string(),
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
            namespace: "layout-assertions".to_string(),
            session: "fixture".to_string(),
            runtime_dir: temp.path().join("runtime"),
            artifacts_dir: temp.path().join("artifacts"),
            active_video_path: None,
        })
        .await
        .expect("connect");
    (temp, session)
}

fn probe(result: serde_json::Value) -> CommandOutput {
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
