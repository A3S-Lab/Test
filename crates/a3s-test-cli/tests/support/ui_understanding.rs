use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

use super::browser_process::assert_process_success;

pub fn verify_testkit_ui_understanding_through_driver(binary: &Path, browser: &Path, origin: &str) {
    let temp = tempfile::tempdir().expect("temporary UI understanding workspace");
    let manifest = temp.path().join("ui-understanding.acl");
    let suite = format!(
        r#"suite "ui-understanding" {{
    version = 1
    scenario "rendered-context" {{
        name = "Capture rendered UI understanding"
        surface = "web"
        timeout_ms = 60000
        navigate "open" {{ url = "{origin}" }}
        snapshot "context" {{ interactive = true }}
        expect "fixture-ready" {{ text = "Embedded TestKit E2E" }}
    }}
}}
"#
    );
    std::fs::write(&manifest, suite).expect("write UI understanding suite");
    let output = Command::new(binary)
        .args([
            "run",
            manifest.to_str().expect("UTF-8 UI understanding path"),
            "--browser-driver",
            "standalone",
            "--browser-executable",
            browser.to_str().expect("UTF-8 browser path"),
            "--command-timeout-ms",
            "60000",
            "--idle-timeout-ms",
            "15000",
            "--cleanup-timeout-ms",
            "15000",
            "--infrastructure-retries",
            "0",
            "--json",
        ])
        .current_dir(temp.path())
        .output()
        .expect("run UI understanding suite");
    assert_process_success("capture UI understanding through the Web driver", &output);
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("UI understanding run report");
    let snapshot = report["scenarios"][0]["steps"]
        .as_array()
        .expect("UI understanding steps")
        .iter()
        .find(|step| step["id"] == "context")
        .and_then(|step| step.pointer("/output/page_context/snapshot"))
        .expect("page context in the stable observation");
    let ui = snapshot
        .pointer("/ui")
        .expect("typed UI understanding in the stable observation");
    assert_eq!(ui["protocol"], "a3s.test.ui-understanding/1");
    assert_projected_ui_node_refs(ui);
    let page_revision = snapshot["revision"]
        .as_u64()
        .expect("page-context revision");
    assert_eq!(ui["pageRevision"].as_u64(), Some(page_revision));
    assert!(
        ui["layout"]["nodes"]
            .as_array()
            .is_some_and(|nodes| !nodes.is_empty()),
        "UI understanding must retain bounded layout evidence: {ui}"
    );
    verify_closed_layout_graph(snapshot, ui);
    verify_overflow_evidence(ui);
    verify_box_model_evidence(ui);
    verify_animation_evidence(ui);
    assert!(
        ui["budget"]["used"]["encodedBytes"]
            .as_u64()
            .zip(ui["budget"]["limits"]["encodedBytes"].as_u64())
            .is_some_and(|(used, limit)| used <= limit),
        "UI understanding must stay within its declared byte budget: {ui}"
    );
}

fn verify_closed_layout_graph(snapshot: &serde_json::Value, ui: &serde_json::Value) {
    let nodes = ui["layout"]["nodes"].as_array().expect("UI layout nodes");
    let node_ids = nodes
        .iter()
        .map(|node| {
            node["nodeId"]
                .as_str()
                .expect("projected UI layout node ref")
        })
        .collect::<HashSet<_>>();
    for node in nodes {
        if let Some(parent) = node["parentNodeId"].as_str() {
            assert!(
                node_ids.contains(parent),
                "UI layout parent is outside the sampled graph: {node}"
            );
        }
    }
    for edge in ui["layout"]["edges"].as_array().expect("UI layout edges") {
        let source = edge["fromNodeId"].as_str().expect("layout edge source");
        let target = edge["toNodeId"].as_str().expect("layout edge target");
        assert!(
            node_ids.contains(source) && node_ids.contains(target),
            "UI layout edge escapes the sampled graph: {edge}"
        );
    }

    let action_ref = snapshot["nodes"]
        .as_array()
        .and_then(|nodes| nodes.iter().find(|node| node["testId"] == "unboxed-action"))
        .and_then(|node| node["ref"].as_str())
        .expect("unboxed action observation ref");
    let action_layout = nodes
        .iter()
        .find(|node| node["nodeId"] == action_ref)
        .expect("unboxed action layout evidence");
    let parent_ref = action_layout["parentNodeId"]
        .as_str()
        .expect("nearest sampled ancestor for unboxed action");
    let parent_layout = nodes
        .iter()
        .find(|node| node["nodeId"] == parent_ref)
        .expect("sampled parent layout evidence");
    assert_ne!(
        parent_layout["display"], "contents",
        "display: contents ancestor must not become an unboxed layout node"
    );
}

fn verify_overflow_evidence(ui: &serde_json::Value) {
    let nested_layout = ui["layout"]["nodes"]
        .as_array()
        .and_then(|nodes| {
            nodes.iter().find(|node| {
                node["overflowY"] == "auto"
                    && node
                        .pointer("/overflowMetrics/clientHeight")
                        .and_then(serde_json::Value::as_f64)
                        == Some(140.0)
                    && node
                        .pointer("/overflowMetrics/scrollHeight")
                        .and_then(serde_json::Value::as_f64)
                        == Some(400.0)
            })
        })
        .expect("fixture overflow layout evidence");
    let overflow = &nested_layout["overflowMetrics"];
    assert!(
        overflow["scrollHeight"]
            .as_f64()
            .zip(overflow["clientHeight"].as_f64())
            .is_some_and(|(scroll, client)| scroll > client)
            && overflow["overflowingY"] == true
            && overflow["clipsY"] == true,
        "vertical overflow and clipping evidence is inconsistent: {nested_layout}"
    );
}

fn verify_box_model_evidence(ui: &serde_json::Value) {
    let box_layout = ui["layout"]["nodes"]
        .as_array()
        .and_then(|nodes| {
            nodes.iter().find(|node| {
                node.pointer("/boxModel/writingMode")
                    .and_then(serde_json::Value::as_str)
                    == Some("vertical-rl")
                    && node
                        .pointer("/boxModel/direction")
                        .and_then(serde_json::Value::as_str)
                        == Some("rtl")
            })
        })
        .expect("fixture box-model layout evidence");
    assert_eq!(box_layout["boxModel"]["boxSizing"], "border-box");
    assert_eq!(box_layout["boxModel"]["margin"]["left"], "16px");
    assert_eq!(box_layout["boxModel"]["borderWidth"]["right"], "2px");
    assert_eq!(box_layout["boxModel"]["padding"]["bottom"], "7px");
}

fn verify_animation_evidence(ui: &serde_json::Value) {
    let animations = ui["motion"]["animations"]
        .as_array()
        .expect("UI animation evidence");
    for kind in ["scroll", "view"] {
        let animation = animations
            .iter()
            .find(|animation| {
                animation["timelines"].as_array().is_some_and(|timelines| {
                    timelines.iter().any(|timeline| timeline["kind"] == kind)
                })
            })
            .unwrap_or_else(|| panic!("animation timeline kind {kind:?} missing: {ui}"));
        assert!(
            animation["rangeStarts"]
                .as_array()
                .is_some_and(|ranges| !ranges.is_empty()),
            "animation range evidence missing for {kind:?}: {animation}"
        );
    }
}

fn assert_projected_ui_node_refs(ui: &serde_json::Value) {
    const SCALAR_NODE_KEYS: &[&str] = &[
        "nodeId",
        "parentNodeId",
        "fromNodeId",
        "toNodeId",
        "representativeNodeId",
    ];
    const NODE_LIST_KEYS: &[&str] = &[
        "sampledNodeIds",
        "nodeIds",
        "memberNodeIds",
        "stickyNodeIds",
        "scrollContainerNodeIds",
        "canvasNodeIds",
        "mediaNodeIds",
    ];

    let mut refs = Vec::new();
    collect_node_refs(ui, SCALAR_NODE_KEYS, NODE_LIST_KEYS, &mut refs);
    assert!(
        !refs.is_empty(),
        "UI understanding contains no node refs: {ui}"
    );
    assert!(
        refs.iter()
            .all(|value| is_numbered_ref(value, "@c") || is_numbered_ref(value, "@u")),
        "UI understanding exposed a private node identity: {refs:?}"
    );
    assert!(
        refs.iter().any(|value| is_numbered_ref(value, "@c")),
        "UI understanding did not reuse any actionable Page Context refs: {refs:?}"
    );
    assert!(
        refs.iter().any(|value| is_numbered_ref(value, "@u")),
        "UI understanding did not project any evidence-only refs: {refs:?}"
    );
}

fn collect_node_refs<'a>(
    value: &'a serde_json::Value,
    scalar_keys: &[&str],
    list_keys: &[&str],
    refs: &mut Vec<&'a str>,
) {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                collect_node_refs(value, scalar_keys, list_keys, refs);
            }
        }
        serde_json::Value::Object(values) => {
            for (key, value) in values {
                if scalar_keys.contains(&key.as_str()) {
                    refs.push(value.as_str().unwrap_or_else(|| {
                        panic!("UI node identity {key:?} is not a string: {value}")
                    }));
                } else if list_keys.contains(&key.as_str()) {
                    for entry in value.as_array().unwrap_or_else(|| {
                        panic!("UI node identity list {key:?} is not an array: {value}")
                    }) {
                        refs.push(entry.as_str().unwrap_or_else(|| {
                            panic!("UI node identity in {key:?} is not a string: {entry}")
                        }));
                    }
                } else {
                    collect_node_refs(value, scalar_keys, list_keys, refs);
                }
            }
        }
        _ => {}
    }
}

fn is_numbered_ref(value: &str, prefix: &str) -> bool {
    value.strip_prefix(prefix).is_some_and(|suffix| {
        !suffix.is_empty() && suffix.chars().all(|character| character.is_ascii_digit())
    })
}
