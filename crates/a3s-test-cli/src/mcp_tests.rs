use super::*;
use std::sync::Arc;
use std::time::Duration;

use a3s_test_core::{
    DriverError, DriverSession, ScenarioContext, StepOutput, Surface, SurfaceDriver,
    SurfaceObservation, TestStep,
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

    let started = call(
        &mut client_writer,
        &mut client_reader,
        3,
        "test_session_start",
        json!({
            "session": "editor",
            "surface": "gui",
            "goal": "Save",
            "success_criteria": ["Saved"]
        }),
    )
    .await;
    assert_eq!(started["result"]["structuredContent"]["surface"], "gui");

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
