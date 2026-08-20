use a3s_test_core::{
    bind_page_context_observation_refs, refresh_page_context_bindings, resolve_page_context_refs,
    Action, PageContextComponent, PageContextDelta, PageContextDeltaStatus,
    PageContextInvalidation, PageContextLocator, PageContextNode, PageContextNodeState,
    PageContextObservation, PageContextPage, PageContextPoint, PageContextSize,
    PageContextSnapshot, PageContextTheme, PageContextViewport, Target, PAGE_CONTEXT_DIFF_PROTOCOL,
    PAGE_CONTEXT_PROTOCOL,
};

#[test]
fn refresh_keeps_only_refs_outside_the_complete_invalidation_set() {
    let mut baseline = observation(1, vec![node("n1", "One"), node("n2", "Two")], None);
    let mut bindings = bind_page_context_observation_refs(&mut baseline);
    assert_eq!(bindings.targets.len(), 2);
    let first = bindings
        .node_fingerprints
        .get("@c1")
        .expect("first node fingerprint");
    let second = bindings
        .node_fingerprints
        .get("@c2")
        .expect("second node fingerprint");
    assert!(first.starts_with("sha256:"));
    assert!(second.starts_with("sha256:"));
    assert_ne!(first, second);
    assert_ne!(first, "n1");
    assert_ne!(second, "n2");

    let changed = observation(
        2,
        vec![node("n1", "One changed")],
        Some(complete_delta(1, 2, vec!["n1"])),
    );
    refresh_page_context_bindings(&mut bindings, &changed).expect("valid delta");

    assert_eq!(bindings.revision, Some(2));
    assert!(!bindings.targets.contains_key("@c1"));
    assert!(!bindings.node_fingerprints.contains_key("@c1"));
    assert!(bindings.targets.contains_key("@c2"));
    assert_eq!(
        resolve_page_context_refs(
            Action::Click {
                target: Target::Ref {
                    value: "@c2".to_string(),
                },
            },
            &bindings,
        )
        .expect("unaffected ref"),
        Action::Click {
            target: Target::Css {
                selector: "#n2".to_string(),
            },
        }
    );
    assert!(resolve_page_context_refs(
        Action::Click {
            target: Target::Ref {
                value: "@c1".to_string(),
            },
        },
        &bindings,
    )
    .is_err());
}

#[test]
fn refresh_clears_all_refs_when_the_diff_baseline_requires_a_reset() {
    let mut baseline = observation(1, vec![node("n1", "One"), node("n2", "Two")], None);
    let mut bindings = bind_page_context_observation_refs(&mut baseline);
    let reset = observation(
        3,
        vec![node("n1", "Current")],
        Some(PageContextDelta {
            protocol: PAGE_CONTEXT_DIFF_PROTOCOL.to_string(),
            from_revision: 1,
            to_revision: 3,
            status: PageContextDeltaStatus::ResetRequired,
            invalidated: PageContextInvalidation {
                all: true,
                page: true,
                facts: true,
                ui: true,
                node_ids: Vec::new(),
                component_ids: Vec::new(),
            },
        }),
    );

    refresh_page_context_bindings(&mut bindings, &reset).expect("valid reset");
    assert_eq!(bindings.revision, Some(3));
    assert!(bindings.targets.is_empty());
    assert!(bindings.node_fingerprints.is_empty());
}

#[test]
fn refresh_rejects_a_revision_that_moves_backwards() {
    let mut baseline = observation(2, vec![node("n1", "One")], None);
    let mut bindings = bind_page_context_observation_refs(&mut baseline);
    let stale = observation(1, vec![node("n1", "One")], None);

    let error = refresh_page_context_bindings(&mut bindings, &stale)
        .expect_err("a revision regression must fail");
    assert!(error.to_string().contains("moved backwards"));
}

#[test]
fn complete_delta_must_name_every_changed_or_removed_node() {
    let delta = complete_delta(1, 2, Vec::new());
    let error = delta
        .validate(Some(2), &[node("n1", "Changed")], &[], &["n2".to_string()])
        .expect_err("missing invalidations must fail");
    assert!(error
        .to_string()
        .contains("omitted changed or removed node evidence"));
}

#[test]
fn complete_delta_must_name_every_changed_component() {
    let delta = complete_delta(1, 2, Vec::new());
    let error = delta
        .validate(Some(2), &[], &[component("checkout")], &[])
        .expect_err("missing component invalidation must fail");
    assert!(error
        .to_string()
        .contains("omitted changed component evidence"));
}

#[test]
fn delta_invalidation_identifiers_are_strictly_sorted_and_unique() {
    for node_ids in [vec!["n2", "n1"], vec!["n1", "n1"]] {
        let delta = complete_delta(1, 2, node_ids);
        let error = delta
            .validate(Some(2), &[], &[], &[])
            .expect_err("non-canonical invalidation identifiers must fail");
        assert!(error.to_string().contains("duplicate, or unsorted node"));
    }

    let mut delta = complete_delta(1, 2, Vec::new());
    delta.invalidated.component_ids = vec!["z".to_string(), "a".to_string()];
    let error = delta
        .validate(Some(2), &[], &[], &[])
        .expect_err("unsorted component identifiers must fail");
    assert!(error
        .to_string()
        .contains("duplicate, or unsorted component"));
}

#[test]
fn delta_rejects_ambiguous_changed_and_removed_evidence() {
    let delta = complete_delta(1, 2, vec!["n1"]);
    let error = delta
        .validate(Some(2), &[node("n1", "Changed")], &[], &["n1".to_string()])
        .expect_err("one node cannot be changed and removed");
    assert!(error.to_string().contains("both changed and removed"));
}

#[test]
fn same_revision_delta_cannot_smuggle_invalidations() {
    let mut delta = complete_delta(2, 2, vec!["n1"]);
    delta.invalidated.ui = false;
    let error = delta
        .validate(Some(2), &[], &[], &[])
        .expect_err("same-revision invalidation must fail");
    assert!(error
        .to_string()
        .contains("same-revision deltas cannot invalidate evidence"));
}

#[test]
fn same_revision_delta_cannot_require_a_reset() {
    let delta = PageContextDelta {
        protocol: PAGE_CONTEXT_DIFF_PROTOCOL.to_string(),
        from_revision: 2,
        to_revision: 2,
        status: PageContextDeltaStatus::ResetRequired,
        invalidated: PageContextInvalidation {
            all: true,
            page: true,
            facts: true,
            ui: true,
            node_ids: Vec::new(),
            component_ids: Vec::new(),
        },
    };
    let error = delta
        .validate(Some(2), &[], &[], &[])
        .expect_err("a same-revision reset must fail");
    assert!(error.to_string().contains("must advance the revision"));
}

fn complete_delta(from_revision: u64, to_revision: u64, node_ids: Vec<&str>) -> PageContextDelta {
    PageContextDelta {
        protocol: PAGE_CONTEXT_DIFF_PROTOCOL.to_string(),
        from_revision,
        to_revision,
        status: PageContextDeltaStatus::Complete,
        invalidated: PageContextInvalidation {
            all: false,
            page: false,
            facts: false,
            ui: from_revision != to_revision,
            node_ids: node_ids.into_iter().map(str::to_string).collect(),
            component_ids: Vec::new(),
        },
    }
}

fn observation(
    revision: u64,
    nodes: Vec<PageContextNode>,
    delta: Option<PageContextDelta>,
) -> PageContextObservation {
    PageContextObservation::from_snapshot(PageContextSnapshot {
        protocol: Some(PAGE_CONTEXT_PROTOCOL.to_string()),
        sdk_version: Some("0.6.0".to_string()),
        revision: Some(revision),
        page: Some(PageContextPage {
            id: "page".to_string(),
            url: "http://127.0.0.1:3000/".to_string(),
            route: "/".to_string(),
            title: "Page".to_string(),
            ready: true,
            viewport: PageContextViewport {
                width: 1_000.0,
                height: 800.0,
                dpr: 1.0,
                visual: None,
            },
            document: PageContextSize {
                width: 1_000.0,
                height: 800.0,
            },
            scroll: PageContextPoint { x: 0.0, y: 0.0 },
            language: "en".to_string(),
            theme: PageContextTheme::Light,
        }),
        components: Vec::<PageContextComponent>::new(),
        nodes,
        facts: serde_json::Map::new(),
        ui: None,
        delta,
        removed_node_ids: Vec::new(),
        truncated: false,
        next_cursor: None,
    })
}

fn node(id: &str, text: &str) -> PageContextNode {
    PageContextNode {
        id: id.to_string(),
        r#ref: None,
        parent_id: None,
        component_id: None,
        tag: "button".to_string(),
        role: Some("button".to_string()),
        name: Some(text.to_string()),
        text: Some(text.to_string()),
        description: None,
        test_id: None,
        geometry: None,
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
        locators: vec![PageContextLocator::Css {
            value: format!("#{id}"),
        }],
        classes: None,
        attributes: None,
        computed_styles: None,
        source_mapping: None,
    }
}

fn component(id: &str) -> PageContextComponent {
    PageContextComponent {
        id: id.to_string(),
        name: id.to_string(),
        parent_id: None,
        source: None,
        ready: true,
        facts: serde_json::Map::new(),
        boxes: Vec::new(),
    }
}
