use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use a3s_test_core::DriverError;
use semver::Version;
use serde_json::{json, Value};

use crate::protocol::{InitializeResult, ToolCallResult, ToolsListResult};
use crate::{
    CuaCompatibility, CuaToolAnnotations, CuaToolDefinition, CuaTransport, CuaTransportError,
    CuaTransportErrorKind, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse,
};

#[derive(Clone, Debug, PartialEq)]
pub struct CuaTool {
    pub description: String,
    pub input_schema: Value,
    pub annotations: CuaToolAnnotations,
    pub capabilities: BTreeSet<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CuaCapabilities {
    pub driver_version: Version,
    pub protocol_version: String,
    pub capability_vocabulary: String,
    pub tools_schema: String,
    pub tools: BTreeMap<String, CuaTool>,
}

pub struct CuaClient {
    transport: Arc<dyn CuaTransport>,
    next_id: AtomicU64,
}

#[derive(Clone, Debug)]
pub(crate) struct CuaToolOutput {
    pub structured: Option<Value>,
}

impl CuaClient {
    #[must_use]
    pub fn new(transport: Arc<dyn CuaTransport>) -> Self {
        Self {
            transport,
            next_id: AtomicU64::new(1),
        }
    }

    pub async fn admit_locked(&self) -> Result<CuaCapabilities, DriverError> {
        let compatibility = CuaCompatibility::locked()?;
        self.admit(&compatibility).await
    }

    pub async fn admit(
        &self,
        compatibility: &CuaCompatibility,
    ) -> Result<CuaCapabilities, DriverError> {
        let initialize: InitializeResult = self
            .request_typed(
                "initialize",
                Some(json!({
                    "protocolVersion": compatibility.mcp_protocol(),
                    "capabilities": { "tools": {} },
                    "clientInfo": {
                        "name": "a3s-test-driver-gui",
                        "version": env!("CARGO_PKG_VERSION"),
                    }
                })),
            )
            .await?;
        validate_initialize(compatibility, &initialize)?;

        self.transport
            .notify(JsonRpcNotification::new("notifications/initialized", None))
            .await
            .map_err(transport_error)?;

        let listed: ToolsListResult = self.request_typed("tools/list", None).await?;
        validate_tools(compatibility, &initialize, listed)
    }

    pub async fn close(&self) -> Result<(), DriverError> {
        self.transport.close().await.map_err(transport_error)
    }

    pub(crate) async fn call_tool(
        &self,
        name: &str,
        arguments: Value,
    ) -> Result<CuaToolOutput, DriverError> {
        let result: ToolCallResult = self
            .request_typed(
                "tools/call",
                Some(json!({
                    "name": name,
                    "arguments": arguments,
                })),
            )
            .await?;
        if result.is_error == Some(true) {
            return Err(tool_error(name, &result));
        }
        Ok(CuaToolOutput {
            structured: result.structured_content,
        })
    }

    async fn request_typed<T>(&self, method: &str, params: Option<Value>) -> Result<T, DriverError>
    where
        T: serde::de::DeserializeOwned,
    {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let response = self
            .transport
            .request(JsonRpcRequest::new(id, method, params))
            .await
            .map_err(transport_error)?;
        let value = response_result(response, id, method)?;
        serde_json::from_value(value).map_err(|error| {
            DriverError::new(
                "test.driver.gui.cua_output_invalid",
                format!("CUA {method} returned an invalid structured result: {error}"),
            )
        })
    }
}

fn tool_error(name: &str, result: &ToolCallResult) -> DriverError {
    let tool_code = result
        .structured_content
        .as_ref()
        .and_then(Value::as_object)
        .and_then(|value| value.get("code").or_else(|| value.get("error")))
        .and_then(Value::as_str);
    let text = result
        .content
        .iter()
        .filter_map(|content| content.get("text").and_then(Value::as_str))
        .next()
        .unwrap_or("CUA tool returned an error");
    let message = text.chars().take(2_048).collect::<String>();
    if tool_code.is_some_and(|code| code.to_ascii_lowercase().contains("stale"))
        || message.to_ascii_lowercase().contains("stale")
    {
        return DriverError::new(
            "test.driver.gui.stale_reference",
            format!("CUA {name} rejected a stale element reference: {message}"),
        );
    }
    let suffix = tool_code.map_or(String::new(), |code| format!(" ({code})"));
    DriverError::new(
        "test.driver.gui.cua_tool_failed",
        format!("CUA {name} failed{suffix}: {message}"),
    )
}

fn validate_initialize(
    compatibility: &CuaCompatibility,
    initialize: &InitializeResult,
) -> Result<(), DriverError> {
    if initialize.protocol_version != compatibility.mcp_protocol() {
        return Err(DriverError::new(
            "test.driver.gui.protocol_unsupported",
            format!(
                "CUA MCP protocol {} is unsupported; expected {}",
                initialize.protocol_version,
                compatibility.mcp_protocol()
            ),
        ));
    }
    if initialize.server_info.name != "cua-driver" {
        return Err(DriverError::new(
            "test.driver.gui.server_identity_invalid",
            format!(
                "MCP server identity '{}' is not cua-driver",
                initialize.server_info.name
            ),
        ));
    }
    if !initialize
        .capabilities
        .get("tools")
        .is_some_and(Value::is_object)
    {
        return Err(DriverError::new(
            "test.driver.gui.capability_missing",
            "CUA initialize result does not advertise MCP tools",
        ));
    }
    let version = Version::parse(&initialize.server_info.version).map_err(|error| {
        DriverError::new(
            "test.driver.gui.version_invalid",
            format!("CUA reported an invalid semantic version: {error}"),
        )
    })?;
    if &version != compatibility.driver_version() {
        return Err(DriverError::new(
            "test.driver.gui.version_unsupported",
            format!(
                "CUA version {version} is unsupported; expected exactly {}",
                compatibility.driver_version()
            ),
        ));
    }
    Ok(())
}

fn validate_tools(
    compatibility: &CuaCompatibility,
    initialize: &InitializeResult,
    listed: ToolsListResult,
) -> Result<CuaCapabilities, DriverError> {
    if listed.schema_version != compatibility.tools_schema()
        || listed.capability_version != compatibility.capability_vocabulary()
    {
        return Err(DriverError::new(
            "test.driver.gui.capability_contract_unsupported",
            format!(
                "CUA tools schema/capability vocabulary {}/{} is unsupported; expected {}/{}",
                listed.schema_version,
                listed.capability_version,
                compatibility.tools_schema(),
                compatibility.capability_vocabulary()
            ),
        ));
    }

    let mut tools = BTreeMap::new();
    for definition in listed.tools {
        let name = definition.name.clone();
        if tools
            .insert(name.clone(), normalize_tool(definition))
            .is_some()
        {
            return Err(DriverError::new(
                "test.driver.gui.cua_output_invalid",
                format!("CUA tools/list returned duplicate tool '{name}'"),
            ));
        }
    }

    for (name, requirement) in compatibility.tools() {
        let tool = tools.get(name).ok_or_else(|| {
            DriverError::new(
                "test.driver.gui.capability_missing",
                format!("CUA is missing required tool '{name}'"),
            )
        })?;
        let missing: Vec<&str> = requirement
            .capabilities()
            .difference(&tool.capabilities)
            .map(String::as_str)
            .collect();
        if !missing.is_empty() {
            return Err(DriverError::new(
                "test.driver.gui.capability_missing",
                format!(
                    "CUA tool '{name}' is missing required capabilities: {}",
                    missing.join(", ")
                ),
            ));
        }
    }

    let driver_version = Version::parse(&initialize.server_info.version).map_err(|error| {
        DriverError::new(
            "test.driver.gui.version_invalid",
            format!("CUA reported an invalid semantic version: {error}"),
        )
    })?;
    Ok(CuaCapabilities {
        driver_version,
        protocol_version: initialize.protocol_version.clone(),
        capability_vocabulary: listed.capability_version,
        tools_schema: listed.schema_version,
        tools,
    })
}

fn normalize_tool(definition: CuaToolDefinition) -> CuaTool {
    CuaTool {
        description: definition.description,
        input_schema: definition.input_schema,
        annotations: definition.annotations,
        capabilities: definition.capabilities.into_iter().collect(),
    }
}

fn response_result(
    response: JsonRpcResponse,
    expected_id: u64,
    method: &str,
) -> Result<Value, DriverError> {
    if response.jsonrpc != "2.0" || response.id.as_u64() != Some(expected_id) {
        return Err(DriverError::new(
            "test.driver.gui.cua_protocol_invalid",
            format!("CUA {method} returned a mismatched JSON-RPC envelope"),
        ));
    }
    match (response.result, response.error) {
        (Some(result), None) => Ok(result),
        (None, Some(error)) => Err(DriverError::new(
            "test.driver.gui.cua_rpc_error",
            format!(
                "CUA {method} failed with JSON-RPC {}: {}",
                error.code, error.message
            ),
        )),
        _ => Err(DriverError::new(
            "test.driver.gui.cua_protocol_invalid",
            format!("CUA {method} response must contain exactly one of result or error"),
        )),
    }
}

fn transport_error(error: CuaTransportError) -> DriverError {
    let (code, retryable) = match error.kind() {
        CuaTransportErrorKind::Unavailable => ("test.driver.gui.cua_unavailable", true),
        CuaTransportErrorKind::TimedOut => ("test.driver.gui.cua_timeout", false),
        CuaTransportErrorKind::Protocol => ("test.driver.gui.cua_protocol_invalid", false),
    };
    DriverError::new(code, error.to_string()).with_retryable(retryable)
}
