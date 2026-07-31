use a3s_test_core::{Action, Expectation, LoadState, Surface, Target, TestSuite, WaitCondition};

const VALID_SUITE: &str = r#"
suite "office-smoke" {
    version = 1

    scenario "word-editor" {
        name = "Create a document"
        surface = "web"
        timeout_ms = 30000

        navigate "open-playground" {
            url = "https://example.test/playground"
        }

        click "choose-word" {
            target = role("button", "Word")
        }

        fill "document-title" {
            target = label("Document title")
            value = "Quarterly plan"
        }

        wait "editor-ready" {
            load = "networkidle"
        }

        expect "title-visible" {
            text = "Quarterly plan"
        }

        screenshot "final-state" {
            path = "word/final.png"
        }
    }
}
"#;

#[test]
fn parses_ordered_typed_web_scenario() {
    let suite = TestSuite::from_acl(VALID_SUITE).expect("valid suite");

    assert_eq!(suite.name, "office-smoke");
    assert_eq!(suite.version, 1);
    assert_eq!(suite.scenarios.len(), 1);

    let scenario = &suite.scenarios[0];
    assert_eq!(scenario.id, "word-editor");
    assert_eq!(scenario.name, "Create a document");
    assert_eq!(scenario.surface, Surface::Web);
    assert_eq!(scenario.timeout_ms, 30_000);
    assert_eq!(scenario.steps.len(), 6);
    assert_eq!(
        scenario.steps[0].action,
        Action::Navigate {
            url: "https://example.test/playground".to_string()
        }
    );
    assert_eq!(
        scenario.steps[1].action,
        Action::Click {
            target: Target::Role {
                role: "button".to_string(),
                name: "Word".to_string(),
            }
        }
    );
    assert_eq!(
        scenario.steps[2].action,
        Action::Fill {
            target: Target::Label("Document title".to_string()),
            value: "Quarterly plan".to_string(),
        }
    );
    assert_eq!(
        scenario.steps[3].action,
        Action::Wait {
            condition: WaitCondition::Load(LoadState::NetworkIdle)
        }
    );
    assert_eq!(
        scenario.steps[4].action,
        Action::Assert {
            expectation: Expectation::TextVisible("Quarterly plan".to_string())
        }
    );
}

#[test]
fn rejects_unknown_actions_with_a_stable_location() {
    let error = TestSuite::from_acl(
        r#"
suite "invalid" {
    scenario "home" {
        surface = "web"
        guess "not-a-real-action" {}
    }
}
"#,
    )
    .expect_err("unknown action must fail");

    assert_eq!(error.code(), "test.spec.action_unknown");
    assert_eq!(error.path(), "suite.invalid.scenario.home.guess");
}

#[test]
fn requires_exactly_one_wait_condition() {
    let error = TestSuite::from_acl(
        r#"
suite "invalid" {
    scenario "home" {
        surface = "web"
        wait "ambiguous" {
            text = "Ready"
            url = "https://example.test"
        }
    }
}
"#,
    )
    .expect_err("ambiguous wait must fail");

    assert_eq!(error.code(), "test.spec.condition_ambiguous");
}

#[test]
fn rejects_unrecognized_attributes_instead_of_silently_ignoring_them() {
    let error = TestSuite::from_acl(
        r#"
suite "invalid" {
    scenario "home" {
        surface = "web"
        navigate "open" {
            url = "https://example.test"
            retries = 10
        }
    }
}
"#,
    )
    .expect_err("unknown attribute must fail");

    assert_eq!(error.code(), "test.spec.attribute_unknown");
    assert_eq!(
        error.path(),
        "suite.invalid.scenario.home.navigate.open.retries"
    );
}

#[test]
fn rejects_identifiers_that_could_escape_artifact_roots() {
    let error = TestSuite::from_acl(
        r#"
suite "invalid" {
    scenario "../outside" {
        surface = "web"
        navigate "open" {
            url = "https://example.test"
        }
    }
}
"#,
    )
    .expect_err("unsafe identifier must fail");

    assert_eq!(error.code(), "test.spec.identifier_invalid");
    assert_eq!(error.path(), "suite.invalid.scenario.../outside");
}
