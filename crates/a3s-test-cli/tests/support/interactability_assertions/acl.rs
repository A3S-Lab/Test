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

        expect "coverage-full-semantic" {{
            target = testid("plain-target")
            viewport_coverage_at_least = 100
        }}
        expect "coverage-left-at-least" {{
            target = testid("partial-viewport-target")
            viewport_coverage_at_least = 50
        }}
        expect "coverage-left-at-most" {{
            target = testid("partial-viewport-target")
            viewport_coverage_at_most = 50
        }}
        expect "coverage-right-at-least" {{
            target = testid("right-clipped-target")
            viewport_coverage_at_least = 50
        }}
        expect "coverage-right-at-most" {{
            target = testid("right-clipped-target")
            viewport_coverage_at_most = 50
        }}
        expect "coverage-top-at-least" {{
            target = testid("top-clipped-target")
            viewport_coverage_at_least = 50
        }}
        expect "coverage-top-at-most" {{
            target = testid("top-clipped-target")
            viewport_coverage_at_most = 50
        }}
        expect "coverage-bottom-at-least" {{
            target = testid("bottom-clipped-target")
            viewport_coverage_at_least = 50
        }}
        expect "coverage-bottom-at-most" {{
            target = testid("bottom-clipped-target")
            viewport_coverage_at_most = 50
        }}
        expect "coverage-one-pixel-at-least" {{
            target = testid("one-pixel-target")
            viewport_coverage_at_least = 1
        }}
        expect "coverage-one-pixel-at-most" {{
            target = testid("one-pixel-target")
            viewport_coverage_at_most = 1
        }}
        expect "coverage-offscreen" {{
            target = testid("offscreen-target")
            viewport_coverage_at_most = 0
        }}
        expect "coverage-large-at-least" {{
            target = testid("large-coverage-target")
            viewport_coverage_at_least = 25
        }}
        expect "coverage-large-at-most" {{
            target = testid("large-coverage-target")
            viewport_coverage_at_most = 25
        }}
        expect "coverage-css-aria-hidden" {{
            target = css("#css-aria-hidden")
            viewport_coverage_at_least = 100
        }}
        expect "coverage-shadow-semantic" {{
            target = role("button", "Shadow pointer target")
            viewport_coverage_at_least = 100
        }}

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
        expect "stable-coverage" {{
            target = testid("stable-viewport-target")
            viewport_coverage_at_least = 100
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
        scenario(
            "coverage-at-least-too-high",
            "target = testid(\"one-pixel-target\")\n            viewport_coverage_at_least = 2",
            "",
        ),
        scenario(
            "coverage-at-most-too-low",
            "target = testid(\"partial-viewport-target\")\n            viewport_coverage_at_most = 49",
            "",
        ),
        scenario(
            "coverage-offscreen-at-least-one",
            "target = testid(\"offscreen-target\")\n            viewport_coverage_at_least = 1",
            "",
        ),
        scenario(
            "coverage-missing",
            "target = testid(\"missing-coverage-target\")\n            viewport_coverage_at_least = 50",
            "",
        ),
        scenario(
            "coverage-ambiguous",
            "target = css(\".ambiguous-target\")\n            viewport_coverage_at_least = 50",
            "",
        ),
        scenario(
            "coverage-invalid-selector",
            "target = css(\"[\")\n            viewport_coverage_at_least = 50",
            "",
        ),
        scenario(
            "coverage-semantic-hidden",
            "target = role(\"button\", \"Hidden semantic pointer target\")\n            viewport_coverage_at_least = 50",
            "",
        ),
        scenario(
            "coverage-shadow-css",
            "target = css(\"#shadow-pointer-target\")\n            viewport_coverage_at_least = 50",
            "",
        ),
        scenario(
            "coverage-invalid-geometry",
            "target = testid(\"invalid-geometry-target\")\n            viewport_coverage_at_least = 50",
            "",
        ),
        scenario(
            "coverage-transient",
            "target = testid(\"transient-viewport-target\")\n            viewport_coverage_at_least = 100",
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
