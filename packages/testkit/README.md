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
      <A3SReviewOverlay enabled={import.meta.env.DEV} />
    </A3STestKit>
  );
}
```

The framework-neutral entry point exports `installTestKit`,
`getPageContextBridge`, and all protocol types. The React entry point exports
the provider, component boundary, and optional Shadow DOM review overlay.
`installTestKit` also requires `enabled: true`; omitted or false-like runtime
configuration fails closed.

The runtime derives context after browser rendering. It reads semantic DOM,
open Shadow DOM, form state, layout, and viewport facts without adding test
attributes to application elements. Observer and navigation signals advance a
versioned snapshot; an unchanged page is not polled.

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
`process.env.NODE_ENV !== "production"`. The overlay supports element, text,
click/drag multi-selection, rectangular and freehand findings, persistent
markers, draft editing/hiding, animation pause, system/light/dark themes,
bounded structured copy, and typed Layout Mode. Layout Mode can draw the
viewport region for a new component or select an existing section and describe
its destination. Its searchable catalog contains 90 component types across ten
categories, while the component field remains free-form for project-specific
types. It emits `placement` or `rearrange` intent for A3S Test and does not move
or style application DOM itself. Submitted findings support human/agent
replies, accept/reject/reopen review actions, and per-finding lifecycle
projection.

The review preferences section persists only bounded presentation choices:
theme, marker color, clear-after-copy, explicit host pointer blocking, panel
dock, and wireframe page fade. Auto-send and animation pause reset on mount.
`Hide until tab restart` is tab-scoped and does not disable the headless page
context bridge. Clipboard failure never clears local drafts.

The overlay exposes a named non-modal dialog, finding-specific action names,
one polite status announcer, visible keyboard focus, and focus restoration when
controls disappear. Unit and real-Chromium accessibility-tree checks protect
these semantics. A separate hands-on screen-reader audit of every workflow is
still required before claiming complete assistive-technology coverage.
