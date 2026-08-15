import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { hydrateRoot } from "react-dom/client";
import axe from "axe-core";
import { A3SReviewOverlay, A3STestBoundary, A3STestKit } from "./react";
import { getPageContextBridge } from "./runtime";
import type { DesignAuditReport, QualityReport } from "./types";

declare global {
  interface Window {
    testkitInitialRepaired?: boolean;
    testkitCopiedText?: string;
    testkitHostClicks?: number;
    testkitFixture?: {
      auditAccessibility(): Promise<Array<{
        id: string;
        impact: string | null;
        help: string;
        nodes: Array<{ target: string[]; summary: string }>;
      }>>;
      seedReviewCandidates(): Promise<boolean>;
      route(): void;
      repair(): void;
      virtualize(): void;
      teardown(): void;
    };
  }
}

let setVirtualRow: ((value: string) => void) | undefined;
let setRepairedState: ((value: boolean) => void) | undefined;

function Fixture() {
  const [row, setRow] = useState("Virtual row 1");
  const [repaired, setRepaired] = useState(() => window.testkitInitialRepaired === true);
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
  return <A3STestKit
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
      roots={() => portal ? [portal] : []}
      as="main"
    >
      <h1>Embedded TestKit E2E</h1>
      <button id="sticky" data-testid="repair-target">{repaired ? "Repaired action" : "Broken action"}</button>
      <button id="host-probe" onClick={() => { window.testkitHostClicks = (window.testkitHostClicks ?? 0) + 1; }}>Host interaction probe</button>
      <button id="zoom-edge" data-testid="zoom-edge">Zoom edge target</button>
      <section id="layout-section" data-testid="layout-section" tabIndex={-1}>Layout source section</section>
      <div id="nested"><div className="virtual-space"><button id="virtual-row">{row}</button></div></div>
      <div id="shadow-host" />
    </A3STestBoundary>
    {portal && createPortal(<dialog open><button id="dialog-action">Confirm dialog</button></dialog>, portal)}
    <A3SReviewOverlay enabled defaultOpen copyToClipboard={(text) => { window.testkitCopiedText = text; }} />
  </A3STestKit>;
}

const root = hydrateRoot(document.querySelector("#root")!, <Fixture />);
window.testkitFixture = {
  async auditAccessibility() {
    const results = await axe.run(document, {
      runOnly: {
        type: "tag",
        values: ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22a", "wcag22aa"],
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
  async seedReviewCandidates() {
    await new Promise<void>((resolve) => {
      requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
    });
    const bridge = getPageContextBridge();
    if (!bridge) return false;
    const snapshot = bridge.snapshot();
    const nodeId = snapshot.nodes.find((node) => node.testId === "repair-target")?.id;
    if (!nodeId) return false;
    const quality: QualityReport = {
      contract: "screen-reader-audit",
      variant: "desktop",
      state: "ready",
      outcome: "failed",
      observation_revision: snapshot.revision,
      matches: [],
      findings: [{
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
      }],
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
      findings: [{
        id: "a11y:design-emphasis",
        dimension: "visual_hierarchy",
        priority: "medium",
        summary: "The primary action lacks emphasis",
        rationale: "Nearby controls have equal visual weight",
        recommendation: "Increase the primary action contrast and surrounding space",
        confidence: 90,
        target: { kind: "node", node_id: nodeId },
      }],
    };
    return bridge.reportQuality(quality) && bridge.reportDesignAudit(designAudit);
  },
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
