# ACL Suite Specification

An A3S Test manifest is an ACL document with exactly one labeled `suite`
block. Unknown blocks and attributes are rejected.

## Suite

```acl
suite "name" {
    version = 1
    scenario "id" {}
}
```

`version` is a positive integer and currently defaults to `1`.

## Scenario

```acl
scenario "stable-id" {
    name = "Human-readable name"
    surface = "web"
    timeout_ms = 30000
}
```

`surface` is required and is one of `web`, `gui`, or `tui`. `timeout_ms`
defaults to 60 seconds and covers surface opening and all scenario steps.

## Actions

Action blocks execute in source order. Each action requires one unique label
inside its scenario.

```acl
navigate "open" {
    url = "https://example.com"
}

snapshot "controls" {
    interactive = true
}

click "submit" {
    target = role("button", "Submit")
}

fill "email" {
    target = label("Email")
    value = "user@example.test"
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

press "confirm" {
    key = "Enter"
}

wait "loaded" {
    load = "networkidle"
}

wait "editor-surface" {
    visible = css("[data-editor-ready]")
}

expect "saved" {
    text = "Saved"
}

screenshot "final" {
    path = "final.png"
}
```

`wait` accepts exactly one of:

- `load = "networkidle"` or `load = "domcontentloaded"`
- `text = "..."`
- `url = "..."`
- `visible = css("...")` or `visible = ref("@e1")`

Visible waits require a direct CSS selector or current observation ref.

`expect` accepts exactly one of:

- `text = "..."`
- `url = "..."`
- `visible = <target>`

## Browser context

Tabs use stable browser tab IDs or an optional user label:

```acl
tab "list-tabs" {
    operation = "list"
}

tab "open-docs" {
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
```

Frames accept `main()`, `ref()`, or `css()`:

```acl
frame "payment" {
    target = css("#payment-frame")
}

frame "return-main" {
    target = main()
}
```

Dialogs expose `status`, `accept`, and `dismiss`. Accept may carry prompt text:

```acl
dialog "prompt" {
    operation = "accept"
    text = "Approved"
}
```

## Files and network

Uploads accept one or more local fixture paths. Uploads and downloads require a
direct `ref()` or `css()` target:

```acl
upload "attachments" {
    target = css("input[type=file]")
    paths = ["tests/fixtures/one.txt", "tests/fixtures/two.txt"]
}

download "report" {
    target = ref("@e8")
    path = "downloads/report.pdf"
}
```

Relative upload paths are resolved against the working directory where the
`a3s-test` process was started before they are sent to the browser adapter.
Absolute paths are preserved. This keeps project-owned fixtures stable even
when a standalone browser daemon runs from a different working directory.

Download paths are artifact-relative. Network routes choose exactly one
response mode:

```acl
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

## Structured evidence

HAR and trace capture require a start action followed by a stop action with an
artifact path:

```acl
har "start-har" {
    operation = "start"
}

har "stop-har" {
    operation = "stop"
    path = "network/session.har"
}

trace "start-trace" {
    operation = "start"
}

trace "stop-trace" {
    operation = "stop"
    path = "traces/session.zip"
}
```

Video declares its artifact at start and attaches it when stopped:

```acl
video "start-video" {
    operation = "start"
    path = "video/session.webm"
}

video "stop-video" {
    operation = "stop"
}
```

All Web evidence paths are relative to a canonical scenario/session artifact
root. Descendant symbolic links, Windows reparse points, and non-directory
components fail with `test.driver.web.artifact_path_invalid` before browser
dispatch or adapter writes. After screenshot, download, HAR, trace, or video
commands complete, the expected fresh output must exist as a regular file that
still resolves inside that root. Existing regular output is removed before a
new capture so a zero-exit command cannot reuse stale evidence. Reconnecting an
active video validates but preserves its in-progress file. Any output failure
causes the turn to fail with
`test.driver.web.artifact_output_invalid` and returns no evidence.

The browser runtime/socket directory is canonicalized and bound to its
filesystem identity when a Web session is opened or reconnected. The driver
revalidates that binding immediately before each command and emergency cleanup.
A missing directory, a symbolic link or Windows reparse point, or a different
directory installed at the same path fails with
`test.driver.web.runtime_binding_lost` before dispatch. Emergency cleanup also
rejects linked namespace components and linked or non-regular PID sidecars.
On Windows, a PID alone never authorizes `taskkill`: a bounded process
command-line query must contain one of the configured browser executable
markers. A failed, timed-out, empty, or mismatched query fails closed without
terminating the process.
Persistent CLI metadata remains subject to its workspace/session ownership
marker, and neither the runtime directory nor that marker may be a link.

Accessibility, browser-console, and page-error output is serialized directly
to JSON evidence:

```acl
accessibility "tree" {
    path = "evidence/tree.json"
    interactive = true
}

console "console-log" {
    path = "evidence/console.json"
    clear = false
}

page_errors "javascript-errors" {
    path = "evidence/errors.json"
    clear = false
}
```

Every evidence path must be non-empty, relative, and free of parent traversal.
The Web driver resolves it beneath the scenario artifact directory.

## Typed targets

```acl
target = ref("@e4")
target = css("[data-testid=save]")
target = role("button", "Save")
target = text("Save")
target = text("Save", true)
target = testid("save")
target = label("Document title")
target = placeholder("Search")
target = automation_id("save-button")
target = visual_point("@v3", 120, 80)
```

The optional second argument to `text` controls exact matching. Visibility
assertions, non-main frame switching, uploads, downloads, focus, double-click,
context-click, type, uncheck, select, drag, and target-scoped wheel require
`ref()` or `css()` because those map directly to the browser protocol. Click,
hover, fill, and check accept every target form.

`automation_id()` is a GUI semantic target. `visual_point()` is a GUI-only,
observation-scoped pixel target: its first argument must be the latest visual
reference returned by a window-vision observation and its coordinates are
unsigned 32-bit image pixels. Web drivers reject both GUI-only target forms.

`select` requires at least one value. `wheel` requires `delta_y`; `delta_x`
defaults to zero, at least one delta must be non-zero, and `modifiers` may
contain unique `alt`, `control`, `meta`, or `shift` values. A wheel without a
target is native. A target-scoped wheel is dispatched at the visible center of
the resolved element. Context-click also resolves the visible center, moves the
pointer there, and dispatches a cancelable page `contextmenu` event; it does not
open the browser-native menu. Viewport width, height, and optional integer
scale must be greater than zero.

## Admission behavior

Validation completes before a driver is opened. Errors expose a stable code and
logical path such as:

```text
test.spec.attribute_unknown
suite.office.scenario.word.navigate.open.retries
```

Source values are not needed to route an error. Coding agents should use the
code and path to repair manifests.

## Browser admission and runner bounds

`a3s-test capabilities --json` probes the configured executable before any
browser session launches. Action protocol revision 5 admits A3S Browser
`>= 0.1.1, < 0.2.0` and standalone agent-browser `>= 0.26.0, < 0.27.0`.
Unverified versions fail with `test.driver.web.version_unsupported`.

Persistent agent sessions derive a browser hostname allowlist from the initial
URL and each `--allow-origin`. `--allow-origin` also permits explicit
navigation to that exact HTTP(S) origin. `--allow-domain` adds a hostname or a
leading `*.` wildcard to the browser's network policy for required
requests, but does not add an A3S navigation origin. The browser filter also
admits document requests for that hostname; explicit URL actions and successful
observations remain separately constrained by scheme, host, and effective port.
The normalized hostname policy is persisted in new agent session metadata.
Legacy metadata without that policy remains readable and terminally cleanable,
but observation and action turns fail with
`test.session.browser_network_policy_missing`; callers must abort or finish the
old session and start a new one.

The runner defaults to one scenario at a time and accepts an explicit
`--max-parallel-scenarios` limit from 1 through 64. Infrastructure retry count
is bounded from 0 through 10. Only errors marked retryable because a command
was not dispatched can be retried; assertions, command timeouts, and non-zero
browser action exits are never retried.

## Agentic boundary

The ACL grammar describes deterministic scenarios only. It does not accept
free-form `agent`, `prompt`, or natural-language action blocks. A coding agent
drives exploratory execution through the persistent `a3s-test agent` CLI and
the generated action schema. A host that intentionally embeds its own model
can instead use `a3s-test-agent` with a typed goal, real `LlmProvider`, explicit
budgets, and an `ActionPolicy`. A future ACL version may project that same
application contract; it must not introduce a keyword intent router.

GUI sessions revalidate their host-selected application identity, PID, and
top-level window binding immediately before each observation and effectful
action. `test.driver.gui.application_binding_lost` and
`test.driver.gui.window_binding_lost` are fail-closed, non-retryable turn
errors: the prior observation generation is invalidated and no input tool is
called. The session remains terminally cleanable with `finish` or `abort`.

GUI screenshot paths must be relative PNG paths. The adapter canonicalizes the
session artifact root, rejects symbolic-link or reparse-point descendants while
preparing the path, and verifies that the generated regular file resolves
inside the same root both after capture and before visual input. Containment or
file replacement failures produce `test.driver.gui.artifact_path_invalid`,
`test.driver.gui.screenshot_invalid`, or `test.driver.gui.stale_image` before an
input tool is called.

Surface-neutral MCP sessions reserve their identifier while terminal cleanup
is running. A caller deadline or cancellation does not cancel a dispatched
driver close; operations return retryable
`test.session.cleanup_in_progress` until the owned background task resolves.
An eventual retryable close failure restores the same driver session in
`cleanup_required` state. Observation and action operations then fail with
`test.session.cleanup_required`; only `finish` or `abort` may retry cleanup.
Success or a non-retryable cleanup failure releases the session identifier.
