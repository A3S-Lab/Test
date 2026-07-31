# Web ACL reference

## Contents

- [Suite and scenario](#suite-and-scenario)
- [Targets](#targets)
- [Core actions](#core-actions)
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
switching, upload, download, and visibility checks require `ref()` or `css()`.

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

wait "dashboard-url" {
    url = "**/dashboard"
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

screenshot "final" {
    path = "screenshots/final.png"
}
```

A wait accepts exactly one of `load`, `text`, or `url`. An expectation accepts
exactly one of `text`, `url`, or `visible`.

Focus, double-click, context-click, type, uncheck, select, drag, and
target-scoped wheel require `ref()` or `css()` with the current browser
protocol. Click, hover, fill, and check accept all semantic targets. Select
requires one or more values. Wheel requires a non-zero delta, accepts unique
`alt`, `control`, `meta`, and `shift` modifiers, and is native when no target
is supplied. Viewport dimensions and optional scale must be positive.

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
