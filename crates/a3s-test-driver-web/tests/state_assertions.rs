use std::collections::VecDeque;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use a3s_test_core::{Action, ElementState, Expectation, Target};
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
async fn semantic_and_css_probes_separate_product_mismatches_from_unknown_targets() {
    let executor = Arc::new(QueueExecutor::new([
        probe(json!({ "status": "ok", "actual": true, "count": 1 })),
        probe(json!({ "status": "ok", "actual": false, "count": 1 })),
        probe(json!({ "status": "not_found", "count": 0 })),
        probe(json!({ "status": "ambiguous", "count": 2 })),
        probe(json!({ "status": "unsupported", "count": 1 })),
    ]));
    let (temp, mut session) = connected(executor.clone()).await;

    let output = session
        .execute_action(
            "enabled",
            assert_state(
                Target::Role {
                    role: "button".to_string(),
                    name: "Submit".to_string(),
                },
                ElementState::Enabled,
                true,
            ),
        )
        .await
        .expect("enabled state");
    assert_eq!(output.data["expected"], true);
    assert_eq!(output.data["actual"], true);

    let mismatch = session
        .execute_action(
            "checked",
            assert_state(
                Target::Css {
                    selector: "#terms".to_string(),
                },
                ElementState::Checked,
                true,
            ),
        )
        .await
        .expect_err("unchecked control must fail a checked assertion");
    assert_eq!(mismatch.code(), "test.assert.checked");

    let missing = session
        .execute_action(
            "missing-unchecked",
            assert_state(
                Target::Css {
                    selector: "#missing".to_string(),
                },
                ElementState::Checked,
                false,
            ),
        )
        .await
        .expect_err("missing target is not proof of unchecked state");
    assert_eq!(missing.code(), "test.driver.web.target_not_found");

    let ambiguous = session
        .execute_action(
            "ambiguous-value",
            Action::Assert {
                expectation: Expectation::Value {
                    target: Target::Label {
                        value: "Name".to_string(),
                    },
                    value: "Ada".to_string(),
                },
            },
        )
        .await
        .expect_err("ambiguous target must remain a driver error");
    assert_eq!(ambiguous.code(), "test.driver.web.target_ambiguous");

    let unsupported = session
        .execute_action(
            "unsupported-selection",
            Action::Assert {
                expectation: Expectation::SelectedValues {
                    target: Target::Css {
                        selector: "#not-a-select".to_string(),
                    },
                    values: Vec::new(),
                },
            },
        )
        .await
        .expect_err("unsupported state must remain unknown");
    assert_eq!(unsupported.code(), "test.driver.web.state_unsupported");

    session.close_surface().await.expect("close");
    let actions = executor.actions();
    assert_eq!(actions.len(), 6);
    for action in &actions[..5] {
        assert_eq!(action[0], "eval");
        assert!(action[1].contains("A3S_ASSERTION_PROBE"));
    }
    drop(temp);
}

#[tokio::test]
async fn value_and_multi_selection_assertions_compare_exact_observed_state() {
    let executor = Arc::new(QueueExecutor::new([
        probe(json!({ "status": "ok", "actual": "Ada", "count": 1 })),
        probe(json!({
            "status": "ok",
            "actual": ["review", "published"],
            "count": 1
        })),
        probe(json!({ "status": "ok", "actual": "Grace", "count": 1 })),
        probe(json!({
            "status": "ok",
            "actual": ["review"],
            "count": 1
        })),
    ]));
    let (_temp, mut session) = connected(executor).await;
    let target = Target::Css {
        selector: "#control".to_string(),
    };

    let value = session
        .execute_action(
            "value",
            Action::Assert {
                expectation: Expectation::Value {
                    target: target.clone(),
                    value: "Ada".to_string(),
                },
            },
        )
        .await
        .expect("matching value");
    assert_eq!(value.data["actual"], "Ada");

    let selection = session
        .execute_action(
            "selection",
            Action::Assert {
                expectation: Expectation::SelectedValues {
                    target: target.clone(),
                    values: vec!["published".to_string(), "review".to_string()],
                },
            },
        )
        .await
        .expect("matching selected set");
    assert_eq!(selection.data["actual"], json!(["published", "review"]));

    let value_error = session
        .execute_action(
            "wrong-value",
            Action::Assert {
                expectation: Expectation::Value {
                    target: target.clone(),
                    value: "Ada".to_string(),
                },
            },
        )
        .await
        .expect_err("mismatching value");
    assert_eq!(value_error.code(), "test.assert.value");
    assert!(value_error.message().contains("Grace"));

    let selection_error = session
        .execute_action(
            "wrong-selection",
            Action::Assert {
                expectation: Expectation::SelectedValues {
                    target,
                    values: vec!["published".to_string(), "review".to_string()],
                },
            },
        )
        .await
        .expect_err("mismatching selected set");
    assert_eq!(selection_error.code(), "test.assert.selected_values");
    session.close_surface().await.expect("close");
}

#[tokio::test]
async fn current_refs_prioritize_native_checked_state_and_use_aria_for_custom_controls() {
    let executor = Arc::new(QueueExecutor::new([
        output(r#"{"success":true,"data":{"value":"Draft"},"error":null}"#),
        output(r#"{"success":true,"data":{"enabled":false},"error":null}"#),
        output(r#"{"success":true,"data":{"value":"checkbox"},"error":null}"#),
        output(r#"{"success":true,"data":{"checked":true},"error":null}"#),
        output(r#"{"success":true,"data":{"value":"true"},"error":null}"#),
        output(r#"{"success":true,"data":{"value":null},"error":null}"#),
        output(r#"{"success":true,"data":{"value":null},"error":null}"#),
        output(r#"{"success":true,"data":{"value":"false"},"error":null}"#),
    ]));
    let (_temp, mut session) = connected(executor.clone()).await;
    let reference = Target::Ref {
        value: "@e4".to_string(),
    };

    session
        .execute_action(
            "value",
            Action::Assert {
                expectation: Expectation::Value {
                    target: reference.clone(),
                    value: "Draft".to_string(),
                },
            },
        )
        .await
        .expect("ref value");
    session
        .execute_action(
            "disabled",
            assert_state(reference.clone(), ElementState::Enabled, false),
        )
        .await
        .expect("ref disabled");
    session
        .execute_action(
            "checked",
            assert_state(reference.clone(), ElementState::Checked, true),
        )
        .await
        .expect("ref checked");
    session
        .execute_action(
            "selected",
            assert_state(reference.clone(), ElementState::Selected, true),
        )
        .await
        .expect("ARIA selected ref");
    let unsupported = session
        .execute_action(
            "native-option",
            assert_state(reference, ElementState::Selected, true),
        )
        .await
        .expect_err("native option property is unavailable through a ref query");
    assert_eq!(unsupported.code(), "test.driver.web.state_unsupported");
    session
        .execute_action(
            "custom-unchecked",
            assert_state(
                Target::Ref {
                    value: "@e5".to_string(),
                },
                ElementState::Checked,
                false,
            ),
        )
        .await
        .expect("ARIA checked state for a custom control");
    session.close_surface().await.expect("close");

    let actions = executor.actions();
    assert_eq!(actions[0], ["get", "value", "@e4"]);
    assert_eq!(actions[1], ["is", "enabled", "@e4"]);
    assert_eq!(actions[2], ["get", "attr", "@e4", "type"]);
    assert_eq!(actions[3], ["is", "checked", "@e4"]);
    assert_eq!(actions[4], ["get", "attr", "@e4", "aria-selected"]);
    assert_eq!(actions[5], ["get", "attr", "@e4", "aria-selected"]);
    assert_eq!(actions[6], ["get", "attr", "@e5", "type"]);
    assert_eq!(actions[7], ["get", "attr", "@e5", "aria-checked"]);
}

#[tokio::test]
async fn exact_and_composed_focus_ownership_are_separate_live_states() {
    let executor = Arc::new(QueueExecutor::new([
        probe(json!({ "status": "ok", "actual": true, "count": 1 })),
        probe(json!({ "status": "ok", "actual": true, "count": 1 })),
        probe(json!({ "status": "ok", "actual": false, "count": 1 })),
        probe(json!({ "status": "ok", "actual": true, "count": 1 })),
    ]));
    let (_temp, mut session) = connected(executor.clone()).await;
    let target = Target::TestId {
        value: "dialog".to_string(),
    };

    let focused = session
        .execute_action(
            "focused",
            assert_state(target.clone(), ElementState::Focused, true),
        )
        .await
        .expect("exact focus ownership");
    assert_eq!(focused.data["state"], "focused");
    assert_eq!(focused.data["actual"], true);

    let within = session
        .execute_action(
            "focus-within",
            assert_state(target.clone(), ElementState::FocusWithin, true),
        )
        .await
        .expect("composed focus ownership");
    assert_eq!(within.data["state"], "focus_within");

    let exact_mismatch = session
        .execute_action(
            "not-exactly-focused",
            assert_state(target.clone(), ElementState::Focused, true),
        )
        .await
        .expect_err("a focused descendant is not exact target focus");
    assert_eq!(exact_mismatch.code(), "test.assert.focused");

    let outside_mismatch = session
        .execute_action(
            "not-outside",
            assert_state(target, ElementState::FocusWithin, false),
        )
        .await
        .expect_err("focus inside the target violates focus_outside");
    assert_eq!(outside_mismatch.code(), "test.assert.focus_outside");
    session.close_surface().await.expect("close");

    let actions = executor.actions();
    for action in &actions[..4] {
        assert_eq!(action[0], "eval");
        assert!(action[1].contains("deepestActiveElement"));
    }
    assert!(actions[1][1].contains("composedContains"));
}

#[tokio::test]
async fn observation_refs_cannot_claim_stable_only_states() {
    let executor = Arc::new(QueueExecutor::new([]));
    let (_temp, mut session) = connected(executor.clone()).await;
    for state in [
        ElementState::Focused,
        ElementState::FocusWithin,
        ElementState::Expanded,
        ElementState::Pressed,
        ElementState::ReadOnly,
        ElementState::Required,
        ElementState::Invalid,
    ] {
        let error = session
            .execute_action(
                "unstable-state-ref",
                assert_state(
                    Target::Ref {
                        value: "@e4".to_string(),
                    },
                    state,
                    true,
                ),
            )
            .await
            .expect_err("state requires a live stable locator");
        assert_eq!(error.code(), "test.driver.web.state_unsupported");
    }
    session.close_surface().await.expect("close");
    assert_eq!(executor.actions(), [vec!["close".to_string()]]);
}

#[tokio::test]
async fn extended_semantic_states_keep_matches_mismatches_and_unknowns_separate() {
    let dimensions = semantic_state_dimensions();
    let mut outputs = Vec::new();
    for actual in [true, false, false, true] {
        outputs.extend(
            dimensions
                .iter()
                .map(|_| probe(json!({ "status": "ok", "actual": actual, "count": 1 }))),
        );
    }
    outputs.extend(
        dimensions
            .iter()
            .map(|_| probe(json!({ "status": "not_found", "count": 0 }))),
    );
    outputs.extend(
        dimensions
            .iter()
            .map(|_| probe(json!({ "status": "unsupported", "count": 1 }))),
    );
    let executor = Arc::new(QueueExecutor::new(outputs));
    let (_temp, mut session) = connected(executor.clone()).await;

    for &(state, probe_name, _, _, _) in &dimensions {
        let output = session
            .execute_action(
                format!("{probe_name}-positive"),
                assert_state(
                    Target::TestId {
                        value: format!("{probe_name}-positive"),
                    },
                    state,
                    true,
                ),
            )
            .await
            .unwrap_or_else(|error| panic!("positive {probe_name} state failed: {error}"));
        assert_eq!(output.data["state"], probe_name);
        assert_eq!(output.data["expected"], true);
        assert_eq!(output.data["actual"], true);
    }
    for &(state, probe_name, negative_name, _, _) in &dimensions {
        let output = session
            .execute_action(
                format!("{negative_name}-negative"),
                assert_state(
                    Target::Css {
                        selector: format!("#{negative_name}-negative"),
                    },
                    state,
                    false,
                ),
            )
            .await
            .unwrap_or_else(|error| panic!("negative {negative_name} state failed: {error}"));
        assert_eq!(output.data["state"], probe_name);
        assert_eq!(output.data["expected"], false);
        assert_eq!(output.data["actual"], false);
    }
    for &(state, probe_name, _, positive_code, _) in &dimensions {
        let error = session
            .execute_action(
                format!("{probe_name}-mismatch"),
                assert_state(
                    Target::TestId {
                        value: format!("{probe_name}-mismatch"),
                    },
                    state,
                    true,
                ),
            )
            .await
            .expect_err("false state must fail its positive assertion");
        assert_eq!(error.code(), positive_code);
    }
    for &(state, _, negative_name, _, negative_code) in &dimensions {
        let error = session
            .execute_action(
                format!("{negative_name}-mismatch"),
                assert_state(
                    Target::TestId {
                        value: format!("{negative_name}-mismatch"),
                    },
                    state,
                    false,
                ),
            )
            .await
            .expect_err("true state must fail its negative assertion");
        assert_eq!(error.code(), negative_code);
    }
    for &(state, _, negative_name, _, _) in &dimensions {
        let error = session
            .execute_action(
                format!("missing-{negative_name}"),
                assert_state(
                    Target::Css {
                        selector: format!("#missing-{negative_name}"),
                    },
                    state,
                    false,
                ),
            )
            .await
            .expect_err("missing target must not prove a negative semantic state");
        assert_eq!(error.code(), "test.driver.web.target_not_found");
    }
    for &(state, probe_name, _, _, _) in &dimensions {
        let error = session
            .execute_action(
                format!("unsupported-{probe_name}"),
                assert_state(
                    Target::Css {
                        selector: format!("#unsupported-{probe_name}"),
                    },
                    state,
                    true,
                ),
            )
            .await
            .expect_err("unsupported semantic state must remain unknown");
        assert_eq!(error.code(), "test.driver.web.state_unsupported");
    }
    session.close_surface().await.expect("close");

    let actions = executor.actions();
    assert_eq!(actions.len(), dimensions.len() * 6 + 1);
    for action in &actions[..dimensions.len() * 6] {
        assert_eq!(action[0], "eval");
        assert!(action[1].contains("A3S_ASSERTION_PROBE"));
    }
}

#[tokio::test]
async fn deterministic_extended_state_dataset_classifies_1000_of_1000_cases() {
    let dimensions = semantic_state_dimensions();
    let mut outputs = Vec::with_capacity(DATASET_SIZE * dimensions.len() * 2);
    for _ in &dimensions {
        for actual in [true, false] {
            outputs.extend(
                (0..DATASET_SIZE)
                    .map(|_| probe(json!({ "status": "ok", "actual": actual, "count": 1 }))),
            );
        }
    }
    let executor = Arc::new(QueueExecutor::new(outputs));
    let (_temp, mut session) = connected(executor.clone()).await;

    for &(state, probe_name, negative_name, _, _) in &dimensions {
        for (expected, assertion_name) in [(true, probe_name), (false, negative_name)] {
            for index in 0..DATASET_SIZE {
                session
                    .execute_action(
                        format!("{assertion_name}-{index}"),
                        assert_state(
                            Target::TestId {
                                value: format!("{assertion_name}-{index}"),
                            },
                            state,
                            expected,
                        ),
                    )
                    .await
                    .unwrap_or_else(|error| {
                        panic!("{assertion_name} case {index} failed: {error}")
                    });
            }
        }
    }
    session.close_surface().await.expect("close");
    assert_eq!(
        executor.actions().len(),
        DATASET_SIZE * dimensions.len() * 2 + 1
    );
}

#[tokio::test]
async fn deterministic_focus_dataset_classifies_600_of_600_cases() {
    let mut outputs = Vec::with_capacity(DATASET_SIZE * 6);
    for actual in [true, false, true, false, false] {
        outputs.extend(
            (0..DATASET_SIZE)
                .map(|_| probe(json!({ "status": "ok", "actual": actual, "count": 1 }))),
        );
    }
    outputs.extend((0..DATASET_SIZE).map(|_| probe(json!({ "status": "not_found", "count": 0 }))));
    let executor = Arc::new(QueueExecutor::new(outputs));
    let (_temp, mut session) = connected(executor.clone()).await;

    for (state, expected, prefix) in [
        (ElementState::Focused, true, "focused"),
        (ElementState::Focused, false, "unfocused"),
        (ElementState::FocusWithin, true, "focus-within"),
        (ElementState::FocusWithin, false, "focus-outside"),
    ] {
        for index in 0..DATASET_SIZE {
            session
                .execute_action(
                    format!("{prefix}-{index}"),
                    assert_state(
                        Target::Css {
                            selector: format!("#{prefix}-{index}"),
                        },
                        state,
                        expected,
                    ),
                )
                .await
                .unwrap_or_else(|error| panic!("{prefix} case {index} failed: {error}"));
        }
    }

    for index in 0..DATASET_SIZE {
        let mismatch = session
            .execute_action(
                format!("focus-mismatch-{index}"),
                assert_state(
                    Target::Css {
                        selector: format!("#focus-mismatch-{index}"),
                    },
                    ElementState::Focused,
                    true,
                ),
            )
            .await
            .expect_err("false exact focus must not pass");
        assert_eq!(mismatch.code(), "test.assert.focused");
    }

    for index in 0..DATASET_SIZE {
        let missing = session
            .execute_action(
                format!("missing-focus-{index}"),
                assert_state(
                    Target::Css {
                        selector: format!("#missing-focus-{index}"),
                    },
                    ElementState::Focused,
                    false,
                ),
            )
            .await
            .expect_err("a missing target cannot prove unfocused state");
        assert_eq!(missing.code(), "test.driver.web.target_not_found");
    }
    session.close_surface().await.expect("close");
    assert_eq!(executor.actions().len(), DATASET_SIZE * 6 + 1);
}

#[tokio::test]
async fn deterministic_state_dataset_classifies_400_of_400_cases() {
    let mut outputs = Vec::with_capacity(DATASET_SIZE * 4);
    for index in 0..DATASET_SIZE {
        outputs.push(probe(json!({
            "status": "ok",
            "actual": format!("value-{index}"),
            "count": 1
        })));
    }
    outputs.extend(
        (0..DATASET_SIZE).map(|_| probe(json!({ "status": "ok", "actual": true, "count": 1 }))),
    );
    for index in 0..DATASET_SIZE {
        outputs.push(probe(json!({
            "status": "ok",
            "actual": [format!("b-{index}"), format!("a-{index}")],
            "count": 1
        })));
    }
    outputs.extend((0..DATASET_SIZE).map(|_| probe(json!({ "status": "not_found", "count": 0 }))));
    let executor = Arc::new(QueueExecutor::new(outputs));
    let (_temp, mut session) = connected(executor.clone()).await;

    for index in 0..DATASET_SIZE {
        session
            .execute_action(
                format!("value-{index}"),
                Action::Assert {
                    expectation: Expectation::Value {
                        target: Target::Css {
                            selector: format!("#value-{index}"),
                        },
                        value: format!("value-{index}"),
                    },
                },
            )
            .await
            .unwrap_or_else(|error| panic!("matching value {index} failed: {error}"));
    }
    for index in 0..DATASET_SIZE {
        session
            .execute_action(
                format!("checked-{index}"),
                assert_state(
                    Target::Css {
                        selector: format!("#checked-{index}"),
                    },
                    ElementState::Checked,
                    true,
                ),
            )
            .await
            .unwrap_or_else(|error| panic!("matching checked state {index} failed: {error}"));
    }
    for index in 0..DATASET_SIZE {
        session
            .execute_action(
                format!("selection-{index}"),
                Action::Assert {
                    expectation: Expectation::SelectedValues {
                        target: Target::Css {
                            selector: format!("#selection-{index}"),
                        },
                        values: vec![format!("a-{index}"), format!("b-{index}")],
                    },
                },
            )
            .await
            .unwrap_or_else(|error| panic!("matching selection {index} failed: {error}"));
    }
    for index in 0..DATASET_SIZE {
        let error = session
            .execute_action(
                format!("missing-{index}"),
                assert_state(
                    Target::Css {
                        selector: format!("#missing-{index}"),
                    },
                    ElementState::Checked,
                    false,
                ),
            )
            .await
            .expect_err("missing target cannot prove false state");
        assert_eq!(error.code(), "test.driver.web.target_not_found");
    }
    session.close_surface().await.expect("close");

    assert_eq!(executor.actions().len(), DATASET_SIZE * 4 + 1);
}

fn assert_state(target: Target, state: ElementState, expected: bool) -> Action {
    Action::Assert {
        expectation: Expectation::State {
            target,
            state,
            expected,
        },
    }
}

fn semantic_state_dimensions() -> [(
    ElementState,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
); 5] {
    [
        (
            ElementState::Expanded,
            "expanded",
            "collapsed",
            "test.assert.expanded",
            "test.assert.collapsed",
        ),
        (
            ElementState::Pressed,
            "pressed",
            "unpressed",
            "test.assert.pressed",
            "test.assert.unpressed",
        ),
        (
            ElementState::ReadOnly,
            "readonly",
            "writable",
            "test.assert.readonly",
            "test.assert.writable",
        ),
        (
            ElementState::Required,
            "required",
            "optional",
            "test.assert.required",
            "test.assert.optional",
        ),
        (
            ElementState::Invalid,
            "invalid",
            "valid",
            "test.assert.invalid",
            "test.assert.valid",
        ),
    ]
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
            namespace: "state-assertions".to_string(),
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
            namespace: "state-assertions".to_string(),
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
