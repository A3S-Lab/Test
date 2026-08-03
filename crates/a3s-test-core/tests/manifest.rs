use a3s_test_core::{
    Action, CaptureOperation, DialogOperation, Expectation, FrameTarget, LoadState, ModifierKey,
    NetworkRoute, Surface, TabOperation, Target, TestSuite, VideoOperation, WaitCondition,
};

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
            target: Target::Label {
                value: "Document title".to_string(),
            },
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
fn parses_gui_automation_id_target() {
    let suite = TestSuite::from_acl(
        r#"
suite "gui-smoke" {
    scenario "editor" {
        surface = "gui"
        click "save" {
            target = automation_id("save-button")
        }
    }
}
"#,
    )
    .expect("valid GUI target");

    assert_eq!(
        suite.scenarios[0].steps[0].action,
        Action::Click {
            target: Target::AutomationId {
                value: "save-button".to_string(),
            },
        }
    );
}

#[test]
fn parses_gui_visual_point_target() {
    let suite = TestSuite::from_acl(
        r#"
suite "gui-vision" {
    scenario "canvas" {
        surface = "gui"
        click "draw" {
            target = visual_point("@v3", 120, 80)
        }
    }
}
"#,
    )
    .expect("valid visual point");

    assert_eq!(
        suite.scenarios[0].steps[0].action,
        Action::Click {
            target: Target::VisualPoint {
                snapshot: "@v3".to_string(),
                x: 120,
                y: 80,
            },
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
fn parses_a_typed_visible_wait_target() {
    let suite = TestSuite::from_acl(
        r#"
suite "visible-wait" {
    scenario "home" {
        surface = "web"
        wait "editor-ready" {
            visible = css("[data-editor-ready]")
        }
    }
}
"#,
    )
    .expect("visible wait must parse");

    assert_eq!(
        suite.scenarios[0].steps[0].action,
        Action::Wait {
            condition: WaitCondition::Visible(Target::Css {
                selector: "[data-editor-ready]".to_string(),
            }),
        }
    );
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

#[test]
fn parses_complete_web_protocol_actions() {
    let suite = TestSuite::from_acl(
        r##"
suite "web-depth" {
    scenario "protocol" {
        surface = "web"

        tab "open-docs" {
            operation = "new"
            url = "https://example.test/docs"
            label = "docs"
        }
        tab "switch-docs" {
            operation = "switch"
            tab = "docs"
        }
        tab "close-docs" {
            operation = "close"
            tab = "docs"
        }
        frame "payment" {
            target = css("#payment-frame")
        }
        frame "main" {
            target = main()
        }
        dialog "confirm" {
            operation = "accept"
            text = "approved"
        }
        upload "attachments" {
            target = ref("@e5")
            paths = ["fixtures/one.txt", "fixtures/two.txt"]
        }
        download "report" {
            target = css("[data-testid=download]")
            path = "downloads/report.pdf"
        }
        network_route "empty-users" {
            pattern = "**/api/users"
            body = "{\"users\":[]}"
        }
        network_route "block-analytics" {
            pattern = "**/analytics"
            abort = true
        }
        network_unroute "reset-users" {
            pattern = "**/api/users"
        }
        har "start-har" {
            operation = "start"
        }
        har "save-har" {
            operation = "stop"
            path = "network/session.har"
        }
        trace "start-trace" {
            operation = "start"
        }
        trace "save-trace" {
            operation = "stop"
            path = "traces/session.zip"
        }
        video "start-video" {
            operation = "start"
            path = "video/session.webm"
            url = "https://example.test"
        }
        video "stop-video" {
            operation = "stop"
        }
        accessibility "tree" {
            path = "evidence/tree.json"
            interactive = false
        }
        console "browser-console" {
            path = "evidence/console.json"
            clear = true
        }
        page_errors "browser-errors" {
            path = "evidence/errors.json"
            clear = false
        }
    }
}
"##,
    )
    .expect("complete web suite");

    let actions = &suite.scenarios[0].steps;
    assert_eq!(
        actions[0].action,
        Action::Tab {
            operation: TabOperation::New {
                url: Some("https://example.test/docs".to_string()),
                label: Some("docs".to_string()),
            }
        }
    );
    assert_eq!(
        actions[3].action,
        Action::Frame {
            target: FrameTarget::Selector("#payment-frame".to_string())
        }
    );
    assert_eq!(
        actions[5].action,
        Action::Dialog {
            operation: DialogOperation::Accept {
                text: Some("approved".to_string())
            }
        }
    );
    assert_eq!(
        actions[8].action,
        Action::NetworkRoute {
            pattern: "**/api/users".to_string(),
            route: NetworkRoute::Body("{\"users\":[]}".to_string()),
        }
    );
    assert_eq!(
        actions[12].action,
        Action::Har {
            operation: CaptureOperation::Stop {
                path: "network/session.har".to_string()
            }
        }
    );
    assert_eq!(
        actions[15].action,
        Action::Video {
            operation: VideoOperation::Start {
                path: "video/session.webm".to_string(),
                url: Some("https://example.test".to_string()),
            }
        }
    );
}

#[test]
fn rejects_capture_stop_without_an_artifact_path() {
    let error = TestSuite::from_acl(
        r#"
suite "invalid" {
    scenario "web" {
        surface = "web"
        trace "stop" {
            operation = "stop"
        }
    }
}
"#,
    )
    .expect_err("trace stop requires a path");

    assert_eq!(error.code(), "test.spec.attribute_required");
    assert_eq!(error.path(), "suite.invalid.scenario.web.trace.stop.path");
}

#[test]
fn rejects_ambiguous_network_routes() {
    let error = TestSuite::from_acl(
        r#"
suite "invalid" {
    scenario "web" {
        surface = "web"
        network_route "ambiguous" {
            pattern = "**/api"
            abort = true
            body = "{}"
        }
    }
}
"#,
    )
    .expect_err("route must choose abort or body");

    assert_eq!(error.code(), "test.spec.condition_ambiguous");
}

#[test]
fn parses_advanced_web_interaction_actions() {
    let suite = TestSuite::from_acl(
        r##"
suite "advanced-web" {
    scenario "interactions" {
        surface = "web"

        hover "toolbar" {
            target = role("button", "Toolbar")
        }
        focus "title" {
            target = label("Title")
        }
        double_click "word" {
            target = ref("@e2")
        }
        context_click "selection" {
            target = css(".selection")
        }
        type "append" {
            target = placeholder("Start writing")
            value = "More text"
        }
        check "comments" {
            target = testid("comments")
        }
        uncheck "readonly" {
            target = css("#readonly")
        }
        select "status" {
            target = ref("@e8")
            values = ["draft", "review"]
        }
        drag "resize" {
            source = ref("@e10")
            target = css("#resize-target")
        }
        wheel "zoom" {
            target = css(".document-page")
            delta_y = -120
            delta_x = 4
            modifiers = ["control", "shift"]
        }
        viewport "desktop" {
            width = 1440
            height = 900
            scale = 2
        }
    }
}
"##,
    )
    .expect("advanced Web suite");

    let actions = &suite.scenarios[0].steps;
    assert_eq!(
        actions[0].action,
        Action::Hover {
            target: Target::Role {
                role: "button".to_string(),
                name: "Toolbar".to_string(),
            }
        }
    );
    assert_eq!(
        actions[2].action,
        Action::DoubleClick {
            target: Target::Ref {
                value: "@e2".to_string()
            }
        }
    );
    assert_eq!(
        actions[4].action,
        Action::Type {
            target: Target::Placeholder {
                value: "Start writing".to_string()
            },
            value: "More text".to_string(),
        }
    );
    assert_eq!(
        actions[7].action,
        Action::Select {
            target: Target::Ref {
                value: "@e8".to_string()
            },
            values: vec!["draft".to_string(), "review".to_string()],
        }
    );
    assert_eq!(
        actions[8].action,
        Action::Drag {
            source: Target::Ref {
                value: "@e10".to_string()
            },
            target: Target::Css {
                selector: "#resize-target".to_string()
            },
        }
    );
    assert_eq!(
        actions[9].action,
        Action::Wheel {
            target: Some(Target::Css {
                selector: ".document-page".to_string()
            }),
            delta_x: 4,
            delta_y: -120,
            modifiers: vec![ModifierKey::Control, ModifierKey::Shift],
        }
    );
    assert_eq!(
        actions[10].action,
        Action::Viewport {
            width: 1440,
            height: 900,
            scale: Some(2),
        }
    );
}

#[test]
fn rejects_invalid_advanced_interaction_values() {
    for (source, code) in [
        (
            r#"
suite "invalid" {
    scenario "web" {
        surface = "web"
        wheel "noop" {
            delta_y = 0
        }
    }
}
"#,
            "test.spec.wheel_delta_required",
        ),
        (
            r#"
suite "invalid" {
    scenario "web" {
        surface = "web"
        wheel "duplicate-modifier" {
            delta_y = 120
            modifiers = ["control", "control"]
        }
    }
}
"#,
            "test.spec.modifier_duplicate",
        ),
        (
            r#"
suite "invalid" {
    scenario "web" {
        surface = "web"
        viewport "zero" {
            width = 0
            height = 900
        }
    }
}
"#,
            "test.spec.number_range",
        ),
    ] {
        let error = TestSuite::from_acl(source).expect_err("invalid advanced action must fail");
        assert_eq!(error.code(), code);
    }
}
