use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use a3s_test_agent::{
    DesignAuditDimension, DesignAuditError, DesignAuditFinding, DesignAuditNormalizedRegion,
    DesignAuditOptions, DesignAuditPriority, DesignAuditProvider, DesignAuditProviderIdentity,
    DesignAuditProviderRequest, DesignAuditProviderResponse, DesignAuditRequest,
    DesignAuditService, DesignAuditTarget, DesignAuditUsage, DESIGN_AUDIT_REPORT_PROTOCOL,
};
use a3s_test_core::{
    PageContextGeometry, PageContextLocator, PageContextNode, PageContextNodeState,
    PageContextPage, PageContextPoint, PageContextPosition, PageContextRect, PageContextSize,
    PageContextSnapshot, PageContextTheme, PageContextViewport,
};
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;

const SCREENSHOT_BYTES: &[u8] = b"a3s-test design audit fixture";

#[derive(Clone, Copy)]
enum Mutation {
    None,
    Observation,
    ContextDigest,
    Cost,
}

struct FakeProvider {
    identity: DesignAuditProviderIdentity,
    findings: Vec<DesignAuditFinding>,
    requests: Mutex<Vec<DesignAuditProviderRequest>>,
    mutation: Mutation,
    delay: Duration,
}

impl FakeProvider {
    fn new(findings: Vec<DesignAuditFinding>) -> Self {
        Self {
            identity: identity(),
            findings,
            requests: Mutex::new(Vec::new()),
            mutation: Mutation::None,
            delay: Duration::ZERO,
        }
    }

    fn mutated(mut self, mutation: Mutation) -> Self {
        self.mutation = mutation;
        self
    }

    fn delayed(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }
}

#[async_trait]
impl DesignAuditProvider for FakeProvider {
    fn identity(&self) -> DesignAuditProviderIdentity {
        self.identity.clone()
    }

    async fn audit(
        &self,
        request: DesignAuditProviderRequest,
    ) -> Result<DesignAuditProviderResponse, DesignAuditError> {
        self.requests.lock().unwrap().push(request.clone());
        if !self.delay.is_zero() {
            tokio::time::sleep(self.delay).await;
        }
        let mut response = DesignAuditProviderResponse {
            identity: self.identity.clone(),
            observation_id: request.observation_id,
            surface_revision: request.surface_revision,
            screenshot_sha256: request.screenshot_sha256.clone(),
            page_context_sha256: request.page_context_sha256.clone(),
            width: request.width,
            height: request.height,
            dimensions: request.dimensions.clone(),
            findings: self.findings.clone(),
            usage: DesignAuditUsage {
                input_units: 10,
                output_units: 3,
                cost_microusd: 25,
            },
            request_id: Some("audit-request-1".to_string()),
        };
        match self.mutation {
            Mutation::None => {}
            Mutation::Observation => response.observation_id += 1,
            Mutation::ContextDigest => response.page_context_sha256 = digest(b"other context"),
            Mutation::Cost => response.usage.cost_microusd = request.max_cost_microusd + 1,
        }
        Ok(response)
    }
}

#[tokio::test]
async fn admits_revision_bound_advice_without_verdict_or_repair_authority() {
    let provider = Arc::new(FakeProvider::new(vec![node_finding("hierarchy", "hero")]));
    let service = service(provider.clone(), Duration::from_secs(1));
    let fixture = screenshot();
    let report = service
        .audit(request(&fixture), CancellationToken::new())
        .await
        .expect("admitted design audit");

    assert_eq!(report.protocol, DESIGN_AUDIT_REPORT_PROTOCOL);
    assert_eq!(report.provenance.identity, identity());
    assert_eq!(report.provenance.observation_id, 17);
    assert_eq!(report.provenance.surface_revision, 42);
    assert_eq!(report.provenance.screenshot_sha256, screenshot_digest());
    assert_eq!(
        report.findings[0].target,
        DesignAuditTarget::Node {
            node_id: "hero".into()
        }
    );
    let value = serde_json::to_value(&report).expect("report JSON");
    assert_eq!(value["provenance"]["authority"], "advisory");
    assert!(value.get("outcome").is_none());
    assert!(value.get("verdict").is_none());
    assert!(value.get("repair").is_none());

    let requests = provider.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].page_context.revision, Some(42));
    assert_eq!(requests[0].dimensions, dimensions());
    assert_eq!(
        requests[0].page_context_sha256,
        digest(&serde_json::to_vec(&requests[0].page_context).unwrap())
    );
    assert!(requests[0].deadline_unix_ms > requests[0].issued_at_unix_ms);
}

#[tokio::test]
async fn accepts_page_and_normalized_region_targets() {
    let findings = vec![
        DesignAuditFinding {
            target: DesignAuditTarget::Page,
            ..node_finding("page", "hero")
        },
        DesignAuditFinding {
            id: "region".to_string(),
            target: DesignAuditTarget::Region {
                region: DesignAuditNormalizedRegion {
                    x: 0.1,
                    y: 0.2,
                    width: 0.4,
                    height: 0.3,
                },
            },
            ..node_finding("unused", "hero")
        },
    ];
    let provider = Arc::new(FakeProvider::new(findings));
    let fixture = screenshot();
    let report = service(provider, Duration::from_secs(1))
        .audit(request(&fixture), CancellationToken::new())
        .await
        .expect("page and region targets");

    assert_eq!(report.findings.len(), 2);
}

#[tokio::test]
async fn rejects_mismatched_provenance_and_cost() {
    for (mutation, code) in [
        (
            Mutation::Observation,
            "test.agent.design_audit.response_mismatch",
        ),
        (
            Mutation::ContextDigest,
            "test.agent.design_audit.response_mismatch",
        ),
        (
            Mutation::Cost,
            "test.agent.design_audit.cost_budget_exceeded",
        ),
    ] {
        let provider =
            Arc::new(FakeProvider::new(vec![node_finding("hierarchy", "hero")]).mutated(mutation));
        let fixture = screenshot();
        let error = service(provider, Duration::from_secs(1))
            .audit(request(&fixture), CancellationToken::new())
            .await
            .expect_err("mismatched response must fail closed");
        assert_eq!(error.code(), code);
    }
}

#[tokio::test]
async fn rejects_unknown_nodes_invalid_regions_duplicate_ids_and_unrequested_dimensions() {
    let invalid_findings = [
        vec![node_finding("unknown", "missing")],
        vec![DesignAuditFinding {
            target: DesignAuditTarget::Region {
                region: DesignAuditNormalizedRegion {
                    x: 0.9,
                    y: 0.2,
                    width: 0.2,
                    height: 0.3,
                },
            },
            ..node_finding("region", "hero")
        }],
        vec![
            node_finding("duplicate", "hero"),
            node_finding("duplicate", "hero"),
        ],
        vec![DesignAuditFinding {
            dimension: DesignAuditDimension::Typography,
            ..node_finding("dimension", "hero")
        }],
    ];

    for findings in invalid_findings {
        let fixture = screenshot();
        let error = service(
            Arc::new(FakeProvider::new(findings)),
            Duration::from_secs(1),
        )
        .audit(request(&fixture), CancellationToken::new())
        .await
        .expect_err("invalid finding must fail closed");
        assert_eq!(error.code(), "test.agent.design_audit.response_invalid");
    }
}

#[tokio::test]
async fn rejects_incomplete_stale_or_oversized_context_before_provider_dispatch() {
    let cases = [
        {
            let mut request = base_request();
            request.page_context.truncated = true;
            request
        },
        {
            let mut request = base_request();
            request.page_context.revision = Some(41);
            request
        },
        {
            let mut request = base_request();
            request.page_context.nodes[0].id.clear();
            request
        },
    ];
    for mut request in cases {
        let fixture = screenshot();
        request.screenshot_path = fixture.path().to_string_lossy().into_owned();
        let provider = Arc::new(FakeProvider::new(Vec::new()));
        let error = service(provider.clone(), Duration::from_secs(1))
            .audit(request, CancellationToken::new())
            .await
            .expect_err("invalid context must fail before dispatch");
        assert!(error.code().starts_with("test.agent.design_audit.context_"));
        assert!(provider.requests.lock().unwrap().is_empty());
    }
}

#[tokio::test]
async fn rejects_replaced_screenshot_before_provider_dispatch() {
    let fixture = screenshot();
    let provider = Arc::new(FakeProvider::new(Vec::new()));
    let request = request(&fixture);
    std::fs::write(fixture.path(), b"replacement").expect("replace screenshot");

    let error = service(provider.clone(), Duration::from_secs(1))
        .audit(request, CancellationToken::new())
        .await
        .expect_err("digest drift must fail closed");

    assert_eq!(error.code(), "test.agent.design_audit.screenshot_mismatch");
    assert!(provider.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn enforces_timeout_and_cancellation() {
    let fixture = screenshot();
    let provider = Arc::new(FakeProvider::new(Vec::new()).delayed(Duration::from_millis(50)));
    let error = service(provider, Duration::from_millis(5))
        .audit(request(&fixture), CancellationToken::new())
        .await
        .expect_err("provider timeout");
    assert_eq!(error.code(), "test.agent.design_audit.timeout");
    assert!(error.retryable());

    let fixture = screenshot();
    let provider = Arc::new(FakeProvider::new(Vec::new()).delayed(Duration::from_secs(1)));
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let error = service(provider, Duration::from_secs(2))
        .audit(request(&fixture), cancellation)
        .await
        .expect_err("cancelled audit");
    assert_eq!(error.code(), "test.agent.design_audit.cancelled");
}

fn service(provider: Arc<FakeProvider>, timeout: Duration) -> DesignAuditService {
    DesignAuditService::new(
        provider,
        DesignAuditOptions {
            timeout,
            ..DesignAuditOptions::default()
        },
    )
    .expect("design audit service")
}

fn base_request() -> DesignAuditRequest {
    DesignAuditRequest {
        screenshot_path: String::new(),
        screenshot_sha256: screenshot_digest(),
        width: 1280,
        height: 720,
        observation_id: 17,
        surface_revision: 42,
        page_context: page_context(),
        dimensions: dimensions(),
        max_cost_microusd: 100,
    }
}

fn request(fixture: &tempfile::NamedTempFile) -> DesignAuditRequest {
    DesignAuditRequest {
        screenshot_path: fixture.path().to_string_lossy().into_owned(),
        ..base_request()
    }
}

fn dimensions() -> Vec<DesignAuditDimension> {
    vec![
        DesignAuditDimension::VisualHierarchy,
        DesignAuditDimension::SpacingRhythm,
    ]
}

fn identity() -> DesignAuditProviderIdentity {
    DesignAuditProviderIdentity {
        provider: "fixture-provider".to_string(),
        model: "fixture-design-model".to_string(),
    }
}

fn node_finding(id: &str, node_id: &str) -> DesignAuditFinding {
    DesignAuditFinding {
        id: id.to_string(),
        dimension: DesignAuditDimension::VisualHierarchy,
        priority: DesignAuditPriority::High,
        summary: "The primary action lacks emphasis".to_string(),
        rationale: "Competing elements have equal visual weight".to_string(),
        recommendation: "Increase the primary action's contrast and spacing".to_string(),
        confidence: 91,
        target: DesignAuditTarget::Node {
            node_id: node_id.to_string(),
        },
    }
}

fn page_context() -> PageContextSnapshot {
    let viewport = PageContextRect {
        x: 80.0,
        y: 120.0,
        width: 640.0,
        height: 240.0,
    };
    PageContextSnapshot {
        protocol: Some("a3s.test.page-context/1".to_string()),
        sdk_version: Some("0.2.0".to_string()),
        revision: Some(42),
        page: Some(PageContextPage {
            id: "checkout".to_string(),
            url: "https://example.test/checkout".to_string(),
            route: "/checkout".to_string(),
            title: "Checkout".to_string(),
            ready: true,
            viewport: PageContextViewport {
                width: 1280.0,
                height: 720.0,
                dpr: 1.0,
                visual: None,
            },
            document: PageContextSize {
                width: 1280.0,
                height: 1200.0,
            },
            scroll: PageContextPoint { x: 0.0, y: 0.0 },
            language: "en".to_string(),
            theme: PageContextTheme::Light,
        }),
        components: Vec::new(),
        nodes: vec![PageContextNode {
            id: "hero".to_string(),
            r#ref: None,
            parent_id: None,
            component_id: None,
            tag: "section".to_string(),
            role: Some("region".to_string()),
            name: Some("Checkout summary".to_string()),
            text: Some("Complete your order".to_string()),
            description: None,
            test_id: Some("checkout-hero".to_string()),
            geometry: Some(PageContextGeometry {
                viewport: viewport.clone(),
                document: viewport,
                normalized: PageContextRect {
                    x: 0.0625,
                    y: 1.0 / 6.0,
                    width: 0.5,
                    height: 1.0 / 3.0,
                },
                visible_ratio: 1.0,
                occluded: false,
                position: PageContextPosition::Static,
                transformed: false,
                scroll_container_node_id: None,
            }),
            state: PageContextNodeState {
                visible: true,
                disabled: None,
                checked: None,
                selected: None,
                expanded: None,
                focused: None,
                readonly: None,
                required: None,
                invalid: None,
            },
            locators: vec![PageContextLocator::TestId {
                value: "checkout-hero".to_string(),
            }],
            classes: Some(vec!["checkout-hero".to_string()]),
            attributes: Some(serde_json::Map::from_iter([(
                "aria-label".to_string(),
                serde_json::Value::String("Checkout summary".to_string()),
            )])),
            computed_styles: Some(serde_json::Map::from_iter(HashMap::from([(
                "font-size".to_string(),
                serde_json::Value::String("16px".to_string()),
            )]))),
            source_mapping: None,
        }],
        facts: serde_json::Map::new(),
        ui: None,
        delta: None,
        removed_node_ids: Vec::new(),
        truncated: false,
        next_cursor: None,
    }
}

fn screenshot() -> tempfile::NamedTempFile {
    let file = tempfile::NamedTempFile::new().expect("screenshot fixture");
    std::fs::write(file.path(), SCREENSHOT_BYTES).expect("write screenshot fixture");
    file
}

fn screenshot_digest() -> String {
    digest(SCREENSHOT_BYTES)
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}
