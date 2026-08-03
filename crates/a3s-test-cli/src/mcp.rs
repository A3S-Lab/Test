use std::sync::Arc;

use a3s_test_core::{Action, ACTION_PROTOCOL_REVISION};
use a3s_test_session::{
    ActSessionRequest, AgentSessionManager, FinishSessionRequest, SessionError, StartSessionRequest,
};
use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};
use serde_json::{json, Value};
use tokio::io::{
    AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader,
};

const MCP_PROTOCOL: &str = "2025-06-18";
const MAX_REQUEST_BYTES: usize = 8 * 1_024 * 1_024;

pub(super) async fn serve(manager: Arc<AgentSessionManager>) -> Result<()> {
    serve_io(tokio::io::stdin(), tokio::io::stdout(), manager).await
}

async fn serve_io<R, W>(reader: R, writer: W, manager: Arc<AgentSessionManager>) -> Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let result = serve_loop(BufReader::new(reader), writer, Arc::clone(&manager)).await;
    manager.close_all().await;
    result
}

async fn serve_loop<R, W>(
    mut reader: R,
    mut writer: W,
    manager: Arc<AgentSessionManager>,
) -> Result<()>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut phase = ConnectionPhase::AwaitingInitialize;
    loop {
        let Some(line) = read_bounded_line(&mut reader).await? else {
            return Ok(());
        };
        let request: RpcRequest = match serde_json::from_slice(&line) {
            Ok(request) => request,
            Err(error) => {
                write_response(
                    &mut writer,
                    &rpc_error(Value::Null, -32_700, format!("invalid JSON: {error}")),
                )
                .await?;
                continue;
            }
        };
        let id = request.id.0;
        if request.jsonrpc != "2.0" || request.method.trim().is_empty() {
            if let Some(id) = id {
                write_response(
                    &mut writer,
                    &rpc_error(id, -32_600, "invalid JSON-RPC request"),
                )
                .await?;
            }
            continue;
        }
        let Some(id) = id else {
            handle_notification(&mut phase, &request.method);
            continue;
        };
        if !valid_request_id(&id) {
            write_response(
                &mut writer,
                &rpc_error(Value::Null, -32_600, "invalid JSON-RPC request id"),
            )
            .await?;
            continue;
        }
        let response =
            match handle_request(&manager, &mut phase, &request.method, request.params).await {
                Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
                Err(error) => rpc_error(id, error.code, error.message),
            };
        write_response(&mut writer, &response).await?;
    }
}

async fn handle_request(
    manager: &AgentSessionManager,
    phase: &mut ConnectionPhase,
    method: &str,
    params: Option<Value>,
) -> std::result::Result<Value, RpcFault> {
    match method {
        "initialize" => initialize(phase, params),
        "ping" => Ok(json!({})),
        "tools/list" => {
            require_ready(*phase)?;
            Ok(json!({ "tools": tool_definitions(&manager.surfaces()) }))
        }
        "tools/call" => {
            require_ready(*phase)?;
            call_tool(manager, params).await
        }
        _ => Err(RpcFault::new(-32_601, format!("unknown method '{method}'"))),
    }
}

fn initialize(
    phase: &mut ConnectionPhase,
    params: Option<Value>,
) -> std::result::Result<Value, RpcFault> {
    if *phase != ConnectionPhase::AwaitingInitialize {
        return Err(RpcFault::new(
            -32_600,
            "initialize is only valid as the first MCP request",
        ));
    }
    let params =
        parse_value::<InitializeParams>(params.unwrap_or(Value::Null), "initialize parameters")?;
    if params.protocol_version != MCP_PROTOCOL {
        return Err(RpcFault::new(
            -32_602,
            format!(
                "unsupported MCP protocol {}; expected {MCP_PROTOCOL}",
                params.protocol_version
            ),
        ));
    }
    if !params.capabilities.is_object()
        || params.client_info.name.trim().is_empty()
        || params.client_info.version.trim().is_empty()
    {
        return Err(RpcFault::new(
            -32_602,
            "initialize requires object capabilities and non-empty clientInfo name/version",
        ));
    }
    *phase = ConnectionPhase::AwaitingInitialized;
    Ok(json!({
        "protocolVersion": MCP_PROTOCOL,
        "capabilities": { "tools": {} },
        "serverInfo": {
            "name": "a3s-test",
            "version": env!("CARGO_PKG_VERSION"),
        },
    }))
}

fn require_ready(phase: ConnectionPhase) -> std::result::Result<(), RpcFault> {
    if phase == ConnectionPhase::Ready {
        Ok(())
    } else {
        Err(RpcFault::new(-32_002, "MCP server is not initialized"))
    }
}

fn handle_notification(phase: &mut ConnectionPhase, method: &str) {
    if method == "notifications/initialized" && *phase == ConnectionPhase::AwaitingInitialized {
        *phase = ConnectionPhase::Ready;
    }
}

fn valid_request_id(id: &Value) -> bool {
    match id {
        Value::String(_) => true,
        Value::Number(number) => number.as_i64().is_some() || number.as_u64().is_some(),
        _ => false,
    }
}

async fn call_tool(
    manager: &AgentSessionManager,
    params: Option<Value>,
) -> std::result::Result<Value, RpcFault> {
    let call: ToolCall = parse_value(params.unwrap_or(Value::Null), "tools/call params")?;
    let arguments = call.arguments.unwrap_or_else(|| json!({}));
    let result = match call.name.as_str() {
        "test_session_start" => {
            let request = parse_value::<StartSessionRequest>(arguments, "start arguments")?;
            manager
                .start(request)
                .await
                .map(|value| tool_success("test session started", value))
        }
        "test_observe" => {
            let request = parse_value::<SessionArgument>(arguments, "observe arguments")?;
            manager
                .observe(&request.session)
                .await
                .map(|value| tool_success("surface observed", value))
        }
        "test_act" => {
            let request = parse_value::<ActSessionRequest>(arguments, "act arguments")?;
            manager
                .act(request)
                .await
                .map(|value| tool_success("typed action completed", value))
        }
        "test_finish" => {
            let request = parse_value::<FinishSessionRequest>(arguments, "finish arguments")?;
            manager
                .finish(request)
                .await
                .map(|value| tool_success("test session finished", value))
        }
        "test_abort" => {
            let request = parse_value::<SessionArgument>(arguments, "abort arguments")?;
            manager
                .abort(&request.session)
                .await
                .map(|value| tool_success("test session aborted", value))
        }
        "test_schema" => Ok(tool_success(
            "typed action schema",
            json!({
                "protocol_revision": ACTION_PROTOCOL_REVISION,
                "supported_surfaces": manager.surfaces(),
                "action_schema": schemars::schema_for!(Action),
            }),
        )),
        _ => {
            return Err(RpcFault::new(
                -32_602,
                format!("unknown MCP tool '{}'", call.name),
            ));
        }
    };
    Ok(result.unwrap_or_else(tool_failure))
}

fn tool_success(summary: &str, value: impl serde::Serialize) -> Value {
    let mut structured = serde_json::to_value(value).unwrap_or_else(|_| json!({}));
    if let Some(object) = structured.as_object_mut() {
        object.insert(
            "protocol_revision".to_string(),
            Value::from(ACTION_PROTOCOL_REVISION),
        );
    }
    json!({
        "content": [{ "type": "text", "text": summary }],
        "structuredContent": structured,
    })
}

fn tool_failure(error: SessionError) -> Value {
    json!({
        "content": [{ "type": "text", "text": error.message() }],
        "isError": true,
        "structuredContent": {
            "protocol_revision": ACTION_PROTOCOL_REVISION,
            "code": error.code(),
            "message": error.message(),
            "retryable": error.retryable(),
        },
    })
}

fn tool_definitions(surfaces: &[a3s_test_core::Surface]) -> Vec<Value> {
    let action_schema = serde_json::to_value(schemars::schema_for!(Action))
        .unwrap_or_else(|_| json!({ "type": "object" }));
    let surfaces = serde_json::to_value(surfaces).unwrap_or_else(|_| json!([]));
    vec![
        tool_definition(
            "test_session_start",
            "Open one typed surface session. Host-side GUI application configuration is fixed when the MCP server starts.",
            json!({
                "type": "object",
                "required": ["session", "surface", "goal", "success_criteria"],
                "properties": {
                    "session": { "type": "string", "minLength": 1, "maxLength": 48 },
                    "surface": { "type": "string", "enum": surfaces },
                    "goal": { "type": "string", "minLength": 1 },
                    "success_criteria": {
                        "type": "array",
                        "minItems": 1,
                        "items": { "type": "string", "minLength": 1 }
                    }
                },
                "additionalProperties": false
            }),
            false,
            false,
        ),
        tool_definition(
            "test_observe",
            "Capture the next semantic observation and bind its refs to a new observation id.",
            session_schema(),
            true,
            false,
        ),
        tool_definition(
            "test_act",
            "Execute exactly one typed action. Semantic and visual refs require the latest observation id.",
            json!({
                "type": "object",
                "required": ["session", "action"],
                "properties": {
                    "session": { "type": "string" },
                    "observation_id": { "type": ["integer", "null"], "minimum": 1 },
                    "action": action_schema,
                },
                "additionalProperties": false
            }),
            false,
            true,
        ),
        tool_definition(
            "test_finish",
            "Close the exact owned surface and return the terminal session result. Retry finish or abort when cleanup_error.retryable is true.",
            json!({
                "type": "object",
                "required": ["session", "status", "summary"],
                "properties": {
                    "session": { "type": "string" },
                    "status": { "type": "string", "enum": ["passed", "failed", "aborted"] },
                    "summary": { "type": "string", "minLength": 1 }
                },
                "additionalProperties": false
            }),
            false,
            true,
        ),
        tool_definition(
            "test_abort",
            "Abort and clean up one active or cleanup-required session.",
            session_schema(),
            false,
            true,
        ),
        tool_definition(
            "test_schema",
            "Return the authoritative typed Action JSON Schema.",
            json!({ "type": "object", "additionalProperties": false }),
            true,
            false,
        ),
    ]
}

fn tool_definition(
    name: &str,
    description: &str,
    input_schema: Value,
    read_only: bool,
    destructive: bool,
) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema,
        "annotations": {
            "readOnlyHint": read_only,
            "destructiveHint": destructive,
            "idempotentHint": false,
            "openWorldHint": false,
        }
    })
}

fn session_schema() -> Value {
    json!({
        "type": "object",
        "required": ["session"],
        "properties": { "session": { "type": "string" } },
        "additionalProperties": false
    })
}

fn parse_value<T>(value: Value, context: &str) -> std::result::Result<T, RpcFault>
where
    T: DeserializeOwned,
{
    serde_json::from_value(value)
        .map_err(|error| RpcFault::new(-32_602, format!("invalid {context}: {error}")))
}

async fn read_bounded_line<R>(reader: &mut R) -> Result<Option<Vec<u8>>>
where
    R: AsyncBufRead + Unpin,
{
    let mut line = Vec::new();
    let mut limited = reader.take((MAX_REQUEST_BYTES + 1) as u64);
    let count = limited
        .read_until(b'\n', &mut line)
        .await
        .context("failed to read MCP request")?;
    if count == 0 {
        return Ok(None);
    }
    if line.len() > MAX_REQUEST_BYTES || line.last() != Some(&b'\n') {
        anyhow::bail!("MCP request exceeds {MAX_REQUEST_BYTES} bytes or is not line-delimited");
    }
    Ok(Some(line))
}

async fn write_response<W>(writer: &mut W, value: &Value) -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    let mut encoded = serde_json::to_vec(value).context("failed to encode MCP response")?;
    encoded.push(b'\n');
    writer
        .write_all(&encoded)
        .await
        .context("failed to write MCP response")?;
    writer.flush().await.context("failed to flush MCP response")
}

fn rpc_error(id: Value, code: i64, message: impl Into<String>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message.into() },
    })
}

#[derive(Deserialize)]
struct RpcRequest {
    jsonrpc: String,
    #[serde(default)]
    id: OptionalRequestId,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConnectionPhase {
    AwaitingInitialize,
    AwaitingInitialized,
    Ready,
}

#[derive(Default)]
struct OptionalRequestId(Option<Value>);

impl<'de> Deserialize<'de> for OptionalRequestId {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Value::deserialize(deserializer).map(|value| Self(Some(value)))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InitializeParams {
    protocol_version: String,
    capabilities: Value,
    client_info: ClientInfo,
}

#[derive(Deserialize)]
struct ClientInfo {
    name: String,
    version: String,
}

#[derive(Deserialize)]
struct ToolCall {
    name: String,
    #[serde(default)]
    arguments: Option<Value>,
}

#[derive(Deserialize)]
struct SessionArgument {
    session: String,
}

struct RpcFault {
    code: i64,
    message: String,
}

impl RpcFault {
    fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod tests;
