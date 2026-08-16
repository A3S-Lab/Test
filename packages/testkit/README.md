# A3S Test Kit

Development-only page context and human review SDK for A3S Test.

```bash
npm install https://github.com/A3S-Lab/Test/releases/latest/download/a3s-testkit.tgz
```

```tsx
import { A3SReviewOverlay, A3STestBoundary, A3STestKit } from "@a3s-lab/testkit/react";

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

The framework-neutral entry point exports `installTestKit`,
`getPageContextBridge`, and all protocol types. The React entry point exports
the provider, component boundary, and optional Shadow DOM review overlay.
`installTestKit` also requires `enabled: true`; omitted or false-like runtime
configuration fails closed.

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
stacking relationships; clusters repeated structures with deterministic
tag/role/subtree/style fingerprints; records real interaction-state
differences; and detects responsive conditions, transitions, CSS and Web
Animations, sticky nodes, canvas, and media. It never guesses a component from
a class name or synthesizes an interaction for collection.

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
Web driver rejects protocol drift, stale bindings, invalid geometry, and
budget violations. It remains untrusted evidence with no action, verdict, or
repair authority.

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
bounded structured copy, and typed Layout Mode. Layout Mode can draw the
viewport region for a new component or select an existing section and describe
its destination. Its searchable catalog contains 90 component types across ten
categories, with complete English and Simplified Chinese labels and search.
Known catalog selections follow a live locale change, while the component
field preserves project-specific free-form values. It emits `placement` or
`rearrange` intent for A3S Test and does not move or style application DOM
itself. Submitted findings support human/agent
replies, accept/reject/reopen review actions, and per-finding lifecycle
projection.

The compact review dock keeps marking controls available without covering the
application. Target-attached editors preserve page context, while saved drafts
move into a separate workspace with pinned single and batch send actions. The
secondary tool tray and findings workspace are mutually exclusive, preventing
stacked floating surfaces. Short viewports keep preferences internally
scrollable, and mobile controls use touch-sized targets and 16-pixel form text.

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

The overlay exposes a named non-modal dialog, finding-specific action names,
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
