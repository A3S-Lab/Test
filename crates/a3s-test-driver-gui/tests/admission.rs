use std::sync::{Arc, Mutex};

use a3s_test_driver_gui::{
    CuaClient, CuaCompatibility, CuaTransport, CuaTransportError, JsonRpcNotification,
    JsonRpcRequest, JsonRpcResponse,
};
use async_trait::async_trait;
use serde_json::{json, Value};

struct FakeTransport {
    compatibility: CuaCompatibility,
    omit_capability: Option<(&'static str, &'static str)>,
    protocol_version: String,
    wrong_response_id: bool,
    notifications: Mutex<Vec<String>>,
}

impl FakeTransport {
    fn valid(compatibility: CuaCompatibility) -> Self {
        Self {
            protocol_version: compatibility.mcp_protocol().to_string(),
            compatibility,
            omit_capability: None,
            wrong_response_id: false,
            notifications: Mutex::new(Vec::new()),
        }
    }

    fn initialize_result(&self) -> Value {
        json!({
            "protocolVersion": self.protocol_version,
            "capabilities": { "tools": {} },
            "serverInfo": {
                "name": "cua-driver",
                "version": self.compatibility.driver_version().to_string(),
            }
        })
    }

    fn tools_result(&self) -> Value {
        let tools: Vec<Value> = self
            .compatibility
            .tools()
            .iter()
            .map(|(name, requirement)| {
                let capabilities: Vec<&str> = requirement
                    .capabilities()
                    .iter()
                    .map(String::as_str)
                    .filter(|capability| self.omit_capability != Some((name.as_str(), *capability)))
                    .collect();
                json!({
                    "name": name,
                    "description": format!("Fixture tool {name}"),
                    "inputSchema": { "type": "object" },
                    "annotations": {
                        "readOnlyHint": false,
                        "destructiveHint": false,
                        "idempotentHint": false,
                        "openWorldHint": false,
                    },
                    "capabilities": capabilities,
                })
            })
            .collect();
        json!({
            "tools": tools,
            "capability_version": self.compatibility.capability_vocabulary(),
            "schema_version": self.compatibility.tools_schema(),
        })
    }
}

#[async_trait]
impl CuaTransport for FakeTransport {
    async fn request(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse, CuaTransportError> {
        let result = match request.method.as_str() {
            "initialize" => self.initialize_result(),
            "tools/list" => self.tools_result(),
            method => return Ok(JsonRpcResponse::failure(request.id, -32601, method)),
        };
        let id = if self.wrong_response_id {
            request.id + 1
        } else {
            request.id
        };
        Ok(JsonRpcResponse::success(id, result))
    }

    async fn notify(&self, notification: JsonRpcNotification) -> Result<(), CuaTransportError> {
        self.notifications
            .lock()
            .map_err(|_| CuaTransportError::protocol("notification lock poisoned"))?
            .push(notification.method);
        Ok(())
    }
}

#[tokio::test]
async fn admits_the_locked_cua_contract() {
    let compatibility = CuaCompatibility::locked().expect("compatibility lock");
    let transport = Arc::new(FakeTransport::valid(compatibility.clone()));
    let client = CuaClient::new(transport.clone());

    let admitted = client
        .admit(&compatibility)
        .await
        .expect("admitted CUA capabilities");

    assert_eq!(admitted.driver_version, *compatibility.driver_version());
    assert!(admitted.tools.contains_key("get_window_state"));
    assert_eq!(
        transport
            .notifications
            .lock()
            .expect("notifications")
            .as_slice(),
        ["notifications/initialized"]
    );
}

#[tokio::test]
async fn rejects_a_missing_tool_capability() {
    let compatibility = CuaCompatibility::locked().expect("compatibility lock");
    let mut transport = FakeTransport::valid(compatibility.clone());
    transport.omit_capability = Some(("click", "input.pointer.click"));
    let client = CuaClient::new(Arc::new(transport));

    let error = client
        .admit(&compatibility)
        .await
        .expect_err("missing capability");

    assert_eq!(error.code(), "test.driver.gui.capability_missing");
}

#[tokio::test]
async fn rejects_an_unreviewed_protocol_version() {
    let compatibility = CuaCompatibility::locked().expect("compatibility lock");
    let mut transport = FakeTransport::valid(compatibility.clone());
    transport.protocol_version = "2099-01-01".to_string();
    let client = CuaClient::new(Arc::new(transport));

    let error = client
        .admit(&compatibility)
        .await
        .expect_err("protocol version");

    assert_eq!(error.code(), "test.driver.gui.protocol_unsupported");
}

#[tokio::test]
async fn rejects_a_mismatched_json_rpc_response_id() {
    let compatibility = CuaCompatibility::locked().expect("compatibility lock");
    let mut transport = FakeTransport::valid(compatibility.clone());
    transport.wrong_response_id = true;
    let client = CuaClient::new(Arc::new(transport));

    let error = client.admit(&compatibility).await.expect_err("response id");

    assert_eq!(error.code(), "test.driver.gui.cua_protocol_invalid");
}
