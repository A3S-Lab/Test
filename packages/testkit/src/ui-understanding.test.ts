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

function setOverflowMetrics(
  element: Element,
  metrics: {
    clientWidth: number;
    clientHeight: number;
    scrollWidth: number;
    scrollHeight: number;
    scrollLeft: number;
    scrollTop: number;
  },
): void {
  for (const [property, value] of Object.entries(metrics)) {
    Object.defineProperty(element, property, { value, configurable: true });
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

  it("distinguishes overflowing content, active clipping, and animation timelines", () => {
    document.body.innerHTML = `
      <section id="clipped" dir="rtl" style="box-sizing:border-box;writing-mode:vertical-rl;direction:rtl;margin:4px 8px 12px 16px;border-style:solid;border-width:1px 2px 3px 4px;padding:5px 6px 7px 8px;overflow-x:hidden;overflow-y:auto">
        <div>Wide content</div>
      </section>
      <section id="visible" style="overflow:visible">Visible overflow</section>
      <div id="animated" style="animation-name:reveal;animation-duration:1s;animation-timeline:view();animation-range-start:entry 10%;animation-range-end:cover 80%">Animated</div>
      <div id="scroll-animated">Scroll driven</div>
      <div id="timed" style="animation-name:pulse;animation-duration:240ms">Timed</div>
      <div id="named" style="animation-name:enter;animation-duration:1s;animation-timeline:--story">Named timeline</div>
    `;
    giveEveryElementGeometry();
    const clipped = document.querySelector("#clipped")!;
    const visible = document.querySelector("#visible")!;
    const animated = document.querySelector("#animated")!;
    const scrollAnimated = document.querySelector("#scroll-animated")!;
    setOverflowMetrics(clipped, {
      clientWidth: 160,
      clientHeight: 80,
      scrollWidth: 280,
      scrollHeight: 80,
      scrollLeft: -24,
      scrollTop: 0,
    });
    setOverflowMetrics(visible, {
      clientWidth: 160,
      clientHeight: 80,
      scrollWidth: 260,
      scrollHeight: 80,
      scrollLeft: 0,
      scrollTop: 0,
    });
    Object.defineProperty(animated, "getAnimations", {
      configurable: true,
      value: () => [
        {
          playState: "running",
          timeline: { constructor: { name: "ViewTimeline" } },
        },
      ],
    });
    Object.defineProperty(scrollAnimated, "getAnimations", {
      configurable: true,
      value: () => [
        {
          playState: "running",
          timeline: { constructor: { name: "ScrollTimeline" } },
        },
      ],
    });
    const bridge = installTestKit({
      enabled: true,
      page: { id: "ui-overflow-and-timeline" },
      repairStorage: "memory",
      maxUiDurationMs: 100,
      maxUiEncodedBytes: 262_144,
    });

    const snapshot = bridge.snapshot({ detail: "forensic" });
    const clippedId = snapshot.nodes.find(
      (node) => node.attributes?.id === "clipped",
    )!.id;
    const visibleId = snapshot.nodes.find(
      (node) => node.attributes?.id === "visible",
    )!.id;
    const animatedId = snapshot.nodes.find(
      (node) => node.attributes?.id === "animated",
    )!.id;
    const timedId = snapshot.nodes.find(
      (node) => node.attributes?.id === "timed",
    )!.id;
    const scrollAnimatedId = snapshot.nodes.find(
      (node) => node.attributes?.id === "scroll-animated",
    )!.id;
    const namedId = snapshot.nodes.find(
      (node) => node.attributes?.id === "named",
    )!.id;
    const clippedLayout = snapshot.ui!.layout.nodes.find(
      (node) => node.nodeId === clippedId,
    )!;
    const visibleLayout = snapshot.ui!.layout.nodes.find(
      (node) => node.nodeId === visibleId,
    )!;
    const animation = snapshot.ui!.motion.animations.find(
      (entry) => entry.nodeId === animatedId,
    )!;
    const timedAnimation = snapshot.ui!.motion.animations.find(
      (entry) => entry.nodeId === timedId,
    )!;
    const scrollAnimation = snapshot.ui!.motion.animations.find(
      (entry) => entry.nodeId === scrollAnimatedId,
    )!;
    const namedAnimation = snapshot.ui!.motion.animations.find(
      (entry) => entry.nodeId === namedId,
    )!;

    expect(clippedLayout.overflowMetrics).toEqual({
      clientWidth: 160,
      clientHeight: 80,
      scrollWidth: 280,
      scrollHeight: 80,
      scrollLeft: -24,
      scrollTop: 0,
      overflowingX: true,
      overflowingY: false,
      clipsX: true,
      clipsY: false,
    });
    expect(clippedLayout.boxModel).toEqual({
      boxSizing: "border-box",
      writingMode: "vertical-rl",
      direction: "rtl",
      margin: { top: "4px", right: "8px", bottom: "12px", left: "16px" },
      borderWidth: {
        top: "1px",
        right: "2px",
        bottom: "3px",
        left: "4px",
      },
      padding: { top: "5px", right: "6px", bottom: "7px", left: "8px" },
    });
    expect(visibleLayout.overflowMetrics).toMatchObject({
      overflowingX: true,
      clipsX: false,
    });
    expect(animation).toMatchObject({
      rangeStarts: ["entry 10%"],
      rangeEnds: ["cover 80%"],
      timelines: expect.arrayContaining([
        { value: "view()", kind: "view", source: "computed_style" },
        {
          value: "(view-timeline)",
          kind: "view",
          source: "web_animations",
        },
      ]),
    });
    expect(timedAnimation.timelines).toContainEqual({
      value: "auto",
      kind: "document",
      source: "computed_style",
    });
    expect(scrollAnimation.timelines).toContainEqual({
      value: "(scroll-timeline)",
      kind: "scroll",
      source: "web_animations",
    });
    expect(namedAnimation.timelines).toContainEqual({
      value: "--story",
      kind: "named",
      source: "computed_style",
    });
  });

  it("keeps layout relationships inside the sampled graph", () => {
    document.body.innerHTML = `
      <main id="layout-root">
        <div id="unboxed-parent" style="display:contents;overflow:hidden">
          <button id="nested-action">Continue</button>
        </div>
      </main>
    `;
    const root = document.querySelector<HTMLElement>("#layout-root")!;
    const unboxedParent =
      document.querySelector<HTMLElement>("#unboxed-parent")!;
    const action = document.querySelector<HTMLButtonElement>("#nested-action")!;
    setRect(root, { x: 20, y: 30, width: 320, height: 120 });
    setRect(unboxedParent, { x: 0, y: 0, width: 0, height: 0 });
    setRect(action, { x: 40, y: 50, width: 120, height: 40 });
    Object.defineProperty(action, "offsetParent", {
      configurable: true,
      value: unboxedParent,
    });

    const bridge = installTestKit({
      enabled: true,
      page: { id: "ui-closed-layout-graph" },
      repairStorage: "memory",
      maxUiDurationMs: 100,
    });
    const snapshot = bridge.snapshot({ detail: "forensic" });
    const rootId = snapshot.nodes.find(
      (node) => node.attributes?.id === "layout-root",
    )!.id;
    const unboxedParentId = snapshot.nodes.find(
      (node) => node.attributes?.id === "unboxed-parent",
    )!.id;
    const actionId = snapshot.nodes.find(
      (node) => node.attributes?.id === "nested-action",
    )!.id;
    const layout = snapshot.ui!.layout;
    const layoutNodeIds = new Set(layout.nodes.map((node) => node.nodeId));

    expect(layoutNodeIds).not.toContain(unboxedParentId);
    expect(
      layout.nodes.find((node) => node.nodeId === actionId)?.parentNodeId,
    ).toBe(rootId);
    expect(layout.edges).toContainEqual({
      fromNodeId: rootId,
      toNodeId: actionId,
      relation: "contains",
    });
    expect(layout.edges).not.toEqual(
      expect.arrayContaining([
        expect.objectContaining({ fromNodeId: unboxedParentId }),
      ]),
    );
    for (const edge of layout.edges) {
      expect(layoutNodeIds.has(edge.fromNodeId)).toBe(true);
      expect(layoutNodeIds.has(edge.toNodeId)).toBe(true);
    }
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
