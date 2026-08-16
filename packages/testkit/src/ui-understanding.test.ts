import { describe, expect, it } from "vitest";
import { installTestKit } from "./runtime";
import { setRect } from "./test-setup";

function giveEveryElementGeometry(): void {
  let index = 0;
  for (const element of document.querySelectorAll("body *")) {
    setRect(element, {
      x: 20 + (index % 3) * 220,
      y: 40 + Math.floor(index / 3) * 80,
      width: 180,
      height: 56,
    });
    index += 1;
  }
}

describe("rendered UI understanding", () => {
  it("extracts observed design tokens, layout, repeated components, and motion", () => {
    document.documentElement.style.setProperty(
      "--brand-color",
      "rgb(24, 80, 220)",
    );
    document.documentElement.style.setProperty("--auth-token", "must-not-leak");
    document.body.innerHTML = `
      <main style="display:flex;flex-direction:column;gap:16px;background-color:rgb(248,249,252)">
        <article class="first-card" style="display:grid;padding:16px;border-radius:12px;background-color:rgb(255,255,255);box-shadow:0 4px 12px rgb(0 0 0 / 0.12)">
          <h2 style="font:600 20px/28px sans-serif">First plan</h2>
          <button style="color:rgb(24,80,220);transition:color 120ms ease">Choose</button>
        </article>
        <article class="second-card" style="display:grid;padding:16px;border-radius:12px;background-color:rgb(255,255,255);box-shadow:0 4px 12px rgb(0 0 0 / 0.12)">
          <h2 style="font:600 20px/28px sans-serif">Second plan</h2>
          <button style="color:rgb(24,80,220);transition:color 120ms ease">Choose</button>
        </article>
        <aside style="position:sticky;top:0">Summary</aside>
        <canvas aria-label="Preview"></canvas>
      </main>
    `;
    giveEveryElementGeometry();
    const bridge = installTestKit({
      enabled: true,
      page: { id: "ui-profile" },
      repairStorage: "memory",
      maxUiNodes: 80,
      maxUiDurationMs: 100,
      maxUiEncodedBytes: 262_144,
    });

    const snapshot = bridge.snapshot({
      detail: "forensic",
      limits: { uiNodes: 80, uiDurationMs: 100 },
    });
    const ui = snapshot.ui!;
    const mainId = snapshot.nodes.find((node) => node.tag === "main")!.id;
    const articleIds = snapshot.nodes
      .filter((node) => node.tag === "article")
      .map((node) => node.id);
    const buttonId = snapshot.nodes.find((node) => node.tag === "button")!.id;

    expect(bridge.probe().capabilities).toEqual(
      expect.arrayContaining([
        "ui_style_profile",
        "ui_layout_graph",
        "ui_component_clusters",
        "ui_state_diffs",
        "ui_motion_profile",
      ]),
    );
    expect(ui).toMatchObject({
      protocol: "a3s.test.ui-understanding/1",
      pageRevision: snapshot.revision,
      viewport: snapshot.page.viewport,
      scope: { kind: "page" },
    });
    expect(ui.observationId).toMatch(/^ui-[0-9]+-[0-9a-f]{16}$/);
    expect(ui.style.colors).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          value: "rgb(255, 255, 255)",
          properties: expect.arrayContaining(["background-color"]),
          confidence: 1,
        }),
      ]),
    );
    expect(ui.style.typography).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ size: "20px", weight: "600" }),
      ]),
    );
    expect(ui.style.customProperties).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          name: "--brand-color",
          value: "rgb(24, 80, 220)",
        }),
      ]),
    );
    expect(JSON.stringify(ui)).not.toContain("must-not-leak");
    expect(
      ui.layout.nodes.find((node) => node.nodeId === mainId),
    ).toMatchObject({
      display: "flex",
      flex: { direction: "column", gap: "16px" },
    });
    expect(ui.layout.nodes.some((node) => node.position === "sticky")).toBe(
      true,
    );
    expect(ui.components).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          memberCount: 2,
          memberNodeIds: expect.arrayContaining(articleIds),
          confidence: 1,
        }),
      ]),
    );
    expect(ui.motion.transitions).toEqual(
      expect.arrayContaining([expect.objectContaining({ nodeId: buttonId })]),
    );
    expect(ui.motion.stickyNodeIds).not.toHaveLength(0);
    expect(ui.motion.canvasNodeIds).not.toHaveLength(0);
    expect(ui.evidence).toMatchObject({
      sourceKinds: expect.arrayContaining([
        "computed_style",
        "dom_structure",
        "layout_geometry",
      ]),
    });
  });

  it("enforces caller and installation budgets and can be disabled per snapshot", () => {
    document.body.innerHTML = Array.from(
      { length: 12 },
      (_, index) => `<div style="padding:${index + 1}px">Row ${index}</div>`,
    ).join("");
    giveEveryElementGeometry();
    const bridge = installTestKit({
      enabled: true,
      page: { id: "ui-budget" },
      repairStorage: "memory",
      maxUiNodes: 3,
      maxUiDurationMs: 100,
      maxUiEncodedBytes: 32_768,
    });

    const snapshot = bridge.snapshot({
      detail: "forensic",
      limits: {
        uiNodes: 2,
        uiDurationMs: 100,
        uiEncodedBytes: 32_768,
      },
    });
    expect(snapshot.ui?.budget.limits.nodes).toBe(2);
    expect(snapshot.ui?.budget.used.nodes).toBeLessThanOrEqual(2);
    expect(snapshot.ui?.budget.used.encodedBytes).toBeLessThanOrEqual(32_768);
    expect(snapshot.ui?.budget.truncated).toBe(true);
    expect(snapshot.ui?.budget.reasons).toContain("node_limit");
    expect(bridge.snapshot({ ui: false }).ui).toBeUndefined();
  });

  it("records real default-to-focus style and accessibility differences", async () => {
    document.body.innerHTML = `<button style="color:rgb(20, 40, 80);outline-color:rgb(20, 40, 80)">Continue</button>`;
    const button = document.querySelector<HTMLButtonElement>("button")!;
    setRect(button, { x: 40, y: 60, width: 120, height: 40 });
    const bridge = installTestKit({
      enabled: true,
      page: { id: "ui-state" },
      repairStorage: "memory",
      maxUiDurationMs: 100,
    });
    const baseline = bridge.snapshot({ detail: "forensic" });
    const buttonId = baseline.nodes.find((node) => node.tag === "button")!.id;

    const nativeMatches = button.matches.bind(button);
    button.matches = ((selector: string) =>
      selector === ":focus-visible" ||
      nativeMatches(selector)) as typeof button.matches;
    button.focus();
    await Promise.resolve();
    const focusOnly = bridge.snapshot({ detail: "forensic" });
    expect(focusOnly.revision).toBe(baseline.revision);
    expect(focusOnly.ui?.observationId).not.toBe(baseline.ui?.observationId);

    button.style.color = "rgb(220, 40, 60)";
    button.style.outlineColor = "rgb(220, 40, 60)";
    await expect(
      bridge.waitForChange(baseline.revision, 100),
    ).resolves.toBeGreaterThan(baseline.revision);

    const focused = bridge.snapshot({ detail: "forensic" });
    expect(focused.revision).toBeGreaterThan(baseline.revision);
    expect(focused.ui?.observationId).not.toBe(baseline.ui?.observationId);
    expect(focused.ui?.stateDiffs).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          nodeId: buttonId,
          from: "default",
          to: "focus",
          styleChanges: expect.arrayContaining([
            {
              property: "color",
              before: "rgb(20, 40, 80)",
              after: "rgb(220, 40, 60)",
            },
          ]),
          accessibilityChanges: expect.arrayContaining([
            { state: "focused", before: false, after: true },
          ]),
        }),
        expect.objectContaining({
          nodeId: buttonId,
          from: "default",
          to: "focus_visible",
        }),
      ]),
    );
  });

  it("reports the exact final encoded size after budget fitting", () => {
    document.body.innerHTML = Array.from(
      { length: 80 },
      (_, index) =>
        `<section style="padding:${index + 1}px;color:rgb(${index}, 40, 80)">` +
        `<button style="transition:color ${index + 1}ms ease">Choice ${index}</button>` +
        "</section>",
    ).join("");
    giveEveryElementGeometry();
    const bridge = installTestKit({
      enabled: true,
      page: { id: "ui-encoded-budget" },
      repairStorage: "memory",
      maxUiNodes: 200,
      maxUiDurationMs: 100,
      maxUiEncodedBytes: 8_192,
    });

    const ui = bridge.snapshot({ detail: "forensic" }).ui!;
    const encodedBytes = new TextEncoder().encode(
      JSON.stringify(ui),
    ).byteLength;
    expect(ui.budget.reasons).toContain("encoded_size_limit");
    expect(ui.budget.used.encodedBytes).toBe(encodedBytes);
    expect(encodedBytes).toBeLessThanOrEqual(ui.budget.limits.encodedBytes);
  });
});
