import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { hydrateRoot } from "react-dom/client";
import axe from "axe-core";
import { A3SReviewOverlay, A3STestBoundary, A3STestKit } from "./react";
import { getPageContextBridge } from "./runtime";
import type { DesignAuditReport, QualityReport, RepairStatus } from "./types";

type AuditRepairState = Extract<
  RepairStatus,
  | "needs_input"
  | "review_ready"
  | "resolved"
  | "dismissed"
  | "cancelled"
  | "failed"
>;

declare global {
  interface Window {
    testkitInitialRepaired?: boolean;
    testkitCopiedText?: string;
    testkitHostClicks?: number;
    testkitFixture?: {
      auditAccessibility(): Promise<
        Array<{
          id: string;
          impact: string | null;
          help: string;
          nodes: Array<{ target: string[]; summary: string }>;
        }>
      >;
      seedReviewCandidates(): Promise<boolean>;
      setRepairState(status: AuditRepairState): string;
      reset(): void;
      route(): void;
      repair(): void;
      virtualize(): void;
      teardown(): void;
    };
  }
}

let setVirtualRow: ((value: string) => void) | undefined;
let setRepairedState: ((value: boolean) => void) | undefined;
let fixtureEventSequence = Date.now();

const AUDIT_REPAIR_TRANSITIONS: Record<RepairStatus, readonly RepairStatus[]> =
  {
    draft: ["queued", "cancelled"],
    queued: ["claimed", "cancelled", "failed"],
    claimed: ["queued", "repairing", "cancelled", "needs_input", "failed"],
    repairing: ["verifying", "needs_input", "failed"],
    verifying: ["review_ready", "verification_failed", "needs_input", "failed"],
    needs_input: ["queued", "cancelled", "failed"],
    verification_failed: ["queued", "cancelled", "failed"],
    review_ready: ["resolved", "reopened", "dismissed"],
    resolved: ["reopened"],
    dismissed: ["reopened"],
    cancelled: ["reopened"],
    failed: ["reopened"],
    reopened: ["queued", "cancelled"],
  };

function auditRepairPath(
  start: RepairStatus,
  target: RepairStatus,
): RepairStatus[] | null {
  const queue: Array<{ status: RepairStatus; path: RepairStatus[] }> = [
    { status: start, path: [] },
  ];
  const visited = new Set<RepairStatus>([start]);
  while (queue.length > 0) {
    const current = queue.shift()!;
    if (current.status === target) return current.path;
    for (const next of AUDIT_REPAIR_TRANSITIONS[current.status]) {
      if (visited.has(next)) continue;
      visited.add(next);
      queue.push({ status: next, path: [...current.path, next] });
    }
  }
  return null;
}

function applyAuditRepairState(target: AuditRepairState): string {
  const bridge = getPageContextBridge();
  const repair = bridge?.listRepairs()[0];
  if (!bridge || !repair)
    return "Submit a finding before applying a repair state.";
  const path = auditRepairPath(repair.status, target);
  if (!path)
    return `No valid fixture path exists from ${repair.status} to ${target}.`;
  for (const status of path) {
    const sequence = ++fixtureEventSequence;
    const updated = bridge.applyRepairEvent({
      requestId: `screen-reader-fixture-${sequence}`,
      findingId: repair.id,
      sequence,
      status,
      actor:
        status === "review_ready" ||
        ["resolved", "dismissed", "cancelled", "failed"].includes(status)
          ? "a3s-test"
          : "agent",
      timestamp: new Date().toISOString(),
      ...(status === "needs_input"
        ? { message: "Should the visible label remain unchanged?" }
        : {}),
    });
    if (!updated)
      return `The fixture rejected the transition to ${status}. Reset and try again.`;
  }
  return `Repair state is now ${bridge.listRepairs()[0]?.status ?? target}.`;
}

function resetAuditFixture(): void {
  for (const storage of [window.localStorage, window.sessionStorage]) {
    const keys = Array.from({ length: storage.length }, (_, index) =>
      storage.key(index),
    ).filter((key): key is string => Boolean(key));
    for (const key of keys) {
      if (key.startsWith("a3s-test.") || key.startsWith("a3s-testkit-"))
        storage.removeItem(key);
    }
  }
  window.location.replace("/testkit.html");
}

async function seedReviewCandidates(): Promise<boolean> {
  await new Promise<void>((resolve) => {
    requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
  });
  const bridge = getPageContextBridge();
  if (!bridge) return false;
  const snapshot = bridge.snapshot();
  const nodeId = snapshot.nodes.find(
    (node) => node.testId === "repair-target",
  )?.id;
  if (!nodeId) return false;
  const quality: QualityReport = {
    contract: "screen-reader-audit",
    variant: "desktop",
    state: "ready",
    outcome: "failed",
    observation_revision: snapshot.revision,
    matches: [],
    findings: [
      {
        id: "a11y:contract-role",
        dimension: "design_conformance",
        rule_id: "contract.element.role",
        severity: "important",
        message: "Use the contracted role",
        expected: "button",
        actual: "link",
        element_id: "repair-target",
        observed_node_id: nodeId,
        confidence: 100,
      },
    ],
  };
  const designAudit: DesignAuditReport = {
    protocol: "a3s.test.design-audit-report/1",
    provenance: {
      identity: { provider: "fixture", model: "screen-reader-audit" },
      observation_id: 1,
      surface_revision: snapshot.revision,
      screenshot_sha256: `sha256:${"a".repeat(64)}`,
      page_context_sha256: `sha256:${"b".repeat(64)}`,
      width: window.innerWidth,
      height: window.innerHeight,
      usage: { input_units: 1, output_units: 1, cost_microusd: 0 },
      request_id: "screen-reader-audit-1",
      authority: "advisory",
    },
    dimensions: ["visual_hierarchy"],
    findings: [
      {
        id: "a11y:design-emphasis",
        dimension: "visual_hierarchy",
        priority: "medium",
        summary: "The primary action lacks emphasis",
        rationale: "Nearby controls have equal visual weight",
        recommendation:
          "Increase the primary action contrast and surrounding space",
        confidence: 90,
        target: { kind: "node", node_id: nodeId },
      },
    ],
  };
  return bridge.reportQuality(quality) && bridge.reportDesignAudit(designAudit);
}

function Fixture() {
  const [row, setRow] = useState("Virtual row 1");
  const [repaired, setRepaired] = useState(
    () => window.testkitInitialRepaired === true,
  );
  const [auditRepairState, setAuditRepairState] =
    useState<AuditRepairState>("needs_input");
  const [auditMessage, setAuditMessage] = useState("");
  useEffect(() => {
    setVirtualRow = setRow;
    setRepairedState = setRepaired;
    document.documentElement.dataset.hydrated = "true";
    const host = document.querySelector<HTMLElement>("#shadow-host");
    if (host && !host.shadowRoot) {
      const shadow = host.attachShadow({ mode: "open" });
      shadow.innerHTML = '<button id="shadow-action">Shadow action</button>';
    }
    return () => {
      setVirtualRow = undefined;
      setRepairedState = undefined;
      document.documentElement.dataset.hydrated = "false";
    };
  }, []);
  const portal = document.querySelector<HTMLElement>("#portal");
  window.testkitHostClicks ??= 0;
  return (
    <A3STestKit
      enabled
      page={{ id: "browser-fixture" }}
      ready={() => document.documentElement.dataset.hydrated === "true"}
      facts={() => ({ fixtureState: "ready" })}
      repairStorage="session"
    >
      <A3STestBoundary
        id="app-shell"
        name="App shell"
        source={{ file: "src/Fixture.tsx", line: 12 }}
        roots={() => (portal ? [portal] : [])}
        as="main"
      >
        <h1>Embedded TestKit E2E</h1>
        <div id="motion-probe" aria-hidden="true">
          <span id="running-motion" />
          <span id="paused-motion" />
          <span id="scroll-motion" />
          <span id="view-motion" />
        </div>
        <div id="box-model-probe">Box model probe</div>
        <section id="audit-controls" aria-labelledby="audit-controls-title">
          <h2 id="audit-controls-title">Screen-reader audit controls</h2>
          <p>
            These controls expose test-only candidate and repair states without
            DevTools.
          </p>
          <div className="audit-actions">
            <button
              type="button"
              onClick={() => {
                setAuditMessage(
                  "Candidate seeding requested. Both candidates will appear in Review.",
                );
                void seedReviewCandidates().then((seeded) => {
                  if (!seeded) setAuditMessage("Candidate seeding failed.");
                });
              }}
            >
              Seed contract and design candidates
            </button>
            <label htmlFor="audit-repair-state">Repair state</label>
            <select
              id="audit-repair-state"
              value={auditRepairState}
              onChange={(event) =>
                setAuditRepairState(
                  event.currentTarget.value as AuditRepairState,
                )
              }
            >
              <option value="needs_input">Clarification needed</option>
              <option value="review_ready">Human review ready</option>
              <option value="resolved">Resolved</option>
              <option value="dismissed">Dismissed</option>
              <option value="cancelled">Cancelled</option>
              <option value="failed">Failed</option>
            </select>
            <button
              type="button"
              onClick={() =>
                setAuditMessage(applyAuditRepairState(auditRepairState))
              }
            >
              Apply repair state
            </button>
            <button type="button" onClick={resetAuditFixture}>
              Reset fixture
            </button>
            <a href="/screen-reader-workflows.json">Audit workflow manifest</a>
          </div>
          <p
            id="audit-status"
            role="status"
            aria-live="polite"
            aria-atomic="true"
          >
            {auditMessage}
          </p>
        </section>
        <button id="sticky" data-testid="repair-target">
          {repaired ? "Repaired action" : "Broken action"}
        </button>
        <button
          id="host-probe"
          onClick={() => {
            window.testkitHostClicks = (window.testkitHostClicks ?? 0) + 1;
          }}
        >
          Host interaction probe
        </button>
        <button id="zoom-edge" data-testid="zoom-edge">
          Zoom edge target
        </button>
        <section id="layout-section" data-testid="layout-section" tabIndex={-1}>
          Layout source section
        </section>
        <div id="unboxed-layout">
          <button id="unboxed-action" data-testid="unboxed-action">
            Unboxed action
          </button>
        </div>
        <div id="nested">
          <div className="virtual-space">
            <button id="virtual-row">{row}</button>
          </div>
        </div>
        <div id="shadow-host" />
      </A3STestBoundary>
      {portal &&
        createPortal(
          <dialog open>
            <button id="dialog-action">Confirm dialog</button>
          </dialog>,
          portal,
        )}
      <A3SReviewOverlay
        enabled
        defaultOpen
        copyToClipboard={(text) => {
          window.testkitCopiedText = text;
        }}
      />
    </A3STestKit>
  );
}

const root = hydrateRoot(document.querySelector("#root")!, <Fixture />);
window.testkitFixture = {
  async auditAccessibility() {
    const results = await axe.run(document, {
      runOnly: {
        type: "tag",
        values: [
          "wcag2a",
          "wcag2aa",
          "wcag21a",
          "wcag21aa",
          "wcag22a",
          "wcag22aa",
        ],
      },
    });
    return results.violations.map((violation) => ({
      id: violation.id,
      impact: violation.impact ?? null,
      help: violation.help,
      nodes: violation.nodes.map((node) => ({
        target: node.target.map(String),
        summary: node.failureSummary ?? "",
      })),
    }));
  },
  seedReviewCandidates,
  setRepairState: applyAuditRepairState,
  reset: resetAuditFixture,
  route() {
    history.pushState(null, "", "/routed?view=2");
  },
  repair() {
    setRepairedState?.(true);
  },
  virtualize() {
    setVirtualRow?.("Virtual row 50");
  },
  teardown() {
    root.unmount();
    getPageContextBridge()?.dispose();
    delete window.testkitFixture;
    delete window.testkitCopiedText;
    delete window.testkitHostClicks;
  },
};
