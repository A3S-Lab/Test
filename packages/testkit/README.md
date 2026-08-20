# A3S Test Kit

Development-only page context, rendered-node source mapping, and human review
SDK for A3S Test.

The browser already knows what it rendered. Test Kit turns that result into a
bounded, revisioned record that a coding agent can inspect without scraping
framework internals or guessing from a screenshot. Its optional right-side
Review Overlay reduces the normal feedback path to two decisions: choose the
page content that needs attention, then describe the requested result. A sketch
or browser-page crop remains one optional addition before explicit submission.

| Layer           | Responsibility                                                                                                 |
| --------------- | -------------------------------------------------------------------------------------------------------------- |
| Context Runtime | Publish semantics, exact revision deltas, components, locators, geometry, layout, state, motion, and controlled facts after rendering |
| Source mapping  | Rank explicit component, DOM-owner, and Source Map v3 spans for a selected rendered node                       |
| Review Overlay  | Keep a two-step element/area flow visible and disclose text, multi-select, drawing, Layout, board, and capture only when needed |
| Repair handoff  | Bind a submitted finding to the current page revision without granting workspace or source-edit authority      |

Test Kit is not a browser driver, test runner, coding agent, or source editor.
The A3S Test CLI continues to own typed actions, evidence, repair state,
verification, and cleanup.

## Install

```bash
npm install --save-dev @a3s-lab/testkit@0.6.1
```

`@a3s-lab/testkit` 0.6.1 is published on the official npm Registry with GitHub
OIDC provenance. The pinned command keeps installation reproducible and locks
the package integrity in the project lockfile.

Verify that the current project can resolve the package:

```bash
npm ls @a3s-lab/testkit
```

## React quick start

```tsx
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { A3SReviewOverlay, A3STestKit } from "@a3s-lab/testkit/react";
import { App } from "./App";

const testKitEnabled = import.meta.env.DEV;

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <A3STestKit enabled={testKitEnabled} page={{ id: "app" }}>
      <App />
      <A3SReviewOverlay enabled={testKitEnabled} locale="auto" />
    </A3STestKit>
  </StrictMode>,
);
```

Start the existing development server. An **A3S Review** launcher in the
lower-right corner confirms that the visible overlay is ready. The same panel
opens with `Ctrl/Command+Shift+F`. Test Kit is a frontend SDK and does not add
an `a3s-testkit` terminal command.

`A3STestBoundary` is optional. Add it only when page context needs component
ownership or a bounded source hint:

```tsx
import { A3STestBoundary } from "@a3s-lab/testkit/react";

<A3STestBoundary
  id="checkout-form"
  name="Checkout form"
  source={{ file: "src/Checkout.tsx" }}
>
  <Checkout />
</A3STestBoundary>;
```

For headless CI context, keep `A3STestKit` enabled and omit
`A3SReviewOverlay`. The framework-neutral entry point exports
`installTestKit`, `getPageContextBridge`, `registerSource`,
`registerSourceMap`, and all protocol types.
`installTestKit` also requires `enabled: true`; omitted or false-like runtime
configuration fails closed.

Test Kit 0.6.0 exposes `a3s.test.testkit-handshake/1`. After the provider and
overlay mount, `a3s-test dev --json` verifies the live package identity, SDK
range, Page Context protocol, required capabilities, and Review Overlay before
it reports a ready review session. The older `probe()` operation remains for
Page Context feature discovery; it is not the CLI compatibility decision.

Once admitted, the same `a3s-test dev --json` process exposes
`a3s.test.local-repair-bridge/1`. A finding explicitly sent from the Review
Overlay is persisted with A3S Test-owned before evidence and emitted once as a
`repair_batch` JSONL event containing the generated session ID. The coding
agent therefore does not need a separate manually coordinated
`repair-watch --session ...` process. The bridge reuses the existing repair
ledger and projects later agent or human state back into this page.

Each selected node can also carry a ranked `a3s.test.source-mapping/1` record.
An enclosing `A3STestBoundary` contributes a coarse declared hint. Framework
adapters can use `registerSource` for an exact DOM owner and
`registerSourceMap` to trace a declared generated location through an encoded
Source Map v3. The runtime never traverses React Fiber, Vue component
instances, or other undeclared framework state, and it discards
`sourcesContent` before the map enters runtime state. Submitted repair context
keeps the same ranked spans, confidence, origin, and exact/ancestor relation so
the coding agent can open the likely owning file without another browser turn.

The review UI supports `locale="auto" | "en" | "zh-CN"`. `auto` is the
default and observes `<html lang>` while mounted, resolving every `zh-*` page
language to Simplified Chinese and other page languages to English. Set the
locale explicitly when the overlay should not follow the host page:

```tsx
<A3SReviewOverlay
  enabled={import.meta.env.DEV}
  locale="zh-CN"
  messages={{ reviewTitle: "页面评审" }}
/>
```

`messages` accepts only known review-message keys. Empty values and strings
longer than 2,048 characters are ignored, so host copy customization remains
bounded presentation data.

The runtime derives context after browser rendering. It reads semantic DOM,
open Shadow DOM, form state, layout, and viewport facts without adding test
attributes to application elements. Observer and navigation signals advance a
versioned snapshot; an unchanged page is not polled.

Each snapshot also includes bounded `a3s.test.ui-understanding/1` evidence by
default. It profiles observed colors, typography, spacing, radii, shadows, and
safe root design properties; publishes Flex/Grid/flow, scroll-container, and
stacking relationships together with exact client/scroll extents, signed
offsets, derived overflow/clipping state, resolved physical margin, border,
and padding edges, box sizing, writing mode, and text direction; clusters
repeated structures with deterministic tag/role/subtree/style fingerprints;
records real
interaction-state differences; and detects responsive conditions, transitions,
CSS and Web Animations, document/scroll/view timelines, animation ranges,
sticky nodes, canvas, and media. It never guesses a component from a class
name or synthesizes an interaction for collection.

```tsx
<A3STestKit
  enabled={import.meta.env.DEV}
  page={{ id: "checkout" }}
  maxUiNodes={200}
  maxUiStateSamples={200}
  maxUiDurationMs={32}
  maxUiEncodedBytes={262_144}
>
  <App />
</A3STestKit>
```

Call `snapshot({ ui: false })` to omit it once, or set
`uiUnderstanding={false}` for the installation. Callers may only lower the
installed node, state, string, byte, and time ceilings. UI evidence binds its
own observation ID to the containing page revision, viewport, and scope; the
Web driver rejects protocol drift, stale bindings, invalid geometry,
duplicate graph relationships, missing parents or edge endpoints, incomplete
or cyclic containment, invalid component membership, and budget violations. It
remains untrusted evidence with no action, verdict, or repair authority.

## Revision-scoped diffs

Test Kit 0.6.0 exposes `waitForDiff` and
`a3s.test.page-context-diff/1`. Capture a normal baseline once, then wait for
only the evidence invalidated by a newer page revision:

```ts
const baseline = bridge.snapshot({ detail: "summary", ui: false });
const diff = await bridge.waitForDiff({
  sinceRevision: baseline.revision,
  timeoutMs: 5_000,
  ui: false,
});
```

A `complete` delta carries changed nodes and components, removed node IDs, and
independent page, facts, and UI invalidation. `reset_required` means the exact
baseline is outside bounded history or complete invalidation metadata cannot
fit the encoded-byte ceiling; discard old evidence and capture a fresh normal
snapshot. The runtime never turns reset into an empty diff or drops partial
IDs silently.

History, payloads, and waits remain bounded. A timeout must be an integer from
0 through 300,000. Continuation cursors bind the full normalized request and
revision. A stale cursor, mismatched baseline, future revision, NaN, infinity,
negative timeout, or oversized wait is rejected. A3S Test validates delta
ordering, identifiers, changed/removed coverage, and reset semantics again in
Rust. Persistent sessions keep only one-way node fingerprints while retaining
an unaffected `@cN` locator; raw Test Kit node IDs are not written to session
metadata.

## Deterministic quality findings

When a deterministic Surface Contract runs, the Web driver can project its
bounded report through `reportQuality`. The Test Kit keeps these reports in a
separate in-memory Quality Store, never in the Repair Ledger. The overlay shows
each finding with an explicit `blocking`, `important`, or `suggestion` label
and asks the reviewer to confirm or choose a current target.

Viewing a finding, opening its editor, or cancelling target selection grants
no repair authority. A finding leaves the Quality Store only when it is
dismissed, saved as a local review draft, successfully submitted to the owning
A3S Test session, or replaced by a newer report for the same
contract/variant/state. A passed report clears earlier candidates in that
scope. Other findings in the report remain independently reviewable.

Reports are bounded to 500 findings, 5,000 matches, 1 MiB encoded JSON, and a
maximum JSON depth of 32. `maxQualityReports` defaults to five and is clamped
from one through twenty. Projection is one-way and advisory: a missing bridge,
an incompatible page, or a rejected report never changes the Runner's report
or verdict.

## Advisory design suggestions

`reportDesignAudit` accepts only `a3s.test.design-audit-report/1` records whose
provenance is advisory and matches the current page revision. Provider/model
identity, observation, screenshot and page-context digests, dimensions, usage,
findings, and targets are structurally bounded. A node target must still
resolve in the live page. These reports live in a separate Design Audit store,
never in the deterministic Quality Store or Repair Ledger. A later page
revision expires them immediately.

The overlay shows the provider's summary, rationale, typed dimension,
priority, confidence, and current target. Reviewing or cancelling a suggestion
does nothing to the product and grants no repair authority. The reviewer can
edit the recommendation or choose a new target, then explicitly save a local
draft or send it through the existing single/batch repair flow. High provider
priority maps only to `important`; design advice can never create a blocking
repair or test verdict by itself.

Reports are bounded to 500 findings and 1 MiB encoded JSON.
`maxDesignAuditReports` defaults to five and is clamped from one through
twenty. `listDesignAuditReports`, `dismissDesignAuditFinding`, and
`dismissDesignAuditReport` manage only these local review candidates.

Geometry remains in CSS pixels at every browser zoom level. The page snapshot
separates the layout viewport and DPR from an optional visual viewport with its
current offset, visible size, and scale, allowing A3S Test to reason about
zoomed and panned content without double-scaling coordinates.

The SDK never receives workspace, shell, MCP, or source-editing credentials.
Submitted DOM context is explicitly marked as untrusted evidence. A same-origin
`repairEndpoint` is optional; without one A3S Test can pick queued repairs up
through its fixed browser bridge integration.

A3S Test captures its own before/after context, screenshot, and browser-error
evidence, serializes workspace mutation across sessions and processes, and
proves an admitted regression candidate in a fresh browser before presenting a
repair for review. Human acceptance is the default; automatic resolution must
be enabled for the owning A3S Test session explicitly.

See the
[Test Kit design and security contract](https://github.com/A3S-Lab/Test/blob/main/docs/testkit.md)
for the complete protocol, security, repair, and verification design.

For requests that cannot both be satisfied, add one draft first and use the
optional **Conflicts with another draft** control on the other. This emits a
typed `conflicts_with` relation; A3S Test does not infer conflicts from repair
instruction wording.

Both the provider and overlay require an explicit `enabled` value. The overlay
also requires a compatible live bridge and therefore fails closed if the
provider is disabled or the protocol is unavailable.

For CI, keep `A3STestKit` enabled and omit `A3SReviewOverlay`. For Next.js,
mount both from a client component and gate them with
`process.env.NODE_ENV !== "production"`. Server rendering does not access the
DOM or emit layout-effect warnings; boundary registration and focus effects
begin after the browser hydrates. `getPageContextBridge()` returns `null` on
the server, while direct enabled installation reports that it requires a
browser. The overlay supports element, text,
click/drag multi-selection, rectangular and freehand findings, persistent
markers, draft editing/hiding, animation pause, system/light/dark themes,
bounded structured copy, finding-level design references, and typed Layout
Mode. After selecting an element or region, a reviewer can open the design
board. Its built-in SVG editor uses a constrained 960 × 600
surface with freehand, rectangle, text, selection, movement, resize, styling,
keyboard, and history tools, and admits at most 250 objects. It can also
open a viewport selection layer so the reviewer can drag over one visible page
area and release to add that crop to the board. The capture excludes the Test
Kit overlay and never requests screen-sharing permission. `Escape` cancels the
selection; upload, paste, and drop remain available for PNG/JPEG references.

Node and text markers resolve their current DOM elements again after page or
nested-container scrolling and viewport changes. Stored viewport regions remain
document-aligned through their captured scroll offset and act only as a fallback
for node targets that no longer resolve.

Keep the review overlay development-only in normal integrations. The board
runs entirely inside the Test Kit Shadow DOM. It does not load a drawing SDK,
remote fonts, icons, translations, or other canvas assets, and it has no
license-key or watermark requirement. The overlay should still remain
development-only because it exposes review authoring controls.

The visual contract comes from `@a3s-lab/ui` without creating a runtime UI
dependency. `npm run sync:a3s-ui` reads the pinned foundation, `task-pane`,
`toolbar`, and `status-badge` CSS exports, scopes root and dark-theme selectors
to `.a3s-root`, and generates TypeScript string constants that ship inside the
Shadow DOM bundle. Host CSS therefore cannot leak in, and consumers do not
load a second stylesheet or UI package at runtime.

The board exports a bounded `designReference` with the finding. Imported files
may be at most 8 MiB; inline PNG/JPEG data URLs are limited to 384 KiB, and the
general contract admits at most 1,600 × 1,200 pixels and 1,920,000 pixels total.
The Web driver validates the encoded header and declared dimensions, writes
`repairs/<finding-id>/design-reference.png|jpg` under its artifact root, and
replaces inline bytes with viewable evidence metadata and a SHA-256 digest
before the repair reaches a coding agent.

Layout Mode can draw the viewport region for a new component or select an
existing section and describe its destination. Its searchable catalog contains
90 component types across ten categories, with complete English and Simplified
Chinese labels and search. Known catalog selections follow a live locale
change, while the component field preserves project-specific free-form values.
It emits `placement` or `rearrange` intent for A3S Test and does not move or
style application DOM itself.
Submitted findings support human/agent replies, accept/reject/reopen review
actions, and per-finding lifecycle projection.

The review flow stays inside one fixed side panel. The first screen exposes
only **Element** and **Area**; **Text**, **Multi**, **Draw**, and **Layout** are
progressively disclosed under **More tools**. Selecting a target replaces the
tools with the requested-fix field in the same panel. **New feedback** and
**Findings** remain the two top-level views, while preferences move to the
header control. No target-attached editor, secondary floating tray, or nested
modal is opened. Saving or sending is the final action, not another navigation
step. The design board temporarily replaces the panel and returns to the same
editor when closed.
Short viewports keep panel content internally scrollable. On mobile, starting
a marking mode uses the same compact finish/cancel bar as desktop. At every
viewport size, the side panel temporarily yields the page so covered targets
remain visible and directly selectable. Mobile controls use touch-sized targets
and 16-pixel form text.

Visible markers and the active candidate resolve live DOM rectangles after
page or nested-container scrolling and disappear whenever the review panel is
closed. Transient hover geometry is discarded as soon as a target is selected,
so a stale fixed-position outline cannot survive a scroll. Stored region
targets retain their captured scroll origin.

Animation pause is ownership-safe: Test Kit freezes running and newly started
page motion while pause is active, then resumes only animations and media it
actually paused. Motion already paused by the host application stays paused.

For keyboard multi-selection, focus an application element and press `Enter`
to add it without activating the host control or moving focus into the review
Shadow DOM. Repeat for additional elements, then press `Shift+Enter` to open
the finding editor. `Escape` or the visible marking Cancel control discards an
incomplete selection and restores application focus. From a completed
multi-select editor, `Escape` discards the editor and restores review-panel
focus.

The review preferences section persists only bounded presentation choices:
theme, marker color, clear-after-copy, explicit host pointer blocking, panel
dock, and wireframe page fade. Auto-send and animation pause reset on mount.
`Hide until tab restart` is tab-scoped and does not disable the headless page
context bridge. Activating it returns focus without scrolling to the last
connected application control. Clipboard failure never clears local drafts.

The overlay exposes a named non-modal review region, finding-specific action names,
one polite status announcer, visible keyboard focus, and focus restoration when
controls disappear, including when tab-scoped hiding removes the Shadow DOM.
Global review commands are published through `aria-keyshortcuts` and a
keyboard-reference section in Review preferences. Letter shortcuts and the
panel toggle are ignored while focus is in an editable control. Active marking
or an open finding editor receives unmodified `Escape` first, including from
an editable target; an idle host editable retains `Escape` and leaves the panel
open. With the panel open, `E`, `M`, `T`, `A`, and `D` start element, multi,
text, area, and draw marking; `L`, `P`, and `H` toggle Layout Mode, page motion,
and marker visibility. Unit and real-Chromium accessibility-tree checks protect
these semantics.
The real-browser suite also runs `axe-core` WCAG A/AA scans across three themes
and the major review, Layout, candidate, clarification, human-review, and
terminal states, including the open Shadow DOM. A separate hands-on VoiceOver,
NVDA, or equivalent screen-reader audit of every workflow is still required
before claiming complete assistive-technology coverage.

The
[independent screen-reader audit guide](../../docs/screen-reader-audit.md)
provides the loopback fixture command, canonical 15-workflow manifest,
evidence rules, strict audit JSON contract, and separate structural and
all-passed verification commands. Verification resolves the named Git commit
and emits a location-independent v2 record with SHA-256 bindings for the audit,
committed workflow manifest, and every evidence file. The harness makes the
audit reproducible; it does not close M8 or grant repair authority.
