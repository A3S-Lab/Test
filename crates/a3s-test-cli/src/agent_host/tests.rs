use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use a3s_test_agent::{HttpLlmCompletionRequest, LLM_PROVIDER_PROTOCOL};
use a3s_test_driver_web::{CommandError, CommandInvocation, CommandOutput};
use async_trait::async_trait;
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

use super::*;

#[test]
fn agent_status_exit_codes_keep_automation_semantics() {
    assert_eq!(
        agent_status_exit_code(a3s_test_agent::AgentStatus::Succeeded),
        std::process::ExitCode::SUCCESS
    );
    assert_eq!(
        agent_status_exit_code(a3s_test_agent::AgentStatus::TimedOut),
        std::process::ExitCode::from(124)
    );
    assert_eq!(
        agent_status_exit_code(a3s_test_agent::AgentStatus::Cancelled),
        std::process::ExitCode::from(130)
    );
}

#[test]
fn verification_cancellation_and_timeout_keep_terminal_statuses() {
    let config = parse_config(
        r#"
agent_run "status" {
  url = "http://127.0.0.1/"
  goal = "Verify status mapping"
  success_criteria = ["The page is ready"]
  allow_actions = ["click"]
  max_turns = 1
  max_total_tokens = 100
  max_cost_microusd = 100
  timeout_ms = 1000

  provider {
    name = "fixture"
    model = "planner"
    endpoint = "http://127.0.0.1:3000/v1/plan"
  }

  verification {
    expect "ready" { text = "Ready" }
  }
}
"#,
    )
    .expect("agent run config");

    for (code, expected_status, expected_exit_code) in [
        (
            "test.agent.cancelled",
            a3s_test_agent::AgentStatus::Cancelled,
            ExitCode::from(130),
        ),
        (
            "test.agent.verification_timeout",
            a3s_test_agent::AgentStatus::TimedOut,
            ExitCode::from(124),
        ),
    ] {
        let mut result = failed_agent_result(
            &config,
            DriverError::new("test.driver.fixture", "placeholder result"),
        );
        result.status = a3s_test_agent::AgentStatus::Succeeded;
        result.summary = Some("model finish".to_string());
        result.error = None;
        let verification = vec![VerificationStepResult {
            id: "ready".to_string(),
            output: None,
            error: Some(HostError {
                code: code.to_string(),
                message: "verification interrupted".to_string(),
                retryable: false,
            }),
        }];

        apply_verification_outcome(&mut result, &verification);

        assert_eq!(result.status, expected_status);
        assert_eq!(
            result.error.as_ref().map(|error| error.code.as_str()),
            Some(code)
        );
        assert_eq!(agent_status_exit_code(result.status), expected_exit_code);
    }
}

#[test]
fn authorization_redaction_registers_the_header_and_bounded_credential() {
    assert_eq!(
        authorization_secrets("Bearer deployment-secret"),
        ["Bearer deployment-secret", "deployment-secret"]
    );
    assert_eq!(authorization_secrets("short x"), ["short x"]);
}

#[tokio::test]
async fn embedded_host_runs_one_model_turn_verifies_locally_and_closes() {
    let temp = tempfile::tempdir().expect("tempdir");
    let provider = ProviderFixture::start(vec![
        json!({
            "status": "success",
            "protocol": LLM_PROVIDER_PROTOCOL,
            "response": {
                "decision": {
                    "type": "act",
                    "action": {
                        "type": "click",
                        "target": { "type": "ref", "value": "@e1" }
                    }
                },
                "usage": { "input_tokens": 10, "output_tokens": 5, "cost_microusd": 9 },
                "request_id": "turn-1"
            }
        }),
        json!({
            "status": "success",
            "protocol": LLM_PROVIDER_PROTOCOL,
            "response": {
                "decision": { "type": "finish", "summary": "Confirmation is visible" },
                "usage": { "input_tokens": 12, "output_tokens": 4, "cost_microusd": 8 },
                "request_id": "turn-2"
            }
        }),
    ])
    .await;
    let config = write_config(temp.path(), &provider.endpoint, "embedded-pass.acl");
    let report_path = temp.path().join("reports/agent.json");
    let executor = Arc::new(HostWebExecutor::new("Order confirmed"));

    let code = execute_with_executor(
        args(&config, &report_path),
        Some(executor.clone()),
        Some(temp.path().to_path_buf()),
    )
    .await
    .expect("agent host");

    assert_eq!(code, ExitCode::SUCCESS);
    let report: serde_json::Value =
        serde_json::from_slice(&tokio::fs::read(&report_path).await.expect("report"))
            .expect("report JSON");
    assert_eq!(report["protocol"], "a3s.test.agent-run/1");
    assert_eq!(report["result"]["status"], "succeeded");
    assert_eq!(report["verification"][0]["id"], "confirmation");
    assert!(report["verification"][0]["error"].is_null());
    assert!(report["cleanup_error"].is_null());
    assert_eq!(
        executor.actions(),
        [
            "open",
            "snapshot",
            "scrollintoview",
            "click",
            "snapshot",
            "wait",
            "close"
        ]
    );

    let requests = provider.finish().await;
    assert_eq!(requests.len(), 2);
    let first: HttpLlmCompletionRequest =
        serde_json::from_slice(&requests[0]).expect("first provider request");
    assert_eq!(first.protocol, LLM_PROVIDER_PROTOCOL);
    assert!(first.request.response_schema.is_object());
    assert_eq!(
        first.request.context.observation.summary,
        "browser accessibility snapshot"
    );
    assert_eq!(
        first.request.context.observation.data["data"]["origin"],
        "http://127.0.0.1/"
    );
}

#[tokio::test]
async fn deterministic_verification_overrides_a_model_finish() {
    let temp = tempfile::tempdir().expect("tempdir");
    let provider = ProviderFixture::start(vec![json!({
        "status": "success",
        "protocol": LLM_PROVIDER_PROTOCOL,
        "response": {
            "decision": { "type": "finish", "summary": "I think it passed" },
            "usage": { "input_tokens": 10, "output_tokens": 4, "cost_microusd": 8 },
            "request_id": "finish-early"
        }
    })])
    .await;
    let config = write_config(temp.path(), &provider.endpoint, "verification-fails.acl");
    let report_path = temp.path().join("reports/failure.json");
    let executor = Arc::new(HostWebExecutor::new("Still on checkout"));

    let code = execute_with_executor(
        args(&config, &report_path),
        Some(executor.clone()),
        Some(temp.path().to_path_buf()),
    )
    .await
    .expect("agent host");

    assert_eq!(code, ExitCode::from(1));
    let report: serde_json::Value =
        serde_json::from_slice(&tokio::fs::read(&report_path).await.expect("report"))
            .expect("report JSON");
    assert_eq!(report["result"]["status"], "failed");
    assert_eq!(
        report["result"]["error"]["code"],
        "test.agent.verification_failed"
    );
    assert_eq!(
        report["verification"][0]["error"]["code"],
        "test.driver.web.command_failed"
    );
    assert_eq!(executor.actions(), ["open", "snapshot", "wait", "close"]);
    assert_eq!(provider.finish().await.len(), 1);
}

#[tokio::test]
async fn surface_open_failure_is_persisted_as_the_complete_report() {
    let temp = tempfile::tempdir().expect("tempdir");
    let provider = ProviderFixture::start(Vec::new()).await;
    let config = write_config(temp.path(), &provider.endpoint, "open-fails.acl");
    let report_path = temp.path().join("reports/open-failure.json");
    let executor = Arc::new(OpenFailureExecutor);

    let code = execute_with_executor(
        args(&config, &report_path),
        Some(executor),
        Some(temp.path().to_path_buf()),
    )
    .await
    .expect("agent host");

    assert_eq!(code, ExitCode::from(1));
    let report: serde_json::Value =
        serde_json::from_slice(&tokio::fs::read(&report_path).await.expect("report"))
            .expect("report JSON");
    assert_eq!(report["protocol"], "a3s.test.agent-run/1");
    assert_eq!(report["result"]["status"], "failed");
    assert_eq!(
        report["result"]["error"]["code"],
        "test.driver.web.capability_unavailable"
    );
    assert!(report["verification"].as_array().is_some_and(Vec::is_empty));
    assert!(report["cleanup_error"].is_null());
    assert!(provider.finish().await.is_empty());
}

fn args(config: &Path, report: &Path) -> AgentRunArgs {
    AgentRunArgs {
        config: config.to_path_buf(),
        browser_driver: BrowserDriverKind::Standalone,
        browser_executable: Some(PathBuf::from("fixture-agent-browser")),
        headed: false,
        command_timeout_ms: 5_000,
        idle_timeout_ms: 30_000,
        cleanup_timeout_ms: 1_000,
        report: Some(report.to_path_buf()),
        force: false,
        json: false,
    }
}

fn write_config(root: &Path, endpoint: &str, name: &str) -> PathBuf {
    let path = root.join(name);
    std::fs::write(
        &path,
        format!(
            r#"
agent_run "checkout" {{
  url = "http://127.0.0.1/"
  goal = "Submit checkout"
  success_criteria = ["Order confirmed is visible"]
  allow_actions = ["click"]
  max_turns = 4
  max_total_tokens = 1000
  max_cost_microusd = 1000
  timeout_ms = 10000

  provider {{
    name = "fixture"
    model = "planner"
    endpoint = "{endpoint}"
  }}

  verification {{
    expect "confirmation" {{ text = "Order confirmed" }}
  }}
}}
"#
        ),
    )
    .expect("config");
    path
}

struct HostWebExecutor {
    confirmation: String,
    invocations: Mutex<Vec<CommandInvocation>>,
}

impl HostWebExecutor {
    fn new(confirmation: &str) -> Self {
        Self {
            confirmation: confirmation.to_string(),
            invocations: Mutex::new(Vec::new()),
        }
    }

    fn actions(&self) -> Vec<String> {
        self.invocations
            .lock()
            .expect("invocations")
            .iter()
            .filter_map(|invocation| browser_action(&invocation.args).first())
            .map(|value| value.to_string_lossy().into_owned())
            .filter(|value| value != "eval" && value != "--version")
            .collect()
    }
}

#[async_trait]
impl CommandExecutor for HostWebExecutor {
    async fn run(&self, invocation: CommandInvocation) -> Result<CommandOutput, CommandError> {
        let version = invocation
            .args
            .last()
            .is_some_and(|argument| argument == "--version");
        let action = browser_action(&invocation.args);
        let stdout = if version {
            "agent-browser 0.26.0".to_string()
        } else if action.first().is_some_and(|value| value == "eval") {
            json!({
                "success": true,
                "data": { "result": { "present": false } }
            })
            .to_string()
        } else if action.first().is_some_and(|value| value == "snapshot") {
            json!({
                "success": true,
                "data": {
                    "origin": "http://127.0.0.1/",
                    "snapshot": "@e1 [button] Submit"
                }
            })
            .to_string()
        } else if action.first().is_some_and(|value| value == "wait") {
            let expected = action.get(2).or_else(|| action.get(1));
            if expected.is_some_and(|value| value == self.confirmation.as_str()) {
                json!({ "success": true }).to_string()
            } else {
                self.invocations
                    .lock()
                    .expect("invocations")
                    .push(invocation);
                return Ok(CommandOutput {
                    exit_code: 1,
                    stdout: String::new(),
                    stderr: "text not visible".to_string(),
                });
            }
        } else {
            json!({ "success": true }).to_string()
        };
        self.invocations
            .lock()
            .expect("invocations")
            .push(invocation);
        Ok(CommandOutput {
            exit_code: 0,
            stdout,
            stderr: String::new(),
        })
    }
}

struct OpenFailureExecutor;

#[async_trait]
impl CommandExecutor for OpenFailureExecutor {
    async fn run(&self, _invocation: CommandInvocation) -> Result<CommandOutput, CommandError> {
        Err(CommandError::unavailable("browser fixture is unavailable"))
    }
}

fn browser_action(args: &[OsString]) -> &[OsString] {
    let index = args
        .iter()
        .position(|value| value == "--headed")
        .map_or(0, |index| index + 2);
    let tail = &args[index..];
    if tail
        .first()
        .is_some_and(|value| value == "--allowed-domains")
    {
        &tail[4..]
    } else {
        tail
    }
}

struct ProviderFixture {
    endpoint: String,
    requests: Arc<Mutex<Vec<Vec<u8>>>>,
    task: tokio::task::JoinHandle<()>,
}

impl ProviderFixture {
    async fn start(responses: Vec<serde_json::Value>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("provider listener");
        let address = listener.local_addr().expect("provider address");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&requests);
        let task = tokio::spawn(async move {
            for response in responses {
                let (stream, _) = listener.accept().await.expect("provider request");
                let body = read_request(stream, response).await;
                captured.lock().expect("requests").push(body);
            }
        });
        Self {
            endpoint: format!("http://{address}/v1/plan"),
            requests,
            task,
        }
    }

    async fn finish(self) -> Vec<Vec<u8>> {
        self.task.await.expect("provider task");
        Arc::try_unwrap(self.requests)
            .expect("request owners")
            .into_inner()
            .expect("requests")
    }
}

async fn read_request(stream: TcpStream, response: serde_json::Value) -> Vec<u8> {
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .await
        .expect("request line");
    let mut headers = BTreeMap::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).await.expect("header");
        if line == "\r\n" {
            break;
        }
        let (name, value) = line.trim_end().split_once(':').expect("header pair");
        headers.insert(name.to_ascii_lowercase(), value.trim().to_string());
    }
    let length = headers["content-length"]
        .parse::<usize>()
        .expect("content length");
    let mut body = vec![0; length];
    reader.read_exact(&mut body).await.expect("request body");

    let response = serde_json::to_vec(&response).expect("response JSON");
    let head = format!(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        response.len()
    );
    let stream = reader.get_mut();
    stream
        .write_all(head.as_bytes())
        .await
        .expect("response head");
    stream.write_all(&response).await.expect("response body");
    stream.shutdown().await.expect("response shutdown");
    body
}
