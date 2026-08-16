use super::*;
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use a3s_test_core::{
    DriverError, DriverSession, PageContextInspectRequest, PageContextObservation, ScenarioContext,
    StepOutput, Surface, SurfaceDriver, SurfaceObservation, TestStep,
};
use a3s_test_driver_web::{
    AgentBrowserConfig, BrowserCommand, BrowserNetworkPolicy, CommandError, CommandExecutor,
    CommandInvocation, CommandOutput,
};
use a3s_test_session::{AgentSessionManager, SessionManagerOptions};
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{Mutex, Notify};

struct FakeDriver {
    closed: Arc<Mutex<usize>>,
    fail_first_close: bool,
    close_gate: Option<Arc<Notify>>,
}

#[async_trait]
impl SurfaceDriver for FakeDriver {
    fn surface(&self) -> Surface {
        Surface::Gui
    }

    async fn open(
        &self,
        _context: &ScenarioContext,
    ) -> Result<Box<dyn DriverSession>, DriverError> {
        Ok(Box::new(FakeSession {
            closed: Arc::clone(&self.closed),
            fail_first_close: self.fail_first_close,
            close_gate: self.close_gate.clone(),
        }))
    }
}

struct FakeSession {
    closed: Arc<Mutex<usize>>,
    fail_first_close: bool,
    close_gate: Option<Arc<Notify>>,
}

#[derive(Default)]
struct RepairWebExecutor {
    invocations: StdMutex<Vec<CommandInvocation>>,
    projected: StdMutex<Vec<Value>>,
    eval_counts: StdMutex<HashMap<&'static str, usize>>,
}

impl RepairWebExecutor {
    fn count(&self, key: &'static str) -> usize {
        let mut counts = self.eval_counts.lock().expect("eval counts");
        let count = counts.entry(key).or_default();
        *count += 1;
        *count
    }

    fn projected_statuses(&self) -> Vec<String> {
        self.projected
            .lock()
            .expect("projected events")
            .iter()
            .filter_map(|value| value["status"].as_str().map(str::to_string))
            .collect()
    }
}

#[async_trait]
impl CommandExecutor for RepairWebExecutor {
    async fn run(&self, invocation: CommandInvocation) -> Result<CommandOutput, CommandError> {
        let version_probe = invocation
            .args
            .last()
            .is_some_and(|argument| argument == "--version");
        let action = browser_action(&invocation.args);
        let stdout = if version_probe {
            "agent-browser 0.26.0".to_string()
        } else if action.first().is_some_and(|value| value == "eval") {
            let script = action
                .get(1)
                .map(|value| value.to_string_lossy())
                .unwrap_or_default();
            if script.contains("applyRepairEvent") {
                if let Some(event) = extract_projected_event(&script) {
                    self.projected.lock().expect("projected events").push(event);
                }
                json!({ "success": true, "data": { "result": null } }).to_string()
            } else if script.contains("takeRepairActions") {
                json!({ "success": true, "data": { "result": [] } }).to_string()
            } else if script.contains("batchWindowMs") {
                let result = if self.count("watch") == 1 {
                    vec![repair_finding_json()]
                } else {
                    Vec::new()
                };
                json!({ "success": true, "data": { "result": result } }).to_string()
            } else if script.contains("bridge.snapshot") {
                json!({ "success": true, "data": { "result": repair_page_context_json() } })
                    .to_string()
            } else {
                json!({ "success": true, "data": { "result": { "present": false } } }).to_string()
            }
        } else if action.first().is_some_and(|value| value == "screenshot") {
            let path = action.get(1).expect("screenshot path");
            std::fs::write(PathBuf::from(path), b"fake png").expect("write fake screenshot");
            json!({ "success": true }).to_string()
        } else if action
            .first()
            .is_some_and(|value| value == "console" || value == "errors")
        {
            json!({ "success": true, "data": { "result": [] } }).to_string()
        } else if action.first().is_some_and(|value| value == "snapshot") {
            json!({ "success": true, "data": { "origin": "http://127.0.0.1/repair", "snapshot": "@e1 [button] Broken" } }).to_string()
        } else {
            json!({ "success": true }).to_string()
        };
        self.invocations
            .lock()
            .expect("browser invocations")
            .push(invocation);
        Ok(CommandOutput {
            exit_code: 0,
            stdout,
            stderr: String::new(),
        })
    }
}

#[async_trait]
impl DriverSession for FakeSession {
    async fn observe(&mut self) -> Result<SurfaceObservation, DriverError> {
        Ok(SurfaceObservation::new("GUI").with_data(json!({
            "elements": [{ "ref": "@g1.1", "role": "AXButton", "name": "Save" }]
        })))
    }

    async fn execute(&mut self, _step: &TestStep) -> Result<StepOutput, DriverError> {
        Ok(StepOutput::new("clicked"))
    }

    async fn inspect_page_context(
        &mut self,
        _request: &PageContextInspectRequest,
    ) -> Result<PageContextObservation, DriverError> {
        Ok(PageContextObservation::absent())
    }

    async fn close(&mut self) -> Result<(), DriverError> {
        let mut closed = self.closed.lock().await;
        *closed += 1;
        if self.fail_first_close && *closed == 1 {
            drop(closed);
            if let Some(gate) = self.close_gate.take() {
                gate.notified().await;
            }
            return Err(DriverError::new(
                "test.driver.fake.cleanup_failed",
                "transient cleanup failure",
            )
            .with_retryable(true));
        }
        Ok(())
    }
}

#[tokio::test]
async fn projects_the_session_application_layer_over_mcp() {
    let closed = Arc::new(Mutex::new(0));
    let manager = Arc::new(
        AgentSessionManager::new(
            vec![Arc::new(FakeDriver {
                closed: Arc::clone(&closed),
                fail_first_close: false,
                close_gate: None,
            })],
            SessionManagerOptions {
                artifacts_root: std::env::temp_dir().join("a3s-test-mcp-tests"),
                cleanup_timeout: Duration::from_secs(1),
                max_sessions: 2,
            },
        )
        .expect("manager"),
    );
    let (mut client_writer, server_reader) = tokio::io::duplex(64 * 1_024);
    let (server_writer, client_reader) = tokio::io::duplex(64 * 1_024);
    let server = tokio::spawn(serve_io(server_reader, server_writer, manager));
    let mut client_reader = BufReader::new(client_reader);

    let initialized = exchange(
        &mut client_writer,
        &mut client_reader,
        initialize_request(1, MCP_PROTOCOL),
    )
    .await;
    assert_eq!(initialized["result"]["protocolVersion"], MCP_PROTOCOL);
    notify(
        &mut client_writer,
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
    )
    .await;

    let listed = exchange(
        &mut client_writer,
        &mut client_reader,
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
    )
    .await;
    assert!(listed["result"]["tools"]
        .as_array()
        .is_some_and(|tools| tools.iter().any(|tool| tool["name"] == "test_act")));
    assert!(listed["result"]["tools"]
        .as_array()
        .is_some_and(|tools| tools.iter().any(|tool| tool["name"] == "test_inspect")));
    let start_tool = listed["result"]["tools"]
        .as_array()
        .and_then(|tools| {
            tools
                .iter()
                .find(|tool| tool["name"] == "test_session_start")
        })
        .expect("start tool");
    assert_eq!(
        start_tool["inputSchema"]["properties"]["surface"]["enum"],
        json!(["gui"])
    );
    assert_eq!(
        start_tool["inputSchema"]["properties"]["auto_resolve_repairs"]["default"],
        false
    );

    let started = call(
        &mut client_writer,
        &mut client_reader,
        3,
        "test_session_start",
        json!({
            "session": "editor",
            "surface": "gui",
            "goal": "Save",
            "success_criteria": ["Saved"],
            "auto_resolve_repairs": true
        }),
    )
    .await;
    assert_eq!(started["result"]["structuredContent"]["surface"], "gui");
    assert_eq!(
        started["result"]["structuredContent"]["auto_resolve_repairs"],
        true
    );

    let observed = call(
        &mut client_writer,
        &mut client_reader,
        4,
        "test_observe",
        json!({ "session": "editor" }),
    )
    .await;
    let observation_id = observed["result"]["structuredContent"]["observation_id"]
        .as_u64()
        .expect("observation id");

    let acted = call(
        &mut client_writer,
        &mut client_reader,
        5,
        "test_act",
        json!({
            "session": "editor",
            "observation_id": observation_id,
            "action": {
                "type": "click",
                "target": { "type": "ref", "value": "@g1.1" }
            }
        }),
    )
    .await;
    assert_eq!(
        acted["result"]["structuredContent"]["output"]["summary"],
        "clicked"
    );

    let finished = call(
        &mut client_writer,
        &mut client_reader,
        6,
        "test_finish",
        json!({ "session": "editor", "status": "passed", "summary": "Saved" }),
    )
    .await;
    assert_eq!(finished["result"]["structuredContent"]["status"], "passed");

    drop(client_writer);
    server.await.expect("server task").expect("MCP server");
    assert_eq!(*closed.lock().await, 1);
}

#[tokio::test]
async fn mcp_exposes_retryable_cleanup_without_allowing_another_turn() {
    let closed = Arc::new(Mutex::new(0));
    let close_gate = Arc::new(Notify::new());
    let manager = Arc::new(
        AgentSessionManager::new(
            vec![Arc::new(FakeDriver {
                closed: Arc::clone(&closed),
                fail_first_close: true,
                close_gate: Some(Arc::clone(&close_gate)),
            })],
            SessionManagerOptions {
                artifacts_root: std::env::temp_dir().join("a3s-test-mcp-cleanup-retry"),
                cleanup_timeout: Duration::from_millis(10),
                max_sessions: 1,
            },
        )
        .expect("manager"),
    );
    let (mut client_writer, server_reader) = tokio::io::duplex(64 * 1_024);
    let (server_writer, client_reader) = tokio::io::duplex(64 * 1_024);
    let server = tokio::spawn(serve_io(server_reader, server_writer, manager));
    let mut client_reader = BufReader::new(client_reader);

    exchange(
        &mut client_writer,
        &mut client_reader,
        initialize_request(1, MCP_PROTOCOL),
    )
    .await;
    notify(
        &mut client_writer,
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
    )
    .await;
    call(
        &mut client_writer,
        &mut client_reader,
        2,
        "test_session_start",
        json!({
            "session": "cleanup-retry",
            "surface": "gui",
            "goal": "Close safely",
            "success_criteria": ["No owned resource survives"]
        }),
    )
    .await;

    let failed_finish = call(
        &mut client_writer,
        &mut client_reader,
        3,
        "test_finish",
        json!({
            "session": "cleanup-retry",
            "status": "passed",
            "summary": "Product behavior passed"
        }),
    )
    .await;
    assert_eq!(
        failed_finish["result"]["structuredContent"]["status"],
        "failed"
    );
    assert_eq!(
        failed_finish["result"]["structuredContent"]["cleanup_error"]["code"],
        "test.session.cleanup_timeout"
    );
    assert_eq!(
        failed_finish["result"]["structuredContent"]["cleanup_error"]["retryable"],
        true
    );

    let rejected_turn = call(
        &mut client_writer,
        &mut client_reader,
        4,
        "test_observe",
        json!({ "session": "cleanup-retry" }),
    )
    .await;
    assert_eq!(rejected_turn["result"]["isError"], true);
    assert_eq!(
        rejected_turn["result"]["structuredContent"]["code"],
        "test.session.cleanup_in_progress"
    );
    assert_eq!(
        rejected_turn["result"]["structuredContent"]["retryable"],
        true
    );

    close_gate.notify_one();
    tokio::time::timeout(Duration::from_secs(1), async {
        let mut id = 5;
        loop {
            let state = call(
                &mut client_writer,
                &mut client_reader,
                id,
                "test_observe",
                json!({ "session": "cleanup-retry" }),
            )
            .await;
            let code = state["result"]["structuredContent"]["code"].as_str();
            if code == Some("test.session.cleanup_required") {
                break;
            }
            assert_eq!(code, Some("test.session.cleanup_in_progress"));
            id += 1;
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("cleanup state transition deadline");

    let aborted = call(
        &mut client_writer,
        &mut client_reader,
        10_000,
        "test_abort",
        json!({ "session": "cleanup-retry" }),
    )
    .await;
    assert_eq!(aborted["result"]["structuredContent"]["status"], "aborted");
    assert!(aborted["result"]["structuredContent"]["cleanup_error"].is_null());

    drop(client_writer);
    server.await.expect("server task").expect("MCP server");
    assert_eq!(*closed.lock().await, 2);
}

#[tokio::test]
async fn enforces_mcp_initialization_and_protocol_negotiation() {
    let manager = Arc::new(
        AgentSessionManager::new(
            vec![Arc::new(FakeDriver {
                closed: Arc::new(Mutex::new(0)),
                fail_first_close: false,
                close_gate: None,
            })],
            SessionManagerOptions {
                artifacts_root: std::env::temp_dir().join("a3s-test-mcp-lifecycle"),
                cleanup_timeout: Duration::from_secs(1),
                max_sessions: 1,
            },
        )
        .expect("manager"),
    );
    let (mut client_writer, server_reader) = tokio::io::duplex(64 * 1_024);
    let (server_writer, client_reader) = tokio::io::duplex(64 * 1_024);
    let server = tokio::spawn(serve_io(server_reader, server_writer, manager));
    let mut client_reader = BufReader::new(client_reader);

    let before_initialize = exchange(
        &mut client_writer,
        &mut client_reader,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }),
    )
    .await;
    assert_eq!(before_initialize["error"]["code"], -32_002);

    let unsupported = exchange(
        &mut client_writer,
        &mut client_reader,
        initialize_request(2, "2025-03-26"),
    )
    .await;
    assert_eq!(unsupported["error"]["code"], -32_602);

    let initialized = exchange(
        &mut client_writer,
        &mut client_reader,
        initialize_request(3, MCP_PROTOCOL),
    )
    .await;
    assert_eq!(initialized["result"]["protocolVersion"], MCP_PROTOCOL);

    let before_notification = exchange(
        &mut client_writer,
        &mut client_reader,
        json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/list" }),
    )
    .await;
    assert_eq!(before_notification["error"]["code"], -32_002);

    notify(
        &mut client_writer,
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
    )
    .await;
    let listed = exchange(
        &mut client_writer,
        &mut client_reader,
        json!({ "jsonrpc": "2.0", "id": 5, "method": "tools/list" }),
    )
    .await;
    assert!(listed["result"]["tools"].is_array());

    let duplicate = exchange(
        &mut client_writer,
        &mut client_reader,
        initialize_request(6, MCP_PROTOCOL),
    )
    .await;
    assert_eq!(duplicate["error"]["code"], -32_600);

    drop(client_writer);
    server.await.expect("server task").expect("MCP server");
}

#[tokio::test]
async fn mcp_web_ingests_claims_and_completes_a_page_repair_durably() {
    let temp = tempfile::tempdir().expect("tempdir");
    let executor = Arc::new(RepairWebExecutor::default());
    let web = super::super::mcp_web::McpWebDriver::with_executor(
        AgentBrowserConfig {
            command: BrowserCommand::Standalone {
                executable: PathBuf::from("/opt/agent-browser"),
            },
            namespace: String::new(),
            headed: false,
            command_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(30),
            microphone: Default::default(),
            network_policy: BrowserNetworkPolicy::restricted_to_domains(["127.0.0.1"])
                .expect("network policy"),
        },
        executor.clone(),
        "http://127.0.0.1/repair".to_string(),
    );
    let manager = Arc::new(
        AgentSessionManager::new(
            vec![Arc::new(web)],
            SessionManagerOptions {
                artifacts_root: temp.path().to_path_buf(),
                cleanup_timeout: Duration::from_secs(1),
                max_sessions: 1,
            },
        )
        .expect("manager"),
    );
    let (mut client_writer, server_reader) = tokio::io::duplex(128 * 1_024);
    let (server_writer, client_reader) = tokio::io::duplex(128 * 1_024);
    let server = tokio::spawn(serve_io(server_reader, server_writer, manager));
    let mut client_reader = BufReader::new(client_reader);

    exchange(
        &mut client_writer,
        &mut client_reader,
        initialize_request(1, MCP_PROTOCOL),
    )
    .await;
    notify(
        &mut client_writer,
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
    )
    .await;
    let started = call(
        &mut client_writer,
        &mut client_reader,
        2,
        "test_session_start",
        json!({
            "session": "web-repair",
            "surface": "web",
            "goal": "Repair the marked control",
            "success_criteria": ["The marked control works"]
        }),
    )
    .await;
    assert_eq!(started["result"]["structuredContent"]["surface"], "web");

    let watched = call(
        &mut client_writer,
        &mut client_reader,
        3,
        "test_repair_watch",
        json!({ "session": "web-repair", "timeout_ms": 100, "batch_window_ms": 0 }),
    )
    .await;
    assert_eq!(
        watched["result"]["structuredContent"]["repairs"][0]["finding"]["id"],
        "finding-1"
    );

    let claimed = call(
        &mut client_writer,
        &mut client_reader,
        4,
        "test_repair_claim",
        json!({
            "session": "web-repair",
            "finding_id": "finding-1",
            "request_id": "claim-1",
            "lease_ms": 300000
        }),
    )
    .await;
    let attempt_id = claimed["result"]["structuredContent"]["attempt_id"]
        .as_str()
        .expect("derived attempt id")
        .to_string();
    assert_eq!(attempt_id, "attempt-claim-1");

    let missing_attempt = call(
        &mut client_writer,
        &mut client_reader,
        5,
        "test_repair_progress",
        json!({
            "session": "web-repair",
            "finding_id": "finding-1",
            "request_id": "progress-missing"
        }),
    )
    .await;
    assert_eq!(missing_attempt["result"]["isError"], true);
    assert_eq!(
        missing_attempt["result"]["structuredContent"]["code"],
        "test.session.repair_attempt_invalid"
    );

    let progressed = call(
        &mut client_writer,
        &mut client_reader,
        6,
        "test_repair_progress",
        json!({
            "session": "web-repair",
            "finding_id": "finding-1",
            "request_id": "progress-1",
            "attempt_id": attempt_id
        }),
    )
    .await;
    assert_eq!(
        progressed["result"]["structuredContent"]["status"],
        "repairing"
    );

    let completed = call(
        &mut client_writer,
        &mut client_reader,
        7,
        "test_repair_complete",
        json!({
            "session": "web-repair",
            "finding_id": "finding-1",
            "request_id": "complete-1",
            "attempt_id": "attempt-claim-1",
            "summary": "Workspace edit reported complete"
        }),
    )
    .await;
    assert_eq!(
        completed["result"]["structuredContent"]["status"],
        "verifying"
    );

    let aborted = call(
        &mut client_writer,
        &mut client_reader,
        8,
        "test_abort",
        json!({ "session": "web-repair" }),
    )
    .await;
    assert_eq!(aborted["result"]["structuredContent"]["status"], "aborted");

    drop(client_writer);
    server.await.expect("server task").expect("MCP server");
    assert_eq!(
        executor.projected_statuses(),
        ["claimed", "repairing", "verifying"]
    );
    let ledger = tokio::fs::read_to_string(temp.path().join("web-repair/repairs.jsonl"))
        .await
        .expect("durable repair ledger");
    assert_eq!(ledger.lines().count(), 6, "{ledger}");
    assert!(ledger.contains("\"kind\":\"before_evidence\""));
    assert!(ledger.contains("\"status\":\"verifying\""));
    assert!(ledger.contains("\"status\":\"needs_input\""));
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

fn extract_projected_event(script: &str) -> Option<Value> {
    let start = script.find("applyRepairEvent?.(")? + "applyRepairEvent?.(".len();
    let end = script[start..].find(") ?? null")? + start;
    serde_json::from_str(&script[start..end]).ok()
}

fn repair_finding_json() -> Value {
    json!({
        "id": "finding-1",
        "batchId": "batch-1",
        "instruction": "Fix the broken control",
        "successCriteria": "The control works",
        "intent": "fix",
        "severity": "important",
        "target": { "kind": "node", "nodeIds": ["n1"] },
        "createdAt": "2026-08-12T00:00:00Z",
        "pageId": "repair-page",
        "url": "http://127.0.0.1/repair",
        "contextRevision": 1,
        "context": { "untrusted": true },
        "status": "queued",
        "submittedAt": "2026-08-12T00:00:01Z"
    })
}

fn repair_page_context_json() -> Value {
    json!({
        "present": true,
        "protocol": "a3s.test.page-context/1",
        "sdkVersion": "0.1.0",
        "revision": 1,
        "page": {
            "id": "repair-page",
            "url": "http://127.0.0.1/repair",
            "route": "/repair",
            "title": "Repair fixture",
            "ready": true,
            "viewport": { "width": 1280.0, "height": 720.0, "dpr": 1.0 },
            "document": { "width": 1280.0, "height": 720.0 },
            "scroll": { "x": 0.0, "y": 0.0 },
            "language": "en",
            "theme": "light"
        },
        "components": [],
        "nodes": [],
        "facts": {},
        "removedNodeIds": [],
        "truncated": false,
        "nextCursor": null
    })
}

async fn call<W, R>(writer: &mut W, reader: &mut R, id: u64, name: &str, arguments: Value) -> Value
where
    W: tokio::io::AsyncWrite + Unpin,
    R: tokio::io::AsyncBufRead + Unpin,
{
    exchange(
        writer,
        reader,
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": { "name": name, "arguments": arguments }
        }),
    )
    .await
}

async fn exchange<W, R>(writer: &mut W, reader: &mut R, request: Value) -> Value
where
    W: tokio::io::AsyncWrite + Unpin,
    R: tokio::io::AsyncBufRead + Unpin,
{
    let mut encoded = serde_json::to_vec(&request).expect("encode request");
    encoded.push(b'\n');
    writer.write_all(&encoded).await.expect("write request");
    writer.flush().await.expect("flush request");
    let mut response = String::new();
    reader
        .read_line(&mut response)
        .await
        .expect("read response");
    serde_json::from_str(&response).expect("JSON-RPC response")
}

fn initialize_request(id: u64, protocol: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "initialize",
        "params": {
            "protocolVersion": protocol,
            "capabilities": {},
            "clientInfo": { "name": "a3s-test-client", "version": "1.0.0" }
        }
    })
}

async fn notify<W>(writer: &mut W, notification: Value)
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut encoded = serde_json::to_vec(&notification).expect("encode notification");
    encoded.push(b'\n');
    writer
        .write_all(&encoded)
        .await
        .expect("write notification");
    writer.flush().await.expect("flush notification");
}
