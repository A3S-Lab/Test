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
`domcontentloaded` is evaluated against the current document readiness state,
so it remains deterministic when navigation completed before the separate wait
command began. `networkidle` uses the browser runtime's bounded idle detector.

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
Every deterministic Web session owns its complete launched process tree.
Unix commands enter dedicated process groups retained until session cleanup.
Each active Unix boundary owns one EOF watchdog containing all recorded groups;
loss of the host-side control pipe, including host `SIGKILL`, terminates those
groups, while normal release stops and reaps the watchdog. A successful
persistent command explicitly releases that temporary boundary so its intended
daemon can outlive the CLI turn. Any group found empty after its command root
is reaped is removed from both registries before its numeric PGID can be reused.
Windows commands are created with `CREATE_SUSPENDED`, assigned to a private Job
Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, and resumed only after the
assignment succeeds. Close, timeout, cancellation, and Drop terminate the Job,
wait for it to empty, and reap the direct command child. A persistent agent
turn uses a temporary Job and clears kill-on-close only after the command exits
successfully; failed, timed-out, or cancelled turns keep the Job armed.
Browser visibility is explicit on every command. The default sends
`--headed false` and enforces Chrome's `--headless=new` launch argument, so
inherited Browser environment or configuration cannot make the run visible.
The enforced argument is appended after existing Browser launch arguments so
required host options such as `--no-sandbox` remain active. Only the A3S Test
`--headed` option sends `--headed true` and removes the enforced headless
launch argument. Windows Browser commands and CUA proxies also use
`CREATE_NO_WINDOW`, including `.cmd` shims, so test execution does not create
or flash a console window.
Command stdout and stderr are written to private temporary regular files and
read on a blocking worker after the direct launcher exits. Each stream is
limited to 8 MiB. A persistent daemon may inherit those file handles without
holding an EOF-sensitive pipe open or back-pressuring the launcher; oversized
or unreadable output fails the command instead of entering the protocol parser.
On Windows, process creation is serialized while the executor temporarily
clears inheritance from its own standard handles, then restores their original
flags immediately after spawn. This prevents generic handle inheritance from
leaking a calling process's capture pipes into the persistent browser daemon.

PID cleanup is a secondary, identity-checked path. On Windows, a PID alone
never authorizes `taskkill`: a bounded process command-line query must contain
one of the configured browser executable markers. A failed, timed-out, empty,
or mismatched query fails closed without terminating the process.
Persistent CLI metadata remains subject to its workspace/session ownership
marker, and neither the runtime directory nor that marker may be a link.
`agent start` publishes the session metadata before dispatching the first
browser command. When the initial action and exact close both fail, the session
is stored as `failed` and its owned runtime remains available to `agent abort`;
cleanup failure must never erase the only PID/socket ownership evidence.

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

## Expected Surface Contracts

A Surface Contract is a versioned ACL document that describes expected
semantic structure for one or more product variants and states. It extends the
same A3S ACL configuration language as suites; it is not a parallel DSL and it
is not a serialized browser accessibility tree.

Inputs such as a PRD or design reference may produce a draft contract. The
generator must retain source provenance and uncertainty. It must not claim
that expected elements were observed in a browser. Blocking rules require at
least one provenance entry with `status = "reviewed"` and `confidence = 100`.

```acl
surface_contract "checkout" {
    version = 1

    context {
        mode = "operate"
        audience = ["customer"]
        primary_outcome = "place_order"
    }

    provenance "requirements" {
        kind = "prd"
        uri = "./checkout.md"
        digest = "sha256:56ea72bad66743f4dadee9515096bb39a200bf9ca8d5669293f41912c55ec14e"
        status = "reviewed"
        confidence = 100
    }

    variant "desktop" {
        state = "ready"
        min_width = 1024

        element "submit" {
            test_id = "place-order"
            role = "button"
            name = "Place order"
            visible = true
            enabled = true
            severity = "blocking"

            citation "prd-submit" {
                provenance = "requirements"
                quote = "Customers can place an order."
                start = 128
                end = 157
            }
        }
    }
}
```

The root accepts exactly one `context`, one or more `provenance` blocks, and
one or more `variant` blocks. Supported context modes are `persuade`,
`operate`, `read`, and `experience`. Provenance kind is `prd`, `design`,
`manual`, or `official_docs`; status is `draft` or `reviewed`; every digest is
`sha256:` followed by 64 lowercase hexadecimal characters.

A variant selects one named application state and may constrain `min_width`,
`max_width`, `theme`, and `language`. Each element has a stable contract ID and
at least one identity field: `test_id`, `component_id`, or `role`. Optional
expectations are `name`, `description`, `required`, `visible`, `enabled`,
`checked`, `selected`, `expanded`, `readonly`, `form_required`, `invalid`, and
`parent`. Parent references stay inside the variant and must be acyclic.

An element may contain zero or more `citation "<id>"` blocks. Citation IDs are
unique within that element. `provenance` must name a contract provenance entry;
`quote` is a non-empty string retained without trimming; and `start` plus `end`
are unsigned UTF-8 byte offsets with `start < end`. This byte interpretation is
deliberate: leading and trailing whitespace and multibyte characters remain
part of the evidence. During `a3s-test check`, the CLI verifies that
`source_bytes[start..end]` exactly equals `quote.as_bytes()` after verifying the
source digest.

`severity` is `blocking`, `important`, or `suggestion`; the default is
`important`. Only blocking findings fail the contract. Important and
suggestion findings remain in a passed report as advisory evidence. Missing,
absent, or truncated Test Kit context cannot prove completeness and produces
an `inconclusive` report, which fails closed at the runner boundary.

The suite invokes an admitted contract with a runner-owned action:

```acl
verify_contract "checkout-ready" {
    contract = "./contracts/checkout.acl"
    variant = "desktop"
    state = "ready"
}
```

Contract and provenance paths must resolve to regular files beneath the suite
directory. The CLI loads every referenced contract and verifies each declared
SHA-256 digest and exact citation byte range before any surface opens.
`verify_contract` is legal in closed ACL suites only. It is absent from
interactive agent and MCP schemas and must never reach a surface driver.

## Source-to-contract provider contract

`a3s-test-agent` exposes a typed `ContractGenerationProvider` for SDK hosts.
The provider request binds the contract name and context to one or more PRD or
design sources, each with a relative URI, local path, kind, and SHA-256 digest.
Design sources additionally require an image media type and positive bounded
dimensions. Requests also carry issue/deadline times and a micro-USD ceiling.

The provider response contains provider/model identity, exact source bindings,
bounded usage, an optional request ID, and candidates. A candidate is not an
Observed Surface and is not an admitted contract. PRD elements require at least
one exact source span. Design elements require an in-bounds pixel or normalized
region whose parent agrees with semantic parentage. Confidence remains
explicit. Open product questions are typed unresolved decisions, and any
selected candidate that depends on one blocks review.

Local admission rejects unknown sources, duplicate variants or elements,
cyclic or missing parents, prefilled citations, mismatched source bytes,
inconsistent design hierarchy, invalid geometry, unbounded output, provider
identity or provenance changes, excess cost, timeout, and cancellation. Source
files are verified both before and after the provider call.

The stable version 1 wire identifier is
`a3s.test.contract-generation-provider/1`. The authoritative discoverable
bundle is printed by:

```bash
a3s-test provider schema contract-generation
```

The bundle contains generated JSON Schema 2020-12 request and response
documents plus `candidate_only` authority and safety invariants. Providers may
propose source-bound candidates only. They cannot approve an Expected Surface,
claim an Observed Surface, decide a test verdict, or authorize repair. Unknown
wire fields are rejected. An incompatible field or semantic change requires a
new protocol identifier.

The discovered bundle also contains the standard HTTP projection. A request is
one `POST application/json` document:

```json
{
  "protocol": "a3s.test.contract-generation-provider/1",
  "request": {}
}
```

A response repeats `protocol` and sets `status` to `success` with one
`response`, or to `failure` with one `error`:

```json
{
  "status": "failure",
  "protocol": "a3s.test.contract-generation-provider/1",
  "error": {
    "code": "capacity_exhausted",
    "message": "queue is full",
    "retryable": true
  }
}
```

An error has bounded `code`, `message`, and `retryable` fields. Unknown fields,
missing or unknown statuses, and envelopes containing both result forms are
rejected. The client requires HTTP 200 and JSON media type, does not follow
redirects or use environment proxies, accepts plaintext only on explicit
loopback addresses, bounds both bodies, and applies the earlier of the
configured timeout and the wire deadline. The typed response is still subject
to local contract generation admission.

Generation merges candidate evidence without choosing a winner. Differing
fields become stable explicit conflicts. A human review must approve or reject
candidates and resolve every applicable conflict with a rationale. The result
uses the existing `surface_contract` ACL grammar; only approved source spans
become citation blocks, and only selected sources become reviewed provenance.

Reconciliation matches each element in this order: exact test ID, component
identity plus optional role/name, exact role and name, then role alone.
Ambiguity is a finding rather than an arbitrary selection. Reports preserve
matches, expected and actual values, severity, confidence, observation
revision, and a stable `finding:<sha256>` identifier. The finding ID is derived
from the contract, variant, state, rule, and contract element, so DOM-private
node IDs and changing actual values do not break human review continuity.

After reconciliation, the Runner may project the bounded report into a
compatible Test Kit overlay. This projection is optional, one-way, and
non-authoritative. Missing Test Kit support, rejection, or projection failure
does not change the report or verdict. Projection runs with a separate bounded
best-effort budget, so a hanging page bridge cannot consume the deterministic
scenario deadline. The page may turn a projected finding into a draft, but
only explicit submission creates repair authorization.

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
browser session launches. Action protocol revision 6 admits A3S Browser
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

The CUA MCP proxy must be admitted into an owned process-tree boundary before
the transport can be returned. Unix uses a dedicated process group and EOF
watchdog, so abrupt host death still terminates the group. Windows creates the
proxy suspended, assigns it to a kill-on-close Job Object, and then resumes it.
Closing stdin is bounded, and every successful close still terminates and waits
for descendants that outlive the proxy root. Cancellation of an in-flight
request, notification, or close signals the same boundary before the transport
lock is released. Timeout, protocol failure, early exit, transport drop, and
the CLI emergency interrupt terminate that boundary and reap the direct child.
If supervision cannot be established, startup fails with
`test.driver.gui.process_supervision_unavailable` after bounded fallback
cleanup.

Surface-neutral MCP sessions reserve their identifier while terminal cleanup
is running. A caller deadline or cancellation does not cancel a dispatched
driver close; operations return retryable
`test.session.cleanup_in_progress` until the owned background task resolves.
An eventual retryable close failure restores the same driver session in
`cleanup_required` state. Observation and action operations then fail with
`test.session.cleanup_required`; only `finish` or `abort` may retry cleanup.
Success or a non-retryable cleanup failure releases the session identifier.

## Optional visual grounding

Visual grounding is a typed SDK-host capability in `a3s-test-agent`; it is not
an ACL action and does not change action protocol revision 6. Deterministic Web
targeting remains role, label, test ID, placeholder, text, current ref, and CSS
in that order of preference. A host may invoke visual grounding only through
`GroundingTrigger::ExplicitRequest` or `GroundingTrigger::SemanticFallback`
with one of the enumerated surface reasons.

The admitted request consists of:

| Field | Rule |
| --- | --- |
| `screenshot_path` | Non-empty bounded path to a regular non-link evidence file |
| `screenshot_sha256` | `sha256:` plus 64 lowercase hexadecimal characters |
| `width`, `height` | Positive bounded screenshot-pixel dimensions |
| `query` | Non-empty bounded natural-language locator query |
| `observation_id` | Positive identifier for the current observation |
| `trigger` | Typed explicit request or semantic-fallback reason |
| `max_cost_microusd` | Provider-reported cost ceiling |

The service rehashes the screenshot before dispatch and adds
`issued_at_unix_ms` and `deadline_unix_ms`. A provider response
must exactly repeat its admitted identity, observation ID, digest, and image
dimensions. It declares either `screenshot_pixels` or `normalized` coordinates
and returns a bounded list of finite, in-bounds points or positive-sized boxes,
each with confidence in `[0, 1]`. The service rejects response mismatch,
page-context observation/revision mismatch or truncation, oversized output,
missing or changed screenshot bytes, timeout, cancellation, or cost overrun
before reconciliation.

Reconciliation uses the center of each box, maps it to current visual-viewport
CSS pixels, and considers only visible, non-occluded nodes with usable semantic
targets. Exactly one hit may yield a semantic result. Zero or multiple hits
yield an image-bound result and preserve ambiguity rather than selecting a
node. Both variants include provider/image/observation provenance and
`authority = advisory`. They are not durable refs, contract evidence, action
authorization, or Repair Ledger entries.

The stable version 1 wire identifier is
`a3s.test.visual-grounding-provider/1`. Run
`a3s-test provider schema visual-grounding` to print its generated JSON Schema
2020-12 request/response documents and explicit advisory safety invariants.
Unknown wire fields are rejected, and an incompatible change requires a new
protocol identifier.

Visual grounding uses the same HTTP projection with protocol
`a3s.test.visual-grounding-provider/1`. The request envelope preserves the
screenshot digest, observation, dimensions, trigger, deadline, and cost
ceiling. The HTTP adapter checks the configured response identity before
returning; `VisualGroundingService` then independently verifies the full image,
observation, geometry, usage, and authority binding.
