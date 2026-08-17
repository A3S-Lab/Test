use a3s_test_core::{
    action_uses_page_context_ref, bind_page_context_refs, resolve_page_context_refs, Action,
    PageContextObservation, PageContextSnapshot, SurfaceObservation, Target, UiContextScope,
    UiCustomProperty, UiCustomPropertySource, UiUnderstandingSnapshot,
};
use serde_json::{json, Value};

fn layout_node(node_id: &str, parent_node_id: Option<&str>) -> Value {
    let mut value = json!({
        "nodeId": node_id,
        "display": "block",
        "position": "static",
        "overflowX": "visible",
        "overflowY": "visible",
        "overflowMetrics": {
            "clientWidth": 100,
            "clientHeight": 40,
            "scrollWidth": 100,
            "scrollHeight": 40,
            "scrollLeft": 0,
            "scrollTop": 0,
            "overflowingX": false,
            "overflowingY": false,
            "clipsX": false,
            "clipsY": false
        },
        "boxModel": {
            "boxSizing": "border-box",
            "writingMode": "horizontal-tb",
            "direction": "ltr",
            "margin": { "top": "0px", "right": "0px", "bottom": "0px", "left": "0px" },
            "borderWidth": { "top": "0px", "right": "0px", "bottom": "0px", "left": "0px" },
            "padding": { "top": "0px", "right": "0px", "bottom": "0px", "left": "0px" }
        },
        "order": "0",
        "stackingContextReasons": []
    });
    if let Some(parent_node_id) = parent_node_id {
        value["parentNodeId"] = json!(parent_node_id);
    }
    value
}

fn observation_with_ui() -> SurfaceObservation {
    let mut snapshot: PageContextSnapshot = serde_json::from_value(json!({
        "protocol": "a3s.test.page-context/1",
        "sdkVersion": "0.4.0",
        "revision": 3,
        "page": {
            "id": "projection",
            "url": "https://example.test/",
            "route": "/",
            "title": "Projection",
            "ready": true,
            "viewport": { "width": 1280, "height": 720, "dpr": 1 },
            "document": { "width": 1280, "height": 720 },
            "scroll": { "x": 0, "y": 0 },
            "language": "en",
            "theme": "light"
        },
        "components": [],
        "nodes": [{
            "id": "private-n1",
            "tag": "button",
            "role": "button",
            "name": "Pay",
            "testId": "pay",
            "state": { "visible": true },
            "locators": [{ "type": "test_id", "value": "pay" }]
        }, {
            "id": "private-n2",
            "tag": "div",
            "state": { "visible": true },
            "locators": []
        }],
        "facts": {},
        "ui": {
            "protocol": "a3s.test.ui-understanding/1",
            "observationId": "ui-3-0123456789abcdef",
            "pageRevision": 3,
            "viewport": { "width": 1280, "height": 720, "dpr": 1 },
            "scope": { "kind": "node", "nodeId": "private-n1" },
            "budget": {
                "limits": {
                    "nodes": 3,
                    "stateSamples": 1,
                    "stringBytes": 4096,
                    "encodedBytes": 262144,
                    "durationMs": 32
                },
                "used": {
                    "nodes": 3,
                    "stateSamples": 1,
                    "encodedBytes": 0,
                    "durationMs": 1
                },
                "truncated": false,
                "reasons": []
            },
            "evidence": {
                "sourceKinds": ["computed_style", "dom_structure", "layout_geometry"],
                "sampledNodeIds": ["private-n1", "private-layout", "private-n2"],
                "totalCandidateNodes": 3,
                "omittedNodes": 0,
                "inaccessibleStyleSheets": 0
            },
            "style": {
                "colors": [{
                    "value": "rgb(0, 0, 0)",
                    "properties": ["color"],
                    "count": 2,
                    "nodeIds": ["private-n1", "private-n2"],
                    "confidence": 1
                }],
                "typography": [{
                    "family": "sans-serif",
                    "size": "16px",
                    "weight": "400",
                    "lineHeight": "normal",
                    "letterSpacing": "normal",
                    "count": 1,
                    "nodeIds": ["private-n1"],
                    "confidence": 1
                }],
                "spacing": [{
                    "value": "8px",
                    "properties": ["gap"],
                    "count": 1,
                    "nodeIds": ["private-layout"],
                    "confidence": 1
                }],
                "radii": [{
                    "value": "4px",
                    "properties": ["border-radius"],
                    "count": 1,
                    "nodeIds": ["private-n1"],
                    "confidence": 1
                }],
                "shadows": [{
                    "value": "none",
                    "properties": ["box-shadow"],
                    "count": 1,
                    "nodeIds": ["private-n2"],
                    "confidence": 1
                }],
                "zIndices": [{
                    "value": "10",
                    "properties": ["z-index"],
                    "count": 1,
                    "nodeIds": ["private-layout"],
                    "confidence": 1
                }],
                "customProperties": [],
                "responsiveConditions": []
            },
            "layout": {
                "nodes": [
                    layout_node("private-layout", None),
                    layout_node("private-n1", Some("private-layout")),
                    layout_node("private-n2", Some("private-layout"))
                ],
                "edges": [{
                    "fromNodeId": "private-layout",
                    "toNodeId": "private-n1",
                    "relation": "contains"
                }, {
                    "fromNodeId": "private-layout",
                    "toNodeId": "private-n2",
                    "relation": "contains"
                }]
            },
            "components": [{
                "id": "cluster-0123456789abcdef",
                "fingerprint": "0123456789abcdef",
                "signature": "button[]",
                "representativeNodeId": "private-n1",
                "memberNodeIds": ["private-n1", "private-n2"],
                "memberCount": 2,
                "confidence": 1
            }],
            "stateDiffs": [{
                "nodeId": "private-n1",
                "from": "default",
                "to": "hover",
                "styleChanges": [],
                "accessibilityChanges": [],
                "confidence": 1
            }],
            "motion": {
                "prefersReducedMotion": false,
                "transitions": [{
                    "nodeId": "private-n2",
                    "properties": ["opacity"],
                    "durations": ["1s"],
                    "delays": ["0s"],
                    "timingFunctions": ["ease"]
                }],
                "animations": [{
                    "nodeId": "private-n1",
                    "names": ["pulse"],
                    "durations": ["1s"],
                    "delays": ["0s"],
                    "iterationCounts": ["1"],
                    "playStates": ["running"],
                    "sources": ["css"],
                    "timelines": [{
                        "value": "auto",
                        "kind": "document",
                        "source": "computed_style"
                    }],
                    "rangeStarts": ["normal"],
                    "rangeEnds": ["normal"]
                }],
                "keyframeNames": ["pulse"],
                "stickyNodeIds": ["private-layout"],
                "scrollContainerNodeIds": ["private-layout"],
                "canvasNodeIds": ["private-n2"],
                "mediaNodeIds": ["private-n2"]
            }
        },
        "removedNodeIds": ["private-removed"],
        "truncated": false,
        "nextCursor": null
    }))
    .expect("typed page context");
    let ui = snapshot.ui.as_mut().expect("UI understanding");
    refresh_ui_encoded_bytes(ui);
    ui.validate(
        snapshot.revision,
        snapshot.page.as_ref().map(|page| &page.viewport),
    )
    .expect("valid source UI");
    SurfaceObservation::new("UI projection")
        .with_page_context(PageContextObservation::from_snapshot(snapshot))
}

fn refresh_ui_encoded_bytes(ui: &mut UiUnderstandingSnapshot) {
    for _ in 0..8 {
        let encoded_bytes =
            u64::try_from(serde_json::to_vec(&*ui).expect("encode UI").len()).expect("UI size");
        if ui.budget.used.encoded_bytes == encoded_bytes {
            return;
        }
        ui.budget.used.encoded_bytes = encoded_bytes;
    }
    panic!("UI encoded size did not converge");
}

fn tighten_ui_budget_to_current_size(ui: &mut UiUnderstandingSnapshot) {
    for _ in 0..8 {
        refresh_ui_encoded_bytes(ui);
        let encoded_bytes = ui.budget.used.encoded_bytes;
        if ui.budget.limits.encoded_bytes == encoded_bytes {
            return;
        }
        ui.budget.limits.encoded_bytes = encoded_bytes;
    }
    panic!("UI encoded-size limit did not converge");
}

fn replace_exact_strings(value: &mut Value, replacements: &[(&str, &str)]) {
    match value {
        Value::String(current) => {
            if let Some((_, replacement)) = replacements
                .iter()
                .find(|(candidate, _)| current == candidate)
            {
                *current = (*replacement).to_string();
            }
        }
        Value::Array(values) => {
            for value in values {
                replace_exact_strings(value, replacements);
            }
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                replace_exact_strings(value, replacements);
            }
        }
        _ => {}
    }
}

#[test]
fn projects_private_ui_ids_into_observation_scoped_refs() {
    let mut observation = observation_with_ui();
    let bindings = bind_page_context_refs(&mut observation);
    let snapshot = observation
        .page_context
        .as_ref()
        .and_then(|context| context.snapshot.as_ref())
        .expect("projected snapshot");
    let ui = snapshot.ui.as_ref().expect("projected UI");

    assert_eq!(snapshot.nodes[0].r#ref.as_deref(), Some("@c1"));
    assert!(snapshot.nodes[0].id.is_empty());
    assert!(snapshot.nodes[1].r#ref.is_none());
    assert!(snapshot.nodes[1].id.is_empty());
    assert_eq!(bindings.targets.len(), 1);
    assert_eq!(ui.layout.nodes[0].node_id, "@u1");
    assert_eq!(ui.layout.nodes[1].node_id, "@c1");
    assert_eq!(ui.layout.nodes[2].node_id, "@u2");
    assert_eq!(ui.layout.nodes[1].parent_node_id.as_deref(), Some("@u1"));
    assert_eq!(ui.layout.nodes[2].parent_node_id.as_deref(), Some("@u1"));
    assert!(matches!(
        &ui.scope,
        UiContextScope::Node { node_id } if node_id == "@c1"
    ));
    assert_eq!(ui.evidence.sampled_node_ids, ["@c1", "@u1", "@u2"]);
    assert_eq!(ui.style.colors[0].node_ids, ["@c1", "@u2"]);
    assert_eq!(ui.style.typography[0].node_ids, ["@c1"]);
    assert_eq!(ui.style.spacing[0].node_ids, ["@u1"]);
    assert_eq!(ui.style.radii[0].node_ids, ["@c1"]);
    assert_eq!(ui.style.shadows[0].node_ids, ["@u2"]);
    assert_eq!(ui.style.z_indices[0].node_ids, ["@u1"]);
    assert_eq!(ui.layout.edges[0].from_node_id, "@u1");
    assert_eq!(ui.layout.edges[0].to_node_id, "@c1");
    assert_eq!(ui.layout.edges[1].from_node_id, "@u1");
    assert_eq!(ui.layout.edges[1].to_node_id, "@u2");
    assert_eq!(ui.components[0].representative_node_id, "@c1");
    assert_eq!(ui.components[0].member_node_ids, ["@c1", "@u2"]);
    assert_eq!(ui.state_diffs[0].node_id, "@c1");
    assert_eq!(ui.motion.transitions[0].node_id, "@u2");
    assert_eq!(ui.motion.animations[0].node_id, "@c1");
    assert_eq!(ui.motion.sticky_node_ids, ["@u1"]);
    assert_eq!(ui.motion.scroll_container_node_ids, ["@u1"]);
    assert_eq!(ui.motion.canvas_node_ids, ["@u2"]);
    assert_eq!(ui.motion.media_node_ids, ["@u2"]);
    assert!(snapshot.removed_node_ids.is_empty());
    assert!(!serde_json::to_string(&observation)
        .expect("serialize observation")
        .contains("private-"));
    assert_eq!(
        ui.budget.used.encoded_bytes,
        u64::try_from(serde_json::to_vec(ui).expect("encode projected UI").len())
            .expect("projected UI size")
    );
    ui.validate(
        snapshot.revision,
        snapshot.page.as_ref().map(|page| &page.viewport),
    )
    .expect("valid projected UI");
}

#[test]
fn rejects_ui_evidence_refs_as_action_targets() {
    let mut observation = observation_with_ui();
    let bindings = bind_page_context_refs(&mut observation);
    let action = Action::Click {
        target: Target::Ref {
            value: "@u1".to_string(),
        },
    };
    assert!(action_uses_page_context_ref(&action));
    let error = resolve_page_context_refs(action, &bindings).expect_err("reject UI evidence ref");
    assert!(error.message().contains("not actionable"));
}

#[test]
fn omits_ui_when_projected_refs_exceed_the_declared_encoded_budget() {
    let mut observation = observation_with_ui();
    let snapshot = observation
        .page_context
        .as_mut()
        .and_then(|context| context.snapshot.as_mut())
        .expect("source snapshot");
    let mut value = serde_json::to_value(&*snapshot).expect("encode source snapshot");
    replace_exact_strings(
        &mut value,
        &[
            ("private-n1", "a"),
            ("private-n2", "b"),
            ("private-layout", "l"),
            ("private-removed", "r"),
        ],
    );
    *snapshot = serde_json::from_value(value).expect("short source identities");

    let revision = snapshot.revision;
    let viewport = snapshot.page.as_ref().map(|page| page.viewport.clone());
    let ui = snapshot.ui.as_mut().expect("source UI");
    ui.style.custom_properties.extend([
        UiCustomProperty {
            name: "--projection-pad-a".to_string(),
            value: "a".repeat(4_000),
            source: UiCustomPropertySource::DocumentRoot,
            confidence: 1.0,
        },
        UiCustomProperty {
            name: "--projection-pad-b".to_string(),
            value: "b".repeat(4_000),
            source: UiCustomPropertySource::DocumentRoot,
            confidence: 1.0,
        },
    ]);
    tighten_ui_budget_to_current_size(ui);
    assert!(ui.budget.limits.encoded_bytes >= 8_192);
    ui.validate(revision, viewport.as_ref())
        .expect("valid source UI at its encoded-size limit");

    let _ = bind_page_context_refs(&mut observation);
    let projected = observation
        .page_context
        .as_ref()
        .and_then(|context| context.snapshot.as_ref())
        .expect("projected snapshot");
    assert!(projected.ui.is_none());
}

#[test]
fn does_not_bind_ambiguous_private_identity_to_an_action_ref() {
    let mut observation = observation_with_ui();
    let snapshot = observation
        .page_context
        .as_mut()
        .and_then(|context| context.snapshot.as_mut())
        .expect("source snapshot");
    snapshot.nodes[1].id = snapshot.nodes[0].id.clone();

    let _ = bind_page_context_refs(&mut observation);
    let ui = observation
        .page_context
        .as_ref()
        .and_then(|context| context.snapshot.as_ref())
        .and_then(|snapshot| snapshot.ui.as_ref())
        .expect("projected UI");
    assert_eq!(ui.layout.nodes[1].node_id, "@u2");
    assert_ne!(ui.layout.nodes[1].node_id, "@c1");
}
