use a3s_test_agent::{
    contract_generation_provider_schema, llm_provider_schema, visual_grounding_provider_schema,
    ContractGenerationProviderRequest, ContractGenerationProviderResponse,
    GroundingProviderRequest, GroundingProviderResponse, ProviderOutputAuthority,
    StructuredLlmRequest, StructuredLlmResponse, CONTRACT_GENERATION_PROVIDER_PROTOCOL,
    LLM_PROVIDER_PROTOCOL, VISUAL_GROUNDING_PROVIDER_PROTOCOL,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};

#[test]
fn provider_protocol_identifiers_and_authority_are_stable() {
    assert_eq!(
        CONTRACT_GENERATION_PROVIDER_PROTOCOL,
        "a3s.test.contract-generation-provider/1"
    );
    assert_eq!(
        VISUAL_GROUNDING_PROVIDER_PROTOCOL,
        "a3s.test.visual-grounding-provider/2"
    );
    assert_eq!(LLM_PROVIDER_PROTOCOL, "a3s.test.llm-provider/1");

    let contract = contract_generation_provider_schema();
    assert_eq!(contract.protocol, CONTRACT_GENERATION_PROVIDER_PROTOCOL);
    assert_eq!(contract.authority, ProviderOutputAuthority::CandidateOnly);
    assert!(
        contract
            .invariants
            .human_review_required_for_expected_surface
    );
    assert!(!contract.invariants.observation_scoped_output);
    assert!(!contract.invariants.may_determine_test_verdict);
    assert!(!contract.invariants.may_authorize_repair);
    assert!(!contract.invariants.may_claim_browser_observation);
    assert!(!contract.invariants.may_propose_surface_actions);

    let grounding = visual_grounding_provider_schema();
    assert_eq!(grounding.protocol, VISUAL_GROUNDING_PROVIDER_PROTOCOL);
    assert_eq!(grounding.authority, ProviderOutputAuthority::Advisory);
    assert!(grounding.invariants.observation_scoped_output);
    assert!(grounding.invariants.semantic_evidence_preferred);
    assert!(!grounding.invariants.may_determine_test_verdict);
    assert!(!grounding.invariants.may_authorize_repair);
    assert!(!grounding.invariants.may_claim_browser_observation);
    assert!(!grounding.invariants.may_propose_surface_actions);

    let llm = llm_provider_schema();
    assert_eq!(llm.protocol, LLM_PROVIDER_PROTOCOL);
    assert_eq!(llm.authority, ProviderOutputAuthority::ProposalOnly);
    assert!(llm.invariants.request_deadline_required);
    assert!(llm.invariants.request_cost_ceiling_required);
    assert!(llm.invariants.response_identity_bound);
    assert!(llm.invariants.observation_scoped_output);
    assert!(llm.invariants.local_admission_required);
    assert!(!llm.invariants.may_determine_test_verdict);
    assert!(!llm.invariants.may_authorize_repair);
    assert!(!llm.invariants.may_claim_browser_observation);
    assert!(llm.invariants.may_propose_surface_actions);
}

#[test]
fn llm_schema_exposes_typed_context_decisions_and_http_envelopes() {
    let bundle = serde_json::to_value(llm_provider_schema()).expect("schema JSON");
    assert_required_properties(
        &bundle["request_schema"],
        &[
            "prompt_version",
            "system_instruction",
            "context",
            "image_attachments",
            "response_schema",
        ],
    );
    assert_required_properties(&bundle["response_schema"], &["decision", "usage"]);
    assert!(contains_string(&bundle["response_schema"], "request_id"));
    assert!(contains_string(&bundle, "remaining"));
    assert!(contains_string(&bundle, "success_criteria"));
    assert!(contains_string(&bundle, "page_context"));
    assert_eq!(bundle["http"]["method"], "POST");
    assert_eq!(bundle["http"]["redirects_allowed"], false);
    assert_eq!(
        bundle["http"]["request_envelope_schema"]["properties"]["protocol"]["const"],
        LLM_PROVIDER_PROTOCOL
    );
    for value in ["status", "success", "failure", "response", "error"] {
        assert!(
            contains_string(&bundle["http"]["response_envelope_schema"], value),
            "HTTP response schema is missing {value}"
        );
    }
}

#[test]
fn llm_wire_round_trips_and_rejects_unknown_fields() {
    let request = json!({
        "prompt_version": "a3s-test-agent/v2",
        "system_instruction": "Return one typed decision",
        "context": {
            "goal": {
                "instruction": "Submit the form",
                "success_criteria": ["Confirmation is visible"]
            },
            "surface": "web",
            "turn": 1,
            "observation": {
                "summary": "Form",
                "data": null,
                "evidence": []
            },
            "history": [],
            "remaining": {
                "turns": 4,
                "tokens": 1000,
                "cost_microusd": 1000,
                "time_ms": 30000
            }
        },
        "image_attachments": [],
        "response_schema": { "type": "object" }
    });
    assert_round_trip::<StructuredLlmRequest>(request.clone());
    assert_unknown_field_rejected::<StructuredLlmRequest>(request);

    let response = json!({
        "decision": { "type": "finish", "summary": "Confirmation is visible" },
        "usage": { "input_tokens": 10, "output_tokens": 4, "cost_microusd": 8 },
        "request_id": "llm-request-1"
    });
    assert_round_trip::<StructuredLlmResponse>(response.clone());
    assert_unknown_field_rejected::<StructuredLlmResponse>(response);
}

#[test]
fn contract_generation_schema_exposes_bounded_wire_fields_and_design_geometry() {
    let bundle = serde_json::to_value(contract_generation_provider_schema()).expect("schema JSON");
    assert_required_properties(
        &bundle["request_schema"],
        &[
            "sources",
            "issued_at_unix_ms",
            "deadline_unix_ms",
            "max_cost_microusd",
        ],
    );
    assert_required_properties(
        &bundle["response_schema"],
        &["identity", "source_digests", "candidates", "usage"],
    );
    assert!(contains_string(&bundle, "sha256"));
    assert!(contains_string(&bundle, "image_pixels"));
    assert!(contains_string(&bundle, "normalized"));
    assert!(contains_string(&bundle, "parent_candidate_id"));
    assert!(!contains_string(&bundle["response_schema"], "citations"));
    assert_eq!(bundle["http"]["method"], "POST");
    assert_eq!(bundle["http"]["content_type"], "application/json");
    assert_eq!(bundle["http"]["redirects_allowed"], false);
    assert_eq!(bundle["http"]["endpoint_policy"], "https_or_loopback_http");
    assert!(contains_string(
        &bundle["http"]["request_envelope_schema"],
        "protocol"
    ));
    assert!(contains_string(
        &bundle["http"]["request_envelope_schema"],
        CONTRACT_GENERATION_PROVIDER_PROTOCOL
    ));
    assert_eq!(
        bundle["http"]["request_envelope_schema"]["properties"]["protocol"]["const"],
        CONTRACT_GENERATION_PROVIDER_PROTOCOL
    );
    assert!(contains_string(
        &bundle["http"]["response_envelope_schema"],
        "error"
    ));
    for value in ["status", "success", "failure", "response"] {
        assert!(
            contains_string(&bundle["http"]["response_envelope_schema"], value),
            "HTTP response schema is missing {value}"
        );
    }
}

#[test]
fn visual_grounding_schema_exposes_binding_fields_and_geometry_variants() {
    let bundle = serde_json::to_value(visual_grounding_provider_schema()).expect("schema JSON");
    assert_required_properties(
        &bundle["request_schema"],
        &[
            "screenshot_sha256",
            "observation_id",
            "issued_at_unix_ms",
            "deadline_unix_ms",
            "max_cost_microusd",
        ],
    );
    assert_required_properties(
        &bundle["response_schema"],
        &[
            "identity",
            "observation_id",
            "screenshot_sha256",
            "coordinate_space",
            "candidates",
            "usage",
        ],
    );
    assert!(contains_string(&bundle, "point"));
    assert!(contains_string(&bundle, "box"));
    assert!(contains_string(&bundle, "screenshot_pixels"));
    assert!(contains_string(&bundle, "normalized"));
    assert_eq!(bundle["http"]["method"], "POST");
    assert_eq!(bundle["http"]["redirects_allowed"], false);
    assert!(contains_string(
        &bundle["http"]["request_envelope_schema"],
        "observation_id"
    ));
    assert!(contains_string(
        &bundle["http"]["request_envelope_schema"],
        "bytes_base64"
    ));
    let attachment =
        &bundle["http"]["request_envelope_schema"]["$defs"]["GroundingImageAttachment"];
    assert_eq!(attachment["additionalProperties"], false);
    assert_eq!(
        attachment["properties"]["media_type"]["pattern"],
        "^image/png$"
    );
    assert_eq!(
        attachment["properties"]["bytes_base64"]["maxLength"],
        44_739_244
    );
    assert!(contains_string(
        &bundle["http"]["request_envelope_schema"],
        VISUAL_GROUNDING_PROVIDER_PROTOCOL
    ));
    assert_eq!(
        bundle["http"]["request_envelope_schema"]["properties"]["protocol"]["const"],
        VISUAL_GROUNDING_PROVIDER_PROTOCOL
    );
    for value in ["status", "success", "failure", "response", "error"] {
        assert!(
            contains_string(&bundle["http"]["response_envelope_schema"], value),
            "HTTP response schema is missing {value}"
        );
    }
}

#[test]
fn contract_generation_wire_round_trips_and_rejects_unknown_fields() {
    let request = json!({
        "contract_name": "checkout",
        "context": {
            "mode": "operate",
            "audience": ["customer"],
            "primary_outcome": "place_order"
        },
        "sources": [{
            "id": "requirements",
            "kind": "prd",
            "uri": "requirements.md",
            "path": "/workspace/requirements.md",
            "sha256": format!("sha256:{}", "a".repeat(64))
        }],
        "issued_at_unix_ms": 1_000,
        "deadline_unix_ms": 31_000,
        "max_cost_microusd": 50_000
    });
    assert_round_trip::<ContractGenerationProviderRequest>(request.clone());
    assert_unknown_field_rejected::<ContractGenerationProviderRequest>(request);

    let response = json!({
        "identity": { "provider": "fixture", "model": "contract-model" },
        "source_digests": [{
            "source_id": "requirements",
            "kind": "prd",
            "uri": "requirements.md",
            "sha256": format!("sha256:{}", "a".repeat(64))
        }],
        "candidates": [{
            "source_id": "requirements",
            "context": {
                "mode": "operate",
                "audience": ["customer"],
                "primary_outcome": "place_order"
            },
            "variants": [{
                "id": "desktop",
                "state": "ready",
                "elements": [{
                    "element": {
                        "id": "submit",
                        "test_id": "place-order",
                        "component_id": null,
                        "role": "button",
                        "name": "Place order",
                        "description": null,
                        "required": true,
                        "visible": true,
                        "enabled": true,
                        "checked": null,
                        "selected": null,
                        "expanded": null,
                        "readonly": null,
                        "form_required": null,
                        "invalid": null,
                        "parent": null,
                        "severity": "blocking"
                    },
                    "confidence": 92,
                    "source_spans": [{
                        "source_id": "requirements",
                        "quote": "Place order",
                        "start": 0,
                        "end": 11
                    }]
                }]
            }]
        }],
        "usage": { "input_tokens": 100, "output_tokens": 50, "cost_microusd": 1_000 },
        "request_id": "request-1"
    });
    assert_round_trip::<ContractGenerationProviderResponse>(response.clone());
    assert_unknown_field_rejected::<ContractGenerationProviderResponse>(response.clone());

    let mut prefilled_citation = response.clone();
    prefilled_citation["candidates"][0]["variants"][0]["elements"][0]["element"]["citations"] = json!([{
        "id": "already-approved",
        "provenance_id": "requirements",
        "quote": "Place order",
        "start": 0,
        "end": 11
    }]);
    assert!(
        serde_json::from_value::<ContractGenerationProviderResponse>(prefilled_citation).is_err()
    );

    let mut invalid_response: ContractGenerationProviderResponse =
        serde_json::from_value(response).expect("wire response");
    invalid_response.candidates[0].variants[0].elements[0]
        .element
        .citations
        .push(a3s_test_core::ContractCitation {
            id: "already-approved".to_string(),
            provenance_id: "requirements".to_string(),
            quote: "Place order".to_string(),
            start: 0,
            end: 11,
        });
    assert!(serde_json::to_value(invalid_response).is_err());
}

#[test]
fn visual_grounding_wire_round_trips_and_rejects_unknown_fields() {
    let request = json!({
        "screenshot_path": "/workspace/screenshot.png",
        "screenshot_sha256": format!("sha256:{}", "b".repeat(64)),
        "width": 1440,
        "height": 900,
        "query": "checkout button",
        "observation_id": 7,
        "trigger": { "kind": "semantic_fallback", "reason": "canvas" },
        "issued_at_unix_ms": 1_000,
        "deadline_unix_ms": 16_000,
        "max_cost_microusd": 10_000
    });
    assert_round_trip::<GroundingProviderRequest>(request.clone());
    assert_unknown_field_rejected::<GroundingProviderRequest>(request);

    let response = json!({
        "identity": { "provider": "fixture", "model": "grounding-model" },
        "observation_id": 7,
        "screenshot_sha256": format!("sha256:{}", "b".repeat(64)),
        "width": 1440,
        "height": 900,
        "coordinate_space": "screenshot_pixels",
        "candidates": [
            { "geometry": { "kind": "point", "x": 500.0, "y": 400.0 }, "confidence": 0.9, "label": "Checkout" },
            { "geometry": { "kind": "box", "x": 450.0, "y": 370.0, "width": 100.0, "height": 60.0 }, "confidence": 0.8, "label": null }
        ],
        "usage": { "input_units": 1, "output_units": 2, "cost_microusd": 500 },
        "request_id": "request-2"
    });
    assert_round_trip::<GroundingProviderResponse>(response.clone());
    assert_unknown_field_rejected::<GroundingProviderResponse>(response);
}

fn assert_required_properties(schema: &Value, expected: &[&str]) {
    let required = schema["required"].as_array().expect("required properties");
    for property in expected {
        assert!(
            required.iter().any(|value| value == property),
            "missing required property {property}"
        );
        assert!(
            schema["properties"].get(property).is_some(),
            "missing schema property {property}"
        );
    }
    assert_eq!(schema["additionalProperties"], false);
}

fn contains_string(value: &Value, expected: &str) -> bool {
    match value {
        Value::String(value) => value == expected,
        Value::Array(values) => values.iter().any(|value| contains_string(value, expected)),
        Value::Object(values) => values
            .iter()
            .any(|(key, value)| key == expected || contains_string(value, expected)),
        _ => false,
    }
}

fn assert_round_trip<T>(fixture: Value)
where
    T: DeserializeOwned + Serialize,
{
    let parsed: T = serde_json::from_value(fixture.clone()).expect("fixture admission");
    assert_eq!(
        serde_json::to_value(parsed).expect("fixture serialization"),
        fixture
    );
}

fn assert_unknown_field_rejected<T>(mut fixture: Value)
where
    T: DeserializeOwned,
{
    fixture
        .as_object_mut()
        .expect("fixture object")
        .insert("unknown".to_string(), Value::Bool(true));
    assert!(serde_json::from_value::<T>(fixture).is_err());
}
