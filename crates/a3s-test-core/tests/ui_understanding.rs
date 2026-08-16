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
                "sourceKinds": ["computed_style", "dom_structure", "layout_geometry", "web_animations"],
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
                    "overflowX": "hidden",
                    "overflowY": "visible",
                    "overflowMetrics": {
                        "clientWidth": 240,
                        "clientHeight": 160,
                        "scrollWidth": 360,
                        "scrollHeight": 160,
                        "scrollLeft": -12,
                        "scrollTop": 0,
                        "overflowingX": true,
                        "overflowingY": false,
                        "clipsX": true,
                        "clipsY": false
                    },
                    "boxModel": {
                        "boxSizing": "border-box",
                        "writingMode": "vertical-rl",
                        "direction": "rtl",
                        "margin": {
                            "top": "4px",
                            "right": "8px",
                            "bottom": "12px",
                            "left": "16px"
                        },
                        "borderWidth": {
                            "top": "1px",
                            "right": "2px",
                            "bottom": "3px",
                            "left": "4px"
                        },
                        "padding": {
                            "top": "5px",
                            "right": "6px",
                            "bottom": "7px",
                            "left": "8px"
                        }
                    },
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
                "animations": [{
                    "nodeId": "n2",
                    "names": ["reveal"],
                    "durations": ["1s"],
                    "delays": ["0s"],
                    "iterationCounts": ["1"],
                    "playStates": ["running"],
                    "sources": ["css", "web_animations"],
                    "timelines": [{
                        "value": "view()",
                        "kind": "view",
                        "source": "computed_style"
                    }, {
                        "value": "(view-timeline)",
                        "kind": "view",
                        "source": "web_animations"
                    }],
                    "rangeStarts": ["entry 10%"],
                    "rangeEnds": ["cover 80%"]
                }],
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

fn assert_ui_validation_fails(value: serde_json::Value, case: &str) {
    let snapshot: PageContextSnapshot =
        serde_json::from_value(value).expect("structurally typed snapshot");
    let result = snapshot.ui.as_ref().expect("UI understanding").validate(
        snapshot.revision,
        snapshot.page.as_ref().map(|page| &page.viewport),
    );
    assert!(result.is_err(), "accepted {case}");
}

#[test]
fn admits_typed_ui_understanding_and_preserves_legacy_snapshots() {
    let snapshot: PageContextSnapshot =
        serde_json::from_value(snapshot_value()).expect("typed UI understanding snapshot");
    let ui = snapshot.ui.as_ref().expect("UI understanding");
    assert_eq!(ui.protocol, UI_UNDERSTANDING_PROTOCOL);
    assert_eq!(ui.page_revision, 9);
    assert_eq!(ui.components[0].member_count, 2);
    assert_eq!(ui.layout.nodes[0].overflow_metrics.scroll_left, -12.0);
    assert!(ui.layout.nodes[0].overflow_metrics.clips_x);
    assert_eq!(ui.layout.nodes[0].box_model.margin.left, "16px");
    assert_eq!(ui.motion.animations[0].timelines.len(), 2);
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

#[test]
fn rejects_inconsistent_overflow_metrics() {
    let mut value = snapshot_value();
    value["ui"]["layout"]["nodes"][0]["overflowX"] = json!("unknown");
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
    value["ui"]["layout"]["nodes"][0]["overflowMetrics"]["scrollWidth"] = json!(120);
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
    value["ui"]["layout"]["nodes"][0]["overflowMetrics"]["clipsX"] = json!(false);
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
    value["ui"]["layout"]["nodes"][0]["overflowMetrics"]["clientHeight"] = json!(-1);
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

#[test]
fn rejects_invalid_box_model_evidence() {
    let mut value = snapshot_value();
    value["ui"]["layout"]["nodes"][0]["boxModel"]["writingMode"] = json!("diagonal");
    assert!(serde_json::from_value::<PageContextSnapshot>(value).is_err());

    let mut value = snapshot_value();
    value["ui"]["layout"]["nodes"][0]["boxModel"]["padding"]["left"] = json!("");
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

#[test]
fn rejects_inconsistent_ui_graph_and_reference_sets() {
    let mut value = snapshot_value();
    let duplicate = value["ui"]["layout"]["nodes"][0].clone();
    value["ui"]["layout"]["nodes"]
        .as_array_mut()
        .expect("layout nodes")
        .push(duplicate);
    assert_ui_validation_fails(value, "duplicate layout node ids");

    let mut value = snapshot_value();
    value["ui"]["layout"]["edges"] = json!([{
        "fromNodeId": "n1",
        "toNodeId": "missing",
        "relation": "contains"
    }]);
    assert_ui_validation_fails(value, "layout edge with a missing target");

    let mut value = snapshot_value();
    value["ui"]["layout"]["nodes"][0]["parentNodeId"] = json!("n2");
    value["ui"]["layout"]["edges"] = json!([{
        "fromNodeId": "n3",
        "toNodeId": "n1",
        "relation": "contains"
    }]);
    assert_ui_validation_fails(value, "contradictory containment edge");

    let mut value = snapshot_value();
    value["ui"]["layout"]["edges"] = json!([{
        "fromNodeId": "n2",
        "toNodeId": "n1",
        "relation": "scroll_container"
    }, {
        "fromNodeId": "n2",
        "toNodeId": "n1",
        "relation": "scroll_container"
    }]);
    assert_ui_validation_fails(value, "duplicate layout edges");

    let mut value = snapshot_value();
    value["ui"]["evidence"]["sourceKinds"] = json!([
        "computed_style",
        "computed_style",
        "dom_structure",
        "layout_geometry"
    ]);
    assert_ui_validation_fails(value, "duplicate evidence sources");

    let mut value = snapshot_value();
    value["ui"]["evidence"]["sampledNodeIds"] = json!(["n1", "n1"]);
    assert_ui_validation_fails(value, "duplicate sampled node ids");

    let mut value = snapshot_value();
    value["ui"]["components"][0]["representativeNodeId"] = json!("n9");
    assert_ui_validation_fails(value, "unbound component representative");

    let mut value = snapshot_value();
    value["ui"]["components"][0]["memberNodeIds"] = json!(["n2", "n2"]);
    assert_ui_validation_fails(value, "duplicate component members");

    let mut value = snapshot_value();
    value["ui"]["budget"]["used"]["nodes"] = json!(0);
    assert_ui_validation_fails(value, "layout larger than the sampled node set");
}

#[test]
fn rejects_inconsistent_animation_timeline_evidence() {
    let mut value = snapshot_value();
    value["ui"]["motion"]["animations"][0]["timelines"] = json!([]);
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
    value["ui"]["motion"]["animations"][0]["sources"] = json!(["css"]);
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
