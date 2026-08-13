use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use a3s_test_agent::{
    ContractCandidate, ContractCandidateElement, ContractCandidateVariant, ContractConflictStatus,
    ContractGenerationError, ContractGenerationOptions, ContractGenerationProvider,
    ContractGenerationProviderIdentity, ContractGenerationProviderRequest,
    ContractGenerationProviderResponse, ContractGenerationReview, ContractGenerationService,
    ContractGenerationUsage, ContractReviewAction, ContractReviewDecision, ContractSource,
    ContractSourceKind, ContractSourceSpan, DesignCoordinateSpace, DesignElementRegion,
    GeneratedContractProvenance, ProductDecision, ProductDecisionStatus,
};
use a3s_test_core::{
    ContractCitation, ContractContext, ContractElement, ContractMode, ContractProvenanceStatus,
    ContractSeverity,
};
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;

struct FakeProvider {
    identity: ContractGenerationProviderIdentity,
    requests: Mutex<Vec<ContractGenerationProviderRequest>>,
    responses: Mutex<VecDeque<Result<ContractGenerationProviderResponse, ContractGenerationError>>>,
    delay: Duration,
    source_mutation: Option<(PathBuf, Vec<u8>)>,
}

impl FakeProvider {
    fn new(response: ContractGenerationProviderResponse) -> Self {
        Self {
            identity: identity(),
            requests: Mutex::new(Vec::new()),
            responses: Mutex::new([Ok(response)].into()),
            delay: Duration::ZERO,
            source_mutation: None,
        }
    }

    fn delayed(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    fn mutates_source(mut self, path: PathBuf, bytes: impl Into<Vec<u8>>) -> Self {
        self.source_mutation = Some((path, bytes.into()));
        self
    }
}

#[async_trait]
impl ContractGenerationProvider for FakeProvider {
    fn identity(&self) -> ContractGenerationProviderIdentity {
        self.identity.clone()
    }

    async fn generate(
        &self,
        request: ContractGenerationProviderRequest,
    ) -> Result<ContractGenerationProviderResponse, ContractGenerationError> {
        self.requests.lock().unwrap().push(request);
        if !self.delay.is_zero() {
            tokio::time::sleep(self.delay).await;
        }
        if let Some((path, bytes)) = &self.source_mutation {
            tokio::fs::write(path, bytes)
                .await
                .expect("mutate source during provider call");
        }
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .expect("scripted generation response")
    }
}

#[tokio::test]
async fn generates_prd_candidates_with_exact_quotes_uncertainty_and_open_decisions() {
    let files = SourceFiles::new();
    let prd = files.prd_source();
    let provider = Arc::new(FakeProvider::new(response(vec![prd_candidate(&prd)])));
    let service = service(provider.clone());

    let draft = service
        .generate(
            "checkout",
            context(),
            vec![prd.clone()],
            1_000,
            CancellationToken::new(),
        )
        .await
        .expect("PRD draft");

    assert_eq!(draft.provenance[0].sha256, prd.sha256);
    assert_eq!(draft.candidates[0].variants[0].elements[0].confidence, 78);
    assert_eq!(
        draft.candidates[0].variants[0].elements[0].source_spans[0].quote,
        "Customers can place an order using the Place order button."
    );
    assert_eq!(draft.unresolved_decisions[0].id, "receipt-format");
    assert!(draft.conflicts.is_empty());

    let requests = provider.requests.lock().unwrap();
    assert_eq!(requests[0].sources[0].sha256, prd.sha256);
    assert!(requests[0].deadline_unix_ms > requests[0].issued_at_unix_ms);
}

#[tokio::test]
async fn generates_design_candidates_with_digest_coordinates_and_hierarchy() {
    let files = SourceFiles::new();
    let design = files.design_source();
    let candidate = design_candidate(&design, "Confirm purchase", 82);
    let provider = Arc::new(FakeProvider::new(response(vec![candidate])));
    let service = service(provider);

    let draft = service
        .generate(
            "checkout",
            context(),
            vec![design.clone()],
            1_000,
            CancellationToken::new(),
        )
        .await
        .expect("design draft");

    let elements = &draft.candidates[0].variants[0].elements;
    let region = elements[1].design_region.as_ref().expect("design region");
    assert_eq!(region.coordinate_space, DesignCoordinateSpace::ImagePixels);
    assert_eq!(region.parent_candidate_id.as_deref(), Some("order-summary"));
    assert_eq!(region.source_id, design.id);
    assert_eq!(draft.provenance[0].sha256, design.sha256);
}

#[tokio::test]
async fn merges_sources_by_emitting_explicit_conflicts_instead_of_choosing() {
    let files = SourceFiles::new();
    let prd = files.prd_source();
    let design = files.design_source();
    let provider = Arc::new(FakeProvider::new(response(vec![
        prd_candidate(&prd),
        design_candidate(&design, "Confirm purchase", 82),
    ])));
    let service = service(provider);

    let draft = service
        .generate(
            "checkout",
            context(),
            vec![prd, design],
            2_000,
            CancellationToken::new(),
        )
        .await
        .expect("merged draft");

    assert_eq!(draft.conflicts.len(), 1);
    assert_eq!(
        draft.conflicts[0].status,
        ContractConflictStatus::Unresolved
    );
    assert_eq!(draft.conflicts[0].field, "name");
    assert_eq!(
        draft.conflicts[0].values,
        ["Place order", "Confirm purchase"]
    );
    assert_eq!(draft.provenance.len(), 2);
}

#[tokio::test]
async fn review_promotes_only_selected_candidates_and_keeps_sources_non_observed() {
    let files = SourceFiles::new();
    let prd = files.prd_source();
    let design = files.design_source();
    let provider = Arc::new(FakeProvider::new(response(vec![
        prd_candidate(&prd),
        design_candidate(&design, "Confirm purchase", 82),
    ])));
    let service = service(provider);
    let draft = service
        .generate(
            "checkout",
            context(),
            vec![prd, design],
            2_000,
            CancellationToken::new(),
        )
        .await
        .expect("merged draft");
    let conflict = draft.conflicts[0].clone();

    let reviewed = service
        .review(
            draft,
            ContractGenerationReview {
                reviewer: "product-owner@example.test".to_string(),
                decisions: vec![ContractReviewDecision {
                    candidate_id: "prd:desktop:place-order".to_string(),
                    action: ContractReviewAction::Approve,
                }],
                conflict_resolutions: vec![a3s_test_agent::ContractConflictResolution {
                    conflict_id: conflict.id,
                    selected_candidate_id: "prd:desktop:place-order".to_string(),
                    rationale: "Approved product terminology".to_string(),
                }],
            },
        )
        .expect("reviewed contract draft");

    assert!(reviewed
        .contract()
        .provenance()
        .iter()
        .all(|entry| entry.status == ContractProvenanceStatus::Reviewed));
    let element = &reviewed.contract().variants()[0].elements[0];
    assert_eq!(element.name.as_deref(), Some("Place order"));
    assert_eq!(element.severity, ContractSeverity::Blocking);
    assert_eq!(element.citations.len(), 1);
    assert_eq!(element.citations[0].provenance_id, "prd");
    assert_eq!(reviewed.generated().candidates.len(), 2);
    assert_eq!(reviewed.generated().conflicts.len(), 1);
    assert_eq!(
        reviewed.generated().conflicts[0].status,
        ContractConflictStatus::Resolved
    );
    assert!(reviewed.generated().candidates[1].variants[0].elements[1]
        .design_region
        .is_some());

    let admitted = reviewed
        .into_contract()
        .admit()
        .expect("reviewed contract admission");
    assert!(admitted.provenance.iter().all(|entry| matches!(
        entry.kind,
        a3s_test_core::ContractProvenanceKind::Prd | a3s_test_core::ContractProvenanceKind::Design
    )));
}

#[tokio::test]
async fn review_blocks_only_selected_candidates_with_unresolved_decisions() {
    let files = SourceFiles::new();
    let prd = files.prd_source();
    let design = files.design_source();
    let mut prd_candidate = prd_candidate(&prd);
    prd_candidate.variants[0].elements[0]
        .unresolved_decision_ids
        .push("receipt-format".to_string());
    let provider = Arc::new(FakeProvider::new(response(vec![
        prd_candidate,
        design_candidate(&design, "Place order", 82),
    ])));
    let service = service(provider);
    let draft = service
        .generate(
            "checkout",
            context(),
            vec![prd, design],
            2_000,
            CancellationToken::new(),
        )
        .await
        .expect("generated draft");

    let reviewed = service
        .review(
            draft.clone(),
            ContractGenerationReview {
                reviewer: "reviewer".to_string(),
                decisions: vec![
                    ContractReviewDecision {
                        candidate_id: "design:desktop:order-summary".to_string(),
                        action: ContractReviewAction::Approve,
                    },
                    ContractReviewDecision {
                        candidate_id: "design:desktop:place-order".to_string(),
                        action: ContractReviewAction::Approve,
                    },
                ],
                conflict_resolutions: Vec::new(),
            },
        )
        .expect("unrelated open decision must not block a design candidate");
    assert_eq!(reviewed.generated().unresolved_decisions.len(), 1);

    let error = service
        .review(
            draft,
            ContractGenerationReview {
                reviewer: "reviewer".to_string(),
                decisions: vec![ContractReviewDecision {
                    candidate_id: "prd:desktop:place-order".to_string(),
                    action: ContractReviewAction::Approve,
                }],
                conflict_resolutions: Vec::new(),
            },
        )
        .expect_err("selected unresolved decision must block review");
    assert_eq!(
        error.code(),
        "test.agent.contract_generation.decision_unresolved"
    );
}

#[tokio::test]
async fn review_fails_closed_for_unresolved_conflicts_or_decisions() {
    let files = SourceFiles::new();
    let prd = files.prd_source();
    let design = files.design_source();
    let provider = Arc::new(FakeProvider::new(response(vec![
        prd_candidate(&prd),
        design_candidate(&design, "Confirm purchase", 82),
    ])));
    let service = service(provider);
    let draft = service
        .generate(
            "checkout",
            context(),
            vec![prd, design],
            2_000,
            CancellationToken::new(),
        )
        .await
        .expect("merged draft");

    let error = service
        .review(
            draft,
            ContractGenerationReview {
                reviewer: "reviewer".to_string(),
                decisions: vec![ContractReviewDecision {
                    candidate_id: "prd:desktop:place-order".to_string(),
                    action: ContractReviewAction::Approve,
                }],
                conflict_resolutions: Vec::new(),
            },
        )
        .expect_err("unresolved conflict must fail closed");

    assert_eq!(
        error.code(),
        "test.agent.contract_generation.conflict_unresolved"
    );
}

#[tokio::test]
async fn generated_contract_reconciles_before_and_after_repair_with_stable_finding_ids() {
    let files = SourceFiles::new();
    let prd = files.prd_source();
    let provider = Arc::new(FakeProvider::new(response(vec![prd_candidate(&prd)])));
    let service = service(provider);
    let draft = service
        .generate(
            "checkout",
            context(),
            vec![prd],
            1_000,
            CancellationToken::new(),
        )
        .await
        .expect("generated draft");
    let reviewed = service
        .review(
            draft,
            ContractGenerationReview {
                reviewer: "reviewer".to_string(),
                decisions: vec![ContractReviewDecision {
                    candidate_id: "prd:desktop:place-order".to_string(),
                    action: ContractReviewAction::Approve,
                }],
                conflict_resolutions: Vec::new(),
            },
        )
        .expect("reviewed draft");
    let acl = reviewed.contract().to_acl();
    let contract = a3s_test_core::SurfaceContractDraft::from_acl(&acl)
        .expect("generated ACL")
        .admit()
        .expect("generated contract admission");

    let before = contract
        .reconcile("desktop", "ready", &surface_observation(Vec::new(), 1))
        .expect("before repair report");
    assert_eq!(before.findings.len(), 1);
    let finding_id = before.findings[0].id.clone();

    let repeated = contract
        .reconcile("desktop", "ready", &surface_observation(Vec::new(), 2))
        .expect("repeated report");
    assert_eq!(repeated.findings[0].id, finding_id);

    let after = contract
        .reconcile(
            "desktop",
            "ready",
            &surface_observation(
                vec![page_node(
                    "submit-node",
                    Some("place-order"),
                    Some("button"),
                    Some("Place order"),
                )],
                3,
            ),
        )
        .expect("after repair report");
    assert!(after.findings.is_empty());
    assert_eq!(after.matches[0].element_id, "place-order");
}

#[tokio::test]
async fn rejects_changed_sources_stale_provenance_and_unbounded_or_cancelled_calls() {
    let files = SourceFiles::new();
    let prd = files.prd_source();

    let mut stale = response(vec![prd_candidate(&prd)]);
    stale.source_digests[0].sha256 = format!("sha256:{}", "b".repeat(64));
    let stale_provider = Arc::new(FakeProvider::new(stale));
    let error = service(stale_provider)
        .generate(
            "checkout",
            context(),
            vec![prd.clone()],
            1_000,
            CancellationToken::new(),
        )
        .await
        .expect_err("stale provider binding");
    assert_eq!(
        error.code(),
        "test.agent.contract_generation.response_mismatch"
    );

    std::fs::write(&prd.path, b"changed after digest").expect("replace PRD");
    let changed_provider = Arc::new(FakeProvider::new(response(Vec::new())));
    let error = service(changed_provider.clone())
        .generate(
            "checkout",
            context(),
            vec![prd],
            1_000,
            CancellationToken::new(),
        )
        .await
        .expect_err("changed source");
    assert_eq!(
        error.code(),
        "test.agent.contract_generation.source_mismatch"
    );
    assert!(changed_provider.requests.lock().unwrap().is_empty());

    let changed_during_call = files.prd_source();
    let mutating_provider = Arc::new(
        FakeProvider::new(response(vec![prd_candidate(&changed_during_call)])).mutates_source(
            PathBuf::from(&changed_during_call.path),
            b"changed during provider call".to_vec(),
        ),
    );
    let error = service(mutating_provider.clone())
        .generate(
            "checkout",
            context(),
            vec![changed_during_call],
            1_000,
            CancellationToken::new(),
        )
        .await
        .expect_err("source changed during provider call");
    assert_eq!(
        error.code(),
        "test.agent.contract_generation.source_mismatch"
    );
    assert_eq!(mutating_provider.requests.lock().unwrap().len(), 1);

    let fresh = files.prd_source();
    let delayed = Arc::new(
        FakeProvider::new(response(vec![prd_candidate(&fresh)])).delayed(Duration::from_secs(1)),
    );
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let error = service(delayed)
        .generate("checkout", context(), vec![fresh], 1_000, cancellation)
        .await
        .expect_err("cancelled generation");
    assert_eq!(error.code(), "test.agent.contract_generation.cancelled");
}

#[tokio::test]
async fn rejects_duplicate_cyclic_or_pre_cited_provider_candidates() {
    let files = SourceFiles::new();
    let prd = files.prd_source();

    let mut duplicate_variant = prd_candidate(&prd);
    duplicate_variant
        .variants
        .push(duplicate_variant.variants[0].clone());
    assert_invalid_response(prd.clone(), duplicate_variant).await;

    let mut duplicate_element = prd_candidate(&prd);
    let repeated_element = duplicate_element.variants[0].elements[0].clone();
    duplicate_element.variants[0]
        .elements
        .push(repeated_element);
    assert_invalid_response(prd.clone(), duplicate_element).await;

    let mut pre_cited = prd_candidate(&prd);
    pre_cited.variants[0].elements[0]
        .element
        .citations
        .push(ContractCitation {
            id: "provider-citation".to_string(),
            provenance_id: "prd".to_string(),
            quote: "Place order".to_string(),
            start: 0,
            end: 11,
        });
    assert_invalid_response(prd, pre_cited).await;

    let design = files.design_source();
    let mut cyclic = design_candidate(&design, "Confirm purchase", 82);
    cyclic.variants[0].elements[0].element.parent = Some("place-order".to_string());
    cyclic.variants[0].elements[0]
        .design_region
        .as_mut()
        .expect("root design region")
        .parent_candidate_id = Some("place-order".to_string());
    assert_invalid_response(design.clone(), cyclic).await;

    let mut inconsistent = design_candidate(&design, "Confirm purchase", 82);
    inconsistent.variants[0].elements[1]
        .design_region
        .as_mut()
        .expect("child design region")
        .parent_candidate_id = None;
    assert_invalid_response(design, inconsistent).await;
}

async fn assert_invalid_response(source: ContractSource, candidate: ContractCandidate) {
    let provider = Arc::new(FakeProvider::new(response(vec![candidate])));
    let error = service(provider)
        .generate(
            "checkout",
            context(),
            vec![source],
            1_000,
            CancellationToken::new(),
        )
        .await
        .expect_err("invalid provider response");
    assert_eq!(
        error.code(),
        "test.agent.contract_generation.response_invalid"
    );
}

fn service(provider: Arc<FakeProvider>) -> ContractGenerationService {
    ContractGenerationService::new(
        provider,
        ContractGenerationOptions {
            timeout: Duration::from_millis(200),
            max_sources: 4,
            max_source_bytes: 1_024 * 1_024,
            max_candidates: 16,
            max_elements: 64,
            max_string_bytes: 4_096,
        },
    )
    .expect("valid generation service")
}

fn response(candidates: Vec<ContractCandidate>) -> ContractGenerationProviderResponse {
    let source_digests = candidates
        .iter()
        .map(|candidate| {
            let source = candidate_source(candidate);
            GeneratedContractProvenance {
                source_id: source.id.clone(),
                kind: source.kind,
                uri: source.uri.clone(),
                sha256: source.sha256.clone(),
            }
        })
        .collect();
    ContractGenerationProviderResponse {
        identity: identity(),
        source_digests,
        candidates,
        usage: ContractGenerationUsage {
            input_tokens: 100,
            output_tokens: 50,
            cost_microusd: 100,
        },
        request_id: Some("generation-fixture".to_string()),
    }
}

fn candidate_source(candidate: &ContractCandidate) -> ContractSource {
    SOURCE_REGISTRY.with(|registry| {
        registry
            .borrow()
            .iter()
            .find(|source| source.id == candidate.source_id)
            .expect("candidate source registry")
            .clone()
    })
}

thread_local! {
    static SOURCE_REGISTRY: std::cell::RefCell<Vec<ContractSource>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn prd_candidate(source: &ContractSource) -> ContractCandidate {
    let quote = "Customers can place an order using the Place order button.";
    let content = std::fs::read_to_string(&source.path).expect("PRD content");
    let start = content.find(quote).expect("quote offset") as u32;
    let end = start + quote.len() as u32;
    ContractCandidate {
        source_id: source.id.clone(),
        context: context(),
        variants: vec![ContractCandidateVariant {
            id: "desktop".to_string(),
            state: "ready".to_string(),
            min_width: Some(1024),
            max_width: None,
            theme: None,
            language: Some("en".to_string()),
            elements: vec![ContractCandidateElement {
                element: element("Place order"),
                confidence: 78,
                source_spans: vec![ContractSourceSpan {
                    source_id: source.id.clone(),
                    quote: quote.to_string(),
                    start,
                    end,
                }],
                design_region: None,
                unresolved_decision_ids: Vec::new(),
            }],
        }],
        unresolved_decisions: vec![ProductDecision {
            id: "receipt-format".to_string(),
            question: "Should the receipt be PDF or HTML?".to_string(),
            status: ProductDecisionStatus::Unresolved,
            source_spans: Vec::new(),
        }],
    }
}

fn design_candidate(
    source: &ContractSource,
    accessible_name: &str,
    confidence: u8,
) -> ContractCandidate {
    ContractCandidate {
        source_id: source.id.clone(),
        context: context(),
        variants: vec![ContractCandidateVariant {
            id: "desktop".to_string(),
            state: "ready".to_string(),
            min_width: Some(1024),
            max_width: None,
            theme: None,
            language: Some("en".to_string()),
            elements: vec![
                ContractCandidateElement {
                    element: ContractElement {
                        id: "order-summary".to_string(),
                        test_id: None,
                        component_id: None,
                        role: Some("region".to_string()),
                        name: Some("Order summary".to_string()),
                        description: None,
                        required: true,
                        visible: Some(true),
                        enabled: None,
                        checked: None,
                        selected: None,
                        expanded: None,
                        readonly: None,
                        form_required: None,
                        invalid: None,
                        parent: None,
                        severity: ContractSeverity::Important,
                        citations: Vec::new(),
                    },
                    confidence: 90,
                    source_spans: Vec::new(),
                    design_region: Some(DesignElementRegion {
                        source_id: source.id.clone(),
                        coordinate_space: DesignCoordinateSpace::ImagePixels,
                        x: 100.0,
                        y: 100.0,
                        width: 900.0,
                        height: 500.0,
                        parent_candidate_id: None,
                    }),
                    unresolved_decision_ids: Vec::new(),
                },
                ContractCandidateElement {
                    element: ContractElement {
                        parent: Some("order-summary".to_string()),
                        ..element(accessible_name)
                    },
                    confidence,
                    source_spans: Vec::new(),
                    design_region: Some(DesignElementRegion {
                        source_id: source.id.clone(),
                        coordinate_space: DesignCoordinateSpace::ImagePixels,
                        x: 760.0,
                        y: 520.0,
                        width: 180.0,
                        height: 48.0,
                        parent_candidate_id: Some("order-summary".to_string()),
                    }),
                    unresolved_decision_ids: Vec::new(),
                },
            ],
        }],
        unresolved_decisions: Vec::new(),
    }
}

fn element(name: &str) -> ContractElement {
    ContractElement {
        id: "place-order".to_string(),
        test_id: Some("place-order".to_string()),
        component_id: None,
        role: Some("button".to_string()),
        name: Some(name.to_string()),
        description: None,
        required: true,
        visible: Some(true),
        enabled: Some(true),
        checked: None,
        selected: None,
        expanded: None,
        readonly: None,
        form_required: None,
        invalid: None,
        parent: None,
        severity: ContractSeverity::Blocking,
        citations: Vec::new(),
    }
}

fn context() -> ContractContext {
    ContractContext {
        mode: ContractMode::Operate,
        audience: vec!["customer".to_string()],
        primary_outcome: "place_order".to_string(),
    }
}

fn surface_observation(
    nodes: Vec<a3s_test_core::PageContextNode>,
    revision: u64,
) -> a3s_test_core::SurfaceObservation {
    use a3s_test_core::{
        PageContextObservation, PageContextPage, PageContextPoint, PageContextSize,
        PageContextSnapshot, PageContextTheme, PageContextViewport,
    };
    let snapshot = PageContextSnapshot {
        protocol: Some("a3s.test.page-context/1".to_string()),
        sdk_version: Some("0.7.0".to_string()),
        revision: Some(revision),
        page: Some(PageContextPage {
            id: "checkout".to_string(),
            url: "https://example.test/checkout".to_string(),
            route: "/checkout".to_string(),
            title: "Checkout".to_string(),
            ready: true,
            viewport: PageContextViewport {
                width: 1280.0,
                height: 800.0,
                dpr: 1.0,
                visual: None,
            },
            document: PageContextSize {
                width: 1280.0,
                height: 800.0,
            },
            scroll: PageContextPoint { x: 0.0, y: 0.0 },
            language: "en".to_string(),
            theme: PageContextTheme::Light,
        }),
        components: Vec::new(),
        nodes,
        facts: serde_json::Map::new(),
        removed_node_ids: Vec::new(),
        truncated: false,
        next_cursor: None,
    };
    a3s_test_core::SurfaceObservation::new("contract observation")
        .with_page_context(PageContextObservation::from_snapshot(snapshot))
}

fn page_node(
    id: &str,
    test_id: Option<&str>,
    role: Option<&str>,
    name: Option<&str>,
) -> a3s_test_core::PageContextNode {
    a3s_test_core::PageContextNode {
        id: id.to_string(),
        r#ref: None,
        parent_id: None,
        component_id: None,
        tag: "button".to_string(),
        role: role.map(str::to_string),
        name: name.map(str::to_string),
        text: None,
        description: None,
        test_id: test_id.map(str::to_string),
        geometry: None,
        state: a3s_test_core::PageContextNodeState {
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
        locators: Vec::new(),
        classes: None,
        attributes: None,
        computed_styles: None,
    }
}

fn identity() -> ContractGenerationProviderIdentity {
    ContractGenerationProviderIdentity {
        provider: "fixture-provider".to_string(),
        model: "fixture-model".to_string(),
    }
}

struct SourceFiles {
    directory: TempDir,
}

impl SourceFiles {
    fn new() -> Self {
        Self {
            directory: tempfile::tempdir().expect("source fixture directory"),
        }
    }

    fn prd_source(&self) -> ContractSource {
        self.source(
            "prd",
            ContractSourceKind::Prd,
            "requirements.md",
            b"# Checkout\n\nCustomers can place an order using the Place order button.\nThe receipt format is undecided.\n",
            None,
        )
    }

    fn design_source(&self) -> ContractSource {
        self.source(
            "design",
            ContractSourceKind::Design,
            "checkout.png",
            b"fixture image bytes",
            Some((1200, 800)),
        )
    }

    fn source(
        &self,
        id: &str,
        kind: ContractSourceKind,
        filename: &str,
        bytes: &[u8],
        dimensions: Option<(u32, u32)>,
    ) -> ContractSource {
        let path = self.directory.path().join(filename);
        std::fs::write(&path, bytes).expect("source fixture");
        let source = ContractSource {
            id: id.to_string(),
            kind,
            uri: format!("./{filename}"),
            path: path.to_string_lossy().into_owned(),
            sha256: format!("sha256:{:x}", Sha256::digest(bytes)),
            media_type: (kind == ContractSourceKind::Design).then(|| "image/png".to_string()),
            width: dimensions.map(|value| value.0),
            height: dimensions.map(|value| value.1),
        };
        SOURCE_REGISTRY.with(|registry| registry.borrow_mut().push(source.clone()));
        source
    }
}
