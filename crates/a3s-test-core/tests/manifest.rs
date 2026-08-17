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
fn parses_runner_owned_surface_contract_verification() {
    let suite = TestSuite::from_acl(
        r#"
suite "contract-smoke" {
    scenario "checkout" {
        surface = "web"
        verify_contract "ready" {
            contract = "./contracts/checkout.acl"
            variant = "desktop"
            state = "ready"
        }
    }
}
"#,
    )
    .expect("contract verification action");

    assert_eq!(
        suite.scenarios[0].steps[0].action,
        Action::VerifyContract {
            contract: "./contracts/checkout.acl".to_string(),
            variant: "desktop".to_string(),
            state: "ready".to_string(),
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
fn rejects_ui_evidence_refs_during_acl_admission() {
    let error = TestSuite::from_acl(
        r#"
suite "invalid-ui-ref" {
    scenario "page" {
        surface = "web"
        click "evidence-only" {
            target = ref("@u1")
        }
    }
}
"#,
    )
    .expect_err("UI evidence ref must not become an ACL action target");

    assert_eq!(error.code(), "test.spec.target_observation_only");
    assert_eq!(
        error.path(),
        "suite.invalid-ui-ref.scenario.page.click.evidence-only.target"
    );
}

#[test]
fn parses_typed_terminal_actions_and_regex_wait() {
    let suite = TestSuite::from_acl(
        r#"
suite "tui-smoke" {
    scenario "editor" {
        surface = "tui"

        terminal_resize "large" {
            columns = 120
            rows = 40
        }
        terminal_paste "command" {
            text = "open document.txt"
        }
        press "submit" {
            key = "Enter"
        }
        wait "ready" {
            regex = "Ready: [0-9]+ files"
        }
        terminal_recording "evidence" {
            path = "terminal/session.vt"
        }
    }
}
"#,
    )
    .expect("valid TUI actions");

    let scenario = &suite.scenarios[0];
    assert_eq!(scenario.surface, Surface::Tui);
    assert_eq!(
        scenario.steps[0].action,
        Action::TerminalResize {
            columns: 120,
            rows: 40,
        }
    );
    assert_eq!(
        scenario.steps[1].action,
        Action::TerminalPaste {
            text: "open document.txt".to_string(),
        }
    );
    assert_eq!(
        scenario.steps[3].action,
        Action::Wait {
            condition: WaitCondition::Regex("Ready: [0-9]+ files".to_string()),
        }
    );
    assert_eq!(
        scenario.steps[4].action,
        Action::TerminalRecording {
            path: "terminal/session.vt".to_string(),
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
fn parses_selection_scoped_text_insertion_without_a_target() {
    let suite = TestSuite::from_acl(
        r#"
suite "selection-scoped-input" {
    scenario "editor" {
        surface = "web"

        insert_text "append-at-caret" {
            value = " additional text"
        }
    }
}
"#,
    )
    .expect("selection-scoped input suite");

    assert_eq!(
        suite.scenarios[0].steps[0].action,
        Action::InsertText {
            value: " additional text".to_string(),
        }
    );
}

#[test]
fn rejects_a_target_on_selection_scoped_text_insertion() {
    let error = TestSuite::from_acl(
        r#"
suite "selection-scoped-input" {
    scenario "editor" {
        surface = "web"

        insert_text "append-at-caret" {
            target = css(".editor")
            value = " additional text"
        }
    }
}
"#,
    )
    .expect_err("selection-scoped input must not refocus a target");

    assert_eq!(error.code(), "test.spec.attribute_unknown");
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

fn repair_acl(scenarios: &str) -> String {
    format!(
        r#"suite "repair" {{
    version = 1
{scenarios}
}}
"#
    )
}

const READ_ONLY_REPAIR_SCENARIO: &str = r#"
    scenario "regression" {
        surface = "web"
        navigate "open" {
            url = "https://example.test:8443/checkout?state=repaired"
        }
        wait "ready" {
            load = "domcontentloaded"
        }
        expect "fixed" {
            text = "Checkout repaired"
        }
        screenshot "proof" {
            path = "proof.png"
        }
    }
"#;

#[test]
fn admits_one_read_only_repair_scenario_on_the_exact_finding_origin() {
    let source = repair_acl(READ_ONLY_REPAIR_SCENARIO);
    let suite =
        TestSuite::from_repair_acl(&source, "https://example.test:8443/checkout?state=broken")
            .expect("bounded repair ACL");
    assert_eq!(suite.scenarios.len(), 1);
}

#[test]
fn rejects_repair_acl_with_unproved_scenarios() {
    let source = repair_acl(&format!(
        "{READ_ONLY_REPAIR_SCENARIO}{}",
        READ_ONLY_REPAIR_SCENARIO.replace("regression", "unproved")
    ));
    let error = TestSuite::from_repair_acl(&source, "https://example.test:8443/checkout")
        .expect_err("every repair candidate must contain exactly one proved scenario");
    assert_eq!(error.code(), "test.spec.repair_scenario_count");
}

#[test]
fn rejects_repair_acl_navigation_outside_the_exact_origin() {
    for url in [
        "https://other.test:8443/checkout",
        "http://example.test:8443/checkout",
        "https://example.test:9443/checkout",
    ] {
        let source = repair_acl(
            &READ_ONLY_REPAIR_SCENARIO
                .replace("https://example.test:8443/checkout?state=repaired", url),
        );
        let error =
            TestSuite::from_repair_acl(&source, "https://example.test:8443/checkout?state=broken")
                .expect_err("scheme, hostname, and port must remain exact");
        assert_eq!(error.code(), "test.spec.repair_origin_denied");
    }
}

#[test]
fn rejects_state_changing_repair_acl_steps() {
    let source = repair_acl(&READ_ONLY_REPAIR_SCENARIO.replace(
        "        wait \"ready\" {\n            load = \"domcontentloaded\"\n        }",
        "        click \"mutate\" {\n            target = css(\"#submit\")\n        }",
    ));
    let error = TestSuite::from_repair_acl(&source, "https://example.test:8443/checkout")
        .expect_err("repair proof must not mutate application state");
    assert_eq!(error.code(), "test.spec.repair_action_denied");
}
