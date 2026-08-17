use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use a3s_test_core::{Action, ModifierKey, ScenarioContext, SurfaceDriver, Target, TestStep};
use a3s_test_driver_web::{
    AgentBrowserConfig, AgentBrowserDriver, BrowserCommand, CommandError, CommandExecutor,
    CommandInvocation, CommandOutput,
};
use async_trait::async_trait;

#[derive(Default)]
struct RecordingExecutor {
    invocations: Mutex<Vec<CommandInvocation>>,
}

#[derive(Default)]
struct FailingWheelExecutor {
    invocations: Mutex<Vec<CommandInvocation>>,
}

#[async_trait]
impl CommandExecutor for RecordingExecutor {
    async fn run(&self, invocation: CommandInvocation) -> Result<CommandOutput, CommandError> {
        let version_probe = invocation
            .args
            .last()
            .is_some_and(|argument| argument == "--version");
        let box_query = invocation
            .args
            .windows(2)
            .any(|arguments| arguments == os(&["get", "box"]));
        self.invocations.lock().unwrap().push(invocation);
        Ok(CommandOutput {
            exit_code: 0,
            stdout: if version_probe {
                "agent-browser 0.26.0".to_string()
            } else if box_query {
                r#"{"success":true,"data":{"x":10,"y":20,"width":100,"height":50}}"#.to_string()
            } else {
                r#"{"success":true}"#.to_string()
            },
            stderr: String::new(),
        })
    }
}

#[async_trait]
impl CommandExecutor for FailingWheelExecutor {
    async fn run(&self, invocation: CommandInvocation) -> Result<CommandOutput, CommandError> {
        let version_probe = invocation
            .args
            .last()
            .is_some_and(|argument| argument == "--version");
        let wheel = invocation
            .args
            .windows(2)
            .any(|arguments| arguments == os(&["mouse", "wheel"]));
        self.invocations.lock().unwrap().push(invocation);
        if wheel {
            return Err(CommandError::timed_out("wheel command timed out"));
        }
        Ok(CommandOutput {
            exit_code: 0,
            stdout: if version_probe {
                "agent-browser 0.26.0".to_string()
            } else {
                r#"{"success":true}"#.to_string()
            },
            stderr: String::new(),
        })
    }
}

#[tokio::test]
async fn maps_advanced_interactions_to_the_verified_browser_protocol() {
    let temp = tempfile::tempdir().expect("tempdir");
    let executor = Arc::new(RecordingExecutor::default());
    let driver = AgentBrowserDriver::with_executor(
        AgentBrowserConfig {
            command: BrowserCommand::Standalone {
                executable: PathBuf::from("/opt/agent-browser"),
            },
            namespace: "advanced".to_string(),
            headed: false,
            command_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(2),
            microphone: Default::default(),
            network_policy: Default::default(),
        },
        executor.clone(),
    );
    let mut session = driver
        .open(&ScenarioContext {
            run_id: "run".to_string(),
            scenario_id: "interactions".to_string(),
            artifacts_dir: temp.path().join("artifacts"),
        })
        .await
        .expect("session");

    let actions = [
        Action::Hover {
            target: Target::Role {
                role: "button".to_string(),
                name: "Toolbar".to_string(),
            },
        },
        Action::Focus {
            target: Target::Ref {
                value: "@e1".to_string(),
            },
        },
        Action::DoubleClick {
            target: Target::Ref {
                value: "@e2".to_string(),
            },
        },
        Action::ContextClick {
            target: Target::Css {
                selector: ".selection".to_string(),
            },
        },
        Action::Type {
            target: Target::Css {
                selector: "#title".to_string(),
            },
            value: "more text".to_string(),
        },
        Action::InsertText {
            value: " at caret".to_string(),
        },
        Action::Check {
            target: Target::TestId {
                value: "comments".to_string(),
            },
        },
        Action::Uncheck {
            target: Target::Css {
                selector: "#readonly".to_string(),
            },
        },
        Action::Select {
            target: Target::Ref {
                value: "@e8".to_string(),
            },
            values: vec!["draft".to_string(), "review".to_string()],
        },
        Action::Drag {
            source: Target::Ref {
                value: "@e10".to_string(),
            },
            target: Target::Css {
                selector: "#resize-target".to_string(),
            },
        },
        Action::Wheel {
            target: None,
            delta_x: 4,
            delta_y: -120,
            modifiers: vec![ModifierKey::Control, ModifierKey::Shift],
        },
        Action::Viewport {
            width: 1440,
            height: 900,
            scale: Some(2),
        },
    ];

    for (index, action) in actions.into_iter().enumerate() {
        session
            .execute(&step(&format!("advanced-{index}"), action))
            .await
            .expect("advanced action");
    }
    session.close().await.expect("close");

    let invocations = executor.invocations.lock().unwrap();
    let mut action_args = invocations
        .iter()
        .skip(1)
        .map(|invocation| strip_session_prefix(&invocation.args))
        .collect::<Vec<_>>();
    assert_eq!(action_args[6][0], "eval");
    let context_menu_script = action_args[6][1].to_string_lossy();
    assert!(context_menu_script.contains("new MouseEvent('contextmenu'"));
    assert!(context_menu_script.contains("document.elementFromPoint(60, 45)"));
    assert!(context_menu_script.contains("button: 2"));
    assert!(context_menu_script.contains("buttons: 2"));
    action_args[6] = os(&["eval", "<context-menu-script>"]);
    let shadow_check = action_args
        .iter()
        .position(|arguments| {
            arguments.first().is_some_and(|argument| argument == "eval")
                && arguments.get(1).is_some_and(|argument| {
                    argument
                        .to_string_lossy()
                        .contains(r#"const target = {"type":"test_id","value":"comments"}"#)
                })
        })
        .expect("semantic Shadow DOM check probe");
    action_args[shadow_check] = os(&["eval", "<shadow-target-check>"]);
    assert_eq!(
        action_args,
        vec![
            os(&["find", "role", "button", "hover", "--name", "Toolbar"]),
            os(&["focus", "@e1"]),
            os(&["dblclick", "@e2"]),
            os(&["scrollintoview", ".selection"]),
            os(&["get", "box", ".selection"]),
            os(&["mouse", "move", "60", "45"]),
            os(&["eval", "<context-menu-script>"]),
            os(&["type", "#title", "more text"]),
            os(&["keyboard", "inserttext", " at caret"]),
            os(&["eval", "<shadow-target-check>"]),
            os(&["find", "testid", "comments", "check"]),
            os(&["uncheck", "#readonly"]),
            os(&["select", "@e8", "draft", "review"]),
            os(&["scrollintoview", "@e10"]),
            os(&["scrollintoview", "#resize-target"]),
            os(&["drag", "@e10", "#resize-target"]),
            os(&["keydown", "Control"]),
            os(&["keydown", "Shift"]),
            os(&["mouse", "wheel", "-120", "4"]),
            os(&["keyup", "Shift"]),
            os(&["keyup", "Control"]),
            os(&["set", "viewport", "1440", "900", "2"]),
            os(&["close"]),
        ]
    );
}

#[tokio::test]
async fn releases_wheel_modifiers_in_reverse_order_after_a_driver_failure() {
    let temp = tempfile::tempdir().expect("tempdir");
    let executor = Arc::new(FailingWheelExecutor::default());
    let driver = AgentBrowserDriver::with_executor(
        AgentBrowserConfig {
            command: BrowserCommand::Standalone {
                executable: PathBuf::from("/opt/agent-browser"),
            },
            namespace: "wheel-cleanup".to_string(),
            headed: false,
            command_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(2),
            microphone: Default::default(),
            network_policy: Default::default(),
        },
        executor.clone(),
    );
    let mut session = driver
        .open(&ScenarioContext {
            run_id: "run".to_string(),
            scenario_id: "wheel".to_string(),
            artifacts_dir: temp.path().join("artifacts"),
        })
        .await
        .expect("session");

    let error = session
        .execute(&step(
            "zoom",
            Action::Wheel {
                target: None,
                delta_x: 0,
                delta_y: -120,
                modifiers: vec![ModifierKey::Control, ModifierKey::Shift],
            },
        ))
        .await
        .expect_err("wheel failure");
    assert_eq!(error.code(), "test.driver.web.command_unavailable");
    session.close().await.expect("close");

    let invocations = executor.invocations.lock().unwrap();
    let action_args = invocations
        .iter()
        .skip(1)
        .map(|invocation| strip_session_prefix(&invocation.args))
        .collect::<Vec<_>>();
    assert_eq!(
        action_args,
        vec![
            os(&["keydown", "Control"]),
            os(&["keydown", "Shift"]),
            os(&["mouse", "wheel", "-120", "0"]),
            os(&["keyup", "Shift"]),
            os(&["keyup", "Control"]),
            os(&["close"]),
        ]
    );
}

#[tokio::test]
async fn maps_targeted_wheel_to_a_scoped_event_with_explicit_modifier_state() {
    let temp = tempfile::tempdir().expect("tempdir");
    let executor = Arc::new(RecordingExecutor::default());
    let driver = AgentBrowserDriver::with_executor(
        AgentBrowserConfig {
            command: BrowserCommand::Standalone {
                executable: PathBuf::from("/opt/agent-browser"),
            },
            namespace: "targeted-wheel".to_string(),
            headed: false,
            command_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(2),
            microphone: Default::default(),
            network_policy: Default::default(),
        },
        executor.clone(),
    );
    let mut session = driver
        .open(&ScenarioContext {
            run_id: "run".to_string(),
            scenario_id: "wheel".to_string(),
            artifacts_dir: temp.path().join("artifacts"),
        })
        .await
        .expect("session");

    session
        .execute(&step(
            "zoom",
            Action::Wheel {
                target: Some(Target::Css {
                    selector: ".document-page".to_string(),
                }),
                delta_x: 3,
                delta_y: -120,
                modifiers: vec![ModifierKey::Meta],
            },
        ))
        .await
        .expect("targeted wheel");
    session.close().await.expect("close");

    let invocations = executor.invocations.lock().unwrap();
    let action_args = invocations
        .iter()
        .skip(1)
        .map(|invocation| strip_session_prefix(&invocation.args))
        .collect::<Vec<_>>();
    assert_eq!(action_args[0], os(&["scrollintoview", ".document-page"]));
    assert_eq!(action_args[1], os(&["get", "box", ".document-page"]));
    assert_eq!(action_args[2], os(&["keydown", "Meta"]));
    assert_eq!(action_args[3][0], "eval");
    let script = action_args[3][1].to_string_lossy();
    assert!(script.contains("document.elementFromPoint(60, 45)"));
    assert!(script.contains("deltaX: 3"));
    assert!(script.contains("deltaY: -120"));
    assert!(script.contains("metaKey: true"));
    assert_eq!(action_args[4], os(&["keyup", "Meta"]));
    assert_eq!(action_args[5], os(&["close"]));
}

fn step(id: &str, action: Action) -> TestStep {
    TestStep {
        id: id.to_string(),
        action,
        stability: None,
        assertion_mode: Default::default(),
        wait_mode: Default::default(),
    }
}

fn os(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

fn strip_session_prefix(args: &[OsString]) -> Vec<OsString> {
    args.iter().skip(5).cloned().collect()
}
