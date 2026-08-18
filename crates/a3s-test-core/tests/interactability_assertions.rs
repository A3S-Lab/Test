use std::collections::BTreeMap;

use a3s_test_core::{
    action_uses_observation_target, action_uses_page_context_ref, resolve_page_context_refs,
    validate_action_page_context_refs, Action, Expectation, LayoutRect, PageContextBindings,
    Target, TestSuite, ACTION_PROTOCOL_REVISION,
};

#[test]
fn acl_admits_orthogonal_viewport_and_pointer_reachability_expectations() {
    let suite = TestSuite::from_acl(
        r#"
suite "interactability" {
    version = 1

    scenario "viewport" {
        surface = "web"

        expect "checkout-in-view" {
            in_viewport = testid("checkout")
            stable_for_ms = 100
            sample_interval_ms = 25
        }

        expect "checkout-pointer-hit" {
            pointer_reachable = role("button", "Checkout")
            stable_for_ms = 100
            sample_interval_ms = 25
        }
    }
}
"#,
    )
    .expect("interactability ACL");

    assert_eq!(suite.scenarios[0].steps.len(), 2);
    assert!(matches!(
        &suite.scenarios[0].steps[0].action,
        Action::Assert {
            expectation: Expectation::InViewport(Target::TestId { value })
        } if value == "checkout"
    ));
    assert!(matches!(
        &suite.scenarios[0].steps[1].action,
        Action::Assert {
            expectation: Expectation::PointerReachable(Target::Role { role, name })
        } if role == "button" && name == "Checkout"
    ));
    assert_eq!(
        suite.scenarios[0].steps[0]
            .stability
            .expect("viewport stability")
            .planned_samples(),
        5
    );
}

#[test]
fn interactability_acl_rejects_observation_bound_targets() {
    for (condition, code) in [
        (
            r#"in_viewport = ref("@e1")"#,
            "test.spec.in_viewport_target_unstable",
        ),
        (
            r#"in_viewport = visual_point("@v1", 10, 20)"#,
            "test.spec.in_viewport_target_unstable",
        ),
        (
            r#"pointer_reachable = ref("@e2")"#,
            "test.spec.pointer_reachable_target_unstable",
        ),
        (
            r#"pointer_reachable = visual_point("@v2", 30, 40)"#,
            "test.spec.pointer_reachable_target_unstable",
        ),
    ] {
        let acl = format!(
            r#"
suite "invalid-interactability" {{
    scenario "invalid" {{
        surface = "web"
        expect "probe" {{ {condition} }}
    }}
}}
"#
        );
        let error = TestSuite::from_acl(&acl).expect_err("unstable interactability target");
        assert_eq!(error.code(), code, "{condition}");
        assert!(
            error.path().ends_with(".probe.in_viewport")
                || error.path().ends_with(".probe.pointer_reachable")
        );
    }
}

#[test]
fn viewport_intersection_truth_table_classifies_1000_geometry_cases() {
    let viewport = rect(0.0, 0.0, 1_000.0, 800.0);
    let mut inside = 0;
    let mut partial = 0;
    let mut outside = 0;

    for index in 0..200 {
        let target = rect(
            10.0 + f64::from(index % 20) * 40.0,
            10.0 + f64::from(index / 20) * 30.0,
            20.0,
            20.0,
        );
        let ratio = target
            .intersection_ratio(viewport)
            .expect("valid inside geometry");
        assert!((ratio - 1.0).abs() < f64::EPSILON);
        inside += 1;
    }

    for edge in 0..4 {
        for index in 0..100 {
            let overlap = if edge < 2 {
                1.0 + f64::from(index % 49)
            } else {
                1.0 + f64::from(index % 39)
            };
            let target = match edge {
                0 => rect(overlap - 50.0, 100.0 + f64::from(index), 50.0, 40.0),
                1 => rect(1_000.0 - overlap, 100.0 + f64::from(index), 50.0, 40.0),
                2 => rect(100.0 + f64::from(index), overlap - 40.0, 50.0, 40.0),
                _ => rect(100.0 + f64::from(index), 800.0 - overlap, 50.0, 40.0),
            };
            let ratio = target
                .intersection_ratio(viewport)
                .expect("valid partial geometry");
            assert!(ratio > 0.0 && ratio < 1.0, "edge={edge} index={index}");
            partial += 1;
        }
    }

    for edge in 0..4 {
        for index in 0..100 {
            let gap = f64::from(index % 25);
            let target = match edge {
                0 => rect(-50.0 - gap, 100.0, 50.0, 40.0),
                1 => rect(1_000.0 + gap, 100.0, 50.0, 40.0),
                2 => rect(100.0, -40.0 - gap, 50.0, 40.0),
                _ => rect(100.0, 800.0 + gap, 50.0, 40.0),
            };
            assert_eq!(
                target
                    .intersection_ratio(viewport)
                    .expect("valid outside geometry"),
                0.0,
                "edge={edge} index={index}"
            );
            outside += 1;
        }
    }

    assert_eq!((inside, partial, outside), (200, 400, 400));
}

#[test]
fn viewport_intersection_rejects_invalid_untrusted_geometry() {
    let viewport = rect(0.0, 0.0, 1_000.0, 800.0);
    assert_eq!(
        rect(0.0, 0.0, 10.0, 10.0).intersection_ratio(viewport),
        Some(1.0)
    );
    assert_eq!(
        LayoutRect {
            x: f64::NAN,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        }
        .intersection_ratio(viewport),
        None
    );
    assert_eq!(rect(0.0, 0.0, 0.0, 10.0).intersection_ratio(viewport), None);
    assert_eq!(
        rect(0.0, 0.0, 10.0, 10.0).intersection_ratio(rect(0.0, 0.0, 0.0, 10.0)),
        None
    );
}

#[test]
fn interactability_assertions_have_a_revision_twelve_wire_contract() {
    assert_eq!(ACTION_PROTOCOL_REVISION, 12);
    for (expectation, encoded_expectation) in [
        (
            Expectation::InViewport(Target::TestId {
                value: "checkout".to_string(),
            }),
            serde_json::json!({
                "type": "in_viewport",
                "value": { "type": "test_id", "value": "checkout" }
            }),
        ),
        (
            Expectation::PointerReachable(Target::TestId {
                value: "checkout".to_string(),
            }),
            serde_json::json!({
                "type": "pointer_reachable",
                "value": { "type": "test_id", "value": "checkout" }
            }),
        ),
    ] {
        let action = Action::Assert {
            expectation: expectation.clone(),
        };
        let encoded = serde_json::to_value(&action).expect("interactability action JSON");
        assert_eq!(encoded["expectation"], encoded_expectation);
        assert_eq!(
            serde_json::from_value::<Action>(encoded).expect("decode interactability action"),
            action
        );
    }
}

#[test]
fn interactability_assertions_resolve_page_context_targets_and_reject_ui_evidence_refs() {
    let bindings = PageContextBindings {
        revision: Some(12),
        targets: BTreeMap::from([(
            "@c1".to_string(),
            Target::TestId {
                value: "checkout".to_string(),
            },
        )]),
    };
    for expectation in [
        Expectation::InViewport(Target::Ref {
            value: "@c1".to_string(),
        }),
        Expectation::PointerReachable(Target::Ref {
            value: "@c1".to_string(),
        }),
    ] {
        let action = Action::Assert { expectation };
        assert!(action_uses_observation_target(&action));
        assert!(action_uses_page_context_ref(&action));
        let resolved = resolve_page_context_refs(action, &bindings).expect("Page Context ref");
        assert!(matches!(
            resolved,
            Action::Assert {
                expectation: Expectation::InViewport(Target::TestId { .. })
                    | Expectation::PointerReachable(Target::TestId { .. })
            }
        ));
    }

    for expectation in [
        Expectation::InViewport(Target::Ref {
            value: "@u1".to_string(),
        }),
        Expectation::PointerReachable(Target::Ref {
            value: "@u2".to_string(),
        }),
    ] {
        let error = validate_action_page_context_refs(&Action::Assert { expectation })
            .expect_err("UI evidence refs are observation-only");
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
