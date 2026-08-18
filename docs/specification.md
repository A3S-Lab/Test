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

insert_text "append-at-caret" {
    value = " additional text"
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
- `hidden = <stable target>`

Visible waits require a direct CSS selector or current observation ref.
`domcontentloaded` is evaluated against the current document readiness state,
so it remains deterministic when navigation completed before the separate wait
command began. `networkidle` uses the browser runtime's bounded idle detector.
`hidden` is runner-owned negative synchronization. It accepts a stable semantic
or CSS locator, completes immediately when no visible match exists, and never
treats an observation-bound ref failure as disappearance.

`expect` accepts exactly one of:

- `text = "..."`
- `url = "..."`
- `visible = <target>`
- `hidden = <stable target>`
- `rendered_text = "..."` with a separate `target`
- `rendered_texts = ["...", "..."]` with a separate `target`
- `visible_count = <non-negative integer>` with a separate `target`

### Negative visibility

`hidden` proves that a locator has no visible match at the observation point.
It deliberately combines two product states that satisfy the same UI
requirement: no element matches the locator, or matching elements exist but
none has a rendered visible box.

```acl
expect "dialog-closed" {
    hidden = role("dialog", "Checkout")
}
```

This is a `TestStep` assertion mode, not a new `Action` or `Expectation`
variant. Core stores the same `Action::Assert` with
`Expectation::Visible(target)`. Before driver dispatch, the runner creates a
positive probe and applies this truth table:

| Positive visibility probe | `hidden` result |
| --- | --- |
| Returns visible evidence | Fail with `test.assert.hidden` and retain that counter-evidence |
| Returns `test.assert.visible` | Pass with `visible = false` and retain the probe error |
| Returns `test.driver.*` or another error | Preserve the original error; the result is inconclusive |

ACL admission accepts semantic and CSS locators. It rejects `ref()` and
`visual_point()` as `test.spec.hidden_target_unstable`: both identify an
observation, so failure to resolve them can mean stale evidence rather than a
hidden product element. Programmatic suites that bypass admission fail closed
as `test.run.assertion_mode_invalid` before driver dispatch.

Web supports its admitted semantic and CSS visibility targets. GUI supports
the semantic targets admitted by its adapter. TUI currently supports text
assertions but not target visibility. A GUI semantic target that has no match
is normalized to `test.assert.visible`; stale refs, ambiguous targets, invalid
targets, and CUA failures remain `test.driver.gui.*` errors.

A successful hidden assertion records:

```json
{
  "expected": "hidden",
  "visible": false,
  "target": {
    "type": "role",
    "role": "dialog",
    "name": "Checkout"
  },
  "probe_error": {
    "code": "test.assert.visible",
    "message": "target is not visible"
  }
}
```

`expect hidden` is an immediate assertion. Use `wait hidden` when disappearance
itself is the readiness condition. Add `stable_for_ms` to an expectation when
the target must remain hidden after the first successful observation; if it
reappears at a later sample, the step fails with `test.assert.unstable`.

### Waiting for disappearance

```acl
wait "dialog-closed" {
    hidden = role("dialog", "Checkout")
}
```

This is `TestStep` wait policy, not a new `Action` or `WaitCondition` variant.
Core stores `Action::Wait { condition: WaitCondition::Visible(target) }` plus
`WaitMode::Hidden`; it reuses the visible condition introduced before the
current action protocol revision 12 and does not change surface-driver command
schemas. Runner admission requires `AssertionMode::Positive`,
no assertion-stability policy, and a locator that is not `ref()` or
`visual_point()`.

The execution sequence is fixed:

1. Build a read-only `Assert(Visible(target))` probe and execute it immediately.
2. If the probe returns `test.assert.visible`, finish successfully. An already
   hidden or absent target therefore requires one probe and no interval wait.
3. If the probe returns visible evidence, retain the first and latest positive
   payloads, wait 50 ms through the scenario cancellation/deadline primitive,
   and probe again.
4. Preserve every other assertion, driver, stale-target, ambiguity, and
   infrastructure error. Unknown state is never converted into success.
5. Stop at 1,201 completed probes even when a programmatic scenario declares a
   longer deadline. The result is `test.run.hidden_wait_probe_limit`.

The scenario deadline remains authoritative. A target that is still visible at
that deadline produces the normal timed-out scenario and exit code 124.
Cancellation produces the normal cancelled result and exit code 130. Both
terminal paths still run exact owned-session cleanup and retain the last known
visible payload under `output.data.last_visible`.

A successful delayed wait records bounded metrics:

```json
{
  "expected": "hidden",
  "visible": false,
  "first_visible": { "visible": true },
  "last_visible": { "visible": true },
  "probe_error": {
    "code": "test.assert.visible",
    "message": "target is not visible"
  },
  "wait": {
    "condition": "hidden",
    "outcome": "matched",
    "poll_interval_ms": 50,
    "max_probes": 1201,
    "probes": 3,
    "observed_ms": 101
  }
}
```

`probes` counts logical visibility observations. `attempts` counts driver
dispatches and can be larger only when a retryable infrastructure failure is
admitted. The output outcomes are `matched`, `timed_out`, `cancelled`,
`probe_limit`, and `inconclusive`; the step status and stable error code remain
the authoritative verdict.

### Assertion stability

An ordinary `expect` proves one observation. Add a stability window when the
product must remain correct through hydration, animation, optimistic updates,
or another bounded settling period:

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

`stable_for_ms` and `sample_interval_ms` are step policy, not new action
variants. Core stores the policy, and the runner executes the same read-only
`Action::Assert` through the selected surface driver. The driver contract does
not change.

| Field | Admission rule | Default |
| --- | --- | --- |
| `stable_for_ms` | Required to enable sampling; 10 through 60,000 ms | Sampling disabled |
| `sample_interval_ms` | Positive, no greater than the stability window | 50 ms, or the shorter window |

The planned sample count is
`ceil(stable_for_ms / sample_interval_ms) + 1`, including the initial sample.
Admission caps it at 1,001. The runner follows this sequence:

1. Execute the expectation once. An initial false result retains its original
   `test.assert.*` error because no stability window began.
2. Start the window after the first successful sample.
3. Sample at the requested interval and always once at the window boundary.
4. Return `test.assert.unstable` if a later sample is false.
5. Preserve the original error when a driver or infrastructure failure makes
   the window inconclusive.

The scenario deadline includes the complete stability window, driver command
time, retry backoff, and preceding steps. Cancellation can interrupt an
interval wait or a sample; exact owned-surface cleanup still runs.

A completed stable assertion writes `output.data.assertion.first`,
`output.data.assertion.last`, and these metrics:

```json
{
  "stability": {
    "outcome": "passed",
    "required_ms": 300,
    "sample_interval_ms": 25,
    "samples": 13,
    "observed_ms": 301
  }
}
```

`samples` counts completed observation points. `attempts` counts driver calls
and can be larger when an admitted infrastructure retry occurs. Sampling
detects changes visible at its observation points; it cannot prove that the
state never changed between two samples. Reduce the interval when shorter
transients matter, accepting the additional driver work.

Stability attributes are legal only on `expect`. `sample_interval_ms` without
`stable_for_ms`, an out-of-range window, an interval longer than the window,
or a plan over 1,001 samples fails static admission before a surface opens.

### Control-state expectations

Action protocol revision 8 adds typed assertions for live control state. An
`expect` block still chooses exactly one condition:

```acl
expect "display-name" {
    target = label("Display name")
    value = "Ada"
}

expect "submit" {
    disabled = role("button", "Submit")
}

expect "terms" {
    checked = label("Accept terms")
}

expect "review" {
    selected = role("option", "Review")
}

expect "status" {
    target = role("listbox", "Publication status")
    selected_values = ["review", "published"]
}
```

`value` and `selected_values` require a separate `target`. The paired state
conditions carry their target directly: `enabled`/`disabled`,
`checked`/`unchecked`, and `selected`/`unselected`. `selected_values` is a
duplicate-free exact set. ACL admission sorts it canonically, rejects duplicate
expected values, permits `[]`, and does not treat ordering as product state.
Extra or missing observed values are mismatches.

The result boundary is evidence-based:

| Observation | Result ownership |
| --- | --- |
| Exactly one target exposes the requested state and it matches | Pass with `target`, `expected`, and `actual` evidence |
| Exactly one target exposes the requested state and it differs | `test.assert.value`, `test.assert.enabled`, `test.assert.disabled`, `test.assert.checked`, `test.assert.unchecked`, `test.assert.selected`, `test.assert.unselected`, or `test.assert.selected_values` |
| No target matches | Surface-specific `test.driver.*.target_not_found` |
| More than one target matches | Surface-specific `test.driver.*.target_ambiguous` |
| The locator is invalid or the matched element has no such state | Surface-specific driver error; never an assertion pass |
| Driver output has the wrong type or duplicate selected values | Surface-specific output error |

Therefore a missing checkbox cannot prove `unchecked`, a missing button cannot
prove `disabled`, and a non-select element cannot prove
`selected_values = []`.

| Surface | `value` | Boolean state | `selected_values` |
| --- | --- | --- | --- |
| Web | Exact live DOM `value` | Native live properties take precedence; admitted ARIA state covers custom controls | Exact selected option values from a native `select` |
| GUI | Exact CUA semantic value when present | `test.driver.gui.assertion_unsupported` | `test.driver.gui.assertion_unsupported` |
| TUI | Unsupported | Unsupported | Unsupported |

Web semantic targets traverse open Shadow DOM and preserve strict
zero/one/many matching. Native checkbox and radio refs use the browser's live
checked query before considering ARIA. Custom refs may use boolean
`aria-checked` or `aria-selected`. The standalone ref protocol does not expose
native option selection or multi-select arrays, so those ref forms fail
honestly; a Page Context ref may still resolve to a stable semantic or CSS
target before dispatch.

Every control-state expectation may use the assertion-stability policy above.
The runner repeats the same read-only typed assertion; an initial mismatch
keeps its specific `test.assert.*` code, while a later mismatch becomes
`test.assert.unstable` with first and last state evidence.

### Rendered-text, rendered-sequence, and visible-count expectations

Action protocol revision 9 binds visible output to a specific element and
admits exact locator-set cardinality. Revision 10 adds exact ordered content
for the complete visible locator set:

```acl
expect "total-copy" {
    target = testid("total")
    rendered_text = "Total $42.00"
}

expect "visible-rows" {
    target = css("[data-row]")
    visible_count = 3
}

expect "line-items" {
    target = css("[data-line-item]")
    rendered_texts = [
        "Keyboard × 1",
        "Mouse × 2",
        "Shipping",
        "Shipping"
    ]
}

expect "no-line-items" {
    target = css("[data-missing-line-item]")
    rendered_texts = []
}

expect "no-errors" {
    target = role("alert", "Checkout error")
    visible_count = 0
}
```

`rendered_text` resolves exactly one visible target, reads `innerText` for an
HTML element or `textContent` for other rendered elements, trims both ends,
and collapses every whitespace run to one ASCII space. The same normalization
is applied again at the Rust comparison boundary, including ref-based native
text results. An exact match passes with target, expected, and actual evidence.
An observed mismatch is `test.assert.rendered_text`; zero matches, multiple
matches, an invalid locator, or malformed driver output retain surface-driver
ownership.

`rendered_texts` evaluates a stable semantic or CSS locator as a collection,
reads and normalizes each visible element with the same rule as
`rendered_text`, and compares the resulting vector exactly. Locator traversal
order and duplicate items are preserved. An empty match set produces `[]` and
can satisfy an empty expectation; it is not a missing-target driver failure.
Only an observed vector difference is `test.assert.rendered_texts`.

ACL rejects `ref()` and `visual_point()` with
`test.spec.rendered_texts_target_unstable`. Both identify a single observation
rather than a repeatable collection. ACL accepts at most
`MAX_RENDERED_TEXT_ITEMS = 256` expected values. Programmatic callers that
bypass ACL receive `test.driver.web.expectation_invalid` for a larger expected
vector. The page probe returns `test.driver.web.collection_limit` before
serializing text when more than 256 visible elements match, and Rust validates
the returned vector against the same bound. This keeps both trusted and
untrusted driver boundaries bounded.

`visible_count` evaluates a stable locator as a set and compares its complete
visible cardinality with a non-negative `u32`. Zero is a first-class observed
value: no visible match passes `visible_count = 0`, while any different count
is `test.assert.visible_count`. An invalid selector and malformed or
out-of-range driver output remain driver failures. ACL admission rejects
`ref()` and `visual_point()` with
`test.spec.visible_count_target_unstable`; each names one observation rather
than a repeatable collection. A Page Context `@cN` may be used only when it is
resolved to a stable semantic or CSS locator before dispatch.

Web uses two explicit visibility planes:

| Locator | Match scope | Visibility rule |
| --- | --- | --- |
| CSS | `document.querySelectorAll` in the current document; CSS locators do not pierce shadow roots | Positive geometry and client rects, with no `hidden`, `display: none`, `visibility: hidden/collapse`, or zero-opacity composed ancestor; `aria-hidden` alone remains visually rendered and counts |
| Semantic | Role, text, test ID, label, or placeholder across the document and open Shadow DOM | The same rendered-box checks plus exclusion of `aria-hidden="true"` across composed ancestry |

Both planes exclude zero-width or zero-height targets. They establish rendered
presence, not pixel occlusion by another element and not viewport
intersection. This distinction prevents accessibility metadata from changing
a CSS visual assertion while keeping semantic locators aligned with the
accessible interaction plane.

`rendered_text` accepts a current browser `ref()` because the ref identifies
one observed element and the native browser command returns its text. The
current standalone protocol cannot enumerate a ref as a locator set, so
`visible_count` and `rendered_texts` never accept refs. A programmatic Page
Context ref may be resolved to a stable semantic or CSS locator before Web
dispatch. GUI and TUI return stable surface-specific unsupported errors for
all three expectations rather than estimating them from pixels, labels, or
terminal output.

All three expectations compose with `stable_for_ms`. After the first match, the
runner repeats the identical read-only action through the scenario deadline
and cancellation boundary. A later scalar text, ordered sequence, or count
mismatch becomes
`test.assert.unstable`; driver errors remain inconclusive driver errors.

### Rendered-layout expectations

Action protocol revision 11 compares the browser-rendered geometry of two
stable targets:

```acl
expect "checkout-below-summary" {
    target = testid("checkout")
    relative_to = role("region", "Order summary")
    layout = "below"
    tolerance_px = 1
    stable_for_ms = 300
    sample_interval_ms = 25
}
```

`target`, `relative_to`, and `layout` are required. `tolerance_px` defaults to
zero and must be an integer from 0 through
`MAX_LAYOUT_TOLERANCE_PX = 1_024`. Both targets accept role, text, test ID,
label, placeholder, or CSS locators. ACL rejects `ref()` and `visual_point()`
with `test.spec.layout_target_unstable`: neither can be re-resolved across
samples. A current Page Context `@cN` remains usable only after Core resolves
it to a stable locator before dispatch; both targets are resolved
independently.

The relation vocabulary is closed:

| Group | Relations | Tolerance meaning |
| --- | --- | --- |
| Direction | `above`, `below`, `left_of`, `right_of` | Permit at most `tolerance_px` boundary intrusion |
| Containment | `contains`, `inside` | Permit each containing edge to miss by at most the tolerance |
| Intersection | `overlaps`, `not_overlapping` | Overlap must exceed the tolerance on both axes; non-overlap needs one axis at or below it |
| Alignment | `aligned_left`, `aligned_right`, `aligned_top`, `aligned_bottom`, `aligned_center_x`, `aligned_center_y` | Absolute edge or center difference is at most the tolerance |
| Size | `same_width`, `same_height`, `same_size` | Absolute dimension difference is at most the tolerance |

Core evaluates only finite admitted rectangles. Width and height must be
positive and no larger than `MAX_LAYOUT_COORDINATE_ABS = 16_777_216`; `x`,
`y`, right, and bottom must also be finite and have absolute values no larger
than that bound. An invalid rectangle cannot satisfy any relation.

Web resolves both targets and reads both `getBoundingClientRect()` values in
one JavaScript evaluation. This atomic probe prevents a mutation between two
driver calls from combining geometry from different page states. CSS uses
current-document query semantics and visual rendered visibility, so an
otherwise visible `aria-hidden` target remains eligible. Semantic locators
traverse the document and open Shadow DOM, apply the same rendered checks, and
exclude accessibility-hidden composed ancestry. Neither plane claims viewport
intersection or pixel-level occlusion.

The result boundary separates observation from product comparison:

| Observation | Result ownership |
| --- | --- |
| Both targets resolve uniquely, both rectangles are valid, and the relation matches | Pass with both targets, both rectangles, relation, tolerance, and `matched = true` |
| Both rectangles are valid but the relation differs | `test.assert.layout` |
| Either target is missing or ambiguous | `test.driver.web.target_not_found` or `test.driver.web.target_ambiguous` |
| Either CSS selector is invalid | `test.driver.web.target_invalid` |
| Either rectangle or the untrusted result envelope is malformed or out of bounds | `test.driver.web.output_invalid` |
| A typed caller bypasses ACL with an excessive tolerance | `test.driver.web.expectation_invalid` |

A passing Web payload has this stable shape:

```json
{
  "target": { "type": "test_id", "value": "checkout" },
  "relative_to": { "type": "role", "role": "region", "name": "Order summary" },
  "relation": "below",
  "tolerance_px": 1,
  "target_rect": { "x": 120.0, "y": 420.0, "width": 240.0, "height": 48.0 },
  "relative_rect": { "x": 96.0, "y": 240.0, "width": 288.0, "height": 160.0 },
  "matched": true
}
```

GUI requires both semantic elements and frames in the same fresh CUA snapshot;
it never joins frames from separate observations. Observation-bound refs are
rejected at admission, and unavailable or malformed semantic geometry fails
closed. TUI returns `test.driver.tui.action_unsupported` because terminal
cells do not establish equivalent rendered-page geometry.

Layout expectations accept assertion stability. A later relation mismatch is
`test.assert.unstable`, and the result retains the first and last complete
dual-rectangle payloads plus the usual bounded sampling metrics. A later
resolution or geometry failure keeps driver ownership. Checked-in evidence
classifies 3,400/3,400 deterministic relation cases, accepts 100/100 sustained
windows, rejects 100/100 transients, and covers every relation plus 15 negative
or driver-error cases in standalone Chromium without leaking the fixture
socket or a private runtime directory.

### Visual-viewport and pointer-reachability expectations

Action protocol revision 12 adds two orthogonal single-target expectations:

```acl
expect "checkout-in-view" {
    in_viewport = testid("checkout")
}

expect "checkout-pointer-hit" {
    pointer_reachable = role("button", "Checkout")
    stable_for_ms = 300
    sample_interval_ms = 25
}
```

`in_viewport` requires a positive-area intersection between the target's
rendered rectangle and the current visual viewport. A fully contained target
has an intersection ratio of `1`; partial intersection has a ratio strictly
between `0` and `1`; a fully offscreen target or boundary-only contact has
ratio `0` and fails as `test.assert.in_viewport`.

`pointer_reachable` first computes the same intersection rectangle. When it is
positive, Web samples the Cartesian 3 by 3 grid formed by the `1/6`, `1/2`,
and `5/6` fractions on both axes. Each point is passed through deep
`elementFromPoint` hit testing. The expectation passes when at least one point
hits the target or one of its composed-tree descendants. The rule observes
browser hit reachability only. It does not claim that the target is enabled,
keyboard accessible, backed by an event listener, or valid for a business
workflow.

Both forms accept role, text, test ID, label, placeholder, or CSS locators.
ACL rejects `ref()` and `visual_point()` with
`test.spec.in_viewport_target_unstable` or
`test.spec.pointer_reachable_target_unstable`. A current Page Context `@cN`
may be used only after Core resolves it to a stable locator before dispatch.

Web performs target resolution, rectangle capture, viewport capture, and all
requested hit tests in one page evaluation. Semantic locators traverse open
Shadow DOM and exclude accessibility-hidden composed ancestry. CSS uses
current-document query semantics and visual rendered visibility, so an
otherwise rendered `aria-hidden` element remains eligible. Deep hit testing
continues through open shadow roots, and composed ancestry makes a child hit a
valid hit for its target. Native browser behavior means a transparent element
that receives pointer events blocks the target, while a `pointer-events: none`
overlay is skipped.

Rust independently admits the response. It validates finite bounded target
and viewport rectangles, recomputes the intersection ratio, requires exactly
nine row-major samples for a positive intersection, recomputes every expected
coordinate, and accepts only boolean hit results. An offscreen pointer probe
must return an empty sample array. No JavaScript result can pass by supplying a
different sample pattern or malformed geometry.

Passing payloads have these stable shapes:

```json
{
  "target": { "type": "test_id", "value": "checkout" },
  "target_rect": { "x": 120.0, "y": 420.0, "width": 240.0, "height": 48.0 },
  "viewport_rect": { "x": 0.0, "y": 0.0, "width": 1280.0, "height": 720.0 },
  "intersection_ratio": 1.0,
  "in_viewport": true
}
```

```json
{
  "target": { "type": "test_id", "value": "checkout" },
  "target_rect": { "x": 120.0, "y": 420.0, "width": 240.0, "height": 48.0 },
  "viewport_rect": { "x": 0.0, "y": 0.0, "width": 1280.0, "height": 720.0 },
  "intersection_ratio": 1.0,
  "pointer_reachable": true,
  "sample_count": 9,
  "reachable_samples": 9,
  "samples": [
    { "x": 160.0, "y": 428.0, "reachable": true },
    { "x": 240.0, "y": 428.0, "reachable": true },
    { "x": 320.0, "y": 428.0, "reachable": true },
    { "x": 160.0, "y": 444.0, "reachable": true },
    { "x": 240.0, "y": 444.0, "reachable": true },
    { "x": 320.0, "y": 444.0, "reachable": true },
    { "x": 160.0, "y": 460.0, "reachable": true },
    { "x": 240.0, "y": 460.0, "reachable": true },
    { "x": 320.0, "y": 460.0, "reachable": true }
  ]
}
```

| Observation | Result ownership |
| --- | --- |
| Positive target/viewport intersection | `in_viewport` passes with both rectangles and the ratio |
| Valid rectangles with no positive intersection | `test.assert.in_viewport` |
| At least one of nine valid points reaches the target or a composed descendant | `pointer_reachable` passes with all points and hit counts |
| No valid point reaches the target, including an offscreen target | `test.assert.pointer_reachable` |
| Target is missing or ambiguous, or its CSS selector is invalid | `test.driver.web.target_not_found`, `.target_ambiguous`, or `.target_invalid` |
| Rectangle, sample count, order, coordinates, types, or envelope is malformed | `test.driver.web.output_invalid` |
| Required browser hit testing is unavailable | `test.driver.web.interactability_unsupported` |

GUI returns `test.driver.gui.assertion_unsupported`; TUI returns its explicit
unsupported-action error. Neither adapter estimates visual-viewport or deep
pointer-hit evidence. Both expectations compose with assertion stability. A
later valid assertion mismatch becomes `test.assert.unstable`; a later driver
failure keeps driver ownership.

Checked-in evidence classifies 1,000/1,000 Core geometry cases and 2,000/2,000
Web protocol cases, accepts 200/200 sustained windows, rejects 200/200
transients, and proves 20 positive assertions plus 15 negative or driver-error
classifications in standalone Chromium with exact fixture and private-runtime
cleanup.

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
active video validates but preserves its in-progress file. A Web screenshot
must contain 1 byte through 32 MiB; empty or oversized output is rejected,
receives a bounded cleanup attempt, and is never returned as evidence. Any
output failure causes the turn to fail with
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

Browser microphone behavior is also explicit. `--browser-microphone` accepts
`disabled` or `synthetic` and defaults to `disabled` for `run`, `agent start`,
`agent run`, and Web MCP hosts. Disabled sessions do not receive an automatic
media permission grant and A3S Test does not select or capture a real device.
The synthetic profile adds only Chromium's
`--use-fake-device-for-media-stream` and
`--use-fake-ui-for-media-stream` launch arguments. It provides deterministic
local media without exposing the host microphone. Persistent agent metadata
stores the selected profile before browser startup and reapplies it to every
turn; legacy metadata without the field is admitted as `disabled`.

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

### Source-to-contract CLI workflow

`a3s-test contract generate --config <acl> --output <json>` operationalizes the
provider contract. Its ACL root is `contract_generation "<name>"`. Required
fields are `max_cost_microusd`, one `context`, one `provider`, and at least one
labeled `source`. Context uses the same `mode`, `audience`, and
`primary_outcome` values as a Surface Contract. Provider requires `name`,
`model`, and `endpoint`; optional `authorization_env` must start with
`A3S_TEST_PROVIDER_AUTHORIZATION_` and contain only uppercase ASCII letters,
digits, or underscores after that prefix.

Each source requires `kind = "prd" | "design"`, a config-directory-contained
regular `path`, and an optional contract-relative `uri`. Design sources also
require an image `media_type`, `width`, and `height`; PRD sources reject those
fields. The CLI computes SHA-256 rather than accepting it from configuration.
Optional generation limits are `timeout_ms`, `max_sources`,
`max_source_bytes`, `max_candidates`, `max_elements`, and `max_string_bytes`.
All are additionally bounded by `ContractGenerationOptions` admission.

The output is strict JSON protocol `a3s.test.contract-workflow/1` with stage
`generated`, its full-payload `integrity_sha256`, the original sources and
admission limits, the complete admitted `GeneratedContractDraft`, and no review
or contract ACL. Unknown fields, incompatible protocol versions, oversized
files, symbolic links, source-directory escapes, altered payloads, stale source
bytes, or derived conflicts and decisions that no longer match candidates are
rejected. `integrity_sha256` is a canonical payload checksum for mutation
detection, not a signature or proof of authorship.

`a3s-test contract review --draft <json> --review <acl> --output <acl>
--audit <json>` accepts only a `generated` artifact. Review ACL has this form:

```acl
contract_review {
    reviewer = "product-owner@example.test"

    candidate "source:variant:element" {
        action = "approve"
    }

    conflict "conflict:stable-digest" {
        select = "source:variant:element"
        rationale = "Approved source and terminology"
    }
}
```

Candidate decisions must be explicit and unique. Each applicable conflict
requires one resolution that selects an approved candidate with non-empty
rationale. An approved candidate depending on an unresolved product decision
is rejected. On success the command publishes canonical Surface Contract ACL
and a `reviewed` workflow audit containing the complete generated artifact,
review, and exact ACL. Validation regenerates that ACL from the recorded review,
and replays source and provider-response admission under the recorded limits,
so altering any member fails closed. Existing files require `--force`; ACL and
audit paths must be distinct and neither may be a symbolic link.

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

`insert_text` writes into the current focused editing context without moving
the caret. Use it only after an explicit focus, click, or key action establishes
the intended insertion point. Unlike `type`, it has no target and therefore
does not refocus a contenteditable root or reset an existing selection.

`automation_id()` is a GUI semantic target. `visual_point()` is a GUI-only,
observation-scoped pixel target: its first argument must be the latest visual
reference returned by a window-vision observation and its coordinates are
unsigned 32-bit image pixels. Web drivers reject both GUI-only target forms.

TUI scenarios share `snapshot`, `press`, `wait`, and `expect`. Their
surface-specific actions are:

```acl
terminal_resize "editor-size" {
    columns = 120
    rows = 40
}

terminal_paste "command" {
    text = "open document.txt"
}

wait "loaded" {
    regex = "Loaded [0-9]+ files"
}

terminal_recording "evidence" {
    path = "terminal/editor.vt"
}
```

Terminal columns and rows are positive bounded integers. Paste is limited to
1 MiB and honors bracketed-paste mode. `press` accepts one character, named
terminal keys, `Control+<letter>`, or `Alt+<character>`. Terminal waits admit
either `text` or a bounded valid `regex`; browser load, URL, and element waits
are rejected on TUI surfaces. Recording paths are relative, traversal-free,
and confined beneath the canonical scenario artifact root.

`select` requires at least one value. `wheel` requires `delta_y`; `delta_x`
defaults to zero, at least one delta must be non-zero, and `modifiers` may
contain unique `alt`, `control`, `meta`, or `shift` values. A wheel without a
target is native. A target-scoped wheel is dispatched at the visible center of
the resolved element. A direct `ref()` or `css()` click scrolls its resolved
element into view before dispatch, including targets below the initial
viewport. Context-click also resolves the visible center, moves the pointer
there, and dispatches a cancelable page `contextmenu` event; it does not open
the browser-native menu. Viewport width, height, and optional integer scale
must be greater than zero.

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
browser session launches. Action protocol revision 12 admits A3S Browser
`>= 0.4.0, < 0.5.0` and standalone agent-browser `>= 0.26.0, < 0.27.0`.
Unverified versions fail with `test.driver.web.version_unsupported`.

Persistent agent sessions derive a browser exact-origin policy from the
initial URL and each `--allow-origin`. `--allow-origin` also permits explicit
navigation to that exact HTTP(S) origin. `--allow-domain` adds a hostname or a
leading `*.` wildcard as a wider network-only exception, but does not add an
A3S navigation origin. A3S Browser 0.4.x enforces scheme, host, and effective
port before requests and redirects are sent. Standalone 0.26.x receives a
hostname projection because its protocol cannot express exact origins.

New session metadata persists both normalized policy lists and a typed
deployment mode: `exact_origin_v1` for A3S Browser or `hostname_v1` for
standalone. A stored mode that does not match the selected driver fails with
`test.session.browser_containment_mismatch`; a policy that is no longer
canonical or no longer matches the session's admitted origins fails with
`test.session.browser_network_policy_mismatch`. Legacy metadata without typed
deployment proof remains readable and terminally cleanable, but observation
and action turns fail with
`test.session.browser_network_policy_missing`; callers must abort or finish the
old session and start a new one.

The runner defaults to one scenario at a time and accepts an explicit
`--max-parallel-scenarios` limit from 1 through 64. Infrastructure retry count
is bounded from 0 through 10. Only errors marked retryable because a command
was not dispatched can be retried; assertions, command timeouts, and non-zero
browser action exits are never retried.

## Worker capability inventory

The worker capability protocol is `a3s.test.worker-capabilities/2`. Its CLI
projection always emits JSON:

```bash
a3s-test worker schema
a3s-test worker inventory --max-parallel-scenarios 1
a3s-test worker inventory \
  --browser-driver standalone \
  --browser-executable agent-browser
a3s-test worker inventory \
  --max-parallel-scenarios 1 \
  --gui-host-profile /etc/a3s-test/gui-host.acl
```

The inventory has four required fields:

- `protocol` is the exact worker capability protocol identifier;
- `runtime` contains a bounded implementation name, semantic version,
  operating system, and architecture;
- `max_parallel_scenarios` is an integer from 1 through 64;
- `surfaces` is a non-empty, unique, canonically ordered list of typed Web,
  GUI, or TUI capability entries.

The order is Web, GUI, then TUI. A Web entry declares headless execution and
embeds the admitted `BrowserCapabilities`. It is emitted only when the caller
explicitly selects `a3s` or `standalone` and the selected executable's real
`--version` probe succeeds. The feature set must exactly match the reviewed
integration. In particular, standalone 0.26.x reports hostname containment
and must not report exact-origin containment. Supplying a browser executable
without a typed integration is invalid, and a requested probe failure fails
the complete command.

A GUI entry represents exactly one deployment-owned desktop and therefore
requires `max_parallel_scenarios = 1`. It contains the fixed profile ID,
locked compatibility profile, endpoint and perception kinds, launch or attach
target, typed application identity, CUA version and schema identifiers,
configuration and policy digests, and the exact host-permission grant and
digest. The CLI emits it only after a real CUA probe validates the locked
contract and returns both `accessibility` and `screen_recording`. The probe is
read-only and does not launch or attach to the application. Installed-daemon
grants must use `driver_daemon` attribution; embedded-socket grants must use
`host` attribution. The digest must exactly match the canonical typed grant.

A TUI entry embeds protocol `a3s.test.driver-tui/1`, the backend compiled for
the host (`unix_pty` or `windows_con_pty`), the reviewed feature set, and hard
limits for columns, rows, scrollback, retained output, and terminal cells. A
platform without a reviewed compiled backend fails closed. The hermetic
Linux/amd64 runner advertises Web and TUI only; it never advertises GUI.

`a3s-test worker schema` returns the generated strict JSON Schema and these
authority invariants:

- the inventory is self-reported scheduling evidence;
- it is not authenticated and does not authorize execution;
- Web evidence requires a real executable probe;
- GUI evidence requires a real host probe, explicit permissions, and one
  exclusive desktop lane;
- TUI evidence must match the compiled backend projection;
- an external image identity is required.

Unknown fields, duplicate surfaces, non-canonical order, unsupported protocol
or browser versions, feature overclaims, invalid protocol revisions, and
concurrency outside 1 through 64 are rejected by local admission. An external
scheduler must bind the release image digest and apply its own identity,
authorization, network, filesystem, credential, and resource policy. This
protocol does not authorize remote dispatch by itself.

## Remote worker protocol

Remote execution uses protocol `a3s.test.remote-worker/3`. Discover its exact
JSON Schema 2020-12 request, response, descriptor, and invariants with:

```bash
a3s-test worker remote schema
```

The strict request envelope contains `protocol`, a bounded `request_id`, and
one tagged command:

- `inspect` returns the descriptor bound to the running service;
- `submit` admits one immutable job and returns its current snapshot;
- `status` reads a job by its exact job and dispatch IDs;
- `renew_lease` monotonically extends a non-terminal claim within the worker
  lease limit and never beyond the job deadline;
- `cancel` records a bounded reason and cancels queued or running work.

Every submission binds all of the following:

- a portable `job_id` and globally immutable `dispatch_id`;
- the exact worker instance ID;
- an externally supplied lowercase SHA-256 image digest;
- the SHA-256 digest of the complete admitted capability inventory;
- issue time, absolute deadline, and renewable lease expiry in Unix
  milliseconds;
- admitted scenario concurrency and a sorted, unique set of required
  surfaces;
- the exact GUI host-permission digest when, and only when, GUI is required;
- a non-empty, sorted, unique set of exact scenario IDs;
- a sorted inline input bundle containing the ACL manifest.

Input paths are portable relative paths with no empty, current-directory,
parent-directory, backslash, root, trailing-dot, reserved Windows device, or
non-portable component. ASCII case-folding collisions are rejected before
filesystem access. Job and dispatch IDs obey the same storage-key safety
rules. File contents are non-empty canonical Base64, individually
SHA-256-bound, and constrained by
per-file, file-count, total decoded-byte, and complete request limits. The
service validates and decodes the full submission before private
materialization. Unknown fields and unknown protocol revisions fail closed.

The reference transport is one HTTP/1.1 `POST /v1/worker` endpoint. It binds
only an IPv4 or IPv6 loopback socket, accepts `application/json`, applies the
descriptor request-byte limit, and requires an exact `Authorization` header
whose expected value comes from `--authorization-env`. The value itself must
not be passed on the command line or inherited by browser capability probes,
Web commands, CUA proxy children, or TUI child processes. TLS termination,
client identity, rate limits, and external resource isolation belong to the
deployment. An example TUI-only host is:

```bash
export A3S_TEST_WORKER_AUTHORIZATION='Bearer replace-with-a-secret'

a3s-test worker serve \
  --listen 127.0.0.1:9400 \
  --state-root /var/lib/a3s-test-worker \
  --instance-id runner-west-1 \
  --image-digest sha256:<64-lowercase-hex-digits> \
  --authorization-env A3S_TEST_WORKER_AUTHORIZATION \
  --tui-executable /opt/example-app/bin/test-console
```

Web execution additionally requires an explicit typed browser integration and
at least one deployment-owned exact origin:

```bash
  --browser-driver standalone \
  --browser-executable agent-browser \
  --web-allow-origin https://preview.example.test
```

GUI execution additionally requires a deployment-owned ACL profile:

```acl
gui_host "desktop-primary" {
  endpoint = "installed_daemon"
  proxy_executable = "/opt/a3s/bin/cua-driver"
  policy_file = "/etc/a3s-test/cua-policy.yaml"
  macos_bundle_id = "com.example.Editor"
  target = "launch"
  arguments = ["--safe-mode"]
  profile = "semantic"
  permission_source = "driver_daemon"
  permissions = ["accessibility", "screen_recording"]
}
```

`endpoint` is `installed_daemon` or `embedded_socket`; the latter also
requires `embedded_socket` and `permission_source = "host"`. `target` is
`launch` or `attach`; attach may specify `attach_pid`, while launch may specify
up to 32 bounded `arguments`. A window may be selected by exactly one of
`window_title` or `window_automation_id`, otherwise the primary window is
used. `profile` is `semantic` or `window_vision`. The profile and policy are
bounded regular non-link files, and the CUA proxy is a regular non-link file.
Unknown blocks, attributes, implicit permissions, wrong ordering, wrong
attribution, and a mismatch between the declaration and live probe all fail
worker startup. Remote requests cannot override any profile field.

Requests cannot select or override an executable, GUI application or target,
its arguments, Web origin/domain policy, credentials, or driver backend. The
reference service
runs one job at a time with a bounded waiting queue. A deployment must treat
the fixed TUI executable as an authority boundary: selecting a shell or an
application with shell escapes grants that authority to authenticated jobs.
Exact duplicate submits return the existing snapshot; reuse of a job or
dispatch ID with different content is rejected. State transitions are
persisted as append-only event files under an exclusive state root bound to the
complete worker descriptor. The last durable non-terminal state is marked
`interrupted` on restart.

Remote command and HTTP body deadlines are capped at five minutes, browser
idle timeout at one hour, cleanup at five minutes, and retry backoff at one
minute. Invalid bounds fail before capability probing or listener startup.

Terminal snapshots may be `passed`, `failed`, `timed_out`, `cancelled`, or
`interrupted`. A completed runner result includes scenario counts and a report
descriptor containing media type, byte length, and SHA-256. The report and
surface artifacts stay in the private job directory. This execution protocol
does not transport those bytes or choose scheduler-side sharding. Version 2
adds the exact scenario selection supplied by a coordinator. Version 3 adds
GUI execution and the exact permission binding. GUI admission requires one
parallel scenario, the GUI surface in both the suite and submission, and a
permission digest identical to the current worker inventory. Non-GUI jobs
reject an unexpected permission binding. The GUI driver revalidates the live
grant again before launching or attaching to the fixed application.

## Remote artifact protocol

Report indexing and artifact transport use the separate
`a3s.test.remote-artifacts/1` protocol. Discover its exact JSON Schema 2020-12
request, response, descriptor, and invariants with:

```bash
a3s-test worker artifacts schema
```

The strict envelope contains `protocol`, a bounded `request_id`, and one
tagged command:

- `inspect` returns worker identity, capability-inventory digest, retention
  policy, and hard service limits;
- `list_reports` queries terminal report-index entries;
- `list_artifacts` pages through one job's report and evidence descriptors;
- `read` returns one bounded Base64 artifact chunk.

A report query contains a non-empty, sorted, unique set of terminal states and
may constrain suite, run ID, and exclusive `finished_after_ms` and
`finished_before_ms` bounds. Pages contain at most 100 reports. Artifact pages
contain at most 256 descriptors. Cursors use canonical unpadded URL-safe
Base64, are limited to 512 bytes, and bind the original query digest or
immutable job request digest. Changing a bound field invalidates the cursor.

Artifact list and read requests bind `job_id`, `dispatch_id`, and
`expected_request_digest`. A read selects either the report digest or an exact
indexed evidence path and digest. `offset` must be before the file end and
`max_bytes` is limited to 1 through 1,048,576. The response repeats the job,
dispatch, request, artifact descriptor, and offset bindings. It never accepts
an arbitrary server path. Before returning a chunk, the worker rejects links,
Windows reparse points, non-regular files, size changes, containment escapes,
and a full-file SHA-256 mismatch.

The deployment configures retention through these `worker serve` options:

```text
--retention-max-jobs          default 256
--retention-max-bytes         default 21474836480
--retention-max-age-ms        default 604800000
--report-index-max-jobs       default 10000
--report-index-max-age-ms     default 7776000000
```

The report-index count and age must be at least the corresponding payload
bounds. Complete inputs, report bytes, and evidence are pruned when any short
tier bound is exceeded. The compact terminal snapshot and artifact
descriptors remain queryable with `payload_state: "pruned"` until an index
bound expires. Index expiry removes the full job record and therefore ends
status lookup and idempotent replay for its job and dispatch IDs.

Retention runs after job completion, during restart, and at the next age
deadline while the worker is idle. A durable per-job index records
`retained`, `pruning`, or `pruned` before and after deletion so startup can
finish interrupted garbage collection. Retained indexes are rebuilt and
compared with actual bytes on restart; malformed descriptors, unsafe files,
or mismatched digests fail closed. A successful executor result that produced
unsafe evidence is converted to a durable failed job without touching the
external link target.

The reference HTTP transport exposes this protocol at `POST /v1/artifacts` on
the same loopback listener and under the same exact Authorization check,
content-type rule, body deadline, request-size limit, concurrency bound, and
`no-store` response policy as `POST /v1/worker`. TLS termination and external
client identity remain deployment responsibilities.

## Distributed-run configuration

`a3s-test distributed plan <path>` and `a3s-test distributed run <path>` read a
separate ACL document with exactly one labeled `distributed_run` block.
`a3s-test distributed schema` prints strict JSON Schemas for protocol
`a3s.test.distributed-run/2` plan requests, plans, analysis requests, and
analyses.

```acl
distributed_run "ci" {
  input_root = "."
  manifest = "tests/e2e/smoke.acl"
  additional_inputs = ["tests/fixtures/account.json"]
  history_root = ".a3s-test/distributed/ci"
  history_window = 20
  history_max_runs = 100
  history_max_age_ms = 7776000000
  job_timeout_ms = 600000
  lease_ms = 60000
  poll_interval_ms = 250
  http_timeout_ms = 30000

  worker "runner-west" {
    endpoint = "https://runner-west.example.test"
    image_digest = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    inventory_digest = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    authorization_env = "A3S_TEST_WORKER_AUTHORIZATION_WEST"
    max_parallel_scenarios = 4
  }

  quarantine "known-checkout-race" {
    reason = "Known checkout state race"
    owner = "checkout-team"
    issue = "https://issues.example.test/123"
    expires_at_ms = 4102444800000
  }
}
```

If a worker advertises GUI, its block must also pin the exact permission
digest from the inspected inventory and keep one exclusive lane:

```acl
  worker "desktop-primary" {
    endpoint = "https://desktop-primary.example.test"
    image_digest = "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
    inventory_digest = "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
    host_permission_digest = "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"
    authorization_env = "A3S_TEST_WORKER_AUTHORIZATION_DESKTOP"
    max_parallel_scenarios = 1
  }
```

The root attributes are:

- `manifest` is required and is relative to `input_root`;
- `input_root` defaults to `.` relative to the config directory;
- `additional_inputs` defaults to an empty string list;
- `history_root` defaults to `.a3s-test/distributed/<run-label>` relative to
  the config directory;
- `history_window` defaults to 20 and is limited to 1 through 100;
- `history_max_runs` defaults to 100 and is limited to 1 through 200;
- `history_max_age_ms` defaults to 90 days and must be at least one second;
- `job_timeout_ms` defaults to ten minutes and is limited to one second
  through 24 hours;
- `lease_ms` defaults to one minute, is limited to one second through one
  hour, and cannot exceed the job timeout;
- `poll_interval_ms` defaults to 250 and is limited to 10 through 60,000;
- `http_timeout_ms` defaults to 30 seconds and is limited to 1 millisecond
  through five minutes.

The config file must be a non-empty regular non-link file no larger than 1
MiB. Every path is relative, contained, and traversal-free. Input preparation
automatically includes the suite manifest, upload sources, referenced Surface
Contracts, their provenance files, and `additional_inputs`. It admits at most
1,024 non-empty regular files, 16 MiB per file, and 32 MiB decoded total. Each
path component rejects symbolic links and Windows reparse points. The suite
digest binds the manifest path and sorted remote path/SHA-256 pairs, not local
absolute paths.

A config requires 1 through 64 unique `worker` blocks. `endpoint` must be an
HTTPS origin or an explicit loopback HTTP origin with no credentials, path,
query, or fragment. `image_digest` is required. `inventory_digest` is an
optional exact pin; an omitted pin still becomes exact after live inspection
and is bound into the plan. `host_permission_digest` is required exactly when
the inspected worker advertises GUI and must match that inventory's permission
grant; it is invalid for a worker without GUI. `authorization_env` is
required, must begin with
`A3S_TEST_WORKER_AUTHORIZATION_`, and may contain only uppercase ASCII letters,
digits, and underscores. Its value is read as the complete Authorization
header, never serialized. `max_parallel_scenarios` defaults to 1, is limited
to 1 through 64, and cannot exceed the inspected inventory. A worker exposing
GUI must use exactly 1.

Each optional `quarantine` label is an exact scenario ID. `reason`, `owner`,
`issue`, and a future Unix-millisecond `expires_at_ms` are all required;
duplicate, unknown, or expired targets reject planning. Admission is frozen at
run start and bound by the plan digest. A quarantine changes disposition only
for `test.assert.*`, `test.contract.mismatch`, or
`test.contract.state_mismatch`. It never suppresses driver, cleanup,
inconclusive-contract, transport, report, timeout, cancellation, interruption,
or other infrastructure failures. A passing quarantined scenario is reported
as `quarantined_pass` so stale entries remain visible.

Planning concurrently inspects both remote endpoints and requires exact
worker/artifact identity, image, inventory, and any GUI host-permission
agreement. Scenarios are sorted by scarce eligible surface, descending
duration estimate, and ID. Duration is
the median of up to 20 recent passed or test-failed observations for the exact
suite digest, or the scenario timeout when no sample exists. Stable lane
scoring assigns every scenario exactly once and emits one shard per used
worker. GUI shards require one exclusive lane and repeat the permission digest
in the plan and remote submission. The plan digest covers all bindings and
quarantines.

Run dispatch validates each submission locally against the inspected worker
limits before transport. Submissions are idempotent, concurrently bounded,
and use independent renewable-lease supervision. On interrupt, every known
job/dispatch pair receives an exact cancel request. Terminal report bytes are
read only through digest-bound artifact chunks and are revalidated against the
summary, suite, run, counts, exact scenario IDs, and surface mapping before
analysis.

The private history root is exclusively locked. Runs and reports are written
atomically, reject links/reparse points and conflicting IDs, and are pruned by
count and age. Compact scheduling records are bounded to 2 MiB and analysis
reports to 16 MiB, which covers the admitted 4,096-scenario protocol maximum.
The latest retained run is the historical-change baseline, including across
suite revisions. Flake counts and duration estimates include only the exact
suite digest. Reports are stored as
`<history_root>/reports/<run-id>.json`; compact scheduling history is stored
under `<history_root>/runs/`.

Distributed exit status is 0 for passed, 1 for required product failures, 2
for infrastructure failure, 124 for timeout, and 130 for cancellation.

## Agent-run configuration

An agent-run config is a separate A3S ACL document for
`a3s-test agent run <path>`. It contains exactly one labeled `agent_run` block;
it is not a deterministic `suite` and cannot be mixed with suite blocks.

```acl
agent_run "checkout" {
  url = "http://127.0.0.1:3000/checkout"
  goal = "Complete checkout with the fixture account"
  success_criteria = ["The order confirmation is visible"]
  allow_origins = ["https://auth.example.test"]
  allow_domains = ["cdn.example.test"]
  allow_actions = ["click", "fill", "wait"]
  max_turns = 8
  max_total_tokens = 20000
  max_cost_microusd = 50000
  max_context_bytes = 524288
  timeout_ms = 120000

  provider {
    name = "deployment"
    model = "planner"
    endpoint = "https://models.example.test/v1/plan"
    authorization_env = "A3S_TEST_PROVIDER_AUTHORIZATION_DEPLOYMENT"
  }

  verification {
    expect "confirmation" { text = "Order confirmed" }
    screenshot "final" { path = "confirmation.png" }
  }
}
```

`url`, `goal`, `success_criteria`, `allow_actions`,
`max_cost_microusd`, one `provider`, and one `verification` block are required.
The run label uses 1 through 64 ASCII letters, digits, `-`, or `_`.
`url` and every `allow_origins` value must be HTTP(S) URLs with a hostname.
The initial origin is always admitted. Origins are deduplicated by scheme,
host, and effective port. They remain exact in A3S Browser's network policy;
`allow_domains` adds network-only hostnames without adding navigation origins.
Standalone projects origin hostnames into its older containment protocol.

`allow_actions` contains unique values from `navigate`, `snapshot`, `click`,
`hover`, `focus`, `double_click`, `context_click`, `fill`, `type`, `check`,
`uncheck`, `select`, `drag`, `press`, `wheel`, `viewport`, `wait`, `assert`,
`screenshot`, `tab`, `frame`, `dialog`, `upload`, `download`,
`network_route`, `network_unroute`, `har`, `trace`, `video`, `accessibility`,
`console`, and `page_errors`. Runner-owned `verify_contract` is never an
agent proposal.

`max_turns` defaults to 12 and is bounded from 1 through 256.
`max_total_tokens` defaults to 64,000 and is bounded from 1 through
100,000,000. `max_cost_microusd` is required, may be zero, and cannot exceed
1,000,000,000. `max_context_bytes` defaults to 524,288 and is bounded from 1
through 67,108,864. `timeout_ms` defaults to 120,000 and is bounded by the
agent runtime to 24 hours. The single workflow deadline covers surface open,
initial navigation, model turns, actions, and local verification; cleanup has
the separate CLI deadline.

The provider block accepts only `name`, `model`, `endpoint`, and optional
`authorization_env`. Provider and model identities must be bounded and
non-empty. The endpoint must use HTTPS or explicit loopback HTTP and cannot
contain credentials, a query, or a fragment. An authorization variable must
start with `A3S_TEST_PROVIDER_AUTHORIZATION_`; the suffix accepts uppercase
ASCII letters, digits, and `_`. The variable value is read at runtime and is
never an ACL value or report field.

Verification accepts only labeled `snapshot`, `wait`, `expect`, `screenshot`,
`accessibility`, `console`, and `page_errors` actions, using the same attribute
grammar as deterministic suites. It requires at least one `expect`. A URL wait
or expectation must remain inside the admitted exact-origin set. The model's
`finish` decision is not a verdict; all verification actions and exact browser
cleanup must succeed before the report can be successful.

## Agentic boundary

The deterministic suite grammar still rejects free-form `agent`, `prompt`, or
natural-language action blocks. A coding agent drives exploratory execution
through persistent `a3s-test agent` commands and the generated action schema.
A deployment that intentionally embeds its own model can use the separate
`agent_run` ACL root above or inject the same typed `a3s-test-agent` contracts
from an SDK host. Neither path introduces keyword intent routing or another
configuration language.

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

A version-tag release must pass `a3s.test.gui-host-certification/1` on a
dedicated macOS arm64 host before release creation. The record must bind the
exact A3S Test revision, locked CUA revision and runtime-reported source
revision, executable and policy SHA-256 digests, host version, permission
attribution, semantic and window-vision observations, session cleanup, and
zero running fixture instances before and after each profile. The record and
detached checksum are release assets, and the JSON record must have GitHub
OIDC/Sigstore SLSA provenance from the reusable certification workflow. A
binary start, version match, fake contract pass, or unsigned local JSON file
does not satisfy this release gate.

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
an ACL action and does not change the action protocol. Deterministic Web
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

The stable version 2 wire identifier is
`a3s.test.visual-grounding-provider/2`. Run
`a3s-test provider schema visual-grounding` to print its generated JSON Schema
2020-12 request/response documents and explicit advisory safety invariants.
Unknown wire fields are rejected, and an incompatible change requires a new
protocol identifier.

Visual grounding uses the same HTTP projection with protocol
`a3s.test.visual-grounding-provider/2`. The request envelope preserves the
screenshot digest, observation, dimensions, trigger, deadline, and cost
ceiling. It replaces the client-local screenshot path with `observation.png`
and carries a Base64 `image/png` attachment bound to the same SHA-256 digest.
The decoded PNG is limited to 32 MiB, while the JSON request envelope is
limited to 64 MiB. The HTTP adapter re-reads and rehashes the image immediately
before serialization, then checks the configured response identity before
returning; `VisualGroundingService` then independently verifies the full image,
observation, geometry, usage, and authority binding.

## Visual-grounding CLI configuration

`a3s-test agent ground <query>` operationalizes the advisory provider for the
latest persistent Web observation. Its ACL root is `visual_grounding` and
accepts these root attributes:

| Attribute | Rule | Default |
| --- | --- | --- |
| `max_cost_microusd` | Required non-negative provider cost ceiling | none |
| `timeout_ms` | 1 millisecond through 5 minutes | `15000` |
| `max_candidates` | 1 through 256 | `32` |
| `max_query_bytes` | 1 through 65536 | `4096` |
| `max_label_bytes` | 1 through 16384 | `1024` |

Exactly one `provider` block is required. It accepts `name`, `model`,
`endpoint`, and optional `authorization_env`. The endpoint must use HTTPS or
explicit loopback HTTP. An authorization variable must start with
`A3S_TEST_PROVIDER_AUTHORIZATION_`; its value is never stored in session
metadata or command arguments.

The command requires `--session`, the positive latest `--observation`, and
`--config`. `--reason` is one of `explicit`, `canvas`, `image-only`,
`remote-desktop`, `design-reference`, or `no-semantic-match`. ACL, credential,
provider, limits, and query admission occur before browser connection. The
page must retain the observation's exact Test Kit revision and `@cN` bindings
through screenshot capture and provider completion. Failure invalidates that
observation. Success records an advisory result and PNG evidence but never
dispatches input, determines a verdict, or authorizes repair.

## Advisory design-quality audit

Design audit is an explicit persistent-session operation, not a deterministic
assertion and not an action. Run:

```bash
a3s-test provider schema design-audit
a3s-test agent audit \
  --session checkout \
  --observation 7 \
  --config examples/design-audit.acl \
  --dimension visual-hierarchy,spacing-rhythm \
  --json
```

The stable provider identifier is `a3s.test.design-audit-provider/1`. Its
generated strict schemas describe a request bound to these fields:

| Field | Rule |
| --- | --- |
| `screenshot_path` | Bounded path to one regular non-link PNG evidence file |
| `screenshot_sha256` | `sha256:` plus 64 lowercase hexadecimal characters |
| `page_context_sha256` | SHA-256 of the canonical complete typed page snapshot |
| `width`, `height` | Positive bounded screenshot-pixel dimensions |
| `observation_id` | Positive latest persistent-agent observation |
| `surface_revision` | Positive exact Test Kit revision for that observation |
| `page_context` | Ready, complete, non-diff forensic Page Context v1 snapshot |
| `dimensions` | Non-empty unique typed audit dimensions |
| `issued_at_unix_ms`, `deadline_unix_ms` | Absolute bounded provider window |
| `max_cost_microusd` | Provider-reported cost ceiling |

Dimensions are `visual_hierarchy`, `layout_composition`, `spacing_rhythm`,
`typography`, `color_use`, `consistency`, `interaction_clarity`,
`content_clarity`, and `responsive_composition`. Omitting `--dimension`
requests all dimensions. Repeating a dimension is rejected before session
access.

The provider response must repeat identity, observation, revision, both
digests, dimensions, image dimensions, and exact requested dimension order.
Each finding has a unique bounded ID, requested dimension, `high`, `medium`,
or `low` advisory priority, summary, rationale, recommendation, integer
confidence from 0 through 100, and one target:

- `page` identifies the complete current page;
- `node` names a visible current Test Kit node with finite geometry;
- `region` is a finite positive-sized rectangle wholly inside normalized
  screenshot coordinates.

Local admission rejects unknown or stale nodes, invalid geometry, duplicate
finding IDs, unrequested dimensions, oversized fields or context, identity or
digest mismatch, cost overrun, timeout, cancellation, image replacement, and
page revision drift. The admitted report protocol is
`a3s.test.design-audit-report/1` with `authority = advisory`. It has no test
outcome, expected-surface authority, action, or repair authorization.

The CLI ACL root is `design_audit` and accepts:

| Attribute | Rule | Default |
| --- | --- | --- |
| `max_cost_microusd` | Required non-negative provider cost ceiling | none |
| `timeout_ms` | 1 millisecond through 5 minutes | `30000` |
| `max_findings` | 1 through 500 | `100` |
| `max_summary_bytes` | 1 through 65536 | `2048` |
| `max_rationale_bytes` | 1 through 65536 | `8192` |
| `max_recommendation_bytes` | 1 through 65536 | `8192` |
| `max_page_context_bytes` | 1 through 33554432 | `8388608` |

Exactly one `provider` block supplies `name`, `model`, `endpoint`, and optional
`authorization_env`. Endpoint and credential rules match the other provider
adapters. HTTP replaces the local screenshot path with `observation.png` and
adds a digest-bound Base64 `image/png` attachment. The deployment remains
responsible for the model runtime and licensing.

After local admission and a final revision check, a compatible Web driver
projects the report through `reportDesignAudit`. Test Kit stores it separately
from deterministic Quality Reports and the Repair Ledger. Dismissal has no
side effect. Opening or cancelling review grants no authority. A suggestion
enters the Repair Ledger only after a human reviews or retargets it and
explicitly saves or sends it through the existing single/batch workflow.
