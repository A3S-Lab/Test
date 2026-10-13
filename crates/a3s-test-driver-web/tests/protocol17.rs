//! Secret references resolve only at execution.
//!
//! An unset, empty, or rejected sink sends no fill or type command. A value
//! that does reach the browser is absent from the action JSON, the step
//! output, and the driver error.

use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use a3s_test_core::{
    Action, DriverError, DriverSession, Expectation, PixelExpectation, PixelMask, ScenarioContext,
    SecretPurpose, StepOutput, SurfaceDriver, Target, TestStep, TestSuite,
};
use a3s_test_driver_web::{
    AgentBrowserConfig, AgentBrowserDriver, BrowserCommand, BrowserNetworkPolicy, CommandError,
    CommandExecutor, CommandInvocation, CommandOutput,
};
use async_trait::async_trait;
use serde_json::json;

const SECRET_NAME: &str = "A3S_TEST_SECRET_PROTOCOL17";
const SECRET_VALUE: &str = "s3cret-protocol17-value";

struct EnvGuard(&'static str);

impl Drop for EnvGuard {
    fn drop(&mut self) {
        unsafe { std::env::remove_var(self.0) }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum SinkMode {
    Password,
    Disabled,
    ReadOnly,
    ReadOnlyNamed,
    ReadOnlyFalse,
    PlainText,
    NotEditable,
    RejectFill,
}

struct SecretBrowser {
    invocations: Mutex<Vec<CommandInvocation>>,
    mode: SinkMode,
}

impl SecretBrowser {
    fn new(mode: SinkMode) -> Arc<Self> {
        Arc::new(Self {
            invocations: Mutex::new(Vec::new()),
            mode,
        })
    }
}

#[async_trait]
impl CommandExecutor for SecretBrowser {
    async fn run(&self, invocation: CommandInvocation) -> Result<CommandOutput, CommandError> {
        if invocation
            .args
            .last()
            .is_some_and(|argument| argument == "--version")
        {
            return Ok(CommandOutput {
                exit_code: 0,
                stdout: "agent-browser 0.26.0".to_string(),
                stderr: String::new(),
            });
        }
        let tail = command_tail(&invocation.args);
        let reject_fill = matches!(self.mode, SinkMode::RejectFill)
            && tail.first().map(String::as_str) == Some("fill");
        let stdout = probe_stdout(self.mode, &tail);
        self.invocations
            .lock()
            .expect("invocations")
            .push(invocation);
        if reject_fill {
            return Ok(CommandOutput {
                exit_code: 1,
                stdout: String::new(),
                stderr: format!("fill rejected {SECRET_VALUE}"),
            });
        }
        Ok(CommandOutput {
            exit_code: 0,
            stdout,
            stderr: String::new(),
        })
    }
}

fn probe_stdout(mode: SinkMode, tail: &[String]) -> String {
    match tail.first().map(String::as_str) {
        Some("is") => {
            let enabled = !matches!(mode, SinkMode::Disabled);
            format!(r#"{{"success":true,"data":{{"enabled":{enabled}}}}}"#)
        }
        Some("get") => {
            let value = match tail.get(3).map(String::as_str) {
                Some("readonly") => match mode {
                    SinkMode::ReadOnly => Some(""),
                    SinkMode::ReadOnlyNamed => Some("ReadOnly"),
                    SinkMode::ReadOnlyFalse => Some("false"),
                    _ => None,
                },
                Some("contenteditable") => None,
                Some("type") => Some(match mode {
                    SinkMode::PlainText => "text",
                    SinkMode::NotEditable => "button",
                    _ => "password",
                }),
                _ => None,
            };
            json!({"success": true, "data": {"value": value}}).to_string()
        }
        _ => r#"{"success":true}"#.to_string(),
    }
}

fn command_tail(args: &[OsString]) -> Vec<String> {
    let args = args
        .iter()
        .filter_map(|argument| argument.to_str())
        .collect::<Vec<_>>();
    let mut index = args
        .iter()
        .position(|argument| *argument == "--headed")
        .map(|position| position + 2)
        .unwrap_or(0);
    if args.get(index) == Some(&"--allowed-domains") {
        index += 2;
        if args.get(index) == Some(&"--engine") {
            index += 2;
        }
    }
    args[index..]
        .iter()
        .map(|argument| (*argument).to_string())
        .collect()
}

fn recorded(executor: &SecretBrowser) -> Vec<CommandInvocation> {
    executor
        .invocations
        .lock()
        .expect("invocations")
        .iter()
        .cloned()
        .collect()
}

fn tails(invocations: &[CommandInvocation]) -> Vec<Vec<String>> {
    invocations
        .iter()
        .map(|invocation| command_tail(&invocation.args))
        .filter(|tail| tail.first().map(String::as_str) != Some("close"))
        .collect()
}

fn env_contains_secret(invocations: &[CommandInvocation]) -> bool {
    invocations.iter().any(|invocation| {
        invocation
            .env
            .values()
            .any(|value| value.to_string_lossy().contains(SECRET_VALUE))
    })
}

fn args_contain_secret(invocations: &[CommandInvocation]) -> bool {
    invocations.iter().any(|invocation| {
        invocation
            .args
            .iter()
            .any(|argument| argument.to_string_lossy().contains(SECRET_VALUE))
    })
}

fn browser_config() -> AgentBrowserConfig {
    AgentBrowserConfig {
        command: BrowserCommand::Standalone {
            executable: PathBuf::from("/opt/agent-browser"),
        },
        namespace: "secrets".to_string(),
        headed: false,
        command_timeout: Duration::from_secs(5),
        idle_timeout: Duration::from_secs(30),
        microphone: Default::default(),
        network_policy: BrowserNetworkPolicy::default(),
    }
}

fn step(action: Action) -> TestStep {
    TestStep {
        id: "secret".to_string(),
        action,
        stability: None,
        assertion_mode: Default::default(),
        wait_mode: Default::default(),
    }
}

fn fill(purpose: SecretPurpose) -> TestStep {
    step(Action::FillSecret {
        target: Target::Css {
            selector: "#password".to_string(),
        },
        name: SECRET_NAME.to_string(),
        purpose,
    })
}

fn type_secret() -> TestStep {
    step(Action::TypeSecret {
        target: Target::Css {
            selector: "#password".to_string(),
        },
        name: SECRET_NAME.to_string(),
        purpose: SecretPurpose::Password,
    })
}

fn role_fill() -> TestStep {
    step(Action::FillSecret {
        target: Target::Role {
            role: "textbox".to_string(),
            name: "Password".to_string(),
        },
        name: SECRET_NAME.to_string(),
        purpose: SecretPurpose::Password,
    })
}

fn pixel_step() -> TestStep {
    step(Action::Assert {
        expectation: Expectation::Pixels(PixelExpectation {
            baseline: "baseline.png".to_string(),
            threshold_millis: 200,
            max_diff_pixels: None,
            max_diff_pixel_ratio_micros: None,
            masks: Vec::new(),
            update: false,
        }),
    })
}

async fn open_session(
    executor: Arc<dyn CommandExecutor>,
) -> (tempfile::TempDir, Box<dyn DriverSession>) {
    let temp = tempfile::tempdir().expect("tempdir");
    let driver = AgentBrowserDriver::with_executor(browser_config(), executor);
    let session = driver
        .open(&ScenarioContext {
            run_id: "secrets".to_string(),
            scenario_id: "fill".to_string(),
            artifacts_dir: temp.path().join("artifacts"),
        })
        .await
        .expect("session");
    (temp, session)
}

async fn execute(
    mode: SinkMode,
    step: TestStep,
) -> (Result<StepOutput, DriverError>, Vec<CommandInvocation>) {
    let executor = SecretBrowser::new(mode);
    let shared: Arc<dyn CommandExecutor> = executor.clone();
    let (temp, mut session) = open_session(shared).await;
    let result = session.execute(&step).await;
    let invocations = recorded(&executor);
    session.close().await.expect("close");
    drop(temp);
    (result, invocations)
}

fn assert_hidden(error: &DriverError) {
    assert!(!error.message().contains(SECRET_VALUE));
    let rendered = format!("{error:?}");
    assert!(!rendered.contains(SECRET_VALUE));
}

fn assert_no_input(invocations: &[CommandInvocation]) {
    assert!(!env_contains_secret(invocations));
    assert!(!args_contain_secret(invocations));
    assert!(tails(invocations)
        .iter()
        .all(|tail| { !matches!(tail.first().map(String::as_str), Some("fill" | "type")) }));
}

#[tokio::test]
async fn secret_reference_hides_the_value_and_rejects_a_bad_sink_first() {
    let _guard = EnvGuard(SECRET_NAME);
    unsafe { std::env::remove_var(SECRET_NAME) };

    let action_json = serde_json::to_string(&fill(SecretPurpose::Password).action).expect("json");
    assert!(action_json.contains(SECRET_NAME));
    assert!(!action_json.contains(SECRET_VALUE));
    assert!(!action_json.contains("\"value\""));

    let (error, invocations) = execute(SinkMode::Password, role_fill()).await;
    let error = error.expect_err("unset");
    assert_eq!(error.code(), "test.driver.web.secret_unavailable");
    assert!(error.message().contains(SECRET_NAME));
    assert_hidden(&error);
    assert!(tails(&invocations).is_empty());
    assert_no_input(&invocations);

    unsafe { std::env::set_var(SECRET_NAME, "") };
    let (error, invocations) = execute(SinkMode::Password, fill(SecretPurpose::Password)).await;
    let error = error.expect_err("empty");
    assert_eq!(error.code(), "test.driver.web.secret_unavailable");
    assert_hidden(&error);
    assert!(tails(&invocations).is_empty());
    assert_no_input(&invocations);

    unsafe { std::env::set_var(SECRET_NAME, SECRET_VALUE) };

    let (error, invocations) = execute(SinkMode::Password, role_fill()).await;
    let error = error.expect_err("role target");
    assert_eq!(error.code(), "test.driver.web.target_unsupported");
    assert_hidden(&error);
    assert!(tails(&invocations).is_empty());
    assert_no_input(&invocations);

    let probes: [&[&str]; 4] = [
        &["is", "enabled", "#password"],
        &["get", "attr", "#password", "readonly"],
        &["get", "attr", "#password", "contenteditable"],
        &["get", "attr", "#password", "type"],
    ];
    let rejected = [
        (SinkMode::Disabled, "test.driver.web.secret_sink_disabled"),
        (SinkMode::ReadOnly, "test.driver.web.secret_sink_readonly"),
        (
            SinkMode::ReadOnlyNamed,
            "test.driver.web.secret_sink_readonly",
        ),
        (
            SinkMode::ReadOnlyFalse,
            "test.driver.web.secret_sink_readonly",
        ),
        (
            SinkMode::NotEditable,
            "test.driver.web.secret_sink_not_editable",
        ),
        (
            SinkMode::PlainText,
            "test.driver.web.secret_purpose_mismatch",
        ),
    ];
    for (mode, code) in rejected {
        let (error, invocations) = execute(mode, fill(SecretPurpose::Password)).await;
        let error = error.expect_err(code);
        assert_eq!(error.code(), code, "{code}");
        assert_hidden(&error);
        assert_no_input(&invocations);
        let got = tails(&invocations);
        assert_eq!(got.len(), probes.len(), "{code} probes: {got:?}");
        for (actual, expected) in got.iter().zip(probes) {
            assert_eq!(
                actual,
                &expected
                    .iter()
                    .copied()
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            );
        }
    }

    let (output, invocations) = execute(SinkMode::Password, fill(SecretPurpose::Text)).await;
    let output = output.expect("text purpose on a password field");
    assert_eq!(output.data, json!({ "env": SECRET_NAME, "filled": true }));
    assert!(!serde_json::to_string(&output)
        .expect("output")
        .contains(SECRET_VALUE));
    assert!(!env_contains_secret(&invocations));
    let got = tails(&invocations);
    assert_eq!(
        got.last().expect("fill"),
        &vec![
            "fill".to_string(),
            "#password".to_string(),
            SECRET_VALUE.to_string()
        ]
    );

    let (output, invocations) = execute(SinkMode::Password, type_secret()).await;
    let output = output.expect("type secret");
    assert_eq!(output.data, json!({ "env": SECRET_NAME, "filled": true }));
    assert!(!serde_json::to_string(&output)
        .expect("output")
        .contains(SECRET_VALUE));
    assert_eq!(
        tails(&invocations).last().expect("type"),
        &vec![
            "type".to_string(),
            "#password".to_string(),
            SECRET_VALUE.to_string()
        ]
    );

    let (error, invocations) = execute(SinkMode::RejectFill, fill(SecretPurpose::Password)).await;
    let error = error.expect_err("fill command");
    assert_eq!(error.code(), "test.driver.web.command_failed");
    assert!(error.message().contains("[redacted]"));
    assert_hidden(&error);
    assert!(!env_contains_secret(&invocations));
    assert_eq!(
        tails(&invocations).last().expect("rejected fill"),
        &vec![
            "fill".to_string(),
            "#password".to_string(),
            SECRET_VALUE.to_string()
        ]
    );

    for mode in [SinkMode::Password, SinkMode::RejectFill] {
        let executor = SecretBrowser::new(mode);
        let shared: Arc<dyn CommandExecutor> = executor.clone();
        let (temp, mut session) = open_session(shared).await;
        let filled = session.execute(&fill(SecretPurpose::Password)).await;
        if mode == SinkMode::Password {
            let output = filled.expect("fill before pixels");
            assert!(!serde_json::to_string(&output)
                .expect("output")
                .contains(SECRET_VALUE));
        } else {
            let error = filled.expect_err("rejected fill before pixels");
            assert!(error.message().contains("[redacted]"));
            assert_hidden(&error);
        }
        let pixel = session.execute(&pixel_step()).await.expect_err("pixels");
        assert_eq!(pixel.code(), "test.driver.web.pixel_after_secret");
        assert_hidden(&pixel);
        let got = tails(&recorded(&executor));
        assert!(
            got.iter()
                .all(|tail| tail.first().map(String::as_str) != Some("screenshot")),
            "pixel refusal sent a screenshot: {got:?}"
        );
        session.close().await.expect("close");
        drop(temp);
    }
}

struct PixelBrowser {
    invocations: Mutex<Vec<CommandInvocation>>,
    frames: Mutex<Vec<Vec<u8>>>,
}

impl PixelBrowser {
    fn new(frames: Vec<Vec<u8>>) -> Arc<Self> {
        Arc::new(Self {
            invocations: Mutex::new(Vec::new()),
            frames: Mutex::new(frames),
        })
    }
}

#[async_trait]
impl CommandExecutor for PixelBrowser {
    async fn run(&self, invocation: CommandInvocation) -> Result<CommandOutput, CommandError> {
        if invocation
            .args
            .last()
            .is_some_and(|argument| argument == "--version")
        {
            return Ok(CommandOutput {
                exit_code: 0,
                stdout: "agent-browser 0.26.0".to_string(),
                stderr: String::new(),
            });
        }
        let tail = command_tail(&invocation.args);
        if tail.first().map(String::as_str) == Some("screenshot") {
            let path = tail.get(1).expect("screenshot path");
            let frame = {
                let mut frames = self.frames.lock().expect("frames");
                if frames.len() > 1 {
                    frames.remove(0)
                } else {
                    frames.first().cloned().unwrap_or_default()
                }
            };
            std::fs::write(path, frame).expect("screenshot bytes");
        }
        self.invocations
            .lock()
            .expect("invocations")
            .push(invocation);
        Ok(CommandOutput {
            exit_code: 0,
            stdout: r#"{"success":true}"#.to_string(),
            stderr: String::new(),
        })
    }
}

fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    let mut encoder = png::Encoder::new(&mut output, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("png header");
    writer.write_image_data(rgba).expect("png data");
    writer.finish().expect("png finish");
    output
}

fn solid_png(width: u32, height: u32, pixel: [u8; 4]) -> Vec<u8> {
    let mut rgba = Vec::new();
    for _ in 0..(width * height) {
        rgba.extend_from_slice(&pixel);
    }
    encode_png(width, height, &rgba)
}

fn pixel_expectation(update: bool, masks: Vec<PixelMask>) -> TestStep {
    step(Action::Assert {
        expectation: Expectation::Pixels(PixelExpectation {
            baseline: "baseline.png".to_string(),
            threshold_millis: 0,
            max_diff_pixels: Some(10),
            max_diff_pixel_ratio_micros: Some(250_000),
            masks,
            update,
        }),
    })
}

fn screenshot_paths(invocations: &[CommandInvocation]) -> Vec<String> {
    tails(invocations)
        .into_iter()
        .filter(|tail| tail.first().map(String::as_str) == Some("screenshot"))
        .map(|tail| tail.get(1).cloned().unwrap_or_default())
        .collect()
}

#[tokio::test]
async fn pixel_expectation_compares_bounded_png_and_refuses_a_missing_baseline() {
    let red = solid_png(2, 2, [255, 0, 0, 255]);
    let wide = solid_png(2, 1, [255, 0, 0, 255]);
    let changed = encode_png(
        2,
        2,
        &[
            0, 0, 255, 255, 0, 0, 255, 255, 255, 0, 0, 255, 255, 0, 0, 255,
        ],
    );

    let missing = PixelBrowser::new(Vec::new());
    let (temp, mut session) = open_session(missing.clone() as Arc<dyn CommandExecutor>).await;
    session.set_pixel_ci(true);
    let error = session
        .execute(&pixel_expectation(false, Vec::new()))
        .await
        .expect_err("missing");
    assert_eq!(error.code(), "test.driver.web.pixel_baseline_missing");
    assert!(screenshot_paths(&recorded_pixel(&missing)).is_empty());
    assert!(!temp.path().join("artifacts").join("baseline.png").exists());
    session.close().await.expect("close");
    drop(temp);

    let updated = PixelBrowser::new(vec![red.clone(), red.clone()]);
    let (temp, mut session) = open_session(updated.clone() as Arc<dyn CommandExecutor>).await;
    session.set_pixel_ci(true);
    let output = session
        .execute(&pixel_expectation(true, Vec::new()))
        .await
        .expect("update");
    assert_eq!(output.data["updated"], serde_json::json!(true));
    let shots = screenshot_paths(&recorded_pixel(&updated));
    assert_eq!(shots.len(), 2);
    assert!(shots.iter().all(|path| !path.ends_with("baseline.png")));
    let baseline =
        std::fs::read(temp.path().join("artifacts").join("baseline.png")).expect("baseline");
    assert_eq!(baseline, red);
    session.close().await.expect("close");
    drop(temp);

    let sized = PixelBrowser::new(vec![solid_png(1, 1, [255, 0, 0, 255]); 2]);
    let (temp, mut session) = open_session(sized.clone() as Arc<dyn CommandExecutor>).await;
    std::fs::write(temp.path().join("artifacts").join("baseline.png"), &wide).expect("baseline");
    let error = session
        .execute(&pixel_expectation(false, Vec::new()))
        .await
        .expect_err("size");
    assert_eq!(error.code(), "test.assert.pixel_size");
    assert!(error.message().contains("1x1"));
    assert!(error.message().contains("2x1"));
    assert!(!error.message().contains("PNG"));
    assert!(!temp
        .path()
        .join("artifacts")
        .join("pixel-mismatch-0-actual.png")
        .exists());
    session.close().await.expect("close");
    drop(temp);

    let unstable = PixelBrowser::new(vec![red.clone(), wide.clone()]);
    let (temp, mut session) = open_session(unstable.clone() as Arc<dyn CommandExecutor>).await;
    std::fs::write(temp.path().join("artifacts").join("baseline.png"), &red).expect("baseline");
    let error = session
        .execute(&pixel_expectation(false, Vec::new()))
        .await
        .expect_err("unstable");
    assert_eq!(error.code(), "test.driver.web.pixel_unstable");
    assert!(!temp
        .path()
        .join("artifacts")
        .join("pixel-mismatch-0-diff.png")
        .exists());
    session.close().await.expect("close");
    drop(temp);

    let capped = PixelBrowser::new(vec![changed.clone(), changed.clone()]);
    let (temp, mut session) = open_session(capped.clone() as Arc<dyn CommandExecutor>).await;
    std::fs::write(temp.path().join("artifacts").join("baseline.png"), &red).expect("baseline");
    let error = session
        .execute(&pixel_expectation(false, Vec::new()))
        .await
        .expect_err("smaller cap");
    assert_eq!(error.code(), "test.assert.pixel_diff");
    assert!(error.message().contains("exceeds allowed 1"));
    let artifacts = temp.path().join("artifacts");
    assert!(artifacts.join("pixel-mismatch-0-actual.png").is_file());
    assert!(artifacts.join("pixel-mismatch-0-expected.png").is_file());
    assert!(artifacts.join("pixel-mismatch-0-diff.png").is_file());
    session.close().await.expect("close");
    drop(temp);

    let masked = PixelBrowser::new(vec![changed.clone(), changed]);
    let (temp, mut session) = open_session(masked as Arc<dyn CommandExecutor>).await;
    std::fs::write(temp.path().join("artifacts").join("baseline.png"), &red).expect("baseline");
    let output = session
        .execute(&pixel_expectation(
            false,
            vec![PixelMask {
                x: 0,
                y: 0,
                width: 2,
                height: 1,
            }],
        ))
        .await
        .expect("mask");
    assert_eq!(output.data["different_pixels"], serde_json::json!(0));
    session.close().await.expect("close");
    drop(temp);
}

#[tokio::test]
async fn missing_browser_verbs_fail_before_any_browser_command() {
    let executor = SecretBrowser::new(SinkMode::Password);
    let mut opened = 0_u32;
    let unsupported = [
        r#"closed_shadow "root" {}"#,
        r#"pierce "node" {}"#,
        r#"network_fulfill "api" { status = 200 }"#,
        r#"network_fulfill "stub" { pattern = "*/api/profile" body = "{\"name\":\"Grace\"}" }"#,
        r#"wait_for_response "api" {}"#,
        r#"evaluate "script" {}"#,
        r#"init_script "boot" {}"#,
        r#"click "box" { target = css(".go") position = "center" }"#,
        r#"click "shifted" { target = css(".go") modifiers = ["shift"] }"#,
        r#"double_click "shifted" { target = css(".go") modifiers = ["shift"] }"#,
        r#"click "coord" { target = css(".go") x = 1 }"#,
        r#"click "down" { target = css(".go") y = 2 }"#,
        r#"click "slow" { target = css(".go") delay = 10 }"#,
        r#"click "right" { target = css(".go") button = "right" }"#,
        r#"click "hold" { target = css(".go") long_press = true }"#,
        r#"network_route "status" { status = 201 }"#,
        r#"network_route "type" { content_type = "text/plain" }"#,
        r#"network_route "headers" { headers = "[]" }"#,
        r#"network_route "json" { json = "{}" }"#,
        r#"network_route "file" { file = "body.txt" }"#,
        r#"network_route "continue" { continue = true }"#,
        r#"network_route "method" { method = "POST" }"#,
        r#"network_route "post" { post_data = "{}" }"#,
        r#"network_route "fulfill" { fulfill = true }"#,
        r#"wait "reply" { response = "/api" }"#,
        r#"wait "code" { status = 200 }"#,
        r#"click "pierce" { target = css(".go") pierce = true }"#,
        r#"snapshot "page" { shadow = true }"#,
        r#"navigate "open" { url = "https://example.test" closed_shadow = true }"#,
    ];
    for step in unsupported {
        match TestSuite::from_acl(&verb_suite(step)) {
            Ok(suite) => {
                opened += 1;
                let shared: Arc<dyn CommandExecutor> = executor.clone();
                let (temp, mut session) = open_session(shared).await;
                let _ = session.execute(&suite.scenarios[0].steps[0]).await;
                session.close().await.expect("close");
                drop(temp);
            }
            Err(error) => {
                assert_eq!(
                    error.code(),
                    "test.spec.browser_verb_unsupported",
                    "{step}: {}",
                    error.message()
                );
            }
        }
    }
    let step = r#"forward "next" {}"#;
    let error = TestSuite::from_acl(&verb_suite(step)).expect_err(step);
    assert_eq!(error.code(), "test.spec.action_unknown", "{step}");
    assert_eq!(opened, 0);
    assert!(
        recorded(&executor).is_empty(),
        "an unsupported verb invoked the browser"
    );
}

fn verb_suite(step: &str) -> String {
    format!(
        r#"
suite "verbs" {{
    version = 1
    scenario "home" {{
        name = "Home"
        surface = "web"
        timeout_ms = 1000
        {step}
    }}
}}
"#
    )
}

fn recorded_pixel(executor: &PixelBrowser) -> Vec<CommandInvocation> {
    executor
        .invocations
        .lock()
        .expect("invocations")
        .iter()
        .cloned()
        .collect()
}
