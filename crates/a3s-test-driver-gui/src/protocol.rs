use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: &'static str,
    pub id: u64,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl JsonRpcRequest {
    #[must_use]
    pub fn new(id: u64, method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            method: method.into(),
            params,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: &'static str,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl JsonRpcNotification {
    #[must_use]
    pub fn new(method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0",
            method: method.into(),
            params,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    #[must_use]
    pub fn success(id: u64, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Value::from(id),
            result: Some(result),
            error: None,
        }
    }

    #[must_use]
    pub fn failure(id: u64, code: i64, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Value::from(id),
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(default)]
    pub data: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct CuaToolAnnotations {
    #[serde(rename = "readOnlyHint", default)]
    pub read_only: bool,
    #[serde(rename = "destructiveHint", default)]
    pub destructive: bool,
    #[serde(rename = "idempotentHint", default)]
    pub idempotent: bool,
    #[serde(rename = "openWorldHint", default)]
    pub open_world: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct CuaToolDefinition {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
    pub annotations: CuaToolAnnotations,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct InitializeResult {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    pub capabilities: Value,
    #[serde(rename = "serverInfo")]
    pub server_info: ServerInfo,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct ServerInfo {
    pub name: String,
    pub version: String,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct ToolsListResult {
    pub tools: Vec<CuaToolDefinition>,
    pub capability_version: String,
    pub schema_version: String,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct ToolCallResult {
    #[serde(default)]
    pub content: Vec<Value>,
    #[serde(rename = "isError", default)]
    pub is_error: Option<bool>,
    #[serde(rename = "structuredContent", default)]
    pub structured_content: Option<Value>,
}
