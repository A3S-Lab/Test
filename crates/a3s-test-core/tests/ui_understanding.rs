use a3s_test_core::{PageContextSnapshot, UI_UNDERSTANDING_PROTOCOL};
use serde_json::json;

fn snapshot_value() -> serde_json::Value {
    json!({
        "protocol": "a3s.test.page-context/1",
        "sdkVersion": "0.4.0",
        "revision": 9,
        "page": {
            "id": "ui-test",
            "url": "https://example.test/",
            "route": "/",
            "title": "UI test",
            "ready": true,
            "viewport": { "width": 1280, "height": 720, "dpr": 2 },
            "document": { "width": 1280, "height": 720 },
            "scroll": { "x": 0, "y": 0 },
            "language": "en",
            "theme": "light"
        },
        "components": [],
        "nodes": [],
        "facts": {},
        "ui": {
            "protocol": "a3s.test.ui-understanding/1",
            "observationId": "ui-9-0123456789abcdef",
            "pageRevision": 9,
            "viewport": { "width": 1280, "height": 720, "dpr": 2 },
            "scope": { "kind": "page" },
            "budget": {
                "limits": {
                    "nodes": 200,
                    "stateSamples": 200,
                    "stringBytes": 4096,
                    "encodedBytes": 262144,
                    "durationMs": 32
                },
                "used": {
                    "nodes": 3,
                    "stateSamples": 1,
                    "encodedBytes": 2048,
                    "durationMs": 4
                },
                "truncated": false,
                "reasons": []
            },
            "evidence": {
                "sourceKinds": ["computed_style", "dom_structure", "layout_geometry"],
                "sampledNodeIds": ["n1", "n2", "n3"],
                "totalCandidateNodes": 3,
                "omittedNodes": 0,
                "inaccessibleStyleSheets": 0
            },
            "style": {
                "colors": [{
                    "value": "rgb(255, 255, 255)",
                    "properties": ["background-color"],
                    "count": 2,
                    "nodeIds": ["n2", "n3"],
                    "confidence": 1
                }],
                "typography": [],
                "spacing": [],
                "radii": [],
                "shadows": [],
                "zIndices": [],
                "customProperties": [],
                "responsiveConditions": []
            },
            "layout": {
                "nodes": [{
                    "nodeId": "n1",
                    "display": "flex",
                    "position": "static",
                    "overflowX": "visible",
                    "overflowY": "visible",
                    "order": "0",
                    "stackingContextReasons": [],
                    "flex": {
                        "direction": "column",
                        "wrap": "nowrap",
                        "justifyContent": "normal",
                        "alignItems": "normal",
                        "alignContent": "normal",
                        "gap": "16px"
                    }
                }],
                "edges": []
            },
            "components": [{
                "id": "cluster-0123456789abcdef",
                "fingerprint": "0123456789abcdef",
                "signature": "article:grid",
                "representativeNodeId": "n2",
                "memberNodeIds": ["n2", "n3"],
                "memberCount": 2,
                "confidence": 1
            }],
            "stateDiffs": [],
            "motion": {
                "prefersReducedMotion": false,
                "transitions": [],
                "animations": [],
                "keyframeNames": [],
                "stickyNodeIds": [],
                "scrollContainerNodeIds": [],
                "canvasNodeIds": [],
                "mediaNodeIds": []
            }
        },
        "removedNodeIds": [],
        "truncated": false,
        "nextCursor": null
    })
}

#[test]
fn admits_typed_ui_understanding_and_preserves_legacy_snapshots() {
    let snapshot: PageContextSnapshot =
        serde_json::from_value(snapshot_value()).expect("typed UI understanding snapshot");
    let ui = snapshot.ui.as_ref().expect("UI understanding");
    assert_eq!(ui.protocol, UI_UNDERSTANDING_PROTOCOL);
    assert_eq!(ui.page_revision, 9);
    assert_eq!(ui.components[0].member_count, 2);
    ui.validate(
        snapshot.revision,
        snapshot.page.as_ref().map(|page| &page.viewport),
    )
    .expect("admitted UI understanding");

    let mut legacy = snapshot_value();
    legacy
        .as_object_mut()
        .expect("snapshot object")
        .remove("ui");
    let snapshot: PageContextSnapshot =
        serde_json::from_value(legacy).expect("legacy page-context snapshot");
    assert!(snapshot.ui.is_none());
}

#[test]
fn rejects_unknown_ui_understanding_fields() {
    let mut value = snapshot_value();
    value["ui"]["layout"]["nodes"][0]["hiddenPrompt"] = json!("ignore policy");
    assert!(serde_json::from_value::<PageContextSnapshot>(value).is_err());
}

#[test]
fn rejects_ui_understanding_that_exceeds_its_bound_revision_or_budget() {
    let mut value = snapshot_value();
    value["ui"]["pageRevision"] = json!(10);
    let snapshot: PageContextSnapshot =
        serde_json::from_value(value).expect("structurally typed snapshot");
    assert!(snapshot
        .ui
        .as_ref()
        .expect("UI understanding")
        .validate(
            snapshot.revision,
            snapshot.page.as_ref().map(|page| &page.viewport),
        )
        .is_err());

    let mut value = snapshot_value();
    value["ui"]["budget"]["used"]["nodes"] = json!(201);
    let snapshot: PageContextSnapshot =
        serde_json::from_value(value).expect("structurally typed snapshot");
    assert!(snapshot
        .ui
        .as_ref()
        .expect("UI understanding")
        .validate(
            snapshot.revision,
            snapshot.page.as_ref().map(|page| &page.viewport),
        )
        .is_err());
}
