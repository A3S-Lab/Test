use std::collections::BTreeMap;
use std::fmt::Write as _;

use a3s_test_core::{
    action_uses_observation_target, action_uses_page_context_ref, resolve_page_context_refs,
    validate_action_page_context_refs, Action, Expectation, LayoutRect, LayoutRelation,
    PageContextBindings, Target, TestSuite, ACTION_PROTOCOL_REVISION, MAX_LAYOUT_COORDINATE_ABS,
    MAX_LAYOUT_TOLERANCE_PX,
};

#[test]
fn acl_admits_every_layout_relation_with_explicit_geometry_policy() {
    let relations = [
        ("above", LayoutRelation::Above),
        ("below", LayoutRelation::Below),
        ("left_of", LayoutRelation::LeftOf),
        ("right_of", LayoutRelation::RightOf),
        ("contains", LayoutRelation::Contains),
        ("inside", LayoutRelation::Inside),
        ("overlaps", LayoutRelation::Overlaps),
        ("not_overlapping", LayoutRelation::NotOverlapping),
        ("aligned_left", LayoutRelation::AlignedLeft),
        ("aligned_right", LayoutRelation::AlignedRight),
        ("aligned_top", LayoutRelation::AlignedTop),
        ("aligned_bottom", LayoutRelation::AlignedBottom),
        ("aligned_center_x", LayoutRelation::AlignedCenterX),
        ("aligned_center_y", LayoutRelation::AlignedCenterY),
        ("same_width", LayoutRelation::SameWidth),
        ("same_height", LayoutRelation::SameHeight),
        ("same_size", LayoutRelation::SameSize),
    ];
    let mut scenarios = String::new();
    for (index, (name, _)) in relations.iter().enumerate() {
        write!(
            scenarios,
            r#"
    scenario "layout-{index}" {{
        surface = "web"
        expect "relation" {{
            target = testid("subject-{index}")
            relative_to = role("region", "reference-{index}")
            layout = "{name}"
            tolerance_px = 2
            stable_for_ms = 100
            sample_interval_ms = 25
        }}
    }}
"#
        )
        .expect("write layout relation scenario");
    }
    let acl = format!(
        r#"
suite "layout-relations" {{
    version = 1
{scenarios}
}}
"#
    );

    let suite = TestSuite::from_acl(&acl).expect("layout relation ACL");
    assert_eq!(suite.scenarios.len(), relations.len());
    for (scenario, (_, expected_relation)) in suite.scenarios.iter().zip(relations) {
        let Action::Assert {
            expectation:
                Expectation::Layout {
                    target,
                    relative_to,
                    relation,
                    tolerance_px,
                },
        } = &scenario.steps[0].action
        else {
            panic!("expected typed layout assertion");
        };
        assert!(matches!(target, Target::TestId { .. }));
        assert!(matches!(relative_to, Target::Role { .. }));
        assert_eq!(*relation, expected_relation);
        assert_eq!(*tolerance_px, 2);
        assert_eq!(scenario.steps[0].stability.unwrap().planned_samples(), 5);
    }
}

#[test]
fn layout_acl_rejects_unstable_targets_unknown_relations_and_invalid_tolerance() {
    for (body, code, path) in [
        (
            r##"target = ref("@e1") relative_to = css("#reference") layout = "above""##,
            "test.spec.layout_target_unstable",
            ".target",
        ),
        (
            r##"target = css("#subject") relative_to = ref("@e2") layout = "above""##,
            "test.spec.layout_target_unstable",
            ".relative_to",
        ),
        (
            r##"target = visual_point("@v1", 10, 10) relative_to = css("#reference") layout = "above""##,
            "test.spec.layout_target_unstable",
            ".target",
        ),
        (
            r##"target = css("#subject") relative_to = visual_point("@v1", 10, 10) layout = "above""##,
            "test.spec.layout_target_unstable",
            ".relative_to",
        ),
        (
            r##"target = css("#subject") relative_to = css("#reference") layout = "near""##,
            "test.spec.layout_relation_unknown",
            ".layout",
        ),
        (
            r##"target = css("#subject") relative_to = css("#reference") layout = "above" tolerance_px = -1"##,
            "test.spec.number_range",
            ".tolerance_px",
        ),
        (
            r##"target = css("#subject") relative_to = css("#reference") layout = "above" tolerance_px = 1.5"##,
            "test.spec.number_range",
            ".tolerance_px",
        ),
        (
            r##"target = css("#subject") relative_to = css("#reference") layout = "above" tolerance_px = 1025"##,
            "test.spec.layout_tolerance_limit",
            ".tolerance_px",
        ),
    ] {
        let acl = format!(
            r#"
suite "invalid-layout" {{
    scenario "layout" {{
        surface = "web"
        expect "relation" {{ {body} }}
    }}
}}
"#
        );
        let error = TestSuite::from_acl(&acl).expect_err("invalid layout ACL");
        assert_eq!(error.code(), code, "{body}");
        assert!(error.path().ends_with(path), "{}", error.path());
    }

    for missing in [
        r##"relative_to = css("#reference") layout = "above""##,
        r##"target = css("#subject") layout = "above""##,
    ] {
        let acl = format!(
            r#"
suite "missing-layout-target" {{
    scenario "layout" {{
        surface = "web"
        expect "relation" {{ {missing} }}
    }}
}}
"#
        );
        let error = TestSuite::from_acl(&acl).expect_err("missing layout target");
        assert_eq!(error.code(), "test.spec.attribute_required");
    }
}

#[test]
fn layout_relation_truth_table_is_explicit_and_complete() {
    let reference = rect(100.0, 100.0, 100.0, 100.0);
    for (relation, matching, violating) in [
        (
            LayoutRelation::Above,
            rect(120.0, 40.0, 40.0, 50.0),
            rect(120.0, 110.0, 40.0, 50.0),
        ),
        (
            LayoutRelation::Below,
            rect(120.0, 210.0, 40.0, 50.0),
            rect(120.0, 140.0, 40.0, 50.0),
        ),
        (
            LayoutRelation::LeftOf,
            rect(40.0, 120.0, 50.0, 40.0),
            rect(110.0, 120.0, 50.0, 40.0),
        ),
        (
            LayoutRelation::RightOf,
            rect(210.0, 120.0, 50.0, 40.0),
            rect(140.0, 120.0, 50.0, 40.0),
        ),
        (
            LayoutRelation::Contains,
            rect(90.0, 90.0, 120.0, 120.0),
            rect(110.0, 90.0, 120.0, 120.0),
        ),
        (
            LayoutRelation::Inside,
            rect(120.0, 120.0, 40.0, 40.0),
            rect(80.0, 120.0, 40.0, 40.0),
        ),
        (
            LayoutRelation::Overlaps,
            rect(180.0, 180.0, 40.0, 40.0),
            rect(210.0, 210.0, 40.0, 40.0),
        ),
        (
            LayoutRelation::NotOverlapping,
            rect(210.0, 210.0, 40.0, 40.0),
            rect(180.0, 180.0, 40.0, 40.0),
        ),
        (
            LayoutRelation::AlignedLeft,
            rect(100.0, 230.0, 40.0, 40.0),
            rect(102.0, 230.0, 40.0, 40.0),
        ),
        (
            LayoutRelation::AlignedRight,
            rect(140.0, 230.0, 60.0, 40.0),
            rect(139.0, 230.0, 60.0, 40.0),
        ),
        (
            LayoutRelation::AlignedTop,
            rect(230.0, 100.0, 40.0, 40.0),
            rect(230.0, 102.0, 40.0, 40.0),
        ),
        (
            LayoutRelation::AlignedBottom,
            rect(230.0, 140.0, 40.0, 60.0),
            rect(230.0, 139.0, 40.0, 60.0),
        ),
        (
            LayoutRelation::AlignedCenterX,
            rect(130.0, 230.0, 40.0, 40.0),
            rect(132.0, 230.0, 40.0, 40.0),
        ),
        (
            LayoutRelation::AlignedCenterY,
            rect(230.0, 130.0, 40.0, 40.0),
            rect(230.0, 132.0, 40.0, 40.0),
        ),
        (
            LayoutRelation::SameWidth,
            rect(230.0, 230.0, 100.0, 40.0),
            rect(230.0, 230.0, 99.0, 40.0),
        ),
        (
            LayoutRelation::SameHeight,
            rect(230.0, 230.0, 40.0, 100.0),
            rect(230.0, 230.0, 40.0, 99.0),
        ),
        (
            LayoutRelation::SameSize,
            rect(230.0, 230.0, 100.0, 100.0),
            rect(230.0, 230.0, 100.0, 99.0),
        ),
    ] {
        assert!(relation.matches(matching, reference, 0), "{relation:?}");
        assert!(!relation.matches(violating, reference, 0), "{relation:?}");
    }
}

#[test]
fn layout_tolerance_has_relation_specific_boundary_semantics() {
    let reference = rect(100.0, 100.0, 100.0, 100.0);
    let one_pixel_vertical_overlap = rect(120.0, 50.0, 40.0, 51.0);
    assert!(!LayoutRelation::Above.matches(one_pixel_vertical_overlap, reference, 0));
    assert!(LayoutRelation::Above.matches(one_pixel_vertical_overlap, reference, 1));

    let one_pixel_left_drift = rect(101.0, 230.0, 40.0, 40.0);
    assert!(!LayoutRelation::AlignedLeft.matches(one_pixel_left_drift, reference, 0));
    assert!(LayoutRelation::AlignedLeft.matches(one_pixel_left_drift, reference, 1));

    let one_pixel_overlap = rect(199.0, 120.0, 40.0, 40.0);
    assert!(LayoutRelation::Overlaps.matches(one_pixel_overlap, reference, 0));
    assert!(!LayoutRelation::Overlaps.matches(one_pixel_overlap, reference, 1));
    assert!(!LayoutRelation::NotOverlapping.matches(one_pixel_overlap, reference, 0));
    assert!(LayoutRelation::NotOverlapping.matches(one_pixel_overlap, reference, 1));
}

#[test]
fn layout_rect_validation_bounds_untrusted_surface_geometry() {
    assert!(rect(0.0, 0.0, 1.0, 1.0).is_valid());
    for invalid in [
        LayoutRect {
            x: f64::NAN,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        },
        LayoutRect {
            x: 0.0,
            y: f64::INFINITY,
            width: 1.0,
            height: 1.0,
        },
        rect(0.0, 0.0, 0.0, 1.0),
        rect(0.0, 0.0, 1.0, 0.0),
        rect(MAX_LAYOUT_COORDINATE_ABS, 0.0, 1.0, 1.0),
        rect(0.0, MAX_LAYOUT_COORDINATE_ABS, 1.0, 1.0),
        rect(0.0, 0.0, MAX_LAYOUT_COORDINATE_ABS + 1.0, 1.0),
    ] {
        assert!(!invalid.is_valid(), "{invalid:?}");
    }
    assert_eq!(MAX_LAYOUT_TOLERANCE_PX, 1024);
}

#[test]
fn layout_assertion_has_a_revision_eleven_wire_contract() {
    assert_eq!(ACTION_PROTOCOL_REVISION, 14);
    let action = Action::Assert {
        expectation: Expectation::Layout {
            target: Target::TestId {
                value: "checkout".to_string(),
            },
            relative_to: Target::TestId {
                value: "summary".to_string(),
            },
            relation: LayoutRelation::Below,
            tolerance_px: 1,
        },
    };
    let encoded = serde_json::to_value(&action).expect("layout action JSON");
    assert_eq!(
        encoded,
        serde_json::json!({
            "type": "assert",
            "expectation": {
                "type": "layout",
                "value": {
                    "target": { "type": "test_id", "value": "checkout" },
                    "relative_to": { "type": "test_id", "value": "summary" },
                    "relation": "below",
                    "tolerance_px": 1
                }
            }
        })
    );
    assert_eq!(
        serde_json::from_value::<Action>(encoded).expect("decode layout action"),
        action
    );
}

#[test]
fn layout_assertion_resolves_both_page_context_targets_and_rejects_ui_evidence_refs() {
    let action = Action::Assert {
        expectation: Expectation::Layout {
            target: Target::Ref {
                value: "@c1".to_string(),
            },
            relative_to: Target::Ref {
                value: "@c2".to_string(),
            },
            relation: LayoutRelation::Inside,
            tolerance_px: 0,
        },
    };
    assert!(action_uses_observation_target(&action));
    assert!(action_uses_page_context_ref(&action));

    let bindings = PageContextBindings {
        revision: Some(7),
        targets: BTreeMap::from([
            (
                "@c1".to_string(),
                Target::TestId {
                    value: "subject".to_string(),
                },
            ),
            (
                "@c2".to_string(),
                Target::Css {
                    selector: "#reference".to_string(),
                },
            ),
        ]),
    };
    let resolved = resolve_page_context_refs(action, &bindings).expect("both context refs");
    let Action::Assert {
        expectation:
            Expectation::Layout {
                target,
                relative_to,
                ..
            },
    } = resolved
    else {
        panic!("expected resolved layout action");
    };
    assert_eq!(
        target,
        Target::TestId {
            value: "subject".to_string()
        }
    );
    assert_eq!(
        relative_to,
        Target::Css {
            selector: "#reference".to_string()
        }
    );

    for (target, relative_to) in [("@u1", "@c2"), ("@c1", "@u2")] {
        let invalid = Action::Assert {
            expectation: Expectation::Layout {
                target: Target::Ref {
                    value: target.to_string(),
                },
                relative_to: Target::Ref {
                    value: relative_to.to_string(),
                },
                relation: LayoutRelation::Inside,
                tolerance_px: 0,
            },
        };
        let error = validate_action_page_context_refs(&invalid)
            .expect_err("UI evidence refs are not actionable");
        assert!(error.message().contains("observation-only"));
    }
}

fn rect(x: f64, y: f64, width: f64, height: f64) -> LayoutRect {
    LayoutRect {
        x,
        y,
        width,
        height,
    }
}
