use super::RELATIONS;

pub(super) fn layout_success_suite(origin: &str) -> String {
    let relations = RELATIONS
        .iter()
        .map(|(fixture_name, relation)| {
            format!(
                r#"        expect "relation-{fixture_name}" {{
            target = testid("{fixture_name}-target")
            relative_to = testid("{fixture_name}-reference")
            layout = "{relation}"
        }}
"#
            )
        })
        .collect::<String>();
    format!(
        r##"suite "web-layout-assertions" {{
    version = 1

    scenario "layout-relations" {{
        name = "Verify rendered layout geometry"
        surface = "web"
        timeout_ms = 60000

        navigate "open" {{ url = "{origin}/layout.html" }}
        wait "loaded" {{ load = "domcontentloaded" }}
{relations}
        expect "role-locator" {{
            target = role("button", "Role layout target")
            relative_to = testid("locator-reference")
            layout = "above"
        }}
        expect "label-locator" {{
            target = label("Label layout target")
            relative_to = testid("locator-reference")
            layout = "above"
        }}
        expect "placeholder-locator" {{
            target = placeholder("Placeholder layout target")
            relative_to = testid("locator-reference")
            layout = "above"
        }}
        expect "text-locator" {{
            target = text("Exact layout copy", true)
            relative_to = testid("locator-reference")
            layout = "above"
        }}
        expect "css-aria-hidden-locator" {{
            target = css("#css-aria-hidden")
            relative_to = testid("locator-reference")
            layout = "above"
        }}
        expect "shadow-semantic-locator" {{
            target = role("button", "Shadow layout target")
            relative_to = testid("shadow-reference")
            layout = "above"
        }}
        expect "tolerance-boundary" {{
            target = testid("tolerance-target")
            relative_to = testid("tolerance-reference")
            layout = "above"
            tolerance_px = 1
        }}
        expect "stable-above" {{
            target = testid("above-target")
            relative_to = testid("above-reference")
            layout = "above"
            stable_for_ms = 100
            sample_interval_ms = 25
        }}
    }}
}}
"##
    )
}

pub(super) fn layout_failure_suite(origin: &str) -> String {
    let failure =
        |scenario: &str, target: &str, relative_to: &str, relation: &str, stability: &str| {
            format!(
                r#"    scenario "{scenario}" {{
        surface = "web"
        timeout_ms = 30000
        navigate "open" {{ url = "{origin}/layout.html" }}
        expect "{scenario}" {{
            target = {target}
            relative_to = {relative_to}
            layout = "{relation}"{stability}
        }}
    }}

"#
            )
        };
    let scenarios = [
        failure(
            "wrong-direction",
            "testid(\"below-target\")",
            "testid(\"below-reference\")",
            "above",
            "",
        ),
        failure(
            "wrong-containment",
            "testid(\"inside-target\")",
            "testid(\"inside-reference\")",
            "contains",
            "",
        ),
        failure(
            "wrong-overlap",
            "testid(\"not-overlapping-target\")",
            "testid(\"not-overlapping-reference\")",
            "overlaps",
            "",
        ),
        failure(
            "wrong-alignment",
            "testid(\"above-target\")",
            "testid(\"above-reference\")",
            "aligned_left",
            "",
        ),
        failure(
            "wrong-size",
            "testid(\"inside-target\")",
            "testid(\"inside-reference\")",
            "same_size",
            "",
        ),
        failure(
            "missing-target",
            "testid(\"missing-layout-target\")",
            "testid(\"error-reference\")",
            "above",
            "",
        ),
        failure(
            "missing-relative",
            "testid(\"above-target\")",
            "testid(\"missing-layout-reference\")",
            "above",
            "",
        ),
        failure(
            "ambiguous-target",
            "css(\".ambiguous-layout\")",
            "testid(\"error-reference\")",
            "above",
            "",
        ),
        failure(
            "ambiguous-relative",
            "testid(\"ambiguous-relative-target\")",
            "css(\".ambiguous-relative\")",
            "above",
            "",
        ),
        failure(
            "invalid-target",
            "css(\"[\")",
            "testid(\"error-reference\")",
            "above",
            "",
        ),
        failure(
            "invalid-relative",
            "testid(\"above-target\")",
            "css(\"[\")",
            "above",
            "",
        ),
        failure(
            "transient-layout",
            "testid(\"transient-target\")",
            "testid(\"transient-reference\")",
            "above",
            "\n            stable_for_ms = 100\n            sample_interval_ms = 25",
        ),
        failure(
            "semantic-hidden",
            "role(\"button\", \"Hidden semantic layout target\")",
            "testid(\"locator-reference\")",
            "above",
            "",
        ),
        failure(
            "shadow-css",
            "css(\"#shadow-target\")",
            "testid(\"shadow-reference\")",
            "above",
            "",
        ),
        failure(
            "invalid-geometry",
            "testid(\"invalid-geometry-target\")",
            "testid(\"transient-reference\")",
            "above",
            "",
        ),
    ]
    .concat();
    format!(
        r#"suite "web-layout-assertion-errors" {{
    version = 1

{scenarios}}}
"#
    )
}
