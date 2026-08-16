# Embedded Web Test Kit

## Purpose

`@a3s-lab/testkit` is a development-only frontend SDK for applications that
want to expose precise, bounded page context to A3S Test. It complements the
browser accessibility snapshot with component ownership, source hints,
geometry, application-declared facts, and a human review surface.

The SDK is not a browser driver, test runner, coding agent, or source-code
editor. A3S Test remains the owner of surface sessions, typed actions,
evidence, reports, and cleanup. An already authorized coding agent remains the
planner and workspace editor.

## Product layers

```text
application
├── Context Runtime                    always headless
├── framework adapter                  optional explicit boundaries
└── Review Overlay                     optional human interaction
        │
        │ a3s.test.page-context/1
        v
A3S Browser adapter
        │
        v
A3S Test session, repair ledger, evidence, and MCP
        │
        v
authorized coding agent
```

The Context Runtime must work without the Review Overlay. CI should normally
enable context and disable the overlay.

## Page Context Bridge v1

The page exposes one non-enumerable, symbol-addressed bridge with protocol
identifier `a3s.test.page-context/1`. The bridge is read only to application
code except for explicit SDK registration APIs. Its browser-facing operations
are:

- `probe()` returns protocol and SDK versions plus supported capabilities.
- `snapshot(request)` returns a bounded page or scoped context snapshot.
- `resolve(nodeId)` returns the live DOM node for a current snapshot node.
- `waitForChange(revision, timeoutMs)` completes when the semantic revision
  advances or the bounded timeout expires.
- `subscribe(listener)` publishes revision and review events and returns an
  unsubscribe function.
- `submitRepair(request)` accepts a human-confirmed single finding or batch
  through the browser-bound repair channel.
- `applyRepairEvent(event)` projects queue and agent state back into the
  overlay.
- `reportQuality(report)` accepts a bounded deterministic Contract Report for
  optional human review.
- `listQualityReports()`, `dismissQualityFinding(...)`, and
  `dismissQualityReport(...)` manage only local review candidates.
- `reportDesignAudit(report)` accepts a revision-bound advisory Design Audit
  Report for optional human review.
- `listDesignAuditReports()`, `dismissDesignAuditFinding(...)`, and
  `dismissDesignAuditReport(...)` manage a separate advisory store.
- `dispose()` removes SDK observers, portals, listeners, and private state.

The bridge does not expose `eval`, filesystem access, cookies, arbitrary
network requests, or shell execution.

### Snapshot request

```json
{
  "detail": "summary",
  "scope": { "kind": "page" },
  "sinceRevision": null,
  "cursor": null,
  "limits": {
    "nodes": 500,
    "stringBytes": 4096,
    "encodedBytes": 1048576
  }
}
```

`detail` is `summary`, `scoped`, `diff`, or `forensic`. A scope selects the
page, a current context node, a registered component, or a viewport/document
rectangle. The SDK may lower caller limits to its configured ceiling but must
never raise them. Truncated results carry an opaque cursor bound to the page
revision and scope.

### Snapshot response

```json
{
  "protocol": "a3s.test.page-context/1",
  "sdkVersion": "0.4.0",
  "revision": 42,
  "page": {
    "id": "checkout",
    "url": "http://127.0.0.1:3000/checkout",
    "route": "/checkout",
    "title": "Checkout",
    "ready": true,
    "viewport": {
      "width": 1440,
      "height": 900,
      "dpr": 2,
      "visual": { "x": 0, "y": 0, "width": 960, "height": 600, "scale": 1.5 }
    },
    "document": { "width": 1440, "height": 2210 },
    "scroll": { "x": 0, "y": 800 },
    "language": "en",
    "theme": "light"
  },
  "components": [],
  "nodes": [],
  "facts": {},
  "removedNodeIds": [],
  "truncated": false,
  "nextCursor": null
}
```

Node IDs are SDK-private handles. A3S Browser maps current node IDs to A3S
observation-bound `@cN` refs. Callers never persist or act on a raw node ID.

### Geometry

Every returned node contains geometry in three coordinate spaces when it has a
rendered box:

- viewport CSS pixels from `getBoundingClientRect()`;
- document CSS pixels after current scroll offsets;
- current visual-viewport-normalized values. Values may be outside `[0, 1]`
  when a rendered box is outside the currently visible zoomed region.

Geometry also records visible ratio, topmost-point occlusion, fixed/sticky
positioning, transform presence, and the nearest scroll container. Device
pixels are not mixed with CSS pixels. Multi-root components expose `boxes`
instead of inventing one inaccurate rectangle.

`page.viewport.width` and `height` describe the layout viewport in CSS pixels,
and `dpr` describes the current device-pixel ratio. Compatible browsers also
publish an additive `visual` object containing its CSS-pixel offset, visible
size, and page scale. Browser zoom therefore changes `visual`, `dpr`, or both,
but never multiplies `getBoundingClientRect()` values into device pixels.
Visible ratio, occlusion sampling, and normalized geometry use the visual
viewport so targets outside a zoomed view are not reported as visible.

Geometry is evidence and a last-resort target. Semantic role, label, test ID,
placeholder, and stable text locators remain preferred.

## Framework-neutral runtime

The base package discovers semantic DOM nodes, open Shadow DOM, form state,
accessibility identity, stable locator candidates, and geometry. It uses
`MutationObserver`, `ResizeObserver`, intersection observation, scroll and
history/navigation signals to invalidate a cached snapshot. It must not poll
an unchanged page. Direct `installTestKit` calls require `enabled: true` just
like the React provider; missing or false-like enablement returns a disabled
bridge and never installs the global protocol.

The runtime never serializes:

- password or hidden input values;
- cookies, local/session storage, request headers, or tokens;
- arbitrary React/Vue/Svelte props, state, fiber internals, or closures;
- text under configured redact selectors;
- cross-origin frame contents;
- full computed styles in summary mode.

Application facts are an explicit callback result and pass through the same
depth, key, string, and encoded-size bounds.

Node IDs are held in a private `WeakMap` side table. The runtime does not add
IDs, coordinates, component ownership, or source paths as attributes on the
host application's DOM. It derives that metadata after layout and invalidates
the projection through mutation, resize, route, viewport, and scroll signals.

## React adapter

The React adapter exposes:

```tsx
<A3STestKit
  enabled={import.meta.env.DEV}
  page={{ id: "checkout" }}
  ready={() => !isBooting}
  facts={() => ({ checkoutStep, cartItemCount: cart.items.length })}
  redact={["[data-private]", "[data-payment-field]"]}
  repairEndpoint="/__a3s-test/repairs"
>
  <Application />
  <A3SReviewOverlay enabled={import.meta.env.DEV} locale="auto" />
</A3STestKit>
```

`repairEndpoint` is an optional same-origin application adapter. It accepts a
bounded `a3s.test.repair/1` POST and forwards it to the owning A3S Test session.
If it is omitted, an active A3S Test browser session can drain the same
page-local queue through the fixed bridge operation. The endpoint is not an
A3S Test control API and receives no workspace or agent credentials.

`A3STestBoundary` registers explicit component identity, optional source hints,
facts, readiness, and all rendered roots beneath the boundary. Automatic DOM
context continues to work when no boundary is present.

### Vite

Mount the provider at the application root and gate it with
`import.meta.env.DEV`:

```tsx
root.render(
  <A3STestKit enabled={import.meta.env.DEV} page={{ id: "app" }}>
    <App />
    <A3SReviewOverlay enabled={import.meta.env.DEV} />
  </A3STestKit>,
);
```

Keep `repairStorage="memory"` for disposable fixtures or use the default
session storage while reviewing a live development page.

### Next.js

Put the Test Kit in a client-only provider and enable it only outside
production:

```tsx
"use client";

export function TestProvider({ children }: { children: React.ReactNode }) {
  return (
    <A3STestKit
      enabled={process.env.NODE_ENV !== "production"}
      page={{ id: "web" }}
    >
      {children}
      <A3SReviewOverlay
        enabled={process.env.NODE_ENV !== "production"}
      />
    </A3STestKit>
  );
}
```

The headless runtime tolerates SSR markup and hydration. The provider,
boundaries, and overlay render on the server without accessing the DOM or
emitting layout-effect warnings; synchronous boundary registration and focus
effects begin in the browser. Both components require an explicit `enabled`
value, and the overlay additionally refuses to mount without a compatible live
bridge. It is created only in the browser and remains isolated in its own
Shadow DOM. Direct framework-neutral bridge inspection returns `null` on the
server; enabling the runtime directly remains a browser-only operation and
fails with a contextual error.

### Review language and host copy

`A3SReviewOverlay` accepts `locale="auto" | "en" | "zh-CN"`. The default,
`auto`, reads `document.documentElement.lang` when the overlay renders. Every
`zh-*` language tag resolves to the Simplified Chinese review UI; other tags
resolve to English. The resolved language is also set on the Shadow DOM review
root so localized control names, status labels, live announcements, and text
use the correct language context.

Set a language explicitly when the review surface should not follow the page:

```tsx
<A3SReviewOverlay
  enabled={import.meta.env.DEV}
  locale="zh-CN"
  messages={{ reviewTitle: "页面评审" }}
/>
```

`messages` is a typed, partial map of known review-message keys. Runtime
admission ignores unknown keys, blank strings, and values longer than 2,048
characters. Overrides affect presentation only; they are never added to page
context, repair instructions, or hidden agent input.

## Human review and repair submission

The optional overlay creates local draft findings from element click, selected
text, explicit multi-select, a rectangular region, or freehand drawing. A
draft includes a human instruction and may include success criteria, intent,
and severity.

The overlay is an operate-mode instrument panel, not a second application
shell. Its header stays one line, explanatory copy truncates, findings use
compact separators instead of nested cards, and the host page remains the
visual authority. Severity always has a text label. Quality markers use a
distinct dashed treatment so color is not the sole distinction. Opening the
secondary tool tray closes the findings workspace and opening the workspace
closes the tray, so floating surfaces cannot obscure one another. Preferences
remain scrollable within short desktop viewports. Mobile controls use
44-CSS-pixel touch targets and 16-pixel form text to avoid accidental zoom.

### Deterministic quality candidates

A closed ACL suite can reconcile an admitted Expected Surface Contract against
the current atomic observation. Core owns matching, rules, severity, stable
finding IDs, and the `passed`, `failed`, or `inconclusive` outcome. The Web
driver may then project the resulting report into a compatible Test Kit:

```text
Runner report --optional one-way projection--> Quality Store
                                                   |
                                             reviewer confirms target
                                                   |
                                    local draft or explicit submission
                                                   |
                                                   v
                                          authoritative Repair Ledger
```

Quality candidates and submitted repairs are deliberately separate stores.
Projection never authorizes workspace changes and never changes the Runner
verdict. Viewing a finding, opening its editor, cancelling the editor, or
cancelling manual target selection leaves the candidate intact. The candidate
is removed only when a reviewer explicitly dismisses it, saves it as a local
draft, or successfully submits a repair. These operations affect one finding,
not its siblings.

If `observed_node_id` still resolves at the current page revision, the overlay
can stage that node for review. A missing or stale node requires the reviewer
to choose a target manually. The private observed node ID is never treated as
durable identity across a route change or hot reload.

The in-memory Quality Store retains at most five reports by default, with a
configurable bound from one through twenty. Each report is limited to 500
findings, 5,000 matches, 1 MiB of encoded JSON, finite numbers, bounded strings,
unique finding IDs, and JSON depth 32. A newer report atomically replaces the
same contract/variant/state scope. A passed report or one with no findings
clears earlier candidates in that scope while still emitting a refresh event.

### Advisory design-audit candidates

Design audit has a different source of truth. Browser facts remain the
revision-bound screenshot and complete forensic Page Context snapshot; the
provider's interpretation of hierarchy, composition, rhythm, typography,
color, consistency, clarity, and responsiveness is advisory. The Web driver
projects only locally admitted `a3s.test.design-audit-report/1` values:

```text
verified screenshot + forensic page context
                    |
         admitted advisory provider report
                    |
             Design Audit Store
                    |
       reviewer dismisses, edits, or retargets
                    |
          local draft or explicit submission
                    |
            authoritative Repair Ledger
```

The Design Audit Store is separate from both the deterministic Quality Store
and Repair Ledger. It requires `authority = advisory`, the current exact page
revision, bounded provider/model and digest provenance, unique requested
dimensions, at most 500 bounded findings, and at most 1 MiB encoded JSON. A
node target must resolve when projected. Page and normalized-region targets
are converted to current visual-viewport CSS pixels for review markers. Any
later page revision clears the stored advice before it can be promoted.

The overlay displays summary, rationale, dimension, priority, confidence, and
target. Opening or cancelling the editor leaves the suggestion intact.
Dismissal removes only that suggestion. A recommendation becomes a repair
draft only after explicit review or retargeting; high priority maps to
`important`, never `blocking`. Saving and sending use the same single/batch
flow and verification gates as a manually marked finding. The provider cannot
skip that human promotion step.

`maxDesignAuditReports` defaults to five and is clamped from one through
twenty. A newer report from the same provider, model, and observation replaces
that scope; an empty report clears it.

### Layout Mode

Layout Mode adds two typed repair intents without turning the overlay into a
page builder:

- **Placement** records the requested component type, `page` or `wireframe`
  canvas, optional purpose, and a target region. The target has kind `region`
  and may have no current node because the requested component does not exist
  yet.
- **Rearrange** selects a current section by private node ID, captures its
  original viewport rectangle, and records a target region plus an optional
  purpose.

The normal workflow is to open **Layout**, describe the page purpose, then use
**Draw placement** for a new component or **Select section on page** followed
by destination coordinates or **Draw destination** for a rearrangement. The
source selection supports pointer input and the same focus-and-Enter keyboard
path as element marking. Layout drafts can be edited, selected, and submitted
in the same stable batch order as other findings.

For placement, the component catalog provides 90 independently defined common
Web component types in ten purpose-based categories. Search matches category,
component name, and bounded local synonyms such as `one-time code`; every
result is a native keyboard-operable button. Catalog selection fills the
explicit component-type field. Reviewers can always ignore the catalog and
enter a free-form component type, including a project-specific component that
is not listed. Search terms and catalog entries are presentation data only and
never become hidden repair instructions.

All layout rectangles use viewport CSS pixels. The wireframe grid and its
adjustable page fade, selected region, and destination preview are
pointer-transparent overlay evidence; they
never add inline styles, move nodes, or otherwise mutate application layout.
The coding agent remains responsible for editing authorized source files, and
A3S Test re-observes the result before review.

The `a3s.test.repair/1` target carries the intent as structured data:

```json
{
  "kind": "region",
  "nodeIds": [],
  "region": { "x": 700, "y": 320, "width": 300, "height": 160 },
  "layout": {
    "kind": "placement",
    "componentType": "Pricing section",
    "canvas": "wireframe",
    "purpose": "Developer tool landing page"
  }
}
```

```json
{
  "kind": "node",
  "nodeIds": ["private-current-node-id"],
  "region": { "x": 40, "y": 420, "width": 560, "height": 180 },
  "layout": {
    "kind": "rearrange",
    "originalRegion": { "x": 24, "y": 140, "width": 560, "height": 180 },
    "purpose": "Developer tool landing page"
  }
}
```

Unknown layout fields, empty component types, invalid canvases, non-finite
rectangles, placement without a region, and rearrangement without a current
node or destination are rejected at the page boundary. Instructions and page
text remain separate untrusted evidence; layout metadata is never converted
into a hidden prompt.

Submission is always explicit by default:

- `Send and auto-fix` submits one draft.
- `Send selected (N)` submits the checked drafts in visible order.
- `Send all` submits every draft in visible order.

Auto-send can be enabled only for the current browser session through the
visible overlay toggle (or the initial `autoSend` prop). It does not persist
across restart.

The review preferences section stores only presentation and local interaction
choices in `localStorage`: system/light/dark theme, marker color,
clear-after-copy, page pointer blocking, left/right dock, and wireframe page
fade. Records are schema-checked, limited to 2 KiB, and discarded atomically
when corrupt or unknown. Auto-send and animation pause are deliberately absent
from that record. `Hide until tab restart` uses `sessionStorage`, removes only
the overlay UI, and leaves the headless page-context bridge active. When the
reviewer activates it, keyboard focus returns without scrolling to the last
connected application control.

Page pointer blocking is an explicit reviewer choice. When enabled, the Test
Kit cancels pointer, mouse, touch, wheel, and context-menu input headed for the
host page while continuing to accept input within its Shadow DOM. It does not
block keyboard input. Clear-after-copy removes only the drafts included in the
copy, and only after the browser or host clipboard adapter confirms success;
failed clipboard writes retain every draft.

The overlay also exposes global review commands. `Ctrl/Command+Shift+F`
toggles the panel from anywhere outside an editable target. While the panel is
open, `L` toggles Layout Mode, `P` pauses or resumes page motion, `H` hides or
shows markers, `C` copies the selected drafts as Markdown, and `X` clears all
local drafts. Inputs, textareas, selects, contenteditable regions, and ARIA
textbox, searchbox, combobox, or spinbutton controls retain those letter
shortcuts and the panel toggle while the reviewer is typing. Unmodified
`Escape` has a narrower ownership rule: active marking or an open finding
editor receives it first, even when focus is editable. An idle host editable
retains `Escape`, and the review panel stays open. Outside editable controls,
`Escape` closes an otherwise idle panel. Each command also has a named button
and visible shortcut tooltip. The launcher and active controls expose the same
bindings through `aria-keyshortcuts`. Review preferences includes a
keyboard-reference section that remains available to keyboard and
screen-reader navigation and states the same ownership boundary.

Motion pause preserves host ownership. Test Kit pauses only animations and
media that are actively running, continues to freeze new motion while the
review pause is enabled, and resumes only the motion it recorded. Animations
or media that the application had already paused are never started by Test Kit.

Keyboard multi-selection remains in the host application until it is ready to
describe. Focus an application element and press `Enter` to add it; the polite
announcer reports the bounded count without mounting the finding editor or
activating the host control. Move focus to each additional element and repeat,
then press `Shift+Enter` to finish and move into the two-textarea finding
editor. Pressing `Escape` from that completed multi-select editor discards it
and restores focus to the review panel. During selection, `Escape`, the marking
Cancel control, panel toggling, and Layout Mode discard the incomplete
candidate and restore application focus instead of leaving a zero-target or
stale editor behind.

Marker rectangles remain pointer-transparent page evidence. A draft marker
adds only a 28 CSS-pixel edit button at its top-start corner; activating it
opens the normal editor, where the draft can be updated or deleted. Reviewers
can hide one draft marker, hide all markers, or clear every local draft without
affecting submitted repairs.

Host applications may observe bounded local workflow events without replacing
the A3S Test repair ledger:

```tsx
<A3SReviewOverlay
  enabled={import.meta.env.DEV}
  copyToClipboard={(text) => applicationClipboard.writeText(text)}
  onDraftAdded={(draft) => auditLocalDraft("added", draft)}
  onDraftUpdated={(draft) => auditLocalDraft("updated", draft)}
  onDraftDeleted={(draft) => auditLocalDraft("deleted", draft)}
  onDraftsCleared={(drafts) => auditLocalClear(drafts)}
  onCopied={({ format, text, drafts }) => auditLocalCopy(format, drafts.length)}
  onSubmitted={(repairs) => auditSubmittedIds(repairs.map((repair) => repair.id))}
/>
```

Callback values are structured clones and callback failures are isolated from
review state. Return values are ignored and never become repair instructions.
`copyToClipboard` replaces only the browser clipboard write; A3S Test still
generates the bounded Markdown or JSON payload.

### Accessibility and audit boundary

The review surface is a named, non-modal dialog inside an open Shadow DOM.
Every repeated draft and repair action includes the finding instruction in its
accessible name. Pause, marker visibility, auto-send, and theme controls expose
stable state-aware names, and keyboard focus returns to a durable control when
closing the dialog, sending or deleting a draft, or completing a clarification
reply. Hiding the overlay until the tab restarts returns focus to the last
connected application control before the Shadow DOM disappears. Repair state
changes and submission results use one visually hidden polite live region so a
screen reader does not announce the entire finding list again. Chinese and
English labels, descriptions, status messages, live announcements, and ARIA
names share the same bounded message catalog rather than diverging into
separate interaction implementations.

Automated React tests cover dialog naming, control names, live-region messages,
shortcut metadata and help content, and Shadow DOM focus restoration. The
ignored real Chromium Test Kit suite also captures the browser accessibility
tree, verifies the keyboard reference, and checks the launcher-to-dialog focus
round trip, editable `Escape` ownership, completed-editor cancellation,
keyboard multi-selection focus, and the hide-to-application focus transfer.
The same real-browser fixture runs an independent `axe-core` WCAG A/AA scan
across system, light, and dark themes plus preferences, marking editors,
restored drafts, Layout Mode, contract and design candidates, submitted
repairs, clarification replies, human review actions, and terminal states.
The scan includes the open review Shadow DOM and reports exact failing nodes.

These checks are regression evidence for DOM, accessibility-tree, contrast,
and keyboard-scroll semantics; they are not a substitute for completing every
workflow with an actual screen reader. M8 remains open until an independent
reviewer audits the full review lifecycle with VoiceOver, NVDA, or an
equivalent supported screen reader in an environment that permits
assistive-technology inspection.

Use the [independent screen-reader audit procedure](screen-reader-audit.md) to
run the loopback fixture, execute the canonical 15-workflow manifest, preserve
bounded evidence, and verify the revision-bound audit artifact. The verifier
loads versioned inputs from the named Git commit and emits a reproducible v2
record with SHA-256 bindings for the audit, workflow manifest, and ordered
evidence set. Structural verification accepts documented failures or blockers;
the separate `--require-pass` gate requires every workflow to pass. Neither
gate grants repair authority, and the harness alone does not close M8.

At submission time, the Test Kit enriches a repair with a fresh context
revision and bounded page context. A submitted target contains current private
node IDs, component/source hints, semantic locator candidates, geometry,
nearby context, route, viewport, and declared facts. A3S Test maps those IDs to
observation-bound refs when it observes or inspects the page. DOM content and
facts are marked untrusted evidence and are never concatenated into hidden
instructions. Before a finding becomes claimable, A3S Test captures and hashes
its own bounded page context and screenshot and records the current console and
page-error counts as the verification baseline.

## Repair state machine

```text
draft -> queued -> claimed -> repairing -> verifying -> review_ready -> resolved
                    |             |           |
                    v             v           v
                cancelled     needs_input  verification_failed
                                  |           |
                                  +-----> failed

review_ready/resolved/dismissed -> reopened -> queued
```

Every transition is append-only and has a monotonic sequence, actor, timestamp,
and active attempt identifier where required. Invalid transitions fail without
changing state. Terminal operations are idempotent for the same request ID.

The overlay renders each finding independently. A batch has stable order and a
typed per-item result, but is not a filesystem transaction. A workspace-local
mutation slot serializes `claimed`, `repairing`, and `verifying` attempts across
sessions and processes. Overlapping node/region targets and shared source hints
are moved to `needs_input`; after hot reload, the coding agent re-observes and
resolves each remaining target instead of reusing stale refs.

Reviewers can also declare that two otherwise disjoint drafts are semantically
incompatible. The editor stores this as a typed relation on either finding:

```json
{
  "relations": [
    { "kind": "conflicts_with", "findingId": "finding-layout-expanded" }
  ]
}
```

The referenced finding may belong to the same batch or another queued batch.
If both findings are queued, A3S Test moves both to `needs_input`. It compares
only declared finding IDs; it never scans instruction text for antonyms,
negation, colors, layout terms, or other conflict keywords. Deleting a local
draft also removes references to that draft before submission.

## Coding-agent handoff

The MCP repair surface is:

- `test_repair_watch`
- `test_repair_claim`
- `test_repair_progress`
- `test_repair_reply`
- `test_repair_complete`
- `test_repair_fail`
- `test_repair_cancel`
- `test_repair_verify`

Start the MCP host with a host-fixed URL and browser policy:

```bash
a3s-test mcp \
  --web-url http://127.0.0.1:3000 \
  --web-allow-domain cdn.example.test
```

The coding agent starts a Web session, calls `test_repair_watch`, claims one
finding, reports `progress` before editing, reports `complete` when editing is
done, and calls `test_repair_verify` with its focused check results. Claims
default to a derived attempt ID and a five-minute lease. The returned attempt
ID must be repeated on progress, reply, complete, and fail transitions. A watch
call is bounded by both its requested timeout and the browser command deadline,
and uses a short batch window to keep findings submitted together in stable
order.

For direct CLI sessions, equivalent commands are available under
`a3s-test agent repair-*`. The `next` field emitted after claim and progress
contains the active `--attempt-id` so a coding agent can continue safely.

Use bounded scoped inspection when the normal observation is too broad:

```bash
a3s-test agent inspect \
  --session checkout \
  --component checkout-form \
  --detail forensic \
  --limit 100 \
  --json
```

MCP exposes the same operation as `test_inspect`, with mutually exclusive
page, node, component, and region scopes. Each inspection replaces the latest
observation and emits fresh `@cN` refs.

`watch` first drains already queued work, then waits with bounded timeout and
batch window. A claim uses a lease and attempt ID. If a worker disappears
before reporting that editing began, the lease can safely return to the queue.
Once editing may have occurred, A3S Test records `needs_input` instead of
silently handing the same attempt to another worker.

Submitting a repair authorizes the connected coding agent to address only the
listed findings inside its already authorized workspace. It does not authorize
commit, push, package installation, publication, deployment, destructive
reversion, or arbitrary commands supplied by the page.

## Verification

After the coding agent reports completion, A3S Test performs browser-owned
verification before `review_ready`. The caller retries the bounded verify
operation until a newer ready revision is available; A3S Test does not add an
unbounded sleep:

1. wait for the Test Kit to report a newer ready revision;
2. observe again and resolve the target or its declared replacement;
3. evaluate explicit browser-verifiable success criteria;
4. compare console and page errors against the A3S Test-owned before baseline;
5. capture and hash an A3S Test-owned after screenshot and bounded context;
6. attach the coding agent's changed-file and focused-check report.

Layout intent always requires an explicit success-criteria result. Placement
also requires an addressable target region within the current viewport;
rearrangement requires the selected node, or a stable locator replacement, to
overlap the requested destination. The continued existence of the source node
alone is not treated as a successful layout repair.

Human acceptance is the default. The overlay can accept, reject, reply to, or
reopen a finding, and the append-only ledger retains every attempt, reply,
evidence bundle, and verification result. A session started with
`--auto-resolve-repairs` may have A3S Test append `resolved`, but only after a
passing `review_ready` event has first been persisted and projected. Failed
verification never auto-resolves.

`test_repair_verify` requires a newer ready context revision, re-inspects the
target through its semantic locators, compares current console and page-error
counts with the owned baseline, and records relative changed-file paths plus
focused check results. It stores a typed verification result in the append-only
ledger. When a stable locator and explicit text criterion exist, A3S Test also
persists a syntax-validated ACL regression candidate and executes its admitted
single Web scenario in a fresh browser session using the owning network policy.
Only a passing proof can reach `review_ready`; the candidate is not committed
to the application repository automatically.

### Real-browser lifecycle matrix

The ignored `a3s-test-cli` integration suite in
`crates/a3s-test-cli/tests/repair_e2e.rs` exercises the repair protocol against
the bundled Test Kit fixture and an admitted standalone Chromium runtime. Its
independent scenarios prove:

| Scenario | Direct evidence |
| --- | --- |
| Single repair | page submission, owned before evidence, claim, progress, completion, verification, and human acceptance |
| Layout overlay batch | pointer placement plus keyboard source selection, typed `placement` and `rearrange` ingestion through `repair-watch`, stable batch order, owned before evidence, and no overlay mutation of the source element |
| Ordered batch | stable finding order, an isolated first-item failure, continued second-item processing, and typed per-item results |
| Clarification | agent question, page-local human reply, authoritative ingestion, and return to the queue |
| Cancellation | cancellation from both queued and claimed states |
| Agent disconnect | a pre-edit claim returns to the queue, while possible editing is quarantined in `needs_input` |
| Hot reload | an observation-bound `@cN` ref is rejected after the page revision changes, then a fresh inspection succeeds |
| Verification failure | explicit failed criteria and project checks produce `verification_failed`, never `resolved`, and permit human retry |
| Restart recovery | independent CLI processes replay the append-only ledger without duplicating events |
| ACL promotion | a generated candidate is persisted and passes in a fresh same-origin browser before `review_ready` |

Run the matrix from the crate workspace with the admitted browser executable
and its Chromium path configured:

```bash
cargo test -p a3s-test-cli --test repair_e2e --locked -- \
  --ignored --test-threads=1
```

The real Test Kit browser suite runs separately. It proves page-local draft
restoration and semantic rebinding, spatial marker editing, keyboard-only
review controls and multi-selection, explicit host-interaction blocking,
searchable component selection, pointer-authored Layout placement, and the
accessibility-tree and focus contracts:

```bash
A3S_TEST_AGENT_BROWSER="$(command -v agent-browser)" \
  cargo test -p a3s-test-cli --test web_e2e \
  real_agent_browser_runs_the_embedded_testkit_suite --locked -- \
  --ignored --exact --nocapture
```

## CI and compatibility

CI should enable `A3STestKit` but omit `A3SReviewOverlay`. Pages without the
SDK continue to use the legacy accessibility observation and action protocol.
Executable-only capability discovery reports the Page Context field as
unknown; bridge presence and protocol are discovered independently from the
loaded page. Unsupported or malformed bridges fail closed for scoped context
operations without exposing arbitrary browser evaluation to the agent.

Treat `a3s.test.page-context/1`, `a3s.test.quality-report/1`,
`a3s.test.design-audit-report/1`, and `a3s.test.repair/1` as versioned
contracts. Additive SDK releases may add
optional fields or capabilities but must retain the hard payload bounds,
private node-ID handling, redaction behavior, latest-observation ref expiry,
and the separation between deterministic quality candidates, advisory design
suggestions, and repair authorization.

## Storage and recovery

The overlay persists at most 100 drafts per page identity and full SPA route in
browser local storage for seven days, capped at 512 KiB per route. Stored node
IDs are discarded. A reload resolves each target again from bounded semantic
role, label, test ID, placeholder, or exact-text locators. A draft is omitted
when any required target is missing, duplicated, or lacks a semantic locator;
corrupt, oversized, expired, future-dated, or structurally invalid records are
removed. Route transitions immediately switch the visible draft set, while a
return transition restores that route's valid drafts. Layout placements with
no existing target node retain their typed viewport region.

Local storage contains only unsent reviewer drafts. It never becomes a repair
ledger, and applications must not put secrets or production data in review
instructions. Submitted repair state is authoritative only in the owning A3S
Test agent-session directory:

```text
.a3s-test/agent-sessions/<session>/
├── session.json
├── events.jsonl
├── report.json
├── repairs.jsonl
└── artifacts/repairs/<finding>/<attempt>/

.a3s-test/
├── repair-workspace.lock
└── repair-workspace.json
```

The OS file lock protects short atomic state updates rather than the duration
of an edit. `repair-workspace.json` persists the active session, finding,
attempt, phase, and lease so separate A3S Test processes obey the same mutation
slot. Browser evidence capture and fresh-browser ACL proof run without holding
the OS lock; verification reacquires it, reloads the ledger, and confirms the
same `verifying` attempt before committing its result. No separate hidden test
session, database, or model loop owns repair state.

## License boundary

Optional perception providers remain separate typed integrations. Model
weights, serving code, and model-specific licenses are not redistributed with
the MIT-licensed Test Kit. Applications must admit each provider and its input
data under their own deployment and license policy.
