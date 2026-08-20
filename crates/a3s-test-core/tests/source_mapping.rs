use a3s_test_core::{
    PageContextSnapshot, PageContextSourceOrigin, PageContextSourceRelation,
    SOURCE_MAPPING_PROTOCOL,
};
use serde_json::json;

#[test]
fn page_context_admits_ranked_source_spans_without_source_contents() {
    let snapshot: PageContextSnapshot = serde_json::from_value(snapshot_json(Some(json!({
        "protocol": SOURCE_MAPPING_PROTOCOL,
        "candidates": [
            {
                "span": {
                    "file": "src/PayButton.tsx",
                    "line": 42,
                    "column": 5,
                    "endLine": 44,
                    "endColumn": 2
                },
                "generatedSpan": {
                    "file": "assets/app.js",
                    "line": 1,
                    "column": 1
                },
                "confidence": 0.97,
                "origin": "source_map",
                "relation": "exact",
                "registrationId": "vite:pay-button",
                "framework": "react"
            },
            {
                "span": { "file": "src/Checkout.tsx", "line": 10 },
                "confidence": 0.83,
                "origin": "boundary_hint",
                "relation": "ancestor",
                "registrationId": "checkout",
                "componentId": "checkout"
            }
        ],
        "truncated": false
    }))))
    .expect("typed source mapping");

    let mapping = snapshot.nodes[0]
        .source_mapping
        .as_ref()
        .expect("source mapping");
    mapping.validate().expect("valid ranked source mapping");
    assert_eq!(mapping.protocol, SOURCE_MAPPING_PROTOCOL);
    assert_eq!(mapping.candidates.len(), 2);
    assert_eq!(
        mapping.candidates[0].origin,
        PageContextSourceOrigin::SourceMap
    );
    assert_eq!(
        mapping.candidates[0].relation,
        PageContextSourceRelation::Exact
    );
    assert_eq!(mapping.candidates[0].span.end_line, Some(44));
    assert_eq!(
        mapping.candidates[1].component_id.as_deref(),
        Some("checkout")
    );
    assert!(!serde_json::to_string(&snapshot)
        .expect("source mapping JSON")
        .contains("sourcesContent"));
}

#[test]
fn page_context_keeps_source_mapping_optional_and_rejects_schema_drift() {
    let legacy: PageContextSnapshot =
        serde_json::from_value(snapshot_json(None)).expect("legacy page context");
    assert!(legacy.nodes[0].source_mapping.is_none());

    let invalid = snapshot_json(Some(json!({
        "protocol": SOURCE_MAPPING_PROTOCOL,
        "candidates": [{
            "span": { "file": "src/App.tsx", "line": 1 },
            "confidence": 1.0,
            "origin": "private_framework_state",
            "relation": "exact",
            "registrationId": "private"
        }],
        "truncated": false
    })));
    assert!(serde_json::from_value::<PageContextSnapshot>(invalid).is_err());

    let mut unranked: PageContextSnapshot = serde_json::from_value(snapshot_json(Some(json!({
        "protocol": SOURCE_MAPPING_PROTOCOL,
        "candidates": [
            {
                "span": { "file": "src/App.tsx", "line": 2 },
                "confidence": 0.5,
                "origin": "framework_adapter",
                "relation": "exact",
                "registrationId": "app:first"
            },
            {
                "span": { "file": "src/App.tsx", "line": 1 },
                "confidence": 0.9,
                "origin": "boundary_hint",
                "relation": "ancestor",
                "registrationId": "app:second"
            }
        ],
        "truncated": false
    }))))
    .expect("typed but unranked source mapping");
    assert!(unranked.nodes[0]
        .source_mapping
        .take()
        .expect("unranked source mapping")
        .validate()
        .is_err());

    let mut inconsistent_truncation: PageContextSnapshot =
        serde_json::from_value(snapshot_json(Some(json!({
            "protocol": SOURCE_MAPPING_PROTOCOL,
            "candidates": [{
                "span": { "file": "src/App.tsx", "line": 1 },
                "confidence": 0.9,
                "origin": "boundary_hint",
                "relation": "exact",
                "registrationId": "app"
            }],
            "truncated": true
        }))))
        .expect("typed but inconsistently truncated source mapping");
    assert!(inconsistent_truncation.nodes[0]
        .source_mapping
        .take()
        .expect("inconsistently truncated source mapping")
        .validate()
        .is_err());
}

fn snapshot_json(source_mapping: Option<serde_json::Value>) -> serde_json::Value {
    let mut node = json!({
        "id": "n1",
        "tag": "button",
        "role": "button",
        "name": "Pay now",
        "state": { "visible": true },
        "locators": [{ "type": "test_id", "value": "pay" }]
    });
    if let Some(mapping) = source_mapping {
        node["sourceMapping"] = mapping;
    }
    json!({
        "protocol": "a3s.test.page-context/1",
        "sdkVersion": "0.5.0",
        "revision": 7,
        "page": {
            "id": "checkout",
            "url": "http://127.0.0.1:3000/checkout",
            "route": "/checkout",
            "title": "Checkout",
            "ready": true,
            "viewport": { "width": 1280.0, "height": 720.0, "dpr": 1.0 },
            "document": { "width": 1280.0, "height": 720.0 },
            "scroll": { "x": 0.0, "y": 0.0 },
            "language": "en",
            "theme": "light"
        },
        "components": [],
        "nodes": [node],
        "facts": {},
        "removedNodeIds": [],
        "truncated": false,
        "nextCursor": null
    })
}
