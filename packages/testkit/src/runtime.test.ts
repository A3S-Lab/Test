import { describe, expect, it } from "vitest";
import { installTestKit, registerBoundary } from "./runtime";
import type { RepairDraft, RepairEvent } from "./types";
import { setRect } from "./test-setup";

describe("page context runtime", () => {
  it("captures semantic context, component ownership, source hints, and geometry", () => {
    document.body.innerHTML = `<main><section id="checkout"><label for="email">Email</label><input id="email" placeholder="you@example.test"><button data-testid="pay">Pay now</button></section></main>`;
    const boundary = document.querySelector("#checkout")!;
    const button = document.querySelector("button")!;
    setRect(boundary, { x: 80, y: 100, width: 500, height: 300 });
    setRect(button, { x: 100, y: 140, width: 120, height: 40 });
    const bridge = installTestKit({
      enabled: true,
      page: { id: "checkout" },
      facts: () => ({ cartItems: 2, authToken: "must-not-leak" }),
      repairStorage: "memory",
    });
    registerBoundary({
      id: "checkout-form",
      name: "Checkout form",
      elements: () => [boundary],
      source: { file: "src/Checkout.tsx", line: 12 },
      facts: () => ({ step: "payment" }),
    });

    const snapshot = bridge.snapshot({ detail: "forensic" });
    const described = snapshot.nodes.find((node) => node.testId === "pay");
    expect(snapshot.page).toMatchObject({ id: "checkout", route: "/test", viewport: { width: 1000, height: 800 } });
    expect(snapshot.facts).toEqual({ cartItems: 2, authToken: "[redacted]" });
    expect(snapshot.components[0]).toMatchObject({ id: "checkout-form", source: { file: "src/Checkout.tsx", line: 12 }, facts: { step: "payment" } });
    expect(described).toMatchObject({ role: "button", name: "Pay now", componentId: "checkout-form" });
    expect(described?.geometry).toMatchObject({
      viewport: { x: 100, y: 140, width: 120, height: 40 },
      document: { x: 100, y: 140, width: 120, height: 40 },
      normalized: { x: 0.1, y: 0.175, width: 0.12, height: 0.05 },
    });
    expect(described?.locators[0]).toEqual({ type: "test_id", value: "pay" });
  });

  it("models browser visual zoom without converting CSS-pixel element rectangles", () => {
    document.body.innerHTML = `<button data-testid="zoom-edge">Zoom edge</button>`;
    const button = document.querySelector("button")!;
    setRect(button, { x: 400, y: 100, width: 200, height: 40 });
    Object.defineProperty(window, "devicePixelRatio", { value: 1.5, configurable: true });
    Object.defineProperty(window, "visualViewport", {
      configurable: true,
      value: {
        width: 500,
        height: 400,
        offsetLeft: 0,
        offsetTop: 0,
        scale: 2,
        addEventListener() {},
        removeEventListener() {},
      },
    });
    const bridge = installTestKit({ enabled: true, page: { id: "zoom" }, repairStorage: "memory" });

    const snapshot = bridge.snapshot();
    const described = snapshot.nodes.find((node) => node.testId === "zoom-edge");
    expect(snapshot.page.viewport).toEqual({
      width: 1000,
      height: 800,
      dpr: 1.5,
      visual: { x: 0, y: 0, width: 500, height: 400, scale: 2 },
    });
    expect(described?.geometry).toMatchObject({
      viewport: { x: 400, y: 100, width: 200, height: 40 },
      document: { x: 400, y: 100, width: 200, height: 40 },
      normalized: { x: 0.8, y: 0.25, width: 0.4, height: 0.1 },
      visibleRatio: 0.5,
    });
  });

  it("redacts sensitive subtrees, hidden/password fields, and application facts", () => {
    document.body.innerHTML = `<div id="parent">Public <span data-private>private value</span></div><input type="password" value="hunter2"><input type="hidden" value="secret">`;
    for (const element of document.body.querySelectorAll("*")) setRect(element, { x: 1, y: 1, width: 20, height: 20 });
    const bridge = installTestKit({
      enabled: true,
      page: { id: "security" },
      redact: ["[data-private]"],
      facts: () => ({ password: "bad", nested: { apiKey: "bad", safe: "ok" } }),
      repairStorage: "memory",
    });
    const encoded = JSON.stringify(bridge.snapshot({ detail: "forensic" }));
    expect(encoded).not.toContain("hunter2");
    expect(encoded).not.toContain("private value");
    expect(encoded).not.toContain('"bad"');
    expect(encoded).toContain("[redacted]");
  });

  it("paginates bounded snapshots and returns only changed/removed nodes", async () => {
    document.body.innerHTML = `<button>One</button><button>Two</button><button>Three</button>`;
    for (const element of document.querySelectorAll("button")) setRect(element, { x: 1, y: 1, width: 20, height: 20 });
    const bridge = installTestKit({ enabled: true, page: { id: "bounds" }, maxNodes: 10, maxEncodedBytes: 16_384, repairStorage: "memory" });
    const first = bridge.snapshot({ limits: { nodes: 1 } });
    expect(first.nodes).toHaveLength(1);
    expect(first.truncated).toBe(true);
    expect(first.nextCursor).not.toBeNull();
    const second = bridge.snapshot({ limits: { nodes: 1 }, cursor: first.nextCursor });
    expect(second.nodes).toHaveLength(1);
    expect(second.nodes[0]?.id).not.toBe(first.nodes[0]?.id);
    expect(new TextEncoder().encode(JSON.stringify(first)).byteLength).toBeLessThanOrEqual(16_384);

    const baseline = bridge.snapshot();
    const removedId = baseline.nodes.find((node) => node.role === "button" && node.text === "One")!.id;
    document.querySelector("button")!.remove();
    const changedRevision = await bridge.waitForChange(baseline.revision, 100);
    const diff = bridge.snapshot({ detail: "diff", sinceRevision: baseline.revision });
    expect(changedRevision).toBeGreaterThan(baseline.revision);
    expect(diff.removedNodeIds).toContain(removedId);
  });

  it("observes open shadow DOM changes and excludes overlay DOM", async () => {
    const host = document.createElement("div");
    const shadow = host.attachShadow({ mode: "open" });
    shadow.innerHTML = "<button>Shadow action</button>";
    document.body.append(host);
    setRect(shadow.querySelector("button")!, { x: 5, y: 5, width: 80, height: 30 });
    const overlay = document.createElement("div");
    overlay.dataset.a3sTestkitOverlay = "";
    overlay.innerHTML = "<button>Never capture me</button>";
    document.body.append(overlay);
    const bridge = installTestKit({ enabled: true, page: { id: "shadow" }, repairStorage: "memory" });
    const first = bridge.snapshot({ detail: "forensic" });
    expect(first.nodes.some((node) => node.text === "Shadow action")).toBe(true);
    expect(first.nodes.some((node) => node.text === "Never capture me")).toBe(false);

    shadow.querySelector("button")!.textContent = "Changed shadow action";
    await expect(bridge.waitForChange(first.revision, 100)).resolves.toBeGreaterThan(first.revision);
  });

  it("tracks route changes, portal roots, transformed and sticky geometry, nested scrolling, virtualization, dialogs, and hydration", async () => {
    document.body.innerHTML = `<div id="app"><section id="list" style="overflow:auto"><button id="row">Row 1</button></section></div><div id="portal"><dialog open><button id="dialog-action">Confirm</button></dialog></div>`;
    document.documentElement.dataset.hydrated = "false";
    const app = document.querySelector("#app")!;
    const portal = document.querySelector("#portal")!;
    const list = document.querySelector<HTMLElement>("#list")!;
    const row = document.querySelector<HTMLElement>("#row")!;
    const dialogAction = document.querySelector<HTMLElement>("#dialog-action")!;
    setRect(app, { x: 0, y: 0, width: 800, height: 600 });
    setRect(portal, { x: 500, y: 20, width: 220, height: 160 });
    setRect(list, { x: 10, y: 20, width: 300, height: 120 });
    setRect(row, { x: 20, y: 30, width: 100, height: 32 });
    setRect(dialogAction, { x: 530, y: 60, width: 100, height: 32 });
    list.style.overflow = "auto";
    row.style.position = "sticky";
    row.style.transform = "matrix(1, 0, 0, 1, 0, 0)";
    const bridge = installTestKit({
      enabled: true,
      page: { id: "edge-cases" },
      ready: () => document.documentElement.dataset.hydrated === "true",
      repairStorage: "memory",
    });
    registerBoundary({ id: "app-shell", name: "App shell", elements: () => [app, portal] });

    const hydrating = bridge.snapshot({ detail: "forensic" });
    expect(hydrating.page.ready).toBe(false);
    expect(hydrating.components[0]?.boxes).toHaveLength(2);
    expect(hydrating.nodes.find((node) => node.testId === undefined && node.locators.some((locator) => "value" in locator && locator.value === "#row"))?.geometry).toMatchObject({ position: "sticky", transformed: true });
    expect(hydrating.nodes.find((node) => node.text === "Confirm")?.componentId).toBe("app-shell");

    document.documentElement.dataset.hydrated = "true";
    row.replaceWith(Object.assign(document.createElement("button"), { id: "row", textContent: "Row 50" }));
    const virtualRow = document.querySelector<HTMLElement>("#row")!;
    setRect(virtualRow, { x: 20, y: 30, width: 100, height: 32 });
    history.pushState(null, "", "/virtualized?page=5");
    window.dispatchEvent(new Event("scroll"));
    await expect(bridge.waitForChange(hydrating.revision, 100)).resolves.toBeGreaterThan(hydrating.revision);
    const updated = bridge.snapshot({ detail: "forensic" });
    expect(updated.page).toMatchObject({ route: "/virtualized?page=5", ready: true });
    expect(updated.nodes.some((node) => node.text === "Row 50")).toBe(true);
    expect(updated.nodes.some((node) => node.text === "Row 1")).toBe(false);
  });

  it("enriches ordered repair submissions and enforces idempotent state transitions", () => {
    document.body.innerHTML = "<button>Fix me</button>";
    const button = document.querySelector("button")!;
    setRect(button, { x: 10, y: 20, width: 90, height: 30 });
    const bridge = installTestKit({ enabled: true, page: { id: "repairs" }, facts: () => ({ state: "broken" }), repairStorage: "memory" });
    const nodeId = bridge.snapshot().nodes.find((node) => node.role === "button")!.id;
    const drafts: RepairDraft[] = ["First fix", "Second fix"].map((instruction, index) => ({
      id: `finding-${index}`,
      instruction,
      intent: "fix",
      severity: "important",
      ...(index === 0 ? { relations: [{ kind: "conflicts_with" as const, findingId: "finding-1" }] } : {}),
      target: { kind: "node", nodeIds: [nodeId] },
      createdAt: new Date(index).toISOString(),
    }));
    const submitted = bridge.submitRepair({ batchId: "batch-1", findings: drafts });
    expect(submitted.map((repair) => repair.instruction)).toEqual(["First fix", "Second fix"]);
    expect(submitted[0]).toMatchObject({ status: "queued", batchId: "batch-1", context: { untrusted: true, nodes: [{ id: nodeId }], facts: { state: "broken" } } });
    expect(bridge.peekRepairBatch(10)).toHaveLength(2);
    expect(bridge.peekRepairBatch(10)).toHaveLength(2);
    expect(bridge.takeRepairBatch(10)).toHaveLength(2);
    expect(bridge.takeRepairBatch(10)).toEqual([]);
    expect(bridge.listRepairBatches()[0]).toMatchObject({
      id: "batch-1",
      status: "queued",
      findingIds: ["finding-0", "finding-1"],
    });
    const exported = bridge.exportRepairs(drafts);
    expect(exported).toMatchObject({
      protocol: "a3s.test.repair/1",
      page: { id: "repairs", revision: expect.any(Number) },
      findings: [{ instruction: "First fix", relations: [{ kind: "conflicts_with", findingId: "finding-1" }], context: { untrusted: true } }, { instruction: "Second fix" }],
    });
    const markdown = bridge.exportRepairsMarkdown(drafts);
    expect(markdown).toContain("# A3S Test repair findings");
    expect(markdown).toContain("First fix");
    expect(markdown).toContain("untrusted evidence");

    const claimed: RepairEvent = { requestId: "request-1", findingId: "finding-0", sequence: 1, status: "claimed", actor: "agent", timestamp: new Date().toISOString() };
    expect(bridge.applyRepairEvent(claimed)?.status).toBe("claimed");
    expect(bridge.applyRepairEvent(claimed)?.status).toBe("claimed");
    expect(bridge.applyRepairEvent({ ...claimed, requestId: "request-2", sequence: 2, status: "resolved" })).toBeNull();
    expect(bridge.listRepairs()[0]?.status).toBe("claimed");
    expect(bridge.applyRepairEvent({
      requestId: "request-3",
      findingId: "finding-0",
      sequence: 2,
      status: "needs_input",
      actor: "agent",
      timestamp: new Date().toISOString(),
      message: "Which state should this button use?",
    })?.status).toBe("needs_input");
    expect(bridge.addRepairReply({
      requestId: "reply-1",
      findingId: "finding-0",
      actor: "human",
      timestamp: new Date().toISOString(),
      message: "Use the enabled checkout state.",
    })).toBe(true);
    expect(bridge.listRepairReplies("finding-0")).toMatchObject([
      { actor: "agent", message: "Which state should this button use?" },
      { actor: "human", message: "Use the enabled checkout state." },
    ]);
  });

  it("queues human clarification and review actions exactly once", () => {
    document.body.innerHTML = "<button>Fix me</button>";
    const button = document.querySelector("button")!;
    setRect(button, { x: 10, y: 20, width: 90, height: 30 });
    const bridge = installTestKit({ enabled: true, page: { id: "human-actions" }, repairStorage: "memory" });
    const nodeId = bridge.snapshot().nodes.find((node) => node.role === "button")!.id;
    bridge.submitRepair({ findings: [{ id: "finding-human", instruction: "Fix me", intent: "fix", severity: "important", target: { kind: "node", nodeIds: [nodeId] }, createdAt: new Date(0).toISOString() }] });
    bridge.applyRepairEvent({ requestId: "claim", findingId: "finding-human", sequence: 1, status: "claimed", actor: "agent", timestamp: new Date().toISOString() });
    bridge.applyRepairEvent({ requestId: "question", findingId: "finding-human", sequence: 2, status: "needs_input", actor: "agent", timestamp: new Date().toISOString(), message: "Which state?" });

    const reply = bridge.submitRepairAction({ findingId: "finding-human", action: "reply", message: "Use the enabled state." });
    expect(reply).toMatchObject({ action: "reply", findingId: "finding-human", message: "Use the enabled state." });
    expect(bridge.takeRepairActions()).toEqual([reply]);
    expect(bridge.takeRepairActions()).toEqual([]);
    expect(bridge.listRepairReplies("finding-human").at(-1)).toMatchObject({ actor: "human", message: "Use the enabled state." });
    expect(bridge.submitRepairAction({ findingId: "finding-human", action: "accept" })).toBeNull();

    bridge.applyRepairEvent({ requestId: reply!.requestId, findingId: "finding-human", sequence: 3, status: "queued", actor: "human", timestamp: new Date().toISOString() });
    expect(bridge.takeRepairActions()).toEqual([]);
    bridge.applyRepairEvent({ requestId: "claim-2", findingId: "finding-human", sequence: 4, status: "claimed", actor: "agent", timestamp: new Date().toISOString() });
    bridge.applyRepairEvent({ requestId: "progress", findingId: "finding-human", sequence: 5, status: "repairing", actor: "agent", timestamp: new Date().toISOString() });
    bridge.applyRepairEvent({ requestId: "complete", findingId: "finding-human", sequence: 6, status: "verifying", actor: "agent", timestamp: new Date().toISOString() });
    bridge.applyRepairEvent({ requestId: "verified", findingId: "finding-human", sequence: 7, status: "review_ready", actor: "a3s-test", timestamp: new Date().toISOString() });
    expect(bridge.submitRepairAction({ findingId: "finding-human", action: "accept" })).toMatchObject({ action: "accept" });
  });

  it("resolves waiters and removes the global bridge on dispose", async () => {
    const bridge = installTestKit({ enabled: true, page: { id: "dispose" }, repairStorage: "memory" });
    const revision = bridge.snapshot().revision;
    const waiting = bridge.waitForChange(revision, 1_000);
    bridge.dispose();
    await expect(waiting).resolves.toBeNull();
    expect(() => bridge.snapshot()).toThrow("disposed");
  });

  it("restores paused animations when disposed", () => {
    const bridge = installTestKit({ enabled: true, page: { id: "paused-dispose" }, repairStorage: "memory" });
    bridge.setAnimationsPaused(true);
    expect(document.documentElement.hasAttribute("data-a3s-testkit-animations-paused")).toBe(true);

    expect(() => bridge.dispose()).not.toThrow();
    expect(document.documentElement.hasAttribute("data-a3s-testkit-animations-paused")).toBe(false);
    expect(() => bridge.dispose()).not.toThrow();
  });
});
