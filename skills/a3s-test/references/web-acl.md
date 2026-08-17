# Web ACL reference

## Contents

- [Suite and scenario](#suite-and-scenario)
- [Targets](#targets)
- [Core actions](#core-actions)
- [Control-state expectations](#control-state-expectations)
- [Assertion stability](#assertion-stability)
- [Tabs, frames, and dialogs](#tabs-frames-and-dialogs)
- [Files and network](#files-and-network)
- [Evidence capture](#evidence-capture)
- [CLI](#cli)
- [Result and exit contract](#result-and-exit-contract)

## Suite and scenario

An ACL file contains exactly one suite. Actions run in source order.

```acl
suite "product-smoke" {
    version = 1

    scenario "sign-in" {
        name = "Sign in with a test account"
        surface = "web"
        timeout_ms = 30000

        navigate "open" {
            url = "http://127.0.0.1:3000/sign-in"
        }
    }
}
```

`surface` is required. Web suites use `"web"`. `timeout_ms` covers opening the
surface, retry backoff, every action, and cleanup initiation. Identifiers allow
ASCII letters, digits, `-`, and `_`.

## Targets

```acl
target = role("button", "Save")
target = label("Email")
target = testid("submit")
target = placeholder("Search")
target = text("Exact label", true)
target = text("Partial label")
target = css("[data-testid=save]")
target = ref("@e4")
```

Prefer semantic targets. `ref()` values come from an accessibility snapshot and
become stale after navigation or dynamic page changes. Non-main frame
switching, upload, download, and visible waits require `ref()` or `css()`.
Visible expectations also accept semantic targets.

## Core actions

```acl
navigate "open" {
    url = "https://example.test"
}

snapshot "controls" {
    interactive = true
}

click "save" {
    target = role("button", "Save")
}

fill "email" {
    target = label("Email")
    value = "tester@example.test"
}

hover "help" {
    target = role("button", "Help")
}

focus "title" {
    target = css("#title")
}

type "append-title" {
    target = css("#title")
    value = " plan"
}

check "comments-on" {
    target = label("Comments")
}

uncheck "comments-off" {
    target = css("#comments")
}

select "status" {
    target = css("#status")
    values = ["draft", "review"]
}

double_click "open-row" {
    target = ref("@e7")
}

context_click "row-menu" {
    target = ref("@e7")
}

drag "move-comment" {
    source = css("#comment-1")
    target = css("#comment-gutter")
}

wheel "zoom" {
    target = css(".document-canvas")
    delta_y = -120
    modifiers = ["control"]
}

viewport "desktop" {
    width = 1440
    height = 900
    scale = 2
}

press "submit" {
    key = "Enter"
}

wait "loaded" {
    load = "networkidle"
}

wait "ready-text" {
    text = "Ready"
}

wait "editor-surface" {
    visible = css("[data-editor-ready]")
}

wait "dashboard-url" {
    url = "**/dashboard"
}

wait "dialog-closed" {
    hidden = role("dialog", "Checkout")
}

expect "saved" {
    text = "Saved"
}

expect "current-url" {
    url = "https://example.test/dashboard"
}

expect "dialog-visible" {
    visible = css("[role=dialog]")
}

expect "dialog-closed" {
    hidden = role("dialog", "Checkout")
    stable_for_ms = 300
    sample_interval_ms = 25
}

expect "display-name" {
    target = label("Display name")
    value = "Ada"
}

expect "terms" {
    checked = label("Accept terms")
}

expect "status" {
    target = role("listbox", "Publication status")
    selected_values = ["review", "published"]
}

screenshot "final" {
    path = "screenshots/final.png"
}
```

A wait accepts exactly one of `load`, `text`, `regex`, `url`, `visible`, or
`hidden`. An expectation accepts exactly one of `text`, `url`, `visible`,
`hidden`, `value`, `enabled`, `disabled`, `checked`, `unchecked`, `selected`,
`unselected`, or `selected_values`. `expect hidden` immediately proves that a
stable semantic or CSS locator has no visible match, including an absent
element. A visible match fails as `test.assert.hidden`; a later visible sample
in a stability window fails as `test.assert.unstable`.

`wait hidden` uses the same positive visibility probe, first immediately and
then every 50 ms through the scenario deadline. It succeeds only on
`test.assert.visible`, caps work at 1,201 probes, and records first/last visible
counter-evidence plus timing metrics. Both negative forms reject `ref()` and
`visual_point()` because an unresolved observation-bound target is not proof
of hidden product state. Driver, stale-target, and ambiguity errors remain
errors. This is runner policy and adds no action variant; control-state
expectations advance the current action protocol to revision 8.

Focus, double-click, context-click, type, uncheck, select, drag, and
target-scoped wheel require `ref()` or `css()` with the current browser
protocol. Click, hover, fill, and check accept all semantic targets. Select
requires one or more values. Wheel requires a non-zero delta, accepts unique
`alt`, `control`, `meta`, and `shift` modifiers, and is native when no target
is supplied. Viewport dimensions and optional scale must be positive.

## Control-state expectations

Revision 8 compares observed control state directly:

```acl
expect "name" {
    target = css("#display-name")
    value = "Ada"
}

expect "submit-enabled" { enabled = role("button", "Submit") }
expect "submit-disabled" { disabled = testid("submit") }
expect "terms-checked" { checked = label("Terms") }
expect "terms-unchecked" { unchecked = css("#terms") }
expect "review-selected" { selected = role("option", "Review") }
expect "draft-unselected" { unselected = css("#status option[value=draft]") }

expect "status" {
    target = role("listbox", "Publication status")
    selected_values = ["review", "published"]
}

expect "empty-status" {
    target = css("#empty-status")
    selected_values = []
}
```

`value` and `selected_values` require `target`. The other state condition
contains its target. Expected selected values must be unique and compare as a
sorted exact set, so extra and missing values fail while order does not matter.
An empty list proves an observed empty selection, not an absent target.

Web reads live native properties first and uses boolean ARIA state for custom
controls where the native state is unavailable. Missing, ambiguous, invalid,
unsupported, or malformed observations remain `test.driver.web.*`. Only an
observed mismatch becomes `test.assert.value`, `.enabled`, `.disabled`,
`.checked`, `.unchecked`, `.selected`, `.unselected`, or `.selected_values`.
GUI supports exact value only when CUA supplies it and rejects boolean or
multi-selection assertions as unsupported. TUI supports visible text only.

All forms accept `stable_for_ms` and `sample_interval_ms`. A direct Web ref can
read value, enabled, native checkbox/radio checked state, and admitted ARIA
state. The standalone ref protocol does not expose native option selection or
multi-select arrays; use a stable semantic/CSS target or a Page Context ref
that resolves to one.

## Assertion stability

An ordinary expectation proves one sample. A stability-enabled expectation
requires the same read-only assertion to remain true across a bounded window:

```acl
expect "settled-total" {
    visible = testid("order-total")
    stable_for_ms = 300
    sample_interval_ms = 25
}

expect "dialog-stays-closed" {
    hidden = role("dialog", "Checkout")
    stable_for_ms = 300
    sample_interval_ms = 25
}
```

`stable_for_ms` is 10 through 60,000 ms. `sample_interval_ms` defaults to 50
ms, or to the window when it is shorter, and cannot exceed the window. A plan
may contain at most 1,001 samples, calculated as
`ceil(window / interval) + 1` including the first sample.

The first sample must pass. The runner then samples at the interval and once at
the window boundary. A later false sample returns `test.assert.unstable` with
first/last assertion data and `required_ms`, `sample_interval_ms`, `samples`,
and `observed_ms`. Driver or infrastructure failures retain their own code and
make the stability outcome inconclusive. Scenario deadline and cancellation
remain authoritative throughout the window.

Sampling cannot prove the state between observation points. Choose a shorter
interval only when the product requires finer temporal resolution, and include
the full window plus command overhead in `timeout_ms`.

## Tabs, frames, and dialogs

```acl
tab "list" {
    operation = "list"
}

tab "new-docs" {
    operation = "new"
    url = "https://example.test/docs"
    label = "docs"
}

tab "switch-docs" {
    operation = "switch"
    tab = "docs"
}

tab "close-current" {
    operation = "close"
}

frame "payment" {
    target = css("#payment-frame")
}

frame "main" {
    target = main()
}

dialog "status" {
    operation = "status"
}

dialog "confirm" {
    operation = "accept"
}

dialog "prompt" {
    operation = "accept"
    text = "Test value"
}

dialog "cancel" {
    operation = "dismiss"
}
```

Tab references are stable tab IDs such as `t2` or user-assigned labels.

## Files and network

```acl
upload "attachment" {
    target = css("input[type=file]")
    paths = ["tests/fixtures/report.pdf"]
}

download "export" {
    target = css("[data-testid=download]")
    path = "downloads/export.pdf"
}

network_route "empty-users" {
    pattern = "**/api/users"
    body = "{\"users\":[]}"
}

network_route "block-analytics" {
    pattern = "**/analytics"
    abort = true
}

network_unroute "users" {
    pattern = "**/api/users"
}

network_unroute "all" {}
```

Relative upload paths are resolved from the directory where `a3s-test` was
started before dispatch to the browser adapter; absolute paths are passed
through unchanged. Keep fixtures inside the project and run the suite from
the documented project working directory.

A route chooses exactly one of `body` or `abort = true`. Download paths are
artifact-relative. Upload paths identify local fixture files and must not
contain secrets.

## Evidence capture

```acl
har "start" {
    operation = "start"
}

har "stop" {
    operation = "stop"
    path = "network/session.har"
}

trace "start" {
    operation = "start"
}

trace "stop" {
    operation = "stop"
    path = "traces/session.zip"
}

video "start" {
    operation = "start"
    path = "video/session.webm"
}

video "stop" {
    operation = "stop"
}

accessibility "tree" {
    path = "evidence/tree.json"
    interactive = true
}

console "console" {
    path = "evidence/console.json"
    clear = false
}

page_errors "errors" {
    path = "evidence/errors.json"
    clear = false
}
```

`video` start may include a `url`. HAR and trace stop actions own their output
path. Video stop attaches the path declared by video start.

## CLI

```bash
a3s-test capabilities --json
a3s-test check tests/e2e/smoke.acl --json
a3s-test run tests/e2e/smoke.acl --json
```

Useful run options:

```text
--browser-driver a3s|standalone
--browser-executable <path>
--headed
--command-timeout-ms <milliseconds>
--idle-timeout-ms <milliseconds>
--cleanup-timeout-ms <milliseconds>
--infrastructure-retries <0..10>
--retry-backoff-ms <milliseconds>
--max-parallel-scenarios <1..64>
```

The default browser integration is `a3s use browser`. Use the standalone option
only for an agent-browser-compatible executable.

Browser runs are headless by default. A3S Test explicitly overrides inherited
Browser visibility settings and enforces Chrome's headless launch argument;
`--headed` is the sole opt-in to a visible debugging window. On Windows, the
Browser command and any `.cmd` shim run without creating a CMD window.

## Result and exit contract

JSON results include:

- run, suite, scenario, and step identity;
- `passed`, `failed`, `timed_out`, or `cancelled` status;
- step duration and attempt count;
- stable error code and message;
- structured action output and evidence paths;
- bounded cleanup errors.

Exit codes are `0` passed, `1` failed, `124` timed out, `130` cancelled, and
`2` invalid invocation or configuration.
