pub(super) fn interactability_success_suite(origin: &str) -> String {
    format!(
        r##"suite "web-interactability-assertions" {{
    version = 1

    scenario "interactability" {{
        name = "Verify viewport intersection and pointer hit reachability"
        surface = "web"
        timeout_ms = 60000

        navigate "open" {{ url = "{origin}/interactability.html" }}
        wait "loaded" {{ load = "domcontentloaded" }}

        expect "viewport-testid" {{ in_viewport = testid("plain-target") }}
        expect "viewport-partial" {{ in_viewport = testid("partial-viewport-target") }}
        expect "viewport-role" {{ in_viewport = role("button", "Role pointer target") }}
        expect "viewport-label" {{ in_viewport = label("Label pointer target") }}
        expect "viewport-placeholder" {{ in_viewport = placeholder("Placeholder pointer target") }}
        expect "viewport-text" {{ in_viewport = text("Exact pointer copy", true) }}
        expect "viewport-css-aria-hidden" {{ in_viewport = css("#css-aria-hidden") }}
        expect "viewport-shadow" {{ in_viewport = role("button", "Shadow pointer target") }}

        expect "pointer-testid" {{ pointer_reachable = testid("plain-target") }}
        expect "pointer-role" {{ pointer_reachable = role("button", "Role pointer target") }}
        expect "pointer-label" {{ pointer_reachable = label("Label pointer target") }}
        expect "pointer-placeholder" {{ pointer_reachable = placeholder("Placeholder pointer target") }}
        expect "pointer-text" {{ pointer_reachable = text("Exact pointer copy", true) }}
        expect "pointer-css-aria-hidden" {{ pointer_reachable = css("#css-aria-hidden") }}
        expect "pointer-shadow" {{ pointer_reachable = role("button", "Shadow pointer target") }}
        expect "pointer-child" {{ pointer_reachable = testid("child-hit-target") }}
        expect "pointer-partial-cover" {{ pointer_reachable = testid("partial-cover-target") }}
        expect "pointer-pass-through" {{ pointer_reachable = testid("pass-through-target") }}

        expect "stable-viewport" {{
            in_viewport = testid("stable-viewport-target")
            stable_for_ms = 100
            sample_interval_ms = 25
        }}
        expect "stable-pointer" {{
            pointer_reachable = testid("stable-pointer-target")
            stable_for_ms = 100
            sample_interval_ms = 25
        }}
    }}
}}
"##
    )
}

pub(super) fn interactability_failure_suite(origin: &str) -> String {
    let scenario = |id: &str, condition: &str, stability: &str| {
        format!(
            r#"    scenario "{id}" {{
        surface = "web"
        timeout_ms = 30000
        navigate "open" {{ url = "{origin}/interactability.html" }}
        expect "{id}" {{ {condition}{stability} }}
    }}

"#
        )
    };
    let scenarios = [
        scenario(
            "offscreen-viewport",
            "in_viewport = testid(\"offscreen-target\")",
            "",
        ),
        scenario(
            "offscreen-pointer",
            "pointer_reachable = testid(\"offscreen-target\")",
            "",
        ),
        scenario(
            "covered-pointer",
            "pointer_reachable = testid(\"covered-target\")",
            "",
        ),
        scenario(
            "transparent-cover-pointer",
            "pointer_reachable = testid(\"transparent-cover-target\")",
            "",
        ),
        scenario(
            "pointer-events-none-target",
            "pointer_reachable = testid(\"pointer-none-target\")",
            "",
        ),
        scenario(
            "missing-viewport",
            "in_viewport = testid(\"missing-viewport-target\")",
            "",
        ),
        scenario(
            "missing-pointer",
            "pointer_reachable = testid(\"missing-pointer-target\")",
            "",
        ),
        scenario(
            "ambiguous-viewport",
            "in_viewport = css(\".ambiguous-target\")",
            "",
        ),
        scenario(
            "invalid-pointer-selector",
            "pointer_reachable = css(\"[\")",
            "",
        ),
        scenario(
            "semantic-hidden",
            "in_viewport = role(\"button\", \"Hidden semantic pointer target\")",
            "",
        ),
        scenario(
            "shadow-css",
            "pointer_reachable = css(\"#shadow-pointer-target\")",
            "",
        ),
        scenario(
            "invalid-viewport-geometry",
            "in_viewport = testid(\"invalid-geometry-target\")",
            "",
        ),
        scenario(
            "invalid-pointer-geometry",
            "pointer_reachable = testid(\"invalid-geometry-target\")",
            "",
        ),
        scenario(
            "transient-viewport",
            "in_viewport = testid(\"transient-viewport-target\")",
            "\n            stable_for_ms = 100\n            sample_interval_ms = 25",
        ),
        scenario(
            "transient-pointer",
            "pointer_reachable = testid(\"transient-pointer-target\")",
            "\n            stable_for_ms = 100\n            sample_interval_ms = 25",
        ),
    ]
    .concat();
    format!(
        r#"suite "web-interactability-assertion-errors" {{
    version = 1

{scenarios}}}
"#
    )
}
