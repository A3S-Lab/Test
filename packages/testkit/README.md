# A3S Test Kit

Development-only page context and human review SDK for A3S Test.

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

The SDK never receives workspace, shell, MCP, or source-editing credentials.
Submitted DOM context is explicitly marked as untrusted evidence. A same-origin
`repairEndpoint` is optional; without one A3S Test can pick queued repairs up
through its fixed browser bridge integration.

A3S Test captures its own before/after context, screenshot, and browser-error
evidence, serializes workspace mutation across sessions and processes, and
proves an admitted regression candidate in a fresh browser before presenting a
repair for review. Human acceptance is the default; automatic resolution must
be enabled for the owning A3S Test session explicitly.

See [`../../docs/testkit.md`](../../docs/testkit.md) for the complete protocol,
security, repair, and verification design.

Both the provider and overlay require an explicit `enabled` value. The overlay
also requires a compatible live bridge and therefore fails closed if the
provider is disabled or the protocol is unavailable.

For CI, keep `A3STestKit` enabled and omit `A3SReviewOverlay`. For Next.js,
mount both from a client component and gate them with
`process.env.NODE_ENV !== "production"`. The overlay supports element, text,
click/drag multi-selection, rectangular and freehand findings, persistent
markers, draft editing/hiding, animation pause, system/light/dark themes, and
bounded structured copy. Submitted findings support human/agent replies,
accept/reject/reopen review actions, and per-finding lifecycle projection.
