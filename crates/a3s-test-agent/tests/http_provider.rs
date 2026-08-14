use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use a3s_test_agent::{
    AgentGoal, ContractGenerationProvider, ContractGenerationProviderIdentity,
    ContractGenerationProviderRequest, DesignAuditDimension, DesignAuditProvider,
    DesignAuditProviderIdentity, DesignAuditProviderRequest, GroundingProviderIdentity,
    GroundingProviderRequest, GroundingTrigger, HttpContractGenerationProvider,
    HttpContractGenerationRequest, HttpDesignAuditProvider, HttpDesignAuditRequest,
    HttpLlmCompletionRequest, HttpLlmProvider, HttpProviderConfig, HttpProviderEndpoint,
    HttpVisualGroundingProvider, HttpVisualGroundingRequest, LlmIdentity, LlmProvider,
    PlannerContext, RemainingBudget, StructuredLlmRequest, VisualGroundingProvider,
    CONTRACT_GENERATION_PROVIDER_PROTOCOL, DESIGN_AUDIT_PROVIDER_PROTOCOL, LLM_PROVIDER_PROTOCOL,
    VISUAL_GROUNDING_PROVIDER_PROTOCOL,
};
use a3s_test_core::{
    ContractContext, ContractMode, PageContextSnapshot, Surface, SurfaceObservation,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

const MAX_TEST_REQUEST_BYTES: usize = 2 * 1_024 * 1_024;
const GROUNDING_IMAGE_BYTES: &[u8] = b"a3s-test HTTP grounding fixture";

thread_local! {
    static GROUNDING_IMAGES: std::cell::RefCell<Vec<tempfile::TempDir>> = const { std::cell::RefCell::new(Vec::new()) };
}

#[derive(Clone, Debug)]
struct CapturedRequest {
    method: String,
    target: String,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
}

#[derive(Clone)]
struct ResponseSpec {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    delay: Duration,
    include_content_length: bool,
}

impl ResponseSpec {
    fn json(value: Value) -> Self {
        Self {
            status: 200,
            headers: vec![("content-type".to_string(), "application/json".to_string())],
            body: serde_json::to_vec(&value).expect("response JSON"),
            delay: Duration::ZERO,
            include_content_length: true,
        }
    }
}

struct FixtureServer {
    endpoint: HttpProviderEndpoint,
    requests: Arc<Mutex<Vec<CapturedRequest>>>,
    task: tokio::task::JoinHandle<()>,
}

impl FixtureServer {
    async fn start(responses: Vec<ResponseSpec>) -> Self {
        let listener = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
            .await
            .expect("bind fixture server");
        let address = listener.local_addr().expect("fixture address");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = requests.clone();
        let task = tokio::spawn(async move {
            for response in responses {
                let (stream, _) = listener.accept().await.expect("accept provider request");
                let request = read_request(stream, response.clone()).await;
                captured.lock().unwrap().push(request);
            }
        });
        Self {
            endpoint: format!("http://{address}/v1/provider")
                .parse()
                .expect("fixture endpoint"),
            requests,
            task,
        }
    }

    async fn finish(self) -> Vec<CapturedRequest> {
        self.task.await.expect("fixture server task");
        Arc::try_unwrap(self.requests)
            .expect("request owners")
            .into_inner()
            .unwrap()
    }
}

#[tokio::test]
async fn contract_generation_adapter_sends_a_versioned_bounded_envelope() {
    let identity = contract_identity();
    let server = FixtureServer::start(vec![ResponseSpec::json(json!({
        "status": "success",
        "protocol": CONTRACT_GENERATION_PROVIDER_PROTOCOL,
        "response": {
            "identity": identity,
            "source_digests": [],
            "candidates": [],
            "usage": { "input_tokens": 3, "output_tokens": 5, "cost_microusd": 7 },
            "request_id": "contract-http-1"
        }
    }))])
    .await;
    let config = HttpProviderConfig::new(server.endpoint.clone())
        .with_authorization("Bearer fixture-secret")
        .expect("authorization");
    let provider = HttpContractGenerationProvider::new(contract_identity(), config)
        .expect("HTTP contract provider");
    let request = contract_request();

    let response = provider
        .generate(request.clone())
        .await
        .expect("contract provider response");

    assert_eq!(response.request_id.as_deref(), Some("contract-http-1"));
    let requests = server.finish().await;
    assert_eq!(requests.len(), 1);
    let captured = &requests[0];
    assert_eq!(captured.method, "POST");
    assert_eq!(captured.target, "/v1/provider");
    assert_eq!(
        captured.headers.get("authorization").map(String::as_str),
        Some("Bearer fixture-secret")
    );
    assert_eq!(
        captured.headers.get("content-type").map(String::as_str),
        Some("application/json")
    );
    let envelope: HttpContractGenerationRequest =
        serde_json::from_slice(&captured.body).expect("contract request envelope");
    assert_eq!(envelope.protocol, CONTRACT_GENERATION_PROVIDER_PROTOCOL);
    assert_eq!(envelope.request, request);
}

#[tokio::test]
async fn visual_grounding_adapter_preserves_digest_deadline_cost_and_identity() {
    let identity = grounding_identity();
    let request = grounding_request();
    let server = FixtureServer::start(vec![ResponseSpec::json(json!({
        "status": "success",
        "protocol": VISUAL_GROUNDING_PROVIDER_PROTOCOL,
        "response": {
            "identity": identity,
            "observation_id": request.observation_id,
            "screenshot_sha256": request.screenshot_sha256,
            "width": request.width,
            "height": request.height,
            "coordinate_space": "normalized",
            "candidates": [{
                "geometry": { "kind": "box", "x": 0.25, "y": 0.4, "width": 0.2, "height": 0.1 },
                "confidence": 0.92,
                "label": "Checkout"
            }],
            "usage": { "input_units": 1, "output_units": 1, "cost_microusd": 10 },
            "request_id": "ground-http-1"
        }
    }))])
    .await;
    let provider = HttpVisualGroundingProvider::new(
        grounding_identity(),
        HttpProviderConfig::new(server.endpoint.clone()),
    )
    .expect("HTTP grounding provider");

    let response = provider
        .locate(request.clone())
        .await
        .expect("grounding provider response");

    assert_eq!(response.identity, grounding_identity());
    assert_eq!(response.request_id.as_deref(), Some("ground-http-1"));
    let requests = server.finish().await;
    let envelope: HttpVisualGroundingRequest =
        serde_json::from_slice(&requests[0].body).expect("grounding request envelope");
    assert_eq!(envelope.protocol, VISUAL_GROUNDING_PROVIDER_PROTOCOL);
    assert_eq!(envelope.request.screenshot_path, "observation.png");
    assert_eq!(
        envelope.request.screenshot_sha256,
        request.screenshot_sha256
    );
    assert_eq!(envelope.image.screenshot_sha256, request.screenshot_sha256);
    assert_eq!(envelope.image.media_type, "image/png");
    assert_eq!(
        envelope.image.bytes_base64,
        "YTNzLXRlc3QgSFRUUCBncm91bmRpbmcgZml4dHVyZQ=="
    );
    let debug = format!("{:?}", envelope.image);
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains(&envelope.image.bytes_base64));
}

#[tokio::test]
async fn design_audit_adapter_sends_digest_bound_image_and_complete_page_context() {
    let request = design_audit_request();
    let identity = design_audit_identity();
    let server = FixtureServer::start(vec![ResponseSpec::json(json!({
        "status": "success",
        "protocol": DESIGN_AUDIT_PROVIDER_PROTOCOL,
        "response": {
            "identity": identity,
            "observation_id": request.observation_id,
            "surface_revision": request.surface_revision,
            "screenshot_sha256": request.screenshot_sha256,
            "page_context_sha256": request.page_context_sha256,
            "width": request.width,
            "height": request.height,
            "dimensions": request.dimensions,
            "findings": [],
            "usage": { "input_units": 3, "output_units": 1, "cost_microusd": 12 },
            "request_id": "design-http-1"
        }
    }))])
    .await;
    let provider = HttpDesignAuditProvider::new(
        design_audit_identity(),
        HttpProviderConfig::new(server.endpoint.clone()),
    )
    .expect("HTTP design-audit provider");

    let response = provider
        .audit(request.clone())
        .await
        .expect("design-audit provider response");

    assert_eq!(response.identity, design_audit_identity());
    assert_eq!(response.request_id.as_deref(), Some("design-http-1"));
    let requests = server.finish().await;
    let envelope: HttpDesignAuditRequest =
        serde_json::from_slice(&requests[0].body).expect("design-audit request envelope");
    assert_eq!(envelope.protocol, DESIGN_AUDIT_PROVIDER_PROTOCOL);
    assert_eq!(envelope.request.screenshot_path, "observation.png");
    assert_eq!(envelope.request.page_context, request.page_context);
    assert_eq!(
        envelope.request.page_context_sha256,
        request.page_context_sha256
    );
    assert_eq!(envelope.image.screenshot_sha256, request.screenshot_sha256);
    assert_eq!(envelope.image.media_type, "image/png");
    assert_eq!(
        envelope.image.bytes_base64,
        "YTNzLXRlc3QgSFRUUCBncm91bmRpbmcgZml4dHVyZQ=="
    );
    let debug = format!("{:?}", envelope.image);
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains(&envelope.image.bytes_base64));
}

#[tokio::test]
async fn design_audit_adapter_rejects_context_digest_drift_before_network_dispatch() {
    let server = FixtureServer::start(Vec::new()).await;
    let provider = HttpDesignAuditProvider::new(
        design_audit_identity(),
        HttpProviderConfig::new(server.endpoint.clone()),
    )
    .expect("HTTP design-audit provider");
    let mut request = design_audit_request();
    request.page_context_sha256 = format!("sha256:{}", "0".repeat(64));

    let error = provider
        .audit(request)
        .await
        .expect_err("context digest drift must fail before dispatch");

    assert_eq!(
        error.code(),
        "test.agent.design_audit.http.context_mismatch"
    );
    assert!(server.finish().await.is_empty());
}

#[tokio::test]
async fn llm_adapter_sends_the_schema_constrained_planner_request() {
    let server = FixtureServer::start(vec![ResponseSpec::json(json!({
        "status": "success",
        "protocol": LLM_PROVIDER_PROTOCOL,
        "response": {
            "decision": { "type": "finish", "summary": "The criterion is visible" },
            "usage": { "input_tokens": 11, "output_tokens": 5, "cost_microusd": 17 },
            "request_id": "llm-http-1"
        }
    }))])
    .await;
    let provider = HttpLlmProvider::new(
        LlmIdentity {
            provider: "fixture".to_string(),
            model: "planner".to_string(),
        },
        HttpProviderConfig::new(server.endpoint.clone())
            .with_authorization("Bearer planner-secret")
            .expect("authorization"),
    )
    .expect("HTTP LLM provider");
    let request = llm_request();

    let response = provider
        .complete(request.clone())
        .await
        .expect("LLM provider response");

    assert_eq!(response.request_id.as_deref(), Some("llm-http-1"));
    let requests = server.finish().await;
    let captured = &requests[0];
    assert_eq!(
        captured.headers.get("authorization").map(String::as_str),
        Some("Bearer planner-secret")
    );
    let envelope: HttpLlmCompletionRequest =
        serde_json::from_slice(&captured.body).expect("LLM request envelope");
    assert_eq!(envelope.protocol, LLM_PROVIDER_PROTOCOL);
    assert_eq!(envelope.request, request);
}

#[test]
fn endpoint_policy_is_https_by_default_and_allows_explicit_loopback_http() {
    for endpoint in [
        "https://models.example.test/v1/ground",
        "http://localhost:8000/v1/ground",
        "http://127.0.0.1:8000/v1/ground",
        "http://[::1]:8000/v1/ground",
    ] {
        assert!(
            endpoint.parse::<HttpProviderEndpoint>().is_ok(),
            "endpoint should be admitted: {endpoint}"
        );
    }

    for endpoint in [
        "http://models.example.test/v1/ground",
        "file:///tmp/provider.sock",
        "https://user:secret@models.example.test/v1/ground",
        "https://models.example.test/v1/ground?token=secret",
        "https://models.example.test/v1/ground#fragment",
    ] {
        assert!(
            endpoint.parse::<HttpProviderEndpoint>().is_err(),
            "endpoint should be rejected: {endpoint}"
        );
    }
}

#[test]
fn config_redacts_authorization_and_rejects_unsafe_or_unbounded_values() {
    let endpoint = "https://models.example.test/v1/provider"
        .parse::<HttpProviderEndpoint>()
        .unwrap();
    let config = HttpProviderConfig::new(endpoint.clone())
        .with_authorization("Bearer exact-secret")
        .unwrap();
    let debug = format!("{config:?}");
    assert!(!debug.contains("exact-secret"));
    assert!(debug.contains("<redacted>"));

    assert!(HttpProviderConfig::new(endpoint.clone())
        .with_authorization("Bearer value\r\ninjected: header")
        .is_err());
    assert!(HttpProviderConfig::new(endpoint.clone())
        .with_timeout(Duration::ZERO)
        .is_err());
    assert!(HttpProviderConfig::new(endpoint)
        .with_body_limits(0, 1)
        .is_err());
}

#[tokio::test]
async fn adapter_rejects_elapsed_deadlines_before_network_dispatch() {
    let server = FixtureServer::start(Vec::new()).await;
    let provider = grounding_provider(&server, None);
    let mut request = grounding_request();
    request.issued_at_unix_ms = 1;
    request.deadline_unix_ms = 2;

    let error = provider
        .locate(request)
        .await
        .expect_err("elapsed deadline");

    assert_eq!(error.code(), "test.agent.grounding.http.deadline_exceeded");
    assert!(!error.retryable());
    assert!(server.finish().await.is_empty());
}

#[tokio::test]
async fn adapter_rejects_redirects_wrong_media_types_protocols_and_oversized_bodies() {
    let redirect = FixtureServer::start(vec![ResponseSpec {
        status: 307,
        headers: vec![(
            "location".to_string(),
            "http://127.0.0.1:1/escape".to_string(),
        )],
        body: Vec::new(),
        delay: Duration::ZERO,
        include_content_length: true,
    }])
    .await;
    let error = grounding_provider(&redirect, None)
        .locate(grounding_request())
        .await
        .expect_err("redirect rejection");
    assert_eq!(error.code(), "test.agent.grounding.http.protocol_invalid");
    assert!(!error.retryable());
    redirect.finish().await;

    let wrong_media = FixtureServer::start(vec![ResponseSpec {
        status: 200,
        headers: vec![("content-type".to_string(), "text/plain".to_string())],
        body: b"not json".to_vec(),
        delay: Duration::ZERO,
        include_content_length: true,
    }])
    .await;
    let error = grounding_provider(&wrong_media, None)
        .locate(grounding_request())
        .await
        .expect_err("media-type rejection");
    assert_eq!(error.code(), "test.agent.grounding.http.protocol_invalid");
    wrong_media.finish().await;

    let wrong_protocol = FixtureServer::start(vec![ResponseSpec::json(json!({
        "status": "failure",
        "protocol": CONTRACT_GENERATION_PROVIDER_PROTOCOL,
        "error": { "code": "wrong", "message": "wrong protocol", "retryable": false }
    }))])
    .await;
    let error = grounding_provider(&wrong_protocol, None)
        .locate(grounding_request())
        .await
        .expect_err("protocol mismatch");
    assert_eq!(error.code(), "test.agent.grounding.http.protocol_invalid");
    wrong_protocol.finish().await;

    let oversized = FixtureServer::start(vec![ResponseSpec::json(json!({
        "status": "failure",
        "protocol": VISUAL_GROUNDING_PROVIDER_PROTOCOL,
        "error": { "code": "large", "message": "x".repeat(1_024), "retryable": false }
    }))])
    .await;
    let error = grounding_provider(&oversized, Some(128))
        .locate(grounding_request())
        .await
        .expect_err("bounded response");
    assert_eq!(error.code(), "test.agent.grounding.http.protocol_invalid");
    oversized.finish().await;

    let streamed = FixtureServer::start(vec![ResponseSpec {
        include_content_length: false,
        ..ResponseSpec::json(json!({
            "status": "failure",
            "protocol": VISUAL_GROUNDING_PROVIDER_PROTOCOL,
            "error": { "code": "large", "message": "x".repeat(1_024), "retryable": false }
        }))
    }])
    .await;
    let error = grounding_provider(&streamed, Some(128))
        .locate(grounding_request())
        .await
        .expect_err("bounded streamed response");
    assert_eq!(error.code(), "test.agent.grounding.http.protocol_invalid");
    streamed.finish().await;
}

#[tokio::test]
async fn adapter_rejects_oversized_requests_before_network_dispatch() {
    let server = FixtureServer::start(Vec::new()).await;
    let config = HttpProviderConfig::new(server.endpoint.clone())
        .with_body_limits(128, MAX_TEST_REQUEST_BYTES)
        .expect("body limits");
    let provider = HttpVisualGroundingProvider::new(grounding_identity(), config)
        .expect("HTTP grounding provider");
    let mut request = grounding_request();
    request.query = "x".repeat(1_024);

    let error = provider.locate(request).await.expect_err("bounded request");

    assert_eq!(error.code(), "test.agent.grounding.http.protocol_invalid");
    assert!(!error.retryable());
    assert!(server.finish().await.is_empty());
}

#[tokio::test]
async fn adapter_rejects_replaced_image_bytes_before_network_dispatch() {
    let server = FixtureServer::start(Vec::new()).await;
    let provider = grounding_provider(&server, None);
    let request = grounding_request();
    std::fs::write(&request.screenshot_path, b"replacement bytes")
        .expect("replace grounding image");

    let error = provider
        .locate(request)
        .await
        .expect_err("digest mismatch must fail closed");

    assert_eq!(error.code(), "test.agent.grounding.http.image_mismatch");
    assert!(!error.retryable());
    assert!(server.finish().await.is_empty());
}

#[tokio::test]
async fn adapter_rejects_unbounded_image_paths_before_filesystem_or_network_access() {
    let server = FixtureServer::start(Vec::new()).await;
    let provider = grounding_provider(&server, None);
    let mut request = grounding_request();
    request.screenshot_path = "x".repeat(16 * 1_024 + 1);

    let error = provider
        .locate(request)
        .await
        .expect_err("unbounded image path");

    assert_eq!(error.code(), "test.agent.grounding.http.image_invalid");
    assert!(server.finish().await.is_empty());
}

#[tokio::test]
async fn adapter_rejects_ambiguous_missing_and_unknown_response_statuses() {
    let server = FixtureServer::start(vec![
        ResponseSpec::json(json!({
            "status": "failure",
            "protocol": VISUAL_GROUNDING_PROVIDER_PROTOCOL,
            "response": {},
            "error": { "code": "ambiguous", "message": "ambiguous", "retryable": false }
        })),
        ResponseSpec::json(json!({
            "protocol": VISUAL_GROUNDING_PROVIDER_PROTOCOL,
            "error": { "code": "missing", "message": "missing", "retryable": false }
        })),
        ResponseSpec::json(json!({
            "status": "unknown",
            "protocol": VISUAL_GROUNDING_PROVIDER_PROTOCOL,
            "error": { "code": "unknown", "message": "unknown", "retryable": false }
        })),
        ResponseSpec::json(json!({
            "status": "failure",
            "protocol": VISUAL_GROUNDING_PROVIDER_PROTOCOL,
            "error": { "code": "INVALID", "message": "invalid code", "retryable": false }
        })),
        ResponseSpec::json(json!({
            "status": "failure",
            "protocol": VISUAL_GROUNDING_PROVIDER_PROTOCOL,
            "error": { "code": "empty_message", "message": "", "retryable": false }
        })),
        ResponseSpec::json(json!({
            "status": "failure",
            "protocol": VISUAL_GROUNDING_PROVIDER_PROTOCOL,
            "error": { "code": "large_message", "message": "x".repeat(65_537), "retryable": false }
        })),
    ])
    .await;
    let provider = grounding_provider(&server, None);

    for reason in [
        "ambiguous result",
        "missing status",
        "unknown status",
        "invalid error code",
        "empty error message",
        "oversized error message",
    ] {
        let error = provider
            .locate(grounding_request())
            .await
            .expect_err(reason);
        assert_eq!(error.code(), "test.agent.grounding.http.protocol_invalid");
        assert!(!error.retryable());
    }

    assert_eq!(server.finish().await.len(), 6);
}

#[tokio::test]
async fn adapter_maps_status_remote_errors_and_timeouts_without_exposing_secrets() {
    let unavailable = FixtureServer::start(vec![ResponseSpec {
        status: 503,
        headers: vec![("content-type".to_string(), "application/json".to_string())],
        body: b"{}".to_vec(),
        delay: Duration::ZERO,
        include_content_length: true,
    }])
    .await;
    let error = grounding_provider(&unavailable, None)
        .locate(grounding_request())
        .await
        .expect_err("server failure");
    assert_eq!(error.code(), "test.agent.grounding.http.transport_failed");
    assert!(error.retryable());
    unavailable.finish().await;

    let remote = FixtureServer::start(vec![ResponseSpec::json(json!({
        "status": "failure",
        "protocol": VISUAL_GROUNDING_PROVIDER_PROTOCOL,
        "error": { "code": "capacity_exhausted", "message": "queue is full", "retryable": true }
    }))])
    .await;
    let error = grounding_provider(&remote, None)
        .locate(grounding_request())
        .await
        .expect_err("remote provider error");
    assert_eq!(
        error.code(),
        "test.agent.grounding.http.remote.capacity_exhausted"
    );
    assert!(error.retryable());
    remote.finish().await;

    let secret = "secret-value-12345";
    let secret_echo = FixtureServer::start(vec![ResponseSpec::json(json!({
        "status": "failure",
        "protocol": VISUAL_GROUNDING_PROVIDER_PROTOCOL,
        "error": {
            "code": "rejected",
            "message": format!("credential Bearer {secret} was rejected"),
            "retryable": false
        }
    }))])
    .await;
    let config = HttpProviderConfig::new(secret_echo.endpoint.clone())
        .with_authorization(format!("Bearer {secret}"))
        .unwrap();
    let provider = HttpVisualGroundingProvider::new(grounding_identity(), config).unwrap();
    let error = provider
        .locate(grounding_request())
        .await
        .expect_err("secret-safe remote error");
    assert!(!error.message().contains(secret));
    assert!(error.message().contains("[REDACTED]"));
    secret_echo.finish().await;

    let timeout = FixtureServer::start(vec![ResponseSpec {
        delay: Duration::from_millis(250),
        ..ResponseSpec::json(json!({
            "status": "failure",
            "protocol": VISUAL_GROUNDING_PROVIDER_PROTOCOL,
            "error": { "code": "late", "message": "late", "retryable": true }
        }))
    }])
    .await;
    let config = HttpProviderConfig::new(timeout.endpoint.clone())
        .with_timeout(Duration::from_millis(30))
        .unwrap()
        .with_authorization("Bearer exact-secret")
        .unwrap();
    let provider = HttpVisualGroundingProvider::new(grounding_identity(), config).unwrap();
    let error = provider
        .locate(grounding_request())
        .await
        .expect_err("HTTP timeout");
    assert_eq!(error.code(), "test.agent.grounding.http.transport_failed");
    assert!(error.retryable());
    assert!(!error.message().contains("exact-secret"));
    timeout.finish().await;
}

fn grounding_provider(
    server: &FixtureServer,
    max_response_bytes: Option<usize>,
) -> HttpVisualGroundingProvider {
    let mut config = HttpProviderConfig::new(server.endpoint.clone());
    if let Some(max_response_bytes) = max_response_bytes {
        config = config
            .with_body_limits(MAX_TEST_REQUEST_BYTES, max_response_bytes)
            .expect("body limits");
    }
    HttpVisualGroundingProvider::new(grounding_identity(), config).expect("HTTP grounding provider")
}

fn contract_identity() -> ContractGenerationProviderIdentity {
    ContractGenerationProviderIdentity {
        provider: "fixture-http".to_string(),
        model: "contract-model".to_string(),
    }
}

fn grounding_identity() -> GroundingProviderIdentity {
    GroundingProviderIdentity {
        provider: "fixture-http".to_string(),
        model: "grounding-model".to_string(),
    }
}

fn design_audit_identity() -> DesignAuditProviderIdentity {
    DesignAuditProviderIdentity {
        provider: "fixture-http".to_string(),
        model: "design-audit-model".to_string(),
    }
}

fn contract_request() -> ContractGenerationProviderRequest {
    let issued_at_unix_ms = unix_ms();
    ContractGenerationProviderRequest {
        contract_name: "checkout".to_string(),
        context: ContractContext {
            mode: ContractMode::Operate,
            audience: vec!["customer".to_string()],
            primary_outcome: "place_order".to_string(),
        },
        sources: Vec::new(),
        issued_at_unix_ms,
        deadline_unix_ms: issued_at_unix_ms + 30_000,
        max_cost_microusd: 50_000,
    }
}

fn grounding_request() -> GroundingProviderRequest {
    let issued_at_unix_ms = unix_ms();
    let directory = tempfile::tempdir().expect("temporary grounding image directory");
    let screenshot_path = directory.path().join("observation.png");
    std::fs::write(&screenshot_path, GROUNDING_IMAGE_BYTES).expect("write grounding image fixture");
    GROUNDING_IMAGES.with(|images| images.borrow_mut().push(directory));
    GroundingProviderRequest {
        screenshot_path: screenshot_path.to_string_lossy().into_owned(),
        screenshot_sha256: format!("sha256:{:x}", Sha256::digest(GROUNDING_IMAGE_BYTES)),
        width: 1_440,
        height: 900,
        query: "Checkout button".to_string(),
        observation_id: 7,
        trigger: GroundingTrigger::ExplicitRequest,
        issued_at_unix_ms,
        deadline_unix_ms: issued_at_unix_ms + 15_000,
        max_cost_microusd: 10_000,
    }
}

fn design_audit_request() -> DesignAuditProviderRequest {
    let issued_at_unix_ms = unix_ms();
    let directory = tempfile::tempdir().expect("temporary design-audit image directory");
    let screenshot_path = directory.path().join("observation.png");
    std::fs::write(&screenshot_path, GROUNDING_IMAGE_BYTES)
        .expect("write design-audit image fixture");
    GROUNDING_IMAGES.with(|images| images.borrow_mut().push(directory));
    let page_context: PageContextSnapshot = serde_json::from_value(json!({
        "protocol": "a3s.test.page-context/1",
        "sdkVersion": "0.3.0",
        "revision": 42,
        "page": {
            "id": "checkout",
            "url": "https://example.test/checkout",
            "route": "/checkout",
            "title": "Checkout",
            "ready": true,
            "viewport": { "width": 1440.0, "height": 900.0, "dpr": 1.0 },
            "document": { "width": 1440.0, "height": 900.0 },
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
    }))
    .expect("page context");
    let page_context_sha256 = format!(
        "sha256:{:x}",
        Sha256::digest(serde_json::to_vec(&page_context).expect("page-context JSON"))
    );
    DesignAuditProviderRequest {
        screenshot_path: screenshot_path.to_string_lossy().into_owned(),
        screenshot_sha256: format!("sha256:{:x}", Sha256::digest(GROUNDING_IMAGE_BYTES)),
        page_context_sha256,
        width: 1_440,
        height: 900,
        observation_id: 7,
        surface_revision: 42,
        page_context,
        dimensions: vec![
            DesignAuditDimension::VisualHierarchy,
            DesignAuditDimension::SpacingRhythm,
        ],
        issued_at_unix_ms,
        deadline_unix_ms: issued_at_unix_ms + 30_000,
        max_cost_microusd: 10_000,
    }
}

fn llm_request() -> StructuredLlmRequest {
    StructuredLlmRequest {
        prompt_version: "a3s-test-agent/v2".to_string(),
        system_instruction: "Return exactly one typed decision".to_string(),
        context: PlannerContext {
            goal: AgentGoal {
                instruction: "Reach the confirmation page".to_string(),
                success_criteria: vec!["The confirmation is visible".to_string()],
            },
            surface: Surface::Web,
            turn: 1,
            observation: SurfaceObservation::new("Checkout form"),
            history: Vec::new(),
            remaining: RemainingBudget {
                turns: 4,
                tokens: 1_000,
                cost_microusd: 1_000,
                time_ms: 30_000,
            },
        },
        image_attachments: Vec::new(),
        response_schema: json!({ "type": "object" }),
    }
}

fn unix_ms() -> u64 {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Unix time")
            .as_millis(),
    )
    .expect("Unix millisecond range")
}

async fn read_request(stream: TcpStream, response: ResponseSpec) -> CapturedRequest {
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .await
        .expect("read request line");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().expect("HTTP method").to_string();
    let target = parts.next().expect("HTTP target").to_string();
    let mut headers = BTreeMap::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).await.expect("read header");
        if line == "\r\n" {
            break;
        }
        let (name, value) = line.trim_end().split_once(':').expect("HTTP header");
        headers.insert(name.to_ascii_lowercase(), value.trim().to_string());
    }
    let content_length = headers
        .get("content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .expect("content length");
    assert!(content_length <= MAX_TEST_REQUEST_BYTES);
    let mut body = vec![0; content_length];
    reader
        .read_exact(&mut body)
        .await
        .expect("read request body");

    if !response.delay.is_zero() {
        tokio::time::sleep(response.delay).await;
    }
    let reason = match response.status {
        200 => "OK",
        307 => "Temporary Redirect",
        503 => "Service Unavailable",
        _ => "Fixture",
    };
    let mut head = format!("HTTP/1.1 {} {reason}\r\n", response.status);
    for (name, value) in &response.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    if response.include_content_length {
        head.push_str(&format!("content-length: {}\r\n", response.body.len()));
    }
    head.push_str("connection: close\r\n\r\n");
    let stream = reader.get_mut();
    if stream.write_all(head.as_bytes()).await.is_ok() {
        let _ = stream.write_all(&response.body).await;
        let _ = stream.shutdown().await;
    }

    CapturedRequest {
        method,
        target,
        headers,
        body,
    }
}
