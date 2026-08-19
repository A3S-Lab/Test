<p align="center">
  <img src="./assets/readme/hero.svg" width="100%" alt="A3S Test turns fresh interface context into typed actions and inspectable evidence">
</p>

<p align="center">
  <a href="https://github.com/A3S-Lab/Test/releases/latest"><img src="https://img.shields.io/github/v/release/A3S-Lab/Test?style=flat-square&color=1264ff&label=release" alt="Latest release"></a>
  <a href="https://github.com/A3S-Lab/Test/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/A3S-Lab/Test/ci.yml?branch=main&style=flat-square&label=CI" alt="CI status"></a>
  <a href="https://a3s-lab.github.io/Test/"><img src="https://img.shields.io/badge/docs-中文%20%7C%20English-1264ff?style=flat-square" alt="Chinese and English documentation"></a>
  <img src="https://img.shields.io/badge/Rust-1.85%2B-56657b?style=flat-square" alt="Rust 1.85 or newer">
  <a href="./LICENSE"><img src="https://img.shields.io/badge/license-MIT-56657b?style=flat-square" alt="MIT License"></a>
</p>

<h3 align="center">Explore unknown interface paths. Preserve proven paths as typed regressions.</h3>

<p align="center">
  A3S Test gives coding agents fresh interface context, admits one typed action at a time,<br>
  and records the evidence needed to explain, reproduce, and preserve the result.
</p>

<p align="center">
  <a href="https://a3s-lab.github.io/Test/"><strong>中文文档</strong></a> ·
  <a href="https://a3s-lab.github.io/Test/en/"><strong>English</strong></a> ·
  <a href="#install">Install</a> ·
  <a href="#prove-one-real-path">Quick start</a> ·
  <a href="#embed-rendered-page-context">Test Kit</a> ·
  <a href="#architecture">Architecture</a>
</p>

## Install

The release installer downloads the matching CLI archive, verifies its
SHA-256, and installs the same portable A3S Test Skill for detected coding
agents. Run it again to upgrade both.

### macOS and Linux

```bash
curl -fsSL https://github.com/A3S-Lab/Test/releases/latest/download/install.sh | sh
```

### Windows PowerShell

```powershell
& ([scriptblock]::Create((irm 'https://github.com/A3S-Lab/Test/releases/latest/download/install.ps1')))
```

Pin a release when the test environment must be reproducible:

```bash
curl -fsSL https://github.com/A3S-Lab/Test/releases/latest/download/install.sh |
  sh -s -- --version v0.16.2
```

```powershell
& ([scriptblock]::Create((irm 'https://github.com/A3S-Lab/Test/releases/latest/download/install.ps1'))) -Version v0.16.2
```

The installers support CLI-only, Skill-only, agent-specific, and custom
installation targets. See the
[installation guide](https://a3s-lab.github.io/Test/guide/installation.html)
for every option, or download a prebuilt archive from
[Releases](https://github.com/A3S-Lab/Test/releases/latest).

## Prove one real path

Start a persistent Web session against a local product and define an
observable goal:

```bash
a3s-test agent start http://127.0.0.1:3000/checkout \
  --session checkout \
  --goal "Complete checkout with the fixture account" \
  --success "The confirmation heading is visible" \
  --json

a3s-test agent observe --session checkout --interactive --json
```

Voice-product tests can opt into a deterministic synthetic browser microphone:

```bash
a3s-test agent start http://127.0.0.1:3000/voice \
  --session voice \
  --goal "Verify the listening state" \
  --success "The listening indicator is visible" \
  --browser-microphone synthetic \
  --json
```

The microphone defaults to `disabled`. The `synthetic` profile never captures
the host microphone; it supplies Chromium's local fake media device and
permission grant, and a persistent agent session retains that profile across
turns. The same explicit option is available to `a3s-test run`, `agent run`,
and Web MCP sessions.

The observation returns a fresh generation and semantic refs instead of a
timing guess:

```text
observation_id: 1
@e1 [button] Continue
```

Bind the action to that observation, capture evidence, and finish explicitly:

```bash
a3s-test agent click @e1 \
  --session checkout \
  --observation 1 \
  --json

a3s-test agent screenshot screenshots/confirmation.png \
  --session checkout \
  --json

a3s-test agent finish \
  --session checkout \
  --status passed \
  --summary "Checkout completed and confirmation was observed" \
  --json
```

The session remains inspectable after the browser closes:

```text
.a3s-test/agent-sessions/checkout/
├── session.json
├── events.jsonl
├── report.json
└── artifacts/
    └── screenshots/confirmation.png
```

[Continue through the quick start](https://a3s-lab.github.io/Test/guide/)

## What A3S Test keeps stable

| Contract | What it prevents |
| --- | --- |
| Fresh observations | Semantic refs cannot silently cross a surface revision. |
| Typed actions | Unknown variants and fields fail before reaching a driver. |
| Scoped policy | Navigation, network, artifacts, and dispatch stay inside admitted boundaries. |
| Inspectable evidence | Events, screenshots, reports, and provenance remain machine-readable. |
| Negative visibility | A visible counterexample cannot be mistaken for a closed or removed UI. |
| Sampled stability | One passing render cannot hide a later flicker or optimistic rollback. |
| Owned cleanup | A run closes only the process tree, browser namespace, sockets, and files it created. |
| Separate authority | Browser facts, model advice, human authorization, and workspace mutation cannot impersonate one another. |

Ordinary assertions, timeouts, and ambiguously dispatched actions are never
replayed automatically. An ACL expectation can explicitly request bounded
stability sampling; the runner then repeats only that read-only assertion.
JSON fields, error codes, and process exit codes remain stable for local runs
and CI.

## Explore first, preserve second

| Workflow | Planner | Best for | Entry point |
| --- | --- | --- | --- |
| Agent session | Calling coding agent | Unknown paths, reproduction, UX review | Persistent Web CLI or Web/GUI MCP |
| ACL suite | Closed typed manifest | Regression, CI, cross-surface checks | `check` and `run` |
| Embedded loop | Host-injected `LlmProvider` | Products embedding A3S Test | `a3s-test-agent` library |

All three paths share the same `Action`, `SurfaceDriver`, evidence, result, and
lifecycle contracts. Once an explored path is stable, preserve it as ACL:

```acl
suite "product-smoke" {
    version = 1

    scenario "home-page" {
        name = "Open the home page"
        surface = "web"
        timeout_ms = 30000

        navigate "open" {
            url = "https://example.com"
        }

        wait "loaded" {
            load = "networkidle"
        }

        expect "heading" {
            text = "Example Domain"
            stable_for_ms = 300
            sample_interval_ms = 50
        }

        screenshot "evidence" {
            path = "home.png"
        }
    }
}
```

```bash
a3s-test check tests/e2e/smoke.acl --json
a3s-test run tests/e2e/smoke.acl --json
```

`stable_for_ms` starts a bounded observation window after the first successful
sample. The runner samples the same expectation every `sample_interval_ms` and
always samples once at the window boundary. A later false sample fails with
`test.assert.unstable`; a passing result records the first and last assertion,
sample count, requested interval, and observed duration. The window is 10 to
60,000 ms, the interval defaults to 50 ms (or the shorter window), and one
expectation may plan at most 1,001 samples.

Use `hidden` when the requirement is that a stable locator has no visible
match. It passes for an element that is absent or rendered without a visible
box, and fails with `test.assert.hidden` when a visible match exists:

```acl
expect "dialog-closed" {
    hidden = role("dialog", "Checkout")
    stable_for_ms = 300
    sample_interval_ms = 25
}
```

When disappearance is the synchronization point, use the same stable locator
on a `wait` instead of guessing an animation duration:

```acl
wait "dialog-closed" {
    hidden = role("dialog", "Checkout")
}
```

The runner evaluates both forms through the existing positive visibility
action introduced before revision 8; neither negative form adds a new action
variant. `expect hidden` probes once;
`wait hidden` probes immediately and then every 50 ms until the first
`test.assert.visible` mismatch proves that no visible match remains. A scenario
deadline or cancellation interrupts the wait and still closes the owned
surface. Work is statically capped at 1,201 probes, and timeout/cancellation
results retain the last visible counter-evidence. A later visible sample fails
a stable hidden assertion as `test.assert.unstable`. Use a semantic or CSS
locator; observation-bound `ref()` and `visual_point()` targets are rejected
because a missing ephemeral reference is not proof that the product element
is hidden.

Sampling is a time-resolution tradeoff. It catches state changes observed at a
sample point, but it cannot prove what happened between two points. Use a
smaller interval for short flicker, allow the complete window in the scenario
timeout, and keep semantic targets stable across renders.

## Assert live control state

Action protocol revision 8 can compare the state users actually interact with,
instead of inferring success from text or element presence:

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
    stable_for_ms = 300
    sample_interval_ms = 25
}
```

The paired conditions are `enabled`/`disabled`, `checked`/`unchecked`, and
`selected`/`unselected`. `selected_values` is a duplicate-free exact set:
order is canonicalized, extra or missing values fail, and `[]` proves a real
empty selection only when the target exists and exposes that state. Missing,
ambiguous, invalid, or unsupported targets remain `test.driver.*`; only a
successfully observed value that differs from the expectation becomes
`test.assert.*`. This prevents an absent checkbox from falsely proving
`unchecked` and an absent button from falsely proving `disabled`.

| Surface | Exact value | Boolean state | Selected values |
| --- | --- | --- | --- |
| Web | Live DOM `value` | Native live properties, then admitted ARIA state for custom controls | Native multi-select exact set |
| GUI | CUA semantic value when present | Unsupported until CUA exposes typed state | Unsupported |
| TUI | Unsupported | Unsupported | Unsupported |

The revision-8 regression set currently proves 400/400 deterministic Web
classifications, 100/100 stable state windows, 100/100 transient-state
rejections, and 15/15 positive plus 4/4 negative classifications in real
Chromium without leaking a private runtime directory.

## Assert rendered output, collection size, and order

Action protocol revisions 9 and 10 close three false-positive gaps left by
page-wide text and one-element visibility checks. Bind expected copy to one
exact target, assert the number of visible matches, or compare the complete
ordered text sequence produced by a stable locator:

```acl
expect "total-copy" {
    target = testid("total")
    rendered_text = "Total $42.00"
    stable_for_ms = 300
    sample_interval_ms = 25
}

expect "three-visible-rows" {
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
    stable_for_ms = 300
    sample_interval_ms = 25
}

expect "no-line-items" {
    target = css("[data-missing-line-item]")
    rendered_texts = []
}

expect "no-visible-errors" {
    target = role("alert", "Checkout error")
    visible_count = 0
}
```

`rendered_text` requires exactly one visible match and compares normalized
rendered text: leading and trailing whitespace is removed and every run of
whitespace becomes one space. A missing or ambiguous target remains a driver
error; only observed copy that differs is `test.assert.rendered_text`.

`visible_count` counts the complete visible match set, including a legitimate
zero. CSS targets use rendered-box visibility, so visually present
`aria-hidden` content still counts; semantic targets use the accessible
locator plane, exclude accessibility-hidden ancestors, and traverse open
Shadow DOM. Hidden, `display: none`, `visibility: hidden`, fully transparent,
and zero-geometry matches do not count. Observation refs and visual points are
rejected because they identify one observation, not a repeatable collection.

`rendered_texts` captures every visible match in locator traversal order and
compares normalized text item by item. Order and duplicates are evidence, so
`["Shipping", "Shipping"]` differs from both `["Shipping"]` and a reordered
sequence. An empty locator set is the observed sequence `[]`, not a
target-not-found error. ACL and the Web driver both enforce a 256-item bound;
oversized observed collections fail with `test.driver.web.collection_limit`.
Refs and visual points are rejected because neither describes a repeatable
collection. Invalid selectors remain driver errors, while an exact observed
sequence difference is `test.assert.rendered_texts`.

The revision-9 evidence set classifies 600/600 deterministic Web cases with no
misclassification: 100 text matches, 100 text mismatches, 100 missing targets,
100 count matches including zero, 100 count mismatches, and 100 invalid
selectors. Runner datasets accept 200/200 consistent text/count windows and
reject 200/200 transients as `test.assert.unstable`. A standalone Chromium CLI
run verifies seven positive observations and seven negative classifications,
including whitespace normalization, CSS and open-Shadow-DOM semantics, two
100 ms stability windows, and no leaked private runtime directory.

Revision 10 adds another 600/600 deterministic cases: ordered matches,
reordered mismatches, duplicate/content mismatches, empty-sequence matches,
empty-versus-expected mismatches, and invalid selectors. Combined runner
datasets accept 300/300 stable scalar-text/sequence/count windows and reject
300/300 transients. A standalone Chromium run proves 12 positive observations
and 12 negative classifications, three accepted and three rejected 100 ms
windows, exact duplicate/order evidence, open Shadow DOM traversal, and no
private runtime leak.

[Compare the workflows](https://a3s-lab.github.io/Test/guide/workflows.html)

## Assert rendered layout relations

Action protocol revision 11 turns geometry that the browser already knows into
repeatable product assertions. Each expectation resolves two stable targets,
captures both rendered rectangles atomically, and compares one explicit
relation:

```acl
expect "checkout-below-summary" {
    target = testid("checkout")
    relative_to = testid("summary")
    layout = "below"
    tolerance_px = 1
    stable_for_ms = 300
    sample_interval_ms = 25
}
```

Supported relations are `above`, `below`, `left_of`, `right_of`, `contains`,
`inside`, `overlaps`, `not_overlapping`, `aligned_left`, `aligned_right`,
`aligned_top`, `aligned_bottom`, `aligned_center_x`, `aligned_center_y`,
`same_width`, `same_height`, and `same_size`. `tolerance_px` defaults to zero
and is bounded to 1,024 integer CSS pixels. Directional tolerance permits that
many pixels of boundary intrusion; alignment and size use absolute difference;
overlap must exceed the tolerance on both axes.

Both targets must be repeatable semantic or CSS locators. ACL rejects browser
refs and visual points because their geometry belongs to one observation. A
Page Context `@cN` is accepted only after it resolves to a stable locator.
Missing, ambiguous, invalid, hidden-semantic, and malformed geometry remain
`test.driver.*`; only two valid rectangles that violate the requested relation
produce `test.assert.layout`. A stability window promotes a later relation
violation to `test.assert.unstable` and retains the first and last dual-rectangle
evidence.

Web captures both rectangles in one page evaluation. CSS follows visual
rendering and therefore admits visually present `aria-hidden` targets, while
semantic locators exclude accessibility-hidden ancestry and traverse open
Shadow DOM. GUI evaluates two elements from one fresh CUA snapshot; TUI fails
closed instead of estimating geometry from terminal cells.

The checked-in evidence classifies 3,400/3,400 deterministic relation cases,
accepts 100/100 sustained layout windows, rejects 100/100 transients, and runs
all 17 relations plus 15 negative/error cases in standalone Chromium with
exact socket and private-runtime cleanup.

## Prove viewport coverage and pointer reachability

A rendered box can exist outside the visual viewport or behind another
element. Action protocol revisions 12 and 15 keep those questions separate:

```acl
expect "checkout-in-view" {
    in_viewport = testid("checkout")
}

expect "checkout-mostly-visible" {
    target = testid("checkout")
    viewport_coverage_at_least = 80
}

expect "drawer-mostly-outside" {
    target = css("#drawer")
    viewport_coverage_at_most = 10
}

expect "checkout-pointer-hit" {
    pointer_reachable = testid("checkout")
    stable_for_ms = 300
    sample_interval_ms = 25
}
```

`in_viewport` requires the rendered target rectangle to have a positive-area
intersection with the visual viewport. Revision 15 adds quantitative coverage:
the intersection area divided by the complete rendered target area.
`viewport_coverage_at_least` accepts `1..=100`, while
`viewport_coverage_at_most` accepts `0..=99`; the two excluded endpoints would
be unconditionally true. These conditions prove geometry only, not occlusion
or pointer access. `pointer_reachable` clips the target to that viewport,
samples a fixed 3 by 3 grid at the `1/6`, `1/2`, and `5/6` fractions on each
axis, and requires at least one deep browser hit on the target or a
composed-tree descendant. None of the forms infers enabled state, keyboard
access, an event listener, or business clickability.

These target-bound expectations require a repeatable semantic or CSS locator.
ACL rejects browser refs and visual points, while a current Page Context `@cN`
may resolve to a stable locator before dispatch. Web acquires geometry and hit
evidence in one page evaluation, validates the untrusted response again in
Rust, traverses open Shadow DOM for semantic targets, and uses native hit
testing for occlusion. A transparent covering element blocks the target;
`pointer-events: none` content does not. GUI and TUI fail closed because their
current protocols do not expose equivalent evidence.

Checked-in evidence covers 1,000/1,000 base intersection cases plus 2,000/2,000
Core threshold cases, 4,000/4,000 Web protocol classifications, 300/300
sustained and 300/300 transient stability windows, and a standalone Chromium
run with 37 passing assertions and 25 negative or driver-error classifications
plus exact cleanup.

## Prove exact and component-scoped focus ownership

A keyboard action is not complete merely because a key was sent. Action
protocol revision 13 can prove which element owns focus and which rendered
component contains it:

```acl
expect "checkout-focused" {
    focused = role("button", "Checkout")
}

expect "cancel-unfocused" {
    unfocused = testid("cancel")
}

expect "dialog-owns-focus" {
    focus_within = role("dialog", "Checkout")
    stable_for_ms = 300
    sample_interval_ms = 25
}

expect "page-does-not-own-focus" {
    focus_outside = testid("page-shell")
}
```

`focused` compares the target with the deepest active element observable from
the current document through nested open shadow roots. `focus_within` accepts
the target itself or a descendant in the rendered flat tree, including
assigned slots. `unfocused` and `focus_outside` invert those observed states
only after the target resolves successfully. A missing target therefore never
passes either negative form.

All four conditions require a repeatable semantic or CSS locator. ACL rejects
browser refs and visual points as `test.spec.focus_target_unstable`; a current
Page Context ref may first resolve to a stable locator. Semantic targets
traverse open Shadow DOM and exclude accessibility-hidden composed ancestry.
CSS retains current-document query semantics. Missing, ambiguous, invalid,
and unsupported observations remain `test.driver.*`; only a resolved focus
mismatch becomes `test.assert.focused`, `.unfocused`, `.focus_within`, or
`.focus_outside`. GUI and TUI fail closed because their current protocols do
not expose equivalent focus evidence.

Checked-in evidence classifies 600/600 deterministic Web cases, accepts
200/200 sustained focus windows, rejects 200/200 transient windows, and runs
17 positive assertions plus 11 negative or driver-error classifications in
standalone Chromium. The real browser fixture covers forward and reverse Tab,
open Shadow DOM, assigned slots, accessibility-hidden ancestry, timed focus
movement, exact socket cleanup, and no private-runtime leak.

## Prove live disclosure, toggle, editability, requirement, and validity state

Rendered copy and geometry do not prove what a control currently means. Action
protocol revision 14 adds five orthogonal state pairs that read authoritative
browser state instead of inferring it from labels or pixels:

```acl
expect "filters-open" { expanded = testid("filters") }
expect "pin-off" { unpressed = role("button", "Pin") }
expect "name-locked" { readonly = label("Display name") }
expect "email-required" { required = placeholder("Email") }
expect "email-invalid" { invalid = testid("email") }
```

The inverse forms are `collapsed`, `pressed`, `writable`, `optional`, and
`valid`. Native state wins where the platform defines it: `<details>.open`,
applicable input or textarea `readOnly`, applicable input, select, or textarea
`required`, and Constraint Validation when `willValidate` is true. Otherwise
Web accepts only valid ARIA state tokens. `aria-pressed="mixed"`, unknown ARIA
tokens, and elements with no authoritative state fail closed as
`test.driver.web.state_unsupported`.

Each pair answers one question. In particular, `writable` means the applicable
read-only state is false; it does not imply `enabled`. Proving editability
therefore requires both `enabled` and `writable`. Negative forms require a
successfully resolved target and observed state, so a missing element never
proves `collapsed`, `unpressed`, `writable`, `optional`, or `valid`.

All ten conditions require a repeatable semantic or CSS locator. ACL rejects
browser refs and visual points as `test.spec.semantic_state_target_unstable`;
a current Page Context ref may first resolve to a stable locator. Missing,
ambiguous, invalid, unsupported, and malformed observations remain
`test.driver.*`. Only an observed mismatch becomes the condition-specific
`test.assert.*` error. GUI and TUI fail closed without equivalent state
evidence, and every form composes with bounded assertion stability.

Checked-in evidence classifies 1,000/1,000 deterministic Web cases, accepts
100/100 sustained windows, rejects 100/100 transient windows, and runs 27
positive assertions plus 17 negative or driver-error classifications in
standalone Chromium with native controls, ARIA, open Shadow DOM, exact fixture
cleanup, and no private-runtime leak.

## Embed rendered page context

Development frontends can embed `@a3s-lab/testkit` so A3S Test can read the
rendered page without relying on pixels alone:

```bash
npm install https://github.com/A3S-Lab/Test/releases/latest/download/a3s-testkit.tgz
```

```tsx
import {
  A3SReviewOverlay,
  A3STestBoundary,
  A3STestKit,
} from "@a3s-lab/testkit/react";

export function App() {
  return (
    <A3STestKit
      enabled={import.meta.env.DEV}
      page={{ id: "checkout" }}
      repairEndpoint="/__a3s-test/repairs"
      redact={["[data-payment-field]"]}
    >
      <A3STestBoundary
        id="checkout-form"
        name="Checkout form"
        source={{ file: "src/Checkout.tsx" }}
      >
        <Checkout />
      </A3STestBoundary>
      <A3SReviewOverlay enabled={import.meta.env.DEV} locale="auto" />
    </A3STestKit>
  );
}
```

After rendering, Test Kit publishes bounded, revisioned context:

- Accessible semantics, DOM and open Shadow DOM structure, and form state.
- Component identity, bounded source hints, and preferred semantic locators.
- Viewport, document, and normalized coordinates for actionable elements.
- Observed color, typography, spacing, radius, shadow, and safe design-token
  profiles with source counts and confidence.
- Flex, Grid, flow, scroll-container, and stacking relationships; exact
  client/scroll extents, signed offsets, and derived overflow/clipping state;
  resolved physical margin, border, and padding edges with box sizing, writing
  mode, and text direction; plus deterministic repeated-component clusters
  that do not guess from class names alone.
- Real default-to-hover/focus/checked/expanded state differences and bounded
  CSS, Web Animations, document/scroll/view timelines, animation ranges,
  sticky, canvas, media, and responsive evidence.
- Product facts, explicit redaction, and node/state/string/byte/time budgets.

Mutation, resize, scroll, viewport, and navigation signals advance the surface
revision. The review overlay lets a person mark one element or an ordered
batch, attach repair intent, save a draft, and explicitly send it to the
session-owning coding agent. A fresh browser run verifies admitted changes
before acceptance.

After selecting an element or rectangular area, the reviewer can also open a
dependency-free SVG design board to sketch the intended UI or attach a
PNG/JPEG screenshot by upload, paste, drop, or permissioned browser capture.
The board supports freehand, rectangle, text, selection, movement, resizing,
styling, and history inside the Test Kit Shadow DOM. Attached references travel
with the typed finding; Web sessions validate and materialize inline images as
bounded SHA-256-addressed artifacts before a coding agent receives them.

UI understanding is an additive `a3s.test.ui-understanding/1` evidence block
inside Page Context. Its observation ID binds transient computed state without
turning every animation frame or focus move into a new page revision. It does
not replace the browser accessibility tree, execute page-authored code, infer
component types from class names, or authorize an action or repair.
At the Web boundary, duplicate graph relationships, missing parents or edge
endpoints, incomplete or cyclic containment, inconsistent component
membership, stale bindings, invalid geometry, and budget drift fail closed
before evidence reaches an agent.

The review surface follows `<html lang>` by default and provides complete
English and Simplified Chinese workflow copy, including status announcements
and accessible names. Applications can pin `locale="en"` or `locale="zh-CN"`
and override known, bounded presentation messages without changing the page
context or repair protocols. Automatic mode observes live language changes;
the Layout catalog displays and searches all 90 built-in component types in
either language while leaving project-specific free-form values untouched.

The Web adapter resolves role, label, test ID, and placeholder targets across
light DOM and open Shadow DOM for click, fill, and check actions. Pointer
clicks use the target's post-scroll coordinates so host-page smooth scrolling
cannot invalidate the hit point.

[Integrate Test Kit](https://a3s-lab.github.io/Test/guide/testkit.html)

## Generate reviewed expectations

PRDs, designs, and rendered pages describe different kinds of truth:

| Source | Authoritative for | Never treated as |
| --- | --- | --- |
| PRD | Product intent, copy, outcomes, constraints | Browser-observed state |
| Design | Regions, hierarchy, geometry, image digest | Accessibility semantics |
| Page context | Rendered semantics, state, components, locators, geometry | Product intent |

A deployment-owned provider can propose cited expectations and explicit
conflicts from PRDs or design images. A person reviews those candidates before
the CLI renders a Surface Contract in ACL:

```bash
a3s-test contract generate \
  --config tests/contracts/checkout.generate.acl \
  --output tests/contracts/checkout.draft.json

a3s-test contract review \
  --draft tests/contracts/checkout.draft.json \
  --review tests/contracts/checkout.review.acl \
  --output tests/contracts/checkout.acl \
  --audit tests/contracts/checkout.reviewed.json
```

Optional visual grounding returns digest-bound point or box candidates and
never clicks. Optional design audit remains advisory and cannot set a verdict
or authorize repair.

[Read the source-to-contract workflow](https://a3s-lab.github.io/Test/guide/contracts.html)

## Surface support

| Surface | Current boundary | Backing adapter |
| --- | --- | --- |
| Web | Persistent Agent sessions and ACL suites | [A3S Browser](https://github.com/A3S-Lab/Browser) or a compatible standalone browser |
| GUI | Contract-tested and release-certified on macOS | Locked A3S CUA semantic and window-vision profiles |
| TUI | Deterministic ACL suites | Owned PTY / ConPTY process tree and bounded VT semantics |

Windows and Linux GUI combinations currently fail closed as unsupported.
Inspect available capabilities without opening a surface:

```bash
a3s-test capabilities --json
a3s-test agent schema
a3s-test provider schema design-audit
a3s-test provider schema visual-grounding
a3s-test worker inventory
```

## Architecture

<p align="center">
  <img src="./assets/readme/architecture.svg" width="100%" alt="Coding-agent sessions and ACL suites converge on one typed core, surface adapters, evidence ledger, and owned cleanup boundary">
</p>

The CLI and MCP server are product boundaries, not backend selectors. Browser,
desktop perception, terminal emulation, and LLM implementations remain typed
adapters outside the framework-independent core.

<details>
<summary><strong>Workspace map</strong></summary>

```text
crates/
├── a3s-test-cli/         # Sessions, local and distributed runs, MCP, CI
├── a3s-test-core/        # Typed suites, actions, observations, contracts
├── a3s-test-runner/      # Deadlines, cancellation, retries, reports
├── a3s-test-session/     # Surface-neutral long-lived session layer
├── a3s-test-worker/      # Inventory and persistent remote worker service
├── a3s-test-driver-gui/  # Locked MCP adapter for A3S CUA
├── a3s-test-driver-tui/  # Owned PTY / ConPTY and bounded VT semantics
├── a3s-test-driver-web/  # A3S Browser / standalone browser adapter
└── a3s-test-agent/       # Providers, grounding, contracts, design audit

packages/
└── testkit/              # Rendered page context and human review SDK

skills/
└── a3s-test/             # Portable coding-agent Skill
```

</details>

[Study the architecture](https://a3s-lab.github.io/Test/concepts/architecture.html)

## Documentation

The Rspress site serves the current documentation in Chinese by default, with
an English locale and immutable historical snapshots:

- [简体中文](https://a3s-lab.github.io/Test/)
- [English](https://a3s-lab.github.io/Test/en/)
- [Capability reference](https://a3s-lab.github.io/Test/en/reference/capabilities.html)
- [Troubleshooting](https://a3s-lab.github.io/Test/en/guide/troubleshooting.html)
- [v0.16.2 snapshot](https://a3s-lab.github.io/Test/v0.16.2/)
- [v0.15.0 snapshot](https://a3s-lab.github.io/Test/v0.15.0/)

Repository specifications remain the source of truth for exhaustive protocol
details: [architecture](docs/architecture.md),
[agentic contract](docs/agentic.md), [ACL specification](docs/specification.md),
[Test Kit contract](docs/testkit.md),
[screen-reader audit](docs/screen-reader-audit.md),
[roadmap](docs/roadmap.md), and [changelog](CHANGELOG.md).

## Development

Run Rust gates from the repository root:

```bash
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 test --workspace --all-targets --locked
cargo +1.85.0 clippy --workspace --all-targets --locked -- -D warnings
```

Run documentation gates from `website/`:

```bash
npm ci
npm run format:check
npm run check
npm run build
npm run check:site
```

Run the production website and embedded Test Kit regression from the
repository root after installing the website, Test Kit, and admitted
`agent-browser` dependencies:

```bash
A3S_TEST_AGENT_BROWSER="$(command -v agent-browser)" \
  cargo test -p a3s-test-cli --test web_e2e \
  real_agent_browser_runs_the_website_testkit_suite \
  --locked -- --ignored --exact --nocapture
```

## License

A3S Test is available under the [MIT License](LICENSE).
