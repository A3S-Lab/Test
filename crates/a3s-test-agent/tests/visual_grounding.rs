use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use a3s_test_agent::{
    GroundingCandidate, GroundingCandidateGeometry, GroundingCoordinateSpace, GroundingError,
    GroundingOptions, GroundingPageContext, GroundingProviderIdentity, GroundingProviderRequest,
    GroundingProviderResponse, GroundingRequest, GroundingResult, GroundingTrigger, GroundingUsage,
    SemanticFallbackReason, VisualGroundingProvider, VisualGroundingService,
};
use a3s_test_core::{
    PageContextGeometry, PageContextLocator, PageContextNode, PageContextNodeState,
    PageContextPage, PageContextPoint, PageContextPosition, PageContextRect, PageContextSize,
    PageContextSnapshot, PageContextTheme, PageContextViewport, PageContextVisualViewport,
};
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;

const SCREENSHOT_BYTES: &[u8] = b"a3s-test visual grounding fixture";

thread_local! {
    static SCREENSHOTS: std::cell::RefCell<Vec<tempfile::TempDir>> = const { std::cell::RefCell::new(Vec::new()) };
}

struct FakeProvider {
    identity: GroundingProviderIdentity,
    requests: Mutex<Vec<GroundingProviderRequest>>,
    responses: Mutex<VecDeque<Result<GroundingProviderResponse, GroundingError>>>,
    delay: Duration,
}

impl FakeProvider {
    fn new(
        responses: impl IntoIterator<Item = Result<GroundingProviderResponse, GroundingError>>,
    ) -> Self {
        Self {
            identity: GroundingProviderIdentity {
                provider: "fixture-provider".to_string(),
                model: "fixture-model".to_string(),
            },
            requests: Mutex::new(Vec::new()),
            responses: Mutex::new(responses.into_iter().collect()),
            delay: Duration::ZERO,
        }
    }

    fn delayed(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }
}

#[async_trait]
impl VisualGroundingProvider for FakeProvider {
    fn identity(&self) -> GroundingProviderIdentity {
        self.identity.clone()
    }

    async fn locate(
        &self,
        request: GroundingProviderRequest,
    ) -> Result<GroundingProviderResponse, GroundingError> {
        self.requests.lock().unwrap().push(request);
        if !self.delay.is_zero() {
            tokio::time::sleep(self.delay).await;
        }
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .expect("scripted grounding response")
    }
}

#[tokio::test]
async fn admits_explicit_canvas_and_image_only_grounding() {
    for trigger in [
        GroundingTrigger::ExplicitRequest,
        GroundingTrigger::SemanticFallback {
            reason: SemanticFallbackReason::Canvas,
        },
        GroundingTrigger::SemanticFallback {
            reason: SemanticFallbackReason::ImageOnly,
        },
    ] {
        let provider = Arc::new(FakeProvider::new([Ok(response(vec![point(
            120.0, 80.0, 0.96,
        )]))]));
        let service = service(provider.clone(), Duration::from_secs(1));

        let result = service
            .ground(
                request(trigger),
                Some(context(&snapshot(Vec::new()))),
                CancellationToken::new(),
            )
            .await
            .expect("grounding result");

        assert!(matches!(result, GroundingResult::ImageBound { .. }));
        let requests = provider.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].screenshot_sha256, digest());
        assert_eq!(requests[0].observation_id, 17);
        assert_eq!(requests[0].max_cost_microusd, 1_000);
        assert!(requests[0].deadline_unix_ms > requests[0].issued_at_unix_ms);
    }
}

#[tokio::test]
async fn admits_remote_desktop_and_design_reference_fallbacks() {
    for reason in [
        SemanticFallbackReason::RemoteDesktop,
        SemanticFallbackReason::DesignReference,
    ] {
        let provider = Arc::new(FakeProvider::new([Ok(response(vec![box_candidate(
            100.0, 60.0, 80.0, 40.0, 0.91,
        )]))]));
        let service = service(provider, Duration::from_secs(1));
        let result = service
            .ground(
                request(GroundingTrigger::SemanticFallback { reason }),
                None,
                CancellationToken::new(),
            )
            .await
            .expect("image-bound fallback");

        let GroundingResult::ImageBound { candidates, .. } = result else {
            panic!("a source without page context must remain image-bound");
        };
        assert_eq!(candidates.len(), 1);
    }
}

#[tokio::test]
async fn uniquely_hit_tests_screenshot_candidates_into_current_semantic_nodes() {
    let provider = Arc::new(FakeProvider::new([Ok(response(vec![point(
        320.0, 180.0, 0.97,
    )]))]));
    let service = service(provider, Duration::from_secs(1));
    let mut page = snapshot(vec![node("pay", "@c1", 300.0, 160.0, 100.0, 50.0)]);
    page.page = Some(page_for_viewport(640.0, 360.0));

    let result = service
        .ground(
            request(GroundingTrigger::SemanticFallback {
                reason: SemanticFallbackReason::NoSemanticMatch,
            }),
            Some(context(&page)),
            CancellationToken::new(),
        )
        .await
        .expect("semantic grounding");

    let GroundingResult::Semantic { matches, .. } = result else {
        panic!("unique hit must upgrade to the current semantic node");
    };
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].node_id, "pay");
    assert_eq!(matches[0].reference.as_deref(), Some("@c1"));
    assert_eq!(matches[0].confidence, 0.97);
}

#[tokio::test]
async fn maps_scaled_screenshot_pixels_and_normalized_coordinates_to_the_visual_viewport() {
    for (coordinate_space, candidate) in [
        (
            GroundingCoordinateSpace::ScreenshotPixels,
            point(400.0, 200.0, 0.95),
        ),
        (GroundingCoordinateSpace::Normalized, point(0.5, 0.5, 0.95)),
    ] {
        let mut provider_response = response(vec![candidate]);
        provider_response.coordinate_space = coordinate_space;
        provider_response.width = 800;
        provider_response.height = 400;
        let provider = Arc::new(FakeProvider::new([Ok(provider_response)]));
        let service = service(provider, Duration::from_secs(1));
        let mut grounding_request = request(GroundingTrigger::ExplicitRequest);
        grounding_request.width = 800;
        grounding_request.height = 400;
        let mut page = snapshot(vec![node("scaled-target", "@c1", 490.0, 290.0, 20.0, 20.0)]);
        page.page = Some(page_with_visual_viewport());

        let result = service
            .ground(
                grounding_request,
                Some(context(&page)),
                CancellationToken::new(),
            )
            .await
            .expect("scaled grounding result");

        let GroundingResult::Semantic { matches, .. } = result else {
            panic!("scaled coordinate must hit the semantic target");
        };
        assert_eq!(matches[0].viewport_point.x, 500.0);
        assert_eq!(matches[0].viewport_point.y, 300.0);
    }
}

#[tokio::test]
async fn never_guesses_when_hit_testing_is_ambiguous() {
    let provider = Arc::new(FakeProvider::new([Ok(response(vec![point(
        320.0, 180.0, 0.97,
    )]))]));
    let service = service(provider, Duration::from_secs(1));
    let mut page = snapshot(vec![
        node("outer", "@c1", 250.0, 120.0, 200.0, 140.0),
        node("inner", "@c2", 300.0, 160.0, 100.0, 50.0),
    ]);
    page.page = Some(page_for_viewport(640.0, 360.0));

    let result = service
        .ground(
            request(GroundingTrigger::ExplicitRequest),
            Some(context(&page)),
            CancellationToken::new(),
        )
        .await
        .expect("advisory grounding result");

    let GroundingResult::ImageBound { candidates, .. } = result else {
        panic!("ambiguous hits must remain image-bound");
    };
    assert_eq!(candidates[0].semantic_node_ids, ["inner", "outer"]);
}

#[tokio::test]
async fn rejects_stale_or_mismatched_provider_provenance() {
    let cases = [
        response_for(
            18,
            &digest(),
            640,
            360,
            GroundingProviderIdentity {
                provider: "fixture-provider".to_string(),
                model: "fixture-model".to_string(),
            },
        ),
        response_for(
            17,
            "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            640,
            360,
            GroundingProviderIdentity {
                provider: "fixture-provider".to_string(),
                model: "fixture-model".to_string(),
            },
        ),
        response_for(
            17,
            &digest(),
            641,
            360,
            GroundingProviderIdentity {
                provider: "fixture-provider".to_string(),
                model: "fixture-model".to_string(),
            },
        ),
        response_for(
            17,
            &digest(),
            640,
            360,
            GroundingProviderIdentity {
                provider: "other-provider".to_string(),
                model: "fixture-model".to_string(),
            },
        ),
    ];

    for provider_response in cases {
        let provider = Arc::new(FakeProvider::new([Ok(provider_response)]));
        let service = service(provider, Duration::from_secs(1));
        let error = service
            .ground(
                request(GroundingTrigger::ExplicitRequest),
                None,
                CancellationToken::new(),
            )
            .await
            .expect_err("mismatched provenance must fail closed");
        assert_eq!(error.code(), "test.agent.grounding.response_mismatch");
    }
}

#[tokio::test]
async fn rejects_page_context_from_another_observation() {
    let provider = Arc::new(FakeProvider::new([Ok(response(vec![point(
        320.0, 180.0, 0.97,
    )]))]));
    let service = service(provider, Duration::from_secs(1));
    let stale_page = snapshot(vec![node("stale-target", "@c1", 300.0, 160.0, 100.0, 50.0)]);
    let error = service
        .ground(
            request(GroundingTrigger::ExplicitRequest),
            Some(GroundingPageContext {
                observation_id: 16,
                surface_revision: 17,
                snapshot: &stale_page,
            }),
            CancellationToken::new(),
        )
        .await
        .expect_err("stale page context must not authorize a semantic target");

    assert_eq!(error.code(), "test.agent.grounding.context_mismatch");
}

#[tokio::test]
async fn keeps_agent_observation_ids_independent_from_surface_revisions() {
    let provider = Arc::new(FakeProvider::new([Ok(response(vec![point(
        320.0, 180.0, 0.97,
    )]))]));
    let service = service(provider, Duration::from_secs(1));
    let mut page = snapshot(vec![node(
        "current-target",
        "@c1",
        300.0,
        160.0,
        100.0,
        50.0,
    )]);
    page.revision = Some(42);
    page.page = Some(page_for_viewport(640.0, 360.0));

    let result = service
        .ground(
            request(GroundingTrigger::ExplicitRequest),
            Some(GroundingPageContext {
                observation_id: 17,
                surface_revision: 42,
                snapshot: &page,
            }),
            CancellationToken::new(),
        )
        .await
        .expect("independent surface revision");

    let GroundingResult::Semantic { matches, .. } = result else {
        panic!("current semantic target expected");
    };
    assert_eq!(matches[0].reference.as_deref(), Some("@c1"));
}

#[tokio::test]
async fn rejects_truncated_or_revision_mismatched_page_context() {
    for (page, surface_revision) in [
        (
            {
                let mut page = snapshot(Vec::new());
                page.truncated = true;
                page
            },
            17,
        ),
        (
            {
                let mut page = snapshot(Vec::new());
                page.revision = Some(16);
                page
            },
            17,
        ),
    ] {
        let provider = Arc::new(FakeProvider::new([]));
        let service = service(provider.clone(), Duration::from_secs(1));
        let error = service
            .ground(
                request(GroundingTrigger::ExplicitRequest),
                Some(GroundingPageContext {
                    observation_id: 17,
                    surface_revision,
                    snapshot: &page,
                }),
                CancellationToken::new(),
            )
            .await
            .expect_err("incomplete context must not be hit-tested");

        assert_eq!(error.code(), "test.agent.grounding.context_incomplete");
        assert!(provider.requests.lock().unwrap().is_empty());
    }
}

#[tokio::test]
async fn rejects_out_of_bounds_non_finite_and_oversized_results() {
    let cases = [
        vec![point(640.0, 20.0, 0.9)],
        vec![point(f64::NAN, 20.0, 0.9)],
        (0..65)
            .map(|index| point(index as f64, 20.0, 0.9))
            .collect(),
    ];

    for candidates in cases {
        let provider = Arc::new(FakeProvider::new([Ok(response(candidates))]));
        let service = service(provider, Duration::from_secs(1));
        let error = service
            .ground(
                request(GroundingTrigger::ExplicitRequest),
                None,
                CancellationToken::new(),
            )
            .await
            .expect_err("invalid candidates must fail closed");
        assert_eq!(error.code(), "test.agent.grounding.response_invalid");
    }
}

#[tokio::test]
async fn fails_closed_on_timeout_cancellation_and_cost_overrun() {
    let delayed = Arc::new(
        FakeProvider::new([Ok(response(vec![point(1.0, 1.0, 0.9)]))])
            .delayed(Duration::from_millis(100)),
    );
    let timeout_error = service(delayed, Duration::from_millis(10))
        .ground(
            request(GroundingTrigger::ExplicitRequest),
            None,
            CancellationToken::new(),
        )
        .await
        .expect_err("provider timeout");
    assert_eq!(timeout_error.code(), "test.agent.grounding.timeout");

    let cancelled_provider = Arc::new(
        FakeProvider::new([Ok(response(vec![point(1.0, 1.0, 0.9)]))])
            .delayed(Duration::from_secs(1)),
    );
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let cancelled_error = service(cancelled_provider, Duration::from_secs(2))
        .ground(
            request(GroundingTrigger::ExplicitRequest),
            None,
            cancellation,
        )
        .await
        .expect_err("provider cancellation");
    assert_eq!(cancelled_error.code(), "test.agent.grounding.cancelled");

    let mut expensive = response(vec![point(1.0, 1.0, 0.9)]);
    expensive.usage.cost_microusd = 1_001;
    let budget_error = service(
        Arc::new(FakeProvider::new([Ok(expensive)])),
        Duration::from_secs(1),
    )
    .ground(
        request(GroundingTrigger::ExplicitRequest),
        None,
        CancellationToken::new(),
    )
    .await
    .expect_err("cost overrun");
    assert_eq!(
        budget_error.code(),
        "test.agent.grounding.cost_budget_exceeded"
    );
}

#[test]
fn rejects_untyped_fallbacks_and_invalid_requests_before_provider_execution() {
    let provider = Arc::new(FakeProvider::new([]));
    let invalid_options = GroundingOptions {
        timeout: Duration::ZERO,
        max_candidates: 64,
        max_query_bytes: 4_096,
        max_label_bytes: 1_024,
    };
    let error = VisualGroundingService::new(provider.clone(), invalid_options)
        .expect_err("zero timeout must be rejected");
    assert_eq!(error.code(), "test.agent.grounding.config_invalid");

    let valid = service(provider, Duration::from_secs(1));
    let mut invalid_request = request(GroundingTrigger::ExplicitRequest);
    invalid_request.screenshot_sha256 = "aaaaaaaa".to_string();
    let error = valid
        .validate_request(&invalid_request)
        .expect_err("unprefixed digest must be rejected");
    assert_eq!(error.code(), "test.agent.grounding.request_invalid");
}

#[tokio::test]
async fn rejects_screenshot_bytes_that_do_not_match_the_admitted_digest() {
    let provider = Arc::new(FakeProvider::new([]));
    let service = service(provider.clone(), Duration::from_secs(1));
    let screenshot_path = screenshot_path(b"different screenshot bytes");
    let mut grounding_request = request(GroundingTrigger::ExplicitRequest);
    grounding_request.screenshot_path = screenshot_path;

    let error = service
        .ground(grounding_request, None, CancellationToken::new())
        .await
        .expect_err("digest mismatch must fail before provider execution");

    assert_eq!(error.code(), "test.agent.grounding.screenshot_mismatch");
    assert!(provider.requests.lock().unwrap().is_empty());
}

fn service(provider: Arc<FakeProvider>, timeout: Duration) -> VisualGroundingService {
    VisualGroundingService::new(
        provider,
        GroundingOptions {
            timeout,
            max_candidates: 64,
            max_query_bytes: 4_096,
            max_label_bytes: 1_024,
        },
    )
    .expect("valid grounding service")
}

fn request(trigger: GroundingTrigger) -> GroundingRequest {
    GroundingRequest {
        screenshot_path: screenshot_path(SCREENSHOT_BYTES),
        screenshot_sha256: digest(),
        width: 640,
        height: 360,
        query: "Locate the primary action".to_string(),
        observation_id: 17,
        trigger,
        max_cost_microusd: 1_000,
    }
}

fn response(candidates: Vec<GroundingCandidate>) -> GroundingProviderResponse {
    let mut response = response_for(
        17,
        &digest(),
        640,
        360,
        GroundingProviderIdentity {
            provider: "fixture-provider".to_string(),
            model: "fixture-model".to_string(),
        },
    );
    response.candidates = candidates;
    response
}

fn digest() -> String {
    format!("sha256:{:x}", Sha256::digest(SCREENSHOT_BYTES))
}

fn screenshot_path(bytes: &[u8]) -> String {
    let directory = tempfile::tempdir().expect("temporary screenshot directory");
    let path = directory.path().join("observation.png");
    std::fs::write(&path, bytes).expect("write grounding screenshot fixture");
    SCREENSHOTS.with(|screenshots| screenshots.borrow_mut().push(directory));
    path.to_string_lossy().into_owned()
}

fn response_for(
    observation_id: u64,
    digest: &str,
    width: u32,
    height: u32,
    identity: GroundingProviderIdentity,
) -> GroundingProviderResponse {
    GroundingProviderResponse {
        identity,
        observation_id,
        screenshot_sha256: digest.to_string(),
        width,
        height,
        coordinate_space: GroundingCoordinateSpace::ScreenshotPixels,
        candidates: vec![point(120.0, 80.0, 0.9)],
        usage: GroundingUsage {
            input_units: 1,
            output_units: 1,
            cost_microusd: 10,
        },
        request_id: Some("fixture-request".to_string()),
    }
}

fn point(x: f64, y: f64, confidence: f64) -> GroundingCandidate {
    GroundingCandidate {
        geometry: GroundingCandidateGeometry::Point { x, y },
        confidence,
        label: Some("primary action".to_string()),
    }
}

fn box_candidate(x: f64, y: f64, width: f64, height: f64, confidence: f64) -> GroundingCandidate {
    GroundingCandidate {
        geometry: GroundingCandidateGeometry::Box {
            x,
            y,
            width,
            height,
        },
        confidence,
        label: None,
    }
}

fn snapshot(nodes: Vec<PageContextNode>) -> PageContextSnapshot {
    PageContextSnapshot {
        protocol: Some("a3s.test.page-context/1".to_string()),
        sdk_version: Some("0.2.0".to_string()),
        revision: Some(17),
        page: None,
        components: Vec::new(),
        nodes,
        facts: serde_json::Map::new(),
        removed_node_ids: Vec::new(),
        truncated: false,
        next_cursor: None,
    }
}

fn context(snapshot: &PageContextSnapshot) -> GroundingPageContext<'_> {
    GroundingPageContext {
        observation_id: 17,
        surface_revision: snapshot.revision.unwrap_or_default(),
        snapshot,
    }
}

fn page_with_visual_viewport() -> PageContextPage {
    PageContextPage {
        id: "fixture".to_string(),
        url: "http://127.0.0.1:3000/fixture".to_string(),
        route: "/fixture".to_string(),
        title: "Fixture".to_string(),
        ready: true,
        viewport: PageContextViewport {
            width: 800.0,
            height: 600.0,
            dpr: 2.0,
            visual: Some(PageContextVisualViewport {
                x: 100.0,
                y: 200.0,
                width: 800.0,
                height: 200.0,
                scale: 2.0,
            }),
        },
        document: PageContextSize {
            width: 800.0,
            height: 1200.0,
        },
        scroll: PageContextPoint { x: 0.0, y: 200.0 },
        language: "en".to_string(),
        theme: PageContextTheme::Light,
    }
}

fn page_for_viewport(width: f64, height: f64) -> PageContextPage {
    PageContextPage {
        id: "fixture".to_string(),
        url: "http://127.0.0.1:3000/fixture".to_string(),
        route: "/fixture".to_string(),
        title: "Fixture".to_string(),
        ready: true,
        viewport: PageContextViewport {
            width,
            height,
            dpr: 1.0,
            visual: None,
        },
        document: PageContextSize { width, height },
        scroll: PageContextPoint { x: 0.0, y: 0.0 },
        language: "en".to_string(),
        theme: PageContextTheme::Light,
    }
}

fn node(id: &str, reference: &str, x: f64, y: f64, width: f64, height: f64) -> PageContextNode {
    let viewport = PageContextRect {
        x,
        y,
        width,
        height,
    };
    PageContextNode {
        id: id.to_string(),
        r#ref: Some(reference.to_string()),
        parent_id: None,
        component_id: None,
        tag: "button".to_string(),
        role: Some("button".to_string()),
        name: Some("Pay".to_string()),
        text: Some("Pay".to_string()),
        description: None,
        test_id: Some(id.to_string()),
        geometry: Some(PageContextGeometry {
            viewport: viewport.clone(),
            document: viewport.clone(),
            normalized: viewport,
            visible_ratio: 1.0,
            occluded: false,
            position: PageContextPosition::Static,
            transformed: false,
            scroll_container_node_id: None,
        }),
        state: PageContextNodeState {
            visible: true,
            disabled: Some(false),
            checked: None,
            selected: None,
            expanded: None,
            focused: None,
            readonly: None,
            required: None,
            invalid: None,
        },
        locators: vec![PageContextLocator::TestId {
            value: id.to_string(),
        }],
        classes: None,
        attributes: None,
        computed_styles: None,
    }
}
