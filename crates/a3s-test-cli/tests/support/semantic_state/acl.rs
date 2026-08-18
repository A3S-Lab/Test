pub(super) fn semantic_state_success_suite(origin: &str) -> String {
    format!(
        r##"suite "web-semantic-state-assertions" {{
    version = 1

    scenario "semantic-state" {{
        name = "Verify live native and ARIA semantic state"
        surface = "web"
        timeout_ms = 60000

        navigate "open" {{ url = "{origin}/semantic-state.html" }}
        wait "loaded" {{ load = "domcontentloaded" }}

        expect "native-expanded" {{ expanded = testid("native-expanded") }}
        expect "native-collapsed" {{ collapsed = css("[data-testid=native-collapsed]") }}
        expect "aria-expanded" {{ expanded = role("button", "ARIA expanded") }}
        expect "aria-collapsed" {{ collapsed = testid("aria-collapsed") }}
        expect "shadow-collapsed" {{ collapsed = role("button", "Shadow collapsed") }}
        expect "hidden-css-expanded" {{ expanded = css("#hidden-css-expanded") }}

        expect "pressed" {{ pressed = role("button", "Pin feature") }}
        expect "unpressed" {{ unpressed = testid("unpressed") }}
        expect "shadow-pressed" {{ pressed = role("button", "Shadow pressed") }}

        expect "readonly" {{ readonly = label("Readonly name") }}
        expect "writable" {{ writable = css("#writable-name") }}
        expect "aria-readonly" {{ readonly = role("textbox", "ARIA readonly") }}
        expect "aria-writable" {{ writable = testid("aria-writable") }}
        expect "disabled-writable" {{ writable = testid("disabled-writable") }}
        expect "shadow-readonly" {{ readonly = role("textbox", "Shadow readonly") }}

        expect "required" {{ required = placeholder("Required email") }}
        expect "optional" {{ optional = testid("optional-email") }}
        expect "aria-required" {{ required = role("textbox", "ARIA required") }}
        expect "aria-optional" {{ optional = testid("aria-optional") }}
        expect "shadow-required" {{ required = role("textbox", "Shadow required") }}

        expect "invalid" {{ invalid = testid("invalid-email") }}
        expect "valid" {{ valid = testid("valid-email") }}
        expect "aria-invalid" {{ invalid = role("textbox", "ARIA invalid") }}
        expect "aria-valid" {{ valid = testid("aria-valid") }}
        expect "grammar-invalid" {{ invalid = testid("grammar-invalid") }}
        expect "shadow-spelling-invalid" {{ invalid = role("textbox", "Shadow spelling") }}

        expect "stable-expanded" {{
            expanded = testid("stable-expanded")
            stable_for_ms = 100
            sample_interval_ms = 25
        }}
    }}
}}
"##
    )
}

pub(super) fn semantic_state_failure_suite(origin: &str) -> String {
    let scenario = |id: &str, condition: &str, before: &str, stability: &str| {
        format!(
            r#"    scenario "{id}" {{
        surface = "web"
        timeout_ms = 30000
        navigate "open" {{ url = "{origin}/semantic-state.html" }}
{before}        expect "{id}" {{ {condition}{stability} }}
    }}

"#
        )
    };
    let scenarios = [
        scenario(
            "missing-collapsed",
            "collapsed = testid(\"missing-disclosure\")",
            "",
            "",
        ),
        scenario(
            "ambiguous-expanded",
            "expanded = css(\".ambiguous-state\")",
            "",
            "",
        ),
        scenario(
            "expanded-mismatch",
            "expanded = testid(\"aria-collapsed\")",
            "",
            "",
        ),
        scenario(
            "collapsed-mismatch",
            "collapsed = testid(\"aria-expanded\")",
            "",
            "",
        ),
        scenario(
            "pressed-mismatch",
            "pressed = testid(\"unpressed\")",
            "",
            "",
        ),
        scenario(
            "unpressed-mismatch",
            "unpressed = testid(\"pressed\")",
            "",
            "",
        ),
        scenario(
            "writable-mismatch",
            "writable = testid(\"readonly-name\")",
            "",
            "",
        ),
        scenario(
            "optional-mismatch",
            "optional = testid(\"required-email\")",
            "",
            "",
        ),
        scenario(
            "valid-mismatch",
            "valid = testid(\"invalid-email\")",
            "",
            "",
        ),
        scenario(
            "mixed-pressed",
            "pressed = testid(\"mixed-pressed\")",
            "",
            "",
        ),
        scenario(
            "unsupported-expanded",
            "expanded = css(\"#unsupported-expanded\")",
            "",
            "",
        ),
        scenario(
            "unsupported-readonly",
            "readonly = testid(\"unsupported-readonly\")",
            "",
            "",
        ),
        scenario(
            "unsupported-invalid",
            "invalid = testid(\"unsupported-invalid\")",
            "",
            "",
        ),
        scenario(
            "invalid-aria-token",
            "invalid = testid(\"unknown-invalid\")",
            "",
            "",
        ),
        scenario("invalid-selector", "expanded = css(\"[\")", "", ""),
        scenario(
            "hidden-semantic",
            "expanded = role(\"button\", \"Hidden semantic disclosure\")",
            "",
            "",
        ),
        scenario(
            "transient-expanded",
            "expanded = testid(\"transient-expanded\")",
            "        click \"arm-transient\" { target = css(\"#arm-transient\") }\n",
            "\n            stable_for_ms = 1000\n            sample_interval_ms = 25",
        ),
    ]
    .concat();
    format!(
        r#"suite "web-semantic-state-errors" {{
    version = 1

{scenarios}}}
"#
    )
}
