use a3s_test_core::{
    ContractCitation, ContractOutcome, ContractSeverity, PageContextNode, PageContextNodeState,
    PageContextObservation, PageContextPage, PageContextPoint, PageContextSize,
    PageContextSnapshot, PageContextTheme, PageContextViewport, SurfaceContractDraft,
    SurfaceObservation,
};
use serde_json::{json, Map, Value};

const CONTRACT: &str = r#"
surface_contract "checkout" {
    version = 1

    context {
        mode = "operate"
        audience = ["returning_customer"]
        primary_outcome = "place_order"
    }

    provenance "product-requirements" {
        kind = "prd"
        uri = "./prd/checkout.md"
        digest = "sha256:56ea72bad66743f4dadee9515096bb39a200bf9ca8d5669293f41912c55ec14e"
        status = "reviewed"
        confidence = 100
    }

    variant "desktop-ready" {
        state = "ready"
        min_width = 1024
        theme = "light"

        element "order-summary" {
            role = "region"
            name = "Order summary"
            required = true
        }

        element "place-order" {
            test_id = "place-order"
            role = "button"
            name = "Place order"
            required = true
            visible = true
            enabled = true
            parent = "order-summary"
            severity = "blocking"
        }

        element "receipt-link" {
            role = "link"
            name = "Download receipt"
            required = false
        }
    }
}
"#;

#[test]
fn round_trips_optional_source_citations_without_changing_acl_authority() {
    let source = CONTRACT.replace(
        "severity = \"blocking\"",
        r#"severity = "blocking"

            citation "prd-place-order" {
                provenance = "product-requirements"
                quote = "The customer can place the order."
                start = 128
                end = 161
            }"#,
    );
    let draft = SurfaceContractDraft::from_acl(&source).expect("citation syntax");
    let element = &draft.variants()[0].elements[1];
    assert_eq!(
        element.citations,
        [ContractCitation {
            id: "prd-place-order".to_string(),
            provenance_id: "product-requirements".to_string(),
            quote: "The customer can place the order.".to_string(),
            start: 128,
            end: 161,
        }]
    );

    let generated = draft.to_acl();
    let reparsed = SurfaceContractDraft::from_acl(&generated).expect("generated citation ACL");
    assert_eq!(reparsed, draft);
    assert_eq!(reparsed.to_acl(), generated);

    let contract = reparsed.admit().expect("reviewed cited contract");
    assert_eq!(contract.variants[0].elements[1].citations.len(), 1);
}

#[test]
fn preserves_citation_quote_utf8_bytes_including_edge_whitespace() {
    let quote = " é ";
    let source = CONTRACT.replace(
        "severity = \"blocking\"",
        &format!(
            r#"severity = "blocking"

            citation "prd-whitespace" {{
                provenance = "product-requirements"
                quote = "{quote}"
                start = 0
                end = {}
            }}"#,
            quote.len()
        ),
    );

    let draft = SurfaceContractDraft::from_acl(&source).expect("citation syntax");
    let citation = &draft.variants()[0].elements[1].citations[0];
    assert_eq!(citation.quote, quote);
    assert_eq!(citation.quote.as_bytes(), quote.as_bytes());

    let generated = draft.to_acl();
    let reparsed = SurfaceContractDraft::from_acl(&generated).expect("generated citation ACL");
    assert_eq!(reparsed.variants()[0].elements[1].citations[0].quote, quote);
}

#[test]
fn checked_constructor_rejects_invalid_and_duplicate_identifiers() {
    let parsed = SurfaceContractDraft::from_acl(CONTRACT).expect("contract syntax");
    let invalid = SurfaceContractDraft::new(
        "checkout invalid",
        1,
        parsed.context().clone(),
        parsed.provenance().to_vec(),
        parsed.variants().to_vec(),
    )
    .expect_err("invalid contract identifier");
    assert_eq!(invalid.code(), "test.contract.identifier_invalid");

    let variant = parsed.variants()[0].clone();
    let duplicate = SurfaceContractDraft::new(
        "checkout",
        1,
        parsed.context().clone(),
        parsed.provenance().to_vec(),
        vec![variant.clone(), variant],
    )
    .expect_err("duplicate variant identifier");
    assert_eq!(duplicate.code(), "test.contract.variant_duplicate");
}

#[test]
fn rejects_citations_with_unknown_provenance_or_invalid_spans() {
    for citation in [
        r#"citation "unknown" {
                provenance = "missing"
                quote = "Place order"
                start = 0
                end = 11
            }"#,
        r#"citation "invalid-span" {
                provenance = "product-requirements"
                quote = "Place order"
                start = 11
                end = 11
            }"#,
    ] {
        let source = CONTRACT.replace(
            "severity = \"blocking\"",
            &format!("severity = \"blocking\"\n\n            {citation}"),
        );
        let error = SurfaceContractDraft::from_acl(&source)
            .expect("citation syntax")
            .admit()
            .expect_err("invalid citation must fail admission");
        assert!(matches!(
            error.code(),
            "test.contract.citation_provenance_unknown" | "test.contract.citation_span_invalid"
        ));
    }
}

#[test]
fn parses_and_admits_a_reviewed_surface_contract() {
    let draft = SurfaceContractDraft::from_acl(CONTRACT).expect("contract syntax");

    assert_eq!(draft.name(), "checkout");
    assert_eq!(draft.variants().len(), 1);
    assert_eq!(draft.provenance().len(), 1);

    let contract = draft.admit().expect("reviewed contract");
    let variant = contract.variant("desktop-ready").expect("variant");
    assert_eq!(variant.state, "ready");
    assert_eq!(variant.elements.len(), 3);
    assert_eq!(variant.elements[1].severity, ContractSeverity::Blocking);
}

#[test]
fn rejects_blocking_contracts_without_reviewed_provenance() {
    let source = CONTRACT.replace("status = \"reviewed\"", "status = \"draft\"");
    let error = SurfaceContractDraft::from_acl(&source)
        .expect("draft syntax")
        .admit()
        .expect_err("draft provenance cannot authorize blocking checks");

    assert_eq!(error.code(), "test.contract.provenance_unreviewed");
    assert_eq!(error.path(), "surface_contract.checkout.provenance");
}

#[test]
fn rejects_malformed_or_uncertain_blocking_provenance() {
    let malformed = CONTRACT.replace(
        "sha256:56ea72bad66743f4dadee9515096bb39a200bf9ca8d5669293f41912c55ec14e",
        "sha256:not-a-digest",
    );
    let malformed_error = SurfaceContractDraft::from_acl(&malformed)
        .expect("contract syntax")
        .admit()
        .expect_err("malformed digest must not be admitted");
    assert_eq!(
        malformed_error.code(),
        "test.contract.provenance_digest_invalid"
    );

    let uncertain = CONTRACT.replace("confidence = 100", "confidence = 99");
    let uncertain_error = SurfaceContractDraft::from_acl(&uncertain)
        .expect("contract syntax")
        .admit()
        .expect_err("uncertain provenance cannot authorize blocking checks");
    assert_eq!(
        uncertain_error.code(),
        "test.contract.provenance_unreviewed"
    );
}

#[test]
fn rejects_unknown_element_references_during_admission() {
    let source = CONTRACT.replace(
        "parent = \"order-summary\"",
        "parent = \"missing-container\"",
    );
    let error = SurfaceContractDraft::from_acl(&source)
        .expect("contract syntax")
        .admit()
        .expect_err("parent must resolve inside the variant");

    assert_eq!(error.code(), "test.contract.element_reference_unknown");
    assert_eq!(
        error.path(),
        "surface_contract.checkout.variant.desktop-ready.element.place-order.parent"
    );
}

#[test]
fn reconciles_explicit_identity_semantics_state_and_parentage() {
    let contract = SurfaceContractDraft::from_acl(CONTRACT)
        .expect("contract syntax")
        .admit()
        .expect("contract admission");
    let observation = observation(vec![
        node(
            "summary-node",
            None,
            None,
            Some("region"),
            Some("Order summary"),
            false,
        ),
        node(
            "submit-node",
            Some("summary-node"),
            Some("place-order"),
            Some("link"),
            Some("Place order"),
            true,
        ),
    ]);

    let report = contract
        .reconcile("desktop-ready", "ready", &observation)
        .expect("known contract selection");

    assert_eq!(report.outcome, ContractOutcome::Failed);
    assert_eq!(report.matches.len(), 2);
    assert_eq!(report.findings.len(), 2);
    assert_eq!(report.findings[0].rule_id, "contract.element.enabled");
    assert_eq!(
        report.findings[0].element_id.as_deref(),
        Some("place-order")
    );
    assert_eq!(
        report.findings[0].observed_node_id.as_deref(),
        Some("submit-node")
    );
    assert_eq!(report.findings[1].rule_id, "contract.element.role");
    assert_eq!(report.findings[1].expected, json!("button"));
    assert_eq!(report.findings[1].actual, json!("link"));
    assert!(report
        .findings
        .iter()
        .all(|finding| finding.id.starts_with("finding:") && finding.id.len() == 72));
    assert_ne!(report.findings[0].id, report.findings[1].id);
}

#[test]
fn advisory_findings_are_reported_without_failing_the_contract() {
    let source = CONTRACT.replace("severity = \"blocking\"", "severity = \"important\"");
    let contract = SurfaceContractDraft::from_acl(&source)
        .expect("contract syntax")
        .admit()
        .expect("contract admission");
    let report = contract
        .reconcile(
            "desktop-ready",
            "ready",
            &observation(vec![
                node(
                    "summary-node",
                    None,
                    None,
                    Some("region"),
                    Some("Order summary"),
                    false,
                ),
                node(
                    "submit-node",
                    Some("summary-node"),
                    Some("place-order"),
                    Some("link"),
                    Some("Place order"),
                    false,
                ),
            ]),
        )
        .expect("known contract selection");

    assert_eq!(report.outcome, ContractOutcome::Passed);
    assert_eq!(report.findings.len(), 1);
    assert_eq!(report.findings[0].severity, ContractSeverity::Important);
    assert_eq!(report.findings[0].rule_id, "contract.element.role");
}

#[test]
fn finding_ids_are_stable_across_observation_revisions_and_actual_values() {
    let contract = SurfaceContractDraft::from_acl(CONTRACT)
        .expect("contract syntax")
        .admit()
        .expect("contract admission");
    let first = contract
        .reconcile(
            "desktop-ready",
            "ready",
            &observation(vec![
                node(
                    "summary-a",
                    None,
                    None,
                    Some("region"),
                    Some("Order summary"),
                    false,
                ),
                node(
                    "submit-a",
                    Some("summary-a"),
                    Some("place-order"),
                    Some("link"),
                    Some("Place order"),
                    false,
                ),
            ]),
        )
        .expect("first report");
    let second = contract
        .reconcile(
            "desktop-ready",
            "ready",
            &observation(vec![
                node(
                    "summary-b",
                    None,
                    None,
                    Some("region"),
                    Some("Order summary"),
                    false,
                ),
                node(
                    "submit-b",
                    Some("summary-b"),
                    Some("place-order"),
                    Some("checkbox"),
                    Some("Place order"),
                    false,
                ),
            ]),
        )
        .expect("second report");

    let first_role = first
        .findings
        .iter()
        .find(|finding| finding.rule_id == "contract.element.role")
        .expect("first role finding");
    let second_role = second
        .findings
        .iter()
        .find(|finding| finding.rule_id == "contract.element.role")
        .expect("second role finding");
    assert_eq!(first_role.id, second_role.id);
    assert_ne!(first_role.actual, second_role.actual);
    assert_ne!(first_role.observed_node_id, second_role.observed_node_id);
}

#[test]
fn missing_optional_elements_do_not_fail_the_contract() {
    let contract = SurfaceContractDraft::from_acl(CONTRACT)
        .expect("contract syntax")
        .admit()
        .expect("contract admission");
    let observation = observation(vec![
        node(
            "summary-node",
            None,
            None,
            Some("region"),
            Some("Order summary"),
            false,
        ),
        node(
            "submit-node",
            Some("summary-node"),
            Some("place-order"),
            Some("button"),
            Some("Place order"),
            false,
        ),
    ]);

    let report = contract
        .reconcile("desktop-ready", "ready", &observation)
        .expect("known contract selection");

    assert_eq!(report.outcome, ContractOutcome::Passed);
    assert!(report.findings.is_empty());
}

#[test]
fn truncated_or_absent_page_context_is_inconclusive() {
    let contract = SurfaceContractDraft::from_acl(CONTRACT)
        .expect("contract syntax")
        .admit()
        .expect("contract admission");
    let mut truncated = observation(Vec::new());
    truncated
        .page_context
        .as_mut()
        .and_then(|context| context.snapshot.as_mut())
        .expect("snapshot")
        .truncated = true;

    let truncated_report = contract
        .reconcile("desktop-ready", "ready", &truncated)
        .expect("known contract selection");
    assert_eq!(truncated_report.outcome, ContractOutcome::Inconclusive);
    assert_eq!(
        truncated_report.findings[0].rule_id,
        "contract.observation.truncated"
    );

    let absent = SurfaceObservation::new("browser accessibility snapshot")
        .with_data(json!({ "snapshot": "@e1 [button] Place order" }))
        .with_page_context(PageContextObservation::absent());
    let absent_report = contract
        .reconcile("desktop-ready", "ready", &absent)
        .expect("known contract selection");
    assert_eq!(absent_report.outcome, ContractOutcome::Inconclusive);
    assert_eq!(
        absent_report.findings[0].rule_id,
        "contract.observation.page_context_required"
    );
}

#[test]
fn rejects_unknown_variant_and_state_as_specification_errors() {
    let contract = SurfaceContractDraft::from_acl(CONTRACT)
        .expect("contract syntax")
        .admit()
        .expect("contract admission");
    let observation = observation(Vec::new());

    let variant_error = contract
        .reconcile("mobile", "ready", &observation)
        .expect_err("unknown variant");
    assert_eq!(variant_error.code(), "test.contract.variant_unknown");

    let state_error = contract
        .reconcile("desktop-ready", "loading", &observation)
        .expect_err("wrong state");
    assert_eq!(state_error.code(), "test.contract.state_mismatch");
}

fn observation(nodes: Vec<PageContextNode>) -> SurfaceObservation {
    let snapshot = PageContextSnapshot {
        protocol: Some("a3s.test.page-context/1".to_string()),
        sdk_version: Some("0.7.0".to_string()),
        revision: Some(7),
        page: Some(PageContextPage {
            id: "checkout".to_string(),
            url: "https://example.test/checkout".to_string(),
            route: "/checkout".to_string(),
            title: "Checkout".to_string(),
            ready: true,
            viewport: PageContextViewport {
                width: 1280.0,
                height: 800.0,
                dpr: 2.0,
                visual: None,
            },
            document: PageContextSize {
                width: 1280.0,
                height: 1400.0,
            },
            scroll: PageContextPoint { x: 0.0, y: 0.0 },
            language: "en".to_string(),
            theme: PageContextTheme::Light,
        }),
        components: Vec::new(),
        nodes,
        facts: Map::new(),
        ui: None,
        removed_node_ids: Vec::new(),
        truncated: false,
        next_cursor: None,
    };
    SurfaceObservation::new("atomic accessibility and page-context observation")
        .with_data(Value::Null)
        .with_page_context(PageContextObservation::from_snapshot(snapshot))
}

fn node(
    id: &str,
    parent_id: Option<&str>,
    test_id: Option<&str>,
    role: Option<&str>,
    name: Option<&str>,
    disabled: bool,
) -> PageContextNode {
    PageContextNode {
        id: id.to_string(),
        r#ref: None,
        parent_id: parent_id.map(str::to_string),
        component_id: None,
        tag: "div".to_string(),
        role: role.map(str::to_string),
        name: name.map(str::to_string),
        text: None,
        description: None,
        test_id: test_id.map(str::to_string),
        geometry: None,
        state: PageContextNodeState {
            visible: true,
            disabled: Some(disabled),
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
        source_mapping: None,
    }
}
