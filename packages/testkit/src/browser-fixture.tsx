import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { hydrateRoot } from "react-dom/client";
import { A3SReviewOverlay, A3STestBoundary, A3STestKit } from "./react";
import { getPageContextBridge } from "./runtime";

declare global {
  interface Window {
    testkitInitialRepaired?: boolean;
    testkitCopiedText?: string;
    testkitHostClicks?: number;
    testkitFixture?: {
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
