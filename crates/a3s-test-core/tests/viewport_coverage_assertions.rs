use std::collections::BTreeMap;

use a3s_test_core::{
    action_uses_observation_target, action_uses_page_context_ref, resolve_page_context_refs,
    validate_action_page_context_refs, Action, Expectation, LayoutRect, PageContextBindings,
    Target, TestSuite, ViewportCoverageComparison, ACTION_PROTOCOL_REVISION,
    MAX_VIEWPORT_COVERAGE_PERCENT,
};

#[test]
fn acl_admits_bounded_viewport_coverage_expectations() {
    let suite = TestSuite::from_acl(
        r##"
suite "viewport-coverage" {
    version = 1

    scenario "coverage" {
        surface = "web"

        expect "hero-mostly-visible" {
            target = testid("hero")
            viewport_coverage_at_least = 80
            stable_for_ms = 100
            sample_interval_ms = 25
        }

        expect "drawer-mostly-outside" {
            target = css("#drawer")
            viewport_coverage_at_most = 10
        }
    }
}
"##,
    )
    .expect("viewport coverage ACL");

    assert_eq!(suite.scenarios[0].steps.len(), 2);
    assert!(matches!(
        &suite.scenarios[0].steps[0].action,
        Action::Assert {
            expectation: Expectation::ViewportCoverage {
                target: Target::TestId { value },
                comparison: ViewportCoverageComparison::AtLeast,
                percent: 80,
            }
        } if value == "hero"
    ));
    assert!(matches!(
        &suite.scenarios[0].steps[1].action,
        Action::Assert {
            expectation: Expectation::ViewportCoverage {
                target: Target::Css { selector },
                comparison: ViewportCoverageComparison::AtMost,
                percent: 10,
            }
        } if selector == "#drawer"
    ));
    assert_eq!(
        suite.scenarios[0].steps[0]
            .stability
            .expect("coverage stability")
            .planned_samples(),
        5
    );
}

#[test]
fn viewport_coverage_acl_rejects_unstable_targets_and_invalid_thresholds() {
    for condition in ["viewport_coverage_at_least", "viewport_coverage_at_most"] {
        for target in ["ref(\"@e1\")", "visual_point(\"@v1\", 10, 20)"] {
            let acl = coverage_acl(condition, "50", target);
            let error = TestSuite::from_acl(&acl).expect_err("unstable coverage target");
            assert_eq!(error.code(), "test.spec.viewport_coverage_target_unstable");
            assert!(error.path().ends_with(".target"));
        }
    }

    for (condition, percent, code) in [
        (
            "viewport_coverage_at_least",
            "0",
            "test.spec.viewport_coverage_threshold_trivial",
        ),
        (
            "viewport_coverage_at_least",
            "101",
            "test.spec.viewport_coverage_percent_limit",
        ),
        (
            "viewport_coverage_at_least",
            "256",
            "test.spec.viewport_coverage_percent_limit",
        ),
        (
            "viewport_coverage_at_most",
            "100",
            "test.spec.viewport_coverage_threshold_trivial",
        ),
        (
            "viewport_coverage_at_most",
            "101",
            "test.spec.viewport_coverage_percent_limit",
        ),
    ] {
        let acl = coverage_acl(condition, percent, "testid(\"hero\")");
        let error = TestSuite::from_acl(&acl).expect_err("invalid coverage threshold");
        assert_eq!(error.code(), code, "{condition}={percent}");
        assert!(error.path().ends_with(condition));
    }
}

#[test]
fn viewport_coverage_truth_table_classifies_2000_of_2000_cases() {
    let mut classified = 0;
    for index in 0..500 {
        let threshold = u8::try_from(index % 100 + 1).expect("bounded threshold");
        let matching = coverage_ratio(threshold);
        let violating = coverage_ratio(threshold - 1);
        assert!(ViewportCoverageComparison::AtLeast.matches(matching, threshold));
        assert!(!ViewportCoverageComparison::AtLeast.matches(violating, threshold));
        classified += 2;
    }
    for index in 0..500 {
        let threshold = u8::try_from(index % 100).expect("bounded threshold");
        let matching = coverage_ratio(threshold);
        let violating = coverage_ratio(threshold + 1);
        assert!(ViewportCoverageComparison::AtMost.matches(matching, threshold));
        assert!(!ViewportCoverageComparison::AtMost.matches(violating, threshold));
        classified += 2;
    }
    assert_eq!(classified, 2_000);
}

#[test]
fn viewport_coverage_comparison_rejects_unknown_or_trivial_claims() {
    assert_eq!(MAX_VIEWPORT_COVERAGE_PERCENT, 100);
    assert!(ViewportCoverageComparison::AtLeast.threshold_is_valid(1));
    assert!(ViewportCoverageComparison::AtLeast.threshold_is_valid(100));
    assert!(!ViewportCoverageComparison::AtLeast.threshold_is_valid(0));
    assert!(ViewportCoverageComparison::AtMost.threshold_is_valid(0));
    assert!(ViewportCoverageComparison::AtMost.threshold_is_valid(99));
    assert!(!ViewportCoverageComparison::AtMost.threshold_is_valid(100));

    for ratio in [f64::NAN, f64::INFINITY, -0.01, 1.01] {
        assert!(!ViewportCoverageComparison::AtLeast.matches(ratio, 50));
        assert!(!ViewportCoverageComparison::AtMost.matches(ratio, 50));
    }
}

#[test]
fn viewport_coverage_has_a_revision_fifteen_wire_contract() {
    assert_eq!(ACTION_PROTOCOL_REVISION, 15);
    for (comparison, percent, encoded_comparison) in [
        (ViewportCoverageComparison::AtLeast, 80, "at_least"),
        (ViewportCoverageComparison::AtMost, 10, "at_most"),
    ] {
        let action = Action::Assert {
            expectation: Expectation::ViewportCoverage {
                target: Target::TestId {
                    value: "hero".to_string(),
                },
                comparison,
                percent,
            },
        };
        let encoded = serde_json::to_value(&action).expect("viewport coverage action JSON");
        assert_eq!(encoded["expectation"]["type"], "viewport_coverage");
        assert_eq!(
            encoded["expectation"]["value"]["comparison"],
            encoded_comparison
        );
        assert_eq!(encoded["expectation"]["value"]["percent"], percent);
        assert_eq!(
            serde_json::from_value::<Action>(encoded).expect("decode viewport coverage action"),
            action
        );
    }
}

#[test]
fn viewport_coverage_resolves_page_context_targets_and_rejects_ui_evidence_refs() {
    let bindings = PageContextBindings {
        revision: Some(15),
        targets: BTreeMap::from([(
            "@c1".to_string(),
            Target::TestId {
                value: "hero".to_string(),
            },
        )]),
    };
    let action = Action::Assert {
        expectation: Expectation::ViewportCoverage {
            target: Target::Ref {
                value: "@c1".to_string(),
            },
            comparison: ViewportCoverageComparison::AtLeast,
            percent: 80,
        },
    };
    assert!(action_uses_observation_target(&action));
    assert!(action_uses_page_context_ref(&action));
    assert!(matches!(
        resolve_page_context_refs(action, &bindings).expect("Page Context ref"),
        Action::Assert {
            expectation: Expectation::ViewportCoverage {
                target: Target::TestId { .. },
                ..
            }
        }
    ));

    let error = validate_action_page_context_refs(&Action::Assert {
        expectation: Expectation::ViewportCoverage {
            target: Target::Ref {
                value: "@u1".to_string(),
            },
            comparison: ViewportCoverageComparison::AtMost,
            percent: 0,
        },
    })
    .expect_err("UI evidence refs are observation-only");
    assert!(error.message().contains("observation-only"));
}

fn coverage_acl(condition: &str, percent: &str, target: &str) -> String {
    format!(
        r#"
suite "invalid-viewport-coverage" {{
    scenario "invalid" {{
        surface = "web"
        expect "coverage" {{
            target = {target}
            {condition} = {percent}
        }}
    }}
}}
"#
    )
}

fn coverage_ratio(percent: u8) -> f64 {
    let target = rect(0.0, 0.0, 100.0, 100.0);
    let viewport = if percent == 0 {
        rect(100.0, 0.0, 100.0, 100.0)
    } else {
        rect(0.0, 0.0, f64::from(percent), 100.0)
    };
    target
        .intersection_ratio(viewport)
        .expect("valid coverage geometry")
}

fn rect(x: f64, y: f64, width: f64, height: f64) -> LayoutRect {
    LayoutRect {
        x,
        y,
        width,
        height,
    }
}
