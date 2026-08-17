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
async fn rendered_text_and_visible_count_classify_observation_outcomes_exactly() {
    let executor = Arc::new(QueueExecutor::new([
        probe(json!({ "status": "ok", "actual": "  Total\n\t$42.00  ", "count": 1 })),
        probe(json!({ "status": "ok", "actual": "Total $41.00", "count": 1 })),
        probe(json!({ "status": "not_found", "count": 0 })),
        probe(json!({ "status": "ambiguous", "count": 2 })),
        probe(json!({ "status": "ok", "actual": 3, "count": 3 })),
        probe(json!({ "status": "ok", "actual": 0, "count": 0 })),
        probe(json!({ "status": "ok", "actual": 4, "count": 4 })),
        probe(json!({
            "status": "invalid_target",
            "message": "invalid selector"
        })),
    ]));
    let (_temp, mut session) = connected(executor.clone()).await;
    let total = Target::TestId {
        value: "total".to_string(),
    };

    let matched = session
        .execute_action("total-copy", rendered_text(total.clone(), "Total $42.00"))
        .await
        .expect("matching rendered text");
    assert_eq!(matched.data["expected"], "Total $42.00");
    assert_eq!(matched.data["actual"], "Total $42.00");

    let mismatch = session
        .execute_action("wrong-total", rendered_text(total.clone(), "Total $42.00"))
        .await
        .expect_err("wrong rendered text must fail");
    assert_eq!(mismatch.code(), "test.assert.rendered_text");

    let missing = session
        .execute_action("missing-total", rendered_text(total.clone(), "Total"))
        .await
        .expect_err("missing text target remains a driver failure");
    assert_eq!(missing.code(), "test.driver.web.target_not_found");

    let ambiguous = session
        .execute_action("duplicate-total", rendered_text(total, "Total"))
        .await
        .expect_err("ambiguous text target remains a driver failure");
    assert_eq!(ambiguous.code(), "test.driver.web.target_ambiguous");

    let rows = Target::Css {
        selector: "[data-row]".to_string(),
    };
    let three = session
        .execute_action("three-rows", visible_count(rows.clone(), 3))
        .await
        .expect("matching visible count");
    assert_eq!(three.data["actual"], 3);

    let zero = session
        .execute_action("no-errors", visible_count(rows.clone(), 0))
        .await
        .expect("zero is an observable count");
    assert_eq!(zero.data["actual"], 0);

    let count_mismatch = session
        .execute_action("wrong-row-count", visible_count(rows.clone(), 3))
        .await
        .expect_err("wrong visible count must fail");
    assert_eq!(count_mismatch.code(), "test.assert.visible_count");

    let invalid = session
        .execute_action("invalid-row-target", visible_count(rows, 0))
        .await
        .expect_err("invalid selector remains a driver failure");
    assert_eq!(invalid.code(), "test.driver.web.target_invalid");

    session.close_surface().await.expect("close");
    for action in &executor.actions()[..8] {
        assert_eq!(action[0], "eval");
        assert!(action[1].contains("A3S_ASSERTION_PROBE"));
    }
}

#[tokio::test]
async fn current_refs_support_rendered_text_but_not_locator_cardinality() {
    let executor = Arc::new(QueueExecutor::new([output(
        r#"{"success":true,"data":{"value":"Ready"},"error":null}"#,
    )]));
    let (_temp, mut session) = connected(executor.clone()).await;
    let reference = Target::Ref {
        value: "@e4".to_string(),
    };

    session
        .execute_action("ref-copy", rendered_text(reference.clone(), "Ready"))
        .await
        .expect("current ref rendered text");
    let unsupported = session
        .execute_action("ref-count", visible_count(reference, 1))
        .await
        .expect_err("a ref is not a locator set");
    assert_eq!(unsupported.code(), "test.driver.web.target_unsupported");
    session.close_surface().await.expect("close");

    assert_eq!(executor.actions()[0], ["get", "text", "@e4"]);
}

#[tokio::test]
async fn deterministic_rendered_dataset_classifies_600_of_600_cases() {
    let mut outputs = Vec::with_capacity(DATASET_SIZE * 6);
    for index in 0..DATASET_SIZE {
        outputs.push(probe(json!({
            "status": "ok",
            "actual": format!("copy-{index}"),
            "count": 1
        })));
    }
    for index in 0..DATASET_SIZE {
        outputs.push(probe(json!({
            "status": "ok",
            "actual": format!("wrong-{index}"),
            "count": 1
        })));
    }
    outputs.extend((0..DATASET_SIZE).map(|_| probe(json!({ "status": "not_found", "count": 0 }))));
    for index in 0..DATASET_SIZE {
        outputs.push(probe(json!({
            "status": "ok",
            "actual": index,
            "count": index
        })));
    }
    for index in 0..DATASET_SIZE {
        outputs.push(probe(json!({
            "status": "ok",
            "actual": index + 1,
            "count": index + 1
        })));
    }
    outputs.extend((0..DATASET_SIZE).map(|_| {
        probe(json!({
            "status": "invalid_target",
            "message": "invalid selector"
        }))
    }));
    let executor = Arc::new(QueueExecutor::new(outputs));
    let (_temp, mut session) = connected(executor.clone()).await;

    for index in 0..DATASET_SIZE {
        session
            .execute_action(
                format!("copy-match-{index}"),
                rendered_text(css(format!("#copy-{index}")), format!("copy-{index}")),
            )
            .await
            .unwrap_or_else(|error| panic!("matching rendered text {index} failed: {error}"));
    }
    for index in 0..DATASET_SIZE {
        let error = session
            .execute_action(
                format!("copy-mismatch-{index}"),
                rendered_text(css(format!("#wrong-{index}")), format!("copy-{index}")),
            )
            .await
            .expect_err("rendered text mismatch");
        assert_eq!(error.code(), "test.assert.rendered_text");
    }
    for index in 0..DATASET_SIZE {
        let error = session
            .execute_action(
                format!("copy-missing-{index}"),
                rendered_text(css(format!("#missing-{index}")), "missing"),
            )
            .await
            .expect_err("missing rendered text target");
        assert_eq!(error.code(), "test.driver.web.target_not_found");
    }
    for index in 0..DATASET_SIZE {
        session
            .execute_action(
                format!("count-match-{index}"),
                visible_count(css(format!(".match-{index}")), index as u32),
            )
            .await
            .unwrap_or_else(|error| panic!("matching visible count {index} failed: {error}"));
    }
    for index in 0..DATASET_SIZE {
        let error = session
            .execute_action(
                format!("count-mismatch-{index}"),
                visible_count(css(format!(".wrong-{index}")), index as u32),
            )
            .await
            .expect_err("visible count mismatch");
        assert_eq!(error.code(), "test.assert.visible_count");
    }
    for index in 0..DATASET_SIZE {
        let error = session
            .execute_action(
                format!("count-invalid-{index}"),
                visible_count(css(format!("[invalid-{index}")), 0),
            )
            .await
            .expect_err("invalid visible count selector");
        assert_eq!(error.code(), "test.driver.web.target_invalid");
    }
    session.close_surface().await.expect("close");

    assert_eq!(executor.actions().len(), DATASET_SIZE * 6 + 1);
}

fn rendered_text(target: Target, value: impl Into<String>) -> Action {
    Action::Assert {
        expectation: Expectation::RenderedText {
            target,
            value: value.into(),
        },
    }
}

fn visible_count(target: Target, count: u32) -> Action {
    Action::Assert {
        expectation: Expectation::VisibleCount { target, count },
    }
}

fn css(selector: String) -> Target {
    Target::Css { selector }
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
            namespace: "rendered-assertions".to_string(),
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
            namespace: "rendered-assertions".to_string(),
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
