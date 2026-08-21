import { StrictMode } from "react";
import { fireEvent, render, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { getPageContextBridge, installTestKit } from "./runtime";
import { A3SReviewOverlay, A3STestBoundary, A3STestKit } from "./react";
import { setRect } from "./test-setup";
import type { DesignAuditReport, QualityReport } from "./types";

const { domToJpeg } = vi.hoisted(() => ({ domToJpeg: vi.fn() }));
vi.mock("modern-screenshot", () => ({ domToJpeg }));

function shadowQuery(selector: string): HTMLElement {
  const host = document.querySelector<HTMLElement>("[data-a3s-testkit-overlay]");
  const element = host?.shadowRoot?.querySelector<HTMLElement>(selector);
  if (!element) throw new Error(`missing shadow element ${selector}`);
  return element;
}

function shadowButton(text: string): HTMLButtonElement {
  const button = Array.from(
    document
      .querySelector<HTMLElement>("[data-a3s-testkit-overlay]")!
      .shadowRoot!.querySelectorAll("button"),
  ).find((candidate) => candidate.textContent === text);
  if (!(button instanceof HTMLButtonElement)) throw new Error(`missing shadow button ${text}`);
  return button;
}

function installCompactMediaQuery(initialMatches = true) {
  const condition = "(max-width: 420px)";
  const listeners = new Set<(event: MediaQueryListEvent) => void>();
  let matches = initialMatches;
  const compactQuery = {
    get matches() { return matches; },
    media: condition,
    addEventListener: (_type: string, listener: EventListenerOrEventListenerObject) => {
      if (typeof listener === "function") listeners.add(listener as (event: MediaQueryListEvent) => void);
    },
    removeEventListener: (_type: string, listener: EventListenerOrEventListenerObject) => {
      if (typeof listener === "function") listeners.delete(listener as (event: MediaQueryListEvent) => void);
    },
  } as unknown as MediaQueryList;
  const desktopQuery = {
    ...compactQuery,
    matches: false,
  } as MediaQueryList;
  Object.defineProperty(window, "matchMedia", {
    configurable: true,
    value: vi.fn((query: string) => query === condition ? compactQuery : desktopQuery),
  });
  return {
    setMatches(value: boolean) {
      matches = value;
      const event = { matches: value, media: condition } as MediaQueryListEvent;
      for (const listener of listeners) listener(event);
    },
  };
}

describe("React adapter and review overlay", () => {
  it("survives StrictMode and registers component context", async () => {
    const view = render(<StrictMode><A3STestKit enabled page={{ id: "react" }} repairStorage="memory"><A3STestBoundary id="card" name="Card" source={{ file: "src/Card.tsx" }}><button>Buy</button></A3STestBoundary></A3STestKit></StrictMode>);
    await waitFor(() => expect(getPageContextBridge()).not.toBeNull());
    const boundary = document.querySelector("div")!;
    const button = document.querySelector("button")!;
    setRect(boundary, { x: 10, y: 10, width: 200, height: 80 });
    setRect(button, { x: 20, y: 20, width: 70, height: 30 });
    await waitFor(() => expect(getPageContextBridge()?.snapshot().components).toHaveLength(1));
    const buttonNode = getPageContextBridge()
      ?.snapshot()
      .nodes.find((node) => node.role === "button");
    expect(buttonNode?.componentId).toBe("card");
    expect(buttonNode?.sourceMapping?.candidates[0]).toMatchObject({
      span: { file: "src/Card.tsx" },
      origin: "boundary_hint",
      relation: "ancestor",
      componentId: "card",
    });
    view.unmount();
    expect(getPageContextBridge()).toBeNull();
  });

  it("follows the page language and applies bounded overlay message overrides", async () => {
    const previousLanguage = document.documentElement.lang;
    document.documentElement.lang = "zh-CN";
    const view = render(<A3STestKit enabled page={{ id: "localized-review" }} repairStorage="memory"><button data-testid="localized-target">Checkout</button><A3SReviewOverlay enabled defaultOpen messages={{ reviewTitle: "页面评审" }} /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());

    const root = shadowQuery(".a3s-root");
    expect(root.lang).toBe("zh-CN");
    expect(shadowQuery(".a3s-panel-title").textContent).toBe("页面评审");
    expect(shadowButton("元素")).toBeTruthy();
    expect(shadowButton("元素").getAttribute("aria-label")).toBe("标记元素");
    expect(shadowButton("新反馈").getAttribute("aria-pressed")).toBe("true");
    expect(shadowQuery(".a3s-panel").querySelector(".a3s-tool-tray")).toBeNull();
    fireEvent.click(shadowButton("问题"));
    expect(shadowButton("问题").getAttribute("aria-pressed")).toBe("true");
    fireEvent.click(shadowButton("偏好设置"));
    expect(shadowQuery("[aria-label='评审主题']")).toBeTruthy();
    fireEvent.click(shadowButton("新反馈"));
    fireEvent.click(shadowButton("布局"));
    const componentType = shadowQuery("[aria-label='布局组件类型']") as HTMLInputElement;
    expect(componentType.value).toBe("区块");
    fireEvent.click(shadowButton("组件目录 · 90"));
    fireEvent.change(shadowQuery("[aria-label='搜索组件目录']"), { target: { value: "结账" } });
    expect(shadowButton("结账表单")).toBeTruthy();
    fireEvent.click(shadowButton("结账表单"));
    expect(componentType.value).toBe("结账表单");

    document.documentElement.lang = "en-US";
    await waitFor(() => expect(root.lang).toBe("en"));
    expect(componentType.value).toBe("Checkout Form");
    expect(shadowButton("Element")).toBeTruthy();
    fireEvent.change(componentType, { target: { value: "Custom orbit panel" } });
    document.documentElement.lang = "zh-CN";
    await waitFor(() => expect(root.lang).toBe("zh-CN"));
    expect(componentType.value).toBe("Custom orbit panel");
    fireEvent.click(shadowButton("布局"));

    const target = document.querySelector<HTMLElement>("[data-testid=localized-target]")!;
    setRect(target, { x: 100, y: 80, width: 120, height: 40 });
    fireEvent.click(shadowButton("元素"));
    target.dispatchEvent(pointerEventWithPath(target, 120, 90));
    await waitFor(() => expect(shadowQuery(".a3s-editor").textContent).toContain("新建问题"));
    expect(shadowQuery(".a3s-editor textarea").getAttribute("placeholder")).toBe("描述需要修改的内容");
    expect(shadowButton("发送并自动修复")).toBeTruthy();

    view.unmount();
    document.documentElement.lang = previousLanguage;
  });

  it("offers one direct action from the empty findings workspace", async () => {
    render(<A3STestKit enabled page={{ id: "empty-workspace" }} repairStorage="memory"><A3SReviewOverlay enabled defaultOpen locale="en" /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());

    fireEvent.click(shadowButton("Findings"));
    const empty = shadowQuery(".a3s-empty");
    expect(empty.querySelector("strong")?.textContent).toBe("No findings yet");
    expect(empty.querySelector(".a3s-empty-icon svg")).toBeTruthy();
    const create = empty.querySelector("button");
    expect(create?.textContent).toBe("Create feedback");
    expect(create?.querySelector("svg")).toBeTruthy();
    const buttons = Array.from((empty.getRootNode() as ShadowRoot).querySelectorAll("button"));
    expect(buttons.filter((button) => button.textContent === "New feedback")).toHaveLength(1);
    expect(buttons.filter((button) => button.textContent === "Create feedback")).toHaveLength(1);

    fireEvent.click(create!);
    expect(shadowQuery(".a3s-compose")).toBeTruthy();
    expect(shadowButton("New feedback").getAttribute("aria-pressed")).toBe("true");
  });

  it("locks host scrolling only while a compact review surface is open", async () => {
    const compactViewport = installCompactMediaQuery();
    document.documentElement.style.setProperty("overflow", "clip", "important");
    document.body.style.setProperty("overflow", "auto");
    const view = render(<A3STestKit enabled page={{ id: "compact-scroll-lock" }} repairStorage="memory"><button id="compact-target">Target</button><A3SReviewOverlay enabled defaultOpen locale="en" /></A3STestKit>);

    const panel = await waitFor(() => shadowQuery(".a3s-panel"));
    await waitFor(() => expect(document.documentElement.style.getPropertyValue("overflow")).toBe("hidden"));
    expect(document.documentElement.style.getPropertyPriority("overflow")).toBe("important");
    expect(document.body.style.getPropertyValue("overflow")).toBe("hidden");

    compactViewport.setMatches(false);
    expect(document.documentElement.style.getPropertyValue("overflow")).toBe("clip");
    expect(document.documentElement.style.getPropertyPriority("overflow")).toBe("important");
    expect(document.body.style.getPropertyValue("overflow")).toBe("auto");
    compactViewport.setMatches(true);
    expect(document.documentElement.style.getPropertyValue("overflow")).toBe("hidden");

    fireEvent.click(shadowButton("Element"));
    await waitFor(() => expect(document.documentElement.style.getPropertyValue("overflow")).toBe("clip"));
    expect(document.body.style.getPropertyValue("overflow")).toBe("auto");
    expect(panel.getAttribute("aria-hidden")).toBe("true");
    expect(panel.hasAttribute("inert")).toBe(true);

    fireEvent.click(shadowQuery(".a3s-mobile-marking-actions button"));
    await waitFor(() => expect(document.documentElement.style.getPropertyValue("overflow")).toBe("hidden"));
    expect(panel.hasAttribute("aria-hidden")).toBe(false);
    expect(panel.hasAttribute("inert")).toBe(false);

    fireEvent.click(shadowQuery(".a3s-panel-header .a3s-close"));
    await waitFor(() => expect(document.documentElement.style.getPropertyValue("overflow")).toBe("clip"));
    expect(document.body.style.getPropertyValue("overflow")).toBe("auto");

    fireEvent.click(shadowQuery(".a3s-launch"));
    await waitFor(() => expect(document.documentElement.style.getPropertyValue("overflow")).toBe("hidden"));
    view.unmount();
    expect(document.documentElement.style.getPropertyValue("overflow")).toBe("clip");
    expect(document.documentElement.style.getPropertyPriority("overflow")).toBe("important");
    expect(document.body.style.getPropertyValue("overflow")).toBe("auto");
    document.documentElement.style.removeProperty("overflow");
    document.body.style.removeProperty("overflow");
  });

  it("opens a localized design board as a simple right-side drawer", async () => {
    const previousLanguage = document.documentElement.lang;
    document.documentElement.lang = "zh-CN";
    const view = render(<A3STestKit enabled page={{ id: "localized-design-board" }} repairStorage="memory"><button data-testid="localized-design-target">旧卡片</button><A3SReviewOverlay enabled defaultOpen /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());
    const target = document.querySelector<HTMLElement>("[data-testid=localized-design-target]")!;
    setRect(target, { x: 80, y: 90, width: 240, height: 120 });
    fireEvent.click(shadowButton("元素"));
    target.dispatchEvent(pointerEventWithPath(target, 120, 110));

    await waitFor(() => expect(shadowQuery(".a3s-design-reference").textContent).toContain("补充设计参考"));
    fireEvent.click(shadowButton("打开画板"));

    const layer = await waitFor(() => shadowQuery(".a3s-design-layer"));
    const board = shadowQuery(".a3s-design-board");
    expect(layer.getAttribute("data-side")).toBe("right");
    expect(board.getAttribute("role")).toBe("region");
    expect(board.hasAttribute("aria-modal")).toBe(false);
    expect(board.textContent).toContain("设计参考");
    expect(shadowButton("选择").querySelector("svg")).toBeTruthy();
    expect(shadowButton("画笔").querySelector("svg")).toBeTruthy();
    expect(shadowButton("矩形").querySelector("svg")).toBeTruthy();
    expect(shadowButton("文字").querySelector("svg")).toBeTruthy();
    expect(shadowButton("框选页面截图").querySelector("svg")).toBeTruthy();
    expect(shadowButton("上传截图").querySelector("svg")).toBeTruthy();
    expect(shadowQuery("[aria-label='设计画布']")).toBeTruthy();

    view.unmount();
    document.documentElement.lang = previousLanguage;
  });

  it("creates an element draft and sends one bounded repair", async () => {
    const onSubmitted = vi.fn();
    render(<A3STestKit enabled page={{ id: "review" }} repairStorage="memory"><main><button data-testid="target">Broken action</button></main><A3SReviewOverlay enabled defaultOpen onSubmitted={onSubmitted} /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());
    const target = document.querySelector<HTMLElement>("[data-testid=target]")!;
    setRect(target, { x: 100, y: 80, width: 120, height: 40 });
    fireEvent.click(shadowQuery(".a3s-tools button"));
    target.dispatchEvent(pointerEventWithPath(target, 120, 90));
    await waitFor(() => expect(shadowQuery("textarea")).toBeTruthy());
    fireEvent.change(shadowQuery("textarea"), { target: { value: "Make this action work" } });
    const send = Array.from(document.querySelector<HTMLElement>("[data-a3s-testkit-overlay]")!.shadowRoot!.querySelectorAll("button")).find((button) => button.textContent === "Send and auto-fix")!;
    expect(shadowButton("Add draft").classList.contains("quiet")).toBe(true);
    expect(send.classList.contains("quiet")).toBe(false);
    fireEvent.click(send);
    await waitFor(() => expect(onSubmitted).toHaveBeenCalledTimes(1));
    const repair = onSubmitted.mock.calls[0]![0][0];
    expect(repair).toMatchObject({ instruction: "Make this action work", status: "queued", target: { kind: "node" }, context: { untrusted: true } });
    expect(getPageContextBridge()?.snapshot({ detail: "forensic" }).nodes.some((node) => node.text?.includes("Review & repair"))).toBe(false);
  });

  it("selects a page target without activating its host action", async () => {
    const hostAction = vi.fn();
    render(<A3STestKit enabled page={{ id: "selection-action" }} repairStorage="memory"><button id="selection-action" onClick={hostAction}>Submit order</button><A3SReviewOverlay enabled defaultOpen /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());
    const target = document.querySelector<HTMLElement>("#selection-action")!;
    setRect(target, { x: 80, y: 60, width: 120, height: 40 });
    fireEvent.click(shadowButton("Element"));
    target.dispatchEvent(pointerEventWithPath(target, 100, 80));
    fireEvent.click(target);
    await waitFor(() => expect(shadowQuery(".a3s-editor")).toBeTruthy());
    expect(hostAction).not.toHaveBeenCalled();
  });

  it("attaches a drawn design reference to a selected element", async () => {
    const onSubmitted = vi.fn();
    render(<A3STestKit enabled page={{ id: "design-reference" }} repairStorage="memory"><button id="design-target">Old card</button><A3SReviewOverlay enabled defaultOpen onSubmitted={onSubmitted} /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());
    const target = document.querySelector<HTMLElement>("#design-target")!;
    setRect(target, { x: 80, y: 90, width: 240, height: 120 });
    fireEvent.click(shadowButton("Element"));
    target.dispatchEvent(pointerEventWithPath(target, 120, 110));

    fireEvent.click(await waitFor(() => shadowButton("Open design board")));
    const board = await waitFor(() => shadowQuery(".a3s-design-board"));
    expect(board.getAttribute("role")).toBe("region");
    expect(board.hasAttribute("aria-modal")).toBe(false);
    const canvas = await waitFor(() => shadowQuery("[data-testid='design-canvas']"));
    expect(board.getAttribute("data-theme")).toBe("system");
    setRect(canvas, { x: 0, y: 0, width: 960, height: 600 });
    fireEvent.click(shadowQuery("[data-testid='design-tool-draw']"));
    canvas.dispatchEvent(pointerEvent("pointerdown", canvas, 40, 50));
    canvas.dispatchEvent(pointerEvent("pointermove", canvas, 160, 120));
    canvas.dispatchEvent(pointerEvent("pointerup", canvas, 240, 180));
    const attach = await waitFor(() => {
      const button = shadowButton("Attach to finding");
      expect(button.disabled).toBe(false);
      return button;
    });
    fireEvent.click(attach);

    await waitFor(() => expect(shadowQuery(".a3s-design-reference").textContent).toContain("Sketch attached"));
    fireEvent.change(shadowQuery(".a3s-editor textarea"), { target: { value: "Replace this card with the attached sketch" } });
    fireEvent.click(shadowButton("Send and auto-fix"));
    await waitFor(() => expect(onSubmitted).toHaveBeenCalledTimes(1));
    expect(onSubmitted.mock.calls[0]![0][0]).toMatchObject({
      designReference: {
        kind: "sketch",
        width: 960,
        height: 600,
        image: { kind: "inline", mediaType: "image/png" },
      },
    });
  });

  it("lets the active canvas tool consume Escape before closing the design board", async () => {
    render(<A3STestKit enabled page={{ id: "design-reference-escape" }} repairStorage="memory"><button id="escape-target">Old card</button><A3SReviewOverlay enabled defaultOpen /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());
    const target = document.querySelector<HTMLElement>("#escape-target")!;
    setRect(target, { x: 80, y: 90, width: 240, height: 120 });
    fireEvent.click(shadowButton("Element"));
    target.dispatchEvent(pointerEventWithPath(target, 120, 110));
    fireEvent.click(await waitFor(() => shadowButton("Open design board")));

    const canvas = await waitFor(() => shadowQuery("[data-testid='design-canvas']"));
    fireEvent.click(shadowQuery("[data-testid='design-tool-draw']"));
    fireEvent.keyDown(canvas, { key: "Escape" });
    expect(shadowQuery(".a3s-design-board")).toBeTruthy();
    fireEvent.keyDown(canvas, { key: "Escape" });
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());
  });

  it("uploads a screenshot design reference to a selected element", async () => {
    vi.spyOn(window, "Image").mockImplementation(() => {
      const image = document.createElement("img");
      Object.defineProperty(image, "naturalWidth", { value: 800 });
      Object.defineProperty(image, "naturalHeight", { value: 500 });
      Object.defineProperty(image, "src", {
        configurable: true,
        set: () => queueMicrotask(() => image.onload?.(new Event("load"))),
      });
      return image;
    });

    const onSubmitted = vi.fn();
    render(<A3STestKit enabled page={{ id: "screenshot-design-reference" }} repairStorage="memory"><button id="screenshot-target">Old panel</button><A3SReviewOverlay enabled defaultOpen onSubmitted={onSubmitted} /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());
    const target = document.querySelector<HTMLElement>("#screenshot-target")!;
    setRect(target, { x: 80, y: 90, width: 240, height: 120 });
    fireEvent.click(shadowButton("Element"));
    target.dispatchEvent(pointerEventWithPath(target, 120, 110));

    fireEvent.click(await waitFor(() => shadowButton("Open design board")));
    const input = await waitFor(() => shadowQuery("input[type=file]"));
    expect(shadowQuery(".a3s-design-board").getAttribute("aria-busy")).toBe("false");
    fireEvent.change(input, {
      target: {
        files: [new File([new Uint8Array([1, 2, 3])], "desired-ui.png", { type: "image/png" })],
      },
    });
    await waitFor(() => expect(shadowQuery(".a3s-design-status").textContent).toContain("Screenshot."));
    expect(shadowButton("Undo").disabled).toBe(false);
    fireEvent.click(shadowButton("Attach to finding"));

    await waitFor(() => expect(shadowQuery(".a3s-design-reference").textContent).toContain("Screenshot attached"));
    expect(shadowQuery(".a3s-design-reference-actions").querySelectorAll("button")).toHaveLength(2);
    fireEvent.change(shadowQuery(".a3s-editor textarea"), { target: { value: "Match the attached screenshot" } });
    fireEvent.click(shadowButton("Send and auto-fix"));
    await waitFor(() => expect(onSubmitted).toHaveBeenCalledTimes(1));
    expect(onSubmitted.mock.calls[0]![0][0]).toMatchObject({
      designReference: {
        kind: "screenshot",
        width: 960,
        height: 600,
        image: { kind: "inline", mediaType: "image/jpeg" },
      },
    });
  });

  it("captures a selected browser region without requesting screen-sharing permission", async () => {
    let decodedImages = 0;
    vi.spyOn(window, "Image").mockImplementation(() => {
      const image = document.createElement("img");
      const viewportImage = decodedImages++ === 0;
      Object.defineProperty(image, "naturalWidth", { value: viewportImage ? 1000 : 320 });
      Object.defineProperty(image, "naturalHeight", { value: viewportImage ? 800 : 300 });
      Object.defineProperty(image, "src", {
        configurable: true,
        set: () => queueMicrotask(() => image.onload?.(new Event("load"))),
      });
      return image;
    });
    const drawImage = vi.fn();
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue({ drawImage } as unknown as CanvasRenderingContext2D);
    domToJpeg.mockReset();
    domToJpeg.mockResolvedValue("data:image/jpeg;base64,AQIDBA==");
    const getDisplayMedia = vi.fn();
    Object.defineProperty(navigator, "mediaDevices", {
      configurable: true,
      value: { getDisplayMedia },
    });

    render(<A3STestKit enabled page={{ id: "browser-content-capture" }} repairStorage="memory"><button id="browser-content-target">Old panel</button><A3SReviewOverlay enabled defaultOpen /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());
    const target = document.querySelector<HTMLElement>("#browser-content-target")!;
    setRect(target, { x: 80, y: 90, width: 240, height: 120 });
    fireEvent.click(shadowButton("Element"));
    target.dispatchEvent(pointerEventWithPath(target, 120, 110));
    fireEvent.click(await waitFor(() => shadowButton("Open design board")));

    const capture = shadowButton("Select screenshot area");
    expect(capture.title).toContain("without screen-sharing permission");
    fireEvent.click(capture);
    const selector = await waitFor(() => shadowQuery(".a3s-page-capture"));
    expect(selector.getAttribute("role")).toBe("dialog");
    expect(selector.dataset.state).toBe("idle");
    expect(selector.textContent).toContain("Drag to select a screenshot area");
    expect(domToJpeg).not.toHaveBeenCalled();

    fireEvent(selector, capturePointerEvent("pointerdown", selector, 100, 150, 1));
    fireEvent(selector, capturePointerEvent("pointerup", selector, 105, 155, 1));
    expect(selector.dataset.state).toBe("error");
    expect(selector.textContent).toContain("Drag a larger area to capture it");

    fireEvent.keyDown(selector, { key: "Escape" });
    await waitFor(() => expect(document.querySelector<HTMLElement>("[data-a3s-testkit-overlay]")!.shadowRoot!.querySelector(".a3s-page-capture")).toBeNull());
    expect(domToJpeg).not.toHaveBeenCalled();

    fireEvent.click(shadowButton("Select screenshot area"));
    const activeSelector = await waitFor(() => shadowQuery(".a3s-page-capture"));
    fireEvent(activeSelector, capturePointerEvent("pointerdown", activeSelector, 100, 150, 1));
    fireEvent(activeSelector, capturePointerEvent("pointermove", activeSelector, 420, 450, 1));
    expect(shadowQuery(".a3s-page-capture-size").textContent).toBe("320 × 300");
    fireEvent(activeSelector, capturePointerEvent("pointerup", activeSelector, 420, 450, 1));

    await waitFor(() => expect(domToJpeg).toHaveBeenCalledTimes(1));
    const [captureRoot, options] = domToJpeg.mock.calls[0]!;
    expect(captureRoot).toBe(document.documentElement);
    expect(options).toMatchObject({ width: 1000, height: 800, quality: 0.9, scale: 1 });
    expect(options.filter(document.querySelector("[data-a3s-testkit-overlay]")!)).toBe(false);
    expect(options.filter(target)).toBe(true);
    await waitFor(() => expect(drawImage).toHaveBeenCalledWith(
      expect.any(HTMLImageElement),
      100,
      150,
      320,
      300,
      0,
      0,
      320,
      300,
    ));
    expect(getDisplayMedia).not.toHaveBeenCalled();
    await waitFor(() => expect(shadowQuery(".a3s-design-status").textContent).toContain("Screenshot."));
  });

  it("mounts the native design tools inside the Test Kit shadow root", async () => {
    render(<A3STestKit enabled page={{ id: "native-design-tools" }} repairStorage="memory"><button id="native-design-target">Old panel</button><A3SReviewOverlay enabled defaultOpen /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());
    const target = document.querySelector<HTMLElement>("#native-design-target")!;
    setRect(target, { x: 80, y: 90, width: 240, height: 120 });
    fireEvent.click(shadowButton("Element"));
    target.dispatchEvent(pointerEventWithPath(target, 120, 110));
    fireEvent.click(await waitFor(() => shadowButton("Open design board")));
    expect(await waitFor(() => shadowQuery("[data-testid='design-canvas']"))).toBeTruthy();
    expect(shadowQuery("[data-testid='design-tool-draw']")).toBeTruthy();
    expect(shadowQuery("[data-testid='design-tool-rectangle']")).toBeTruthy();
    expect(shadowQuery("[data-testid='design-tool-text']")).toBeTruthy();
    const toolbar = shadowQuery(".a3s-design-toolbar");
    expect(toolbar.dataset.wrap).toBe("true");
    expect(toolbar.querySelectorAll(":scope > [role='group']")).toHaveLength(2);
    expect(toolbar.querySelectorAll(":scope > hr[role='separator']")).toHaveLength(1);
    expect(shadowQuery(".a3s-design-tools").querySelectorAll("button")).toHaveLength(4);
    expect(document.querySelector<HTMLElement>("[data-a3s-testkit-overlay]")!.shadowRoot!.querySelector(".a3s-design-history")).toBeNull();
    expect(document.querySelector<HTMLElement>("[data-a3s-testkit-overlay]")!.shadowRoot!.querySelector(".a3s-design-style")).toBeNull();
    fireEvent.click(shadowQuery("[data-testid='design-tool-draw']"));
    expect(shadowQuery(".a3s-design-style").getAttribute("role")).toBe("group");
    expect(document.querySelector<HTMLElement>("[data-a3s-testkit-overlay]")!.shadowRoot!.querySelector("style")!.textContent).toContain(".a3s-design-canvas-surface");
  });

  it("creates, moves, resizes, and deletes native design objects", async () => {
    render(<A3STestKit enabled page={{ id: "native-design-objects" }} repairStorage="memory"><button id="native-object-target">Old panel</button><A3SReviewOverlay enabled defaultOpen /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());
    const target = document.querySelector<HTMLElement>("#native-object-target")!;
    setRect(target, { x: 80, y: 90, width: 240, height: 120 });
    fireEvent.click(shadowButton("Element"));
    target.dispatchEvent(pointerEventWithPath(target, 120, 110));
    fireEvent.click(await waitFor(() => shadowButton("Open design board")));
    const canvas = await waitFor(() => shadowQuery("[data-testid='design-canvas']"));
    setRect(canvas, { x: 0, y: 0, width: 960, height: 600 });

    fireEvent.click(shadowQuery("[data-testid='design-tool-rectangle']"));
    canvas.dispatchEvent(pointerEvent("pointerdown", canvas, 100, 100));
    canvas.dispatchEvent(pointerEvent("pointerup", canvas, 300, 220));
    await waitFor(() => expect(shadowQuery("[aria-label='Design object count']").textContent).toBe("1/250"));

    fireEvent.click(shadowQuery("[data-testid='design-tool-select']"));
    const rectangle = shadowQuery(".a3s-design-element.is-rectangle");
    rectangle.dispatchEvent(pointerEvent("pointerdown", rectangle, 150, 150));
    canvas.dispatchEvent(pointerEvent("pointermove", canvas, 250, 250));
    canvas.dispatchEvent(pointerEvent("pointerup", canvas, 250, 250));
    await waitFor(() => expect(Number(shadowQuery(".a3s-design-element.is-rectangle > rect").getAttribute("x"))).toBeCloseTo(200));

    const resize = shadowQuery("[data-resize-id]");
    resize.dispatchEvent(pointerEvent("pointerdown", resize, 404, 324));
    canvas.dispatchEvent(pointerEvent("pointerup", canvas, 500, 400));
    await waitFor(() => expect(Number(shadowQuery(".a3s-design-element.is-rectangle > rect").getAttribute("width"))).toBeCloseTo(300));

    fireEvent.click(shadowQuery("[data-testid='design-tool-text']"));
    canvas.dispatchEvent(pointerEvent("pointerdown", canvas, 420, 80));
    const textInput = await waitFor(() => shadowQuery("[aria-label='Design text']"));
    fireEvent.change(textInput, { target: { value: "New card" } });
    fireEvent.keyDown(textInput, { key: "Enter" });
    await waitFor(() => expect(shadowQuery(".a3s-design-element.is-text").textContent).toContain("New card"));
    expect(shadowQuery("[aria-label='Design object count']").textContent).toBe("2/250");

    fireEvent.keyDown(canvas, { key: "Delete" });
    await waitFor(() => expect(shadowQuery("[aria-label='Design object count']").textContent).toBe("1/250"));
  });

  it("requires an explicit save or send before removing each contract finding", async () => {
    render(<A3STestKit enabled page={{ id: "quality-review" }} repairStorage="memory"><button data-testid="quality-target">Checkout action</button><A3SReviewOverlay enabled defaultOpen /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());
    const bridge = getPageContextBridge()!;
    const target = document.querySelector<HTMLElement>("[data-testid=quality-target]")!;
    setRect(target, { x: 40, y: 60, width: 160, height: 36 });
    const nodeId = bridge.snapshot().nodes.find((node) => node.testId === "quality-target")!.id;
    const report: QualityReport = {
      contract: "checkout",
      variant: "desktop",
      state: "ready",
      outcome: "failed",
      observation_revision: 1,
      matches: [],
      findings: [
        {
          id: "finding:located",
          dimension: "design_conformance",
          rule_id: "contract.element.role",
          severity: "blocking",
          message: "Use the contracted role",
          expected: "button",
          actual: "link",
          element_id: "checkout-action",
          observed_node_id: nodeId,
          confidence: 100,
        },
        {
          id: "finding:missing",
          dimension: "design_conformance",
          rule_id: "contract.element.required",
          severity: "important",
          message: "Add the required checkout action",
          expected: true,
          actual: false,
          element_id: "missing-action",
          confidence: 100,
        },
      ],
    };

    expect(bridge.reportQuality(report)).toBe(true);
    await waitFor(() => expect(shadowQuery(".a3s-quality").textContent).toContain("Use the contracted role"));
    const qualityItem = shadowQuery(".a3s-quality-item");
    expect(qualityItem.querySelector("header .a3s-status")?.textContent).toBe("Blocking");
    expect(qualityItem.querySelector("footer")?.querySelectorAll("button")).toHaveLength(2);
    fireEvent.click(shadowQuery("[aria-label='Review contract finding: Use the contracted role']"));
    await waitFor(() => expect(shadowQuery(".a3s-editor").textContent).toContain("checkout-action"));
    expect(bridge.listQualityReports()[0]!.findings).toHaveLength(2);
    fireEvent.click(shadowButton("Cancel"));
    expect(bridge.listQualityReports()[0]!.findings).toHaveLength(2);

    fireEvent.click(shadowQuery("[aria-label='Review contract finding: Use the contracted role']"));
    fireEvent.click(shadowButton("Add draft"));
    await waitFor(() => expect(bridge.listQualityReports()[0]!.findings.map((finding) => finding.id)).toEqual(["finding:missing"]));
    expect(bridge.listRepairs()).toEqual([]);

    fireEvent.click(shadowButton("Choose target"));
    fireEvent.click(shadowButton("Cancel"));
    expect(bridge.listQualityReports()[0]!.findings[0]!.id).toBe("finding:missing");
    fireEvent.click(shadowButton("Choose target"));
    target.dispatchEvent(pointerEventWithPath(target, 60, 70));
    await waitFor(() => expect(shadowQuery(".a3s-editor").textContent).toContain("missing-action"));
    fireEvent.click(shadowButton("Send and auto-fix"));
    await waitFor(() => expect(bridge.listQualityReports()).toEqual([]));
    expect(bridge.listRepairs()).toHaveLength(1);
    expect(bridge.listRepairs()[0]).toMatchObject({
      instruction: "Add the required checkout action",
      status: "queued",
    });
  });

  it("requires human promotion before an advisory design suggestion enters repair", async () => {
    render(<A3STestKit enabled page={{ id: "design-review" }} repairStorage="memory"><button data-testid="design-target">Complete purchase</button><A3SReviewOverlay enabled defaultOpen /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());
    const bridge = getPageContextBridge()!;
    const target = document.querySelector<HTMLElement>("[data-testid=design-target]")!;
    setRect(target, { x: 80, y: 120, width: 180, height: 44 });
    const snapshot = bridge.snapshot();
    const nodeId = snapshot.nodes.find((node) => node.testId === "design-target")!.id;
    const report: DesignAuditReport = {
      protocol: "a3s.test.design-audit-report/1",
      provenance: {
        identity: { provider: "fixture", model: "design-review" },
        observation_id: 9,
        surface_revision: snapshot.revision,
        screenshot_sha256: `sha256:${"a".repeat(64)}`,
        page_context_sha256: `sha256:${"b".repeat(64)}`,
        width: 1280,
        height: 720,
        usage: { input_units: 12, output_units: 3, cost_microusd: 25 },
        request_id: "design-review-1",
        authority: "advisory",
      },
      dimensions: ["visual_hierarchy"],
      findings: [{
        id: "design:primary-action",
        dimension: "visual_hierarchy",
        priority: "high",
        summary: "The primary action lacks emphasis",
        rationale: "Nearby controls have the same visual weight",
        recommendation: "Increase the primary action contrast and surrounding space",
        confidence: 92,
        target: { kind: "node", node_id: nodeId },
      }],
    };

    expect(bridge.reportDesignAudit(report)).toBe(true);
    await waitFor(() => expect(shadowQuery(".a3s-design-audit").textContent).toContain("The primary action lacks emphasis"));
    const designAuditItem = shadowQuery(".a3s-design-audit .a3s-quality-item");
    expect(designAuditItem.querySelector("header .a3s-status")?.textContent).toBe("high");
    expect(designAuditItem.querySelector("footer")?.querySelectorAll("button")).toHaveLength(2);
    expect(bridge.listRepairs()).toEqual([]);
    fireEvent.click(shadowQuery("[aria-label='Review design suggestion: The primary action lacks emphasis']"));
    await waitFor(() => expect((shadowQuery("textarea") as HTMLTextAreaElement).value).toContain("Increase the primary action contrast"));
    expect(bridge.listDesignAuditReports()).toHaveLength(1);
    fireEvent.click(shadowButton("Cancel"));
    expect(bridge.listDesignAuditReports()).toHaveLength(1);

    fireEvent.click(shadowQuery("[aria-label='Review design suggestion: The primary action lacks emphasis']"));
    fireEvent.click(shadowButton("Send and auto-fix"));
    await waitFor(() => expect(bridge.listDesignAuditReports()).toEqual([]));
    expect(bridge.listRepairs()).toHaveLength(1);
    expect(bridge.listRepairs()[0]).toMatchObject({
      instruction: "Increase the primary action contrast and surrounding space",
      intent: "change",
      severity: "important",
      status: "queued",
    });
  });

  it("submits selected drafts in visible order as one batch", async () => {
    const onSubmitted = vi.fn();
    render(<A3STestKit enabled page={{ id: "batch" }} repairStorage="memory"><button id="one">One</button><button id="two">Two</button><A3SReviewOverlay enabled defaultOpen onSubmitted={onSubmitted} /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());
    for (const [selector, instruction] of [["#one", "Fix one"], ["#two", "Fix two"]] as const) {
      const target = document.querySelector<HTMLElement>(selector)!;
      setRect(target, { x: 10, y: 10, width: 40, height: 20 });
      fireEvent.click(shadowButton("New feedback"));
      fireEvent.click(shadowButton("Element"));
      target.dispatchEvent(pointerEventWithPath(target, 20, 15));
      await waitFor(() => expect(shadowQuery("textarea")).toBeTruthy());
      fireEvent.change(shadowQuery("textarea"), { target: { value: instruction } });
      if (selector === "#two") {
        fireEvent.click(shadowQuery(".a3s-editor-details"));
        const conflict = shadowQuery(".a3s-conflicts input");
        fireEvent.click(conflict);
      }
      const addDraft = Array.from(document.querySelector<HTMLElement>("[data-a3s-testkit-overlay]")!.shadowRoot!.querySelectorAll("button")).find((button) => button.textContent === "Add draft")!;
      fireEvent.click(addDraft);
      await waitFor(() => expect(shadowQuery(".a3s-list").textContent).toContain(instruction));
    }
    const sendSelected = Array.from(document.querySelector<HTMLElement>("[data-a3s-testkit-overlay]")!.shadowRoot!.querySelectorAll("button")).find((button) => button.textContent === "Send selected (2)")!;
    fireEvent.click(sendSelected);
    await waitFor(() => expect(onSubmitted).toHaveBeenCalledTimes(1));
    expect(onSubmitted.mock.calls[0]![0].map((repair: { instruction: string }) => repair.instruction)).toEqual(["Fix one", "Fix two"]);
    expect(new Set(onSubmitted.mock.calls[0]![0].map((repair: { batchId: string }) => repair.batchId)).size).toBe(1);
    expect(onSubmitted.mock.calls[0]![0][1].relations).toEqual([
      { kind: "conflicts_with", findingId: onSubmitted.mock.calls[0]![0][0].id },
    ]);
  });

  it("repositions multi-selected nodes from their live DOM rectangles while scrolling", async () => {
    render(<A3STestKit enabled page={{ id: "multi" }} repairStorage="memory"><div id="scroll-container"><button id="one">One</button><button id="two">Two</button></div><A3SReviewOverlay enabled defaultOpen /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());
    const first = document.querySelector<HTMLElement>("#one")!;
    const second = document.querySelector<HTMLElement>("#two")!;
    let firstRect = DOMRect.fromRect({ x: 10, y: 10, width: 40, height: 20 });
    let secondRect = DOMRect.fromRect({ x: 70, y: 10, width: 40, height: 20 });
    Object.defineProperty(first, "getBoundingClientRect", { configurable: true, value: () => firstRect });
    Object.defineProperty(first, "getClientRects", { configurable: true, value: () => [firstRect] });
    Object.defineProperty(second, "getBoundingClientRect", { configurable: true, value: () => secondRect });
    Object.defineProperty(second, "getClientRects", { configurable: true, value: () => [secondRect] });

    fireEvent.click(shadowButton("Multi"));
    document.body.dispatchEvent(pointerEvent("pointerdown", document.body, 0, 0));
    document.body.dispatchEvent(pointerEvent("pointermove", document.body, 120, 40));
    document.body.dispatchEvent(pointerEvent("pointerup", document.body, 120, 40));
    await waitFor(() => expect(shadowQuery(".a3s-editor").textContent).toContain("2 elements"));
    expect(shadowQuery(".a3s-highlight.is-candidate").style.cssText).toContain("left: 10px");
    expect(shadowQuery(".a3s-highlight.is-candidate").style.cssText).toContain("width: 100px");

    firstRect = DOMRect.fromRect({ x: 10, y: -70, width: 40, height: 20 });
    secondRect = DOMRect.fromRect({ x: 70, y: -70, width: 40, height: 20 });
    fireEvent.scroll(document.querySelector("#scroll-container")!);
    await waitFor(() => expect(shadowQuery(".a3s-highlight.is-candidate").style.top).toBe("-70px"));

    fireEvent.change(shadowQuery("textarea"), { target: { value: "Align both actions" } });
    fireEvent.click(shadowButton("Add draft"));
    await waitFor(() => expect(shadowQuery(".a3s-markers").children).toHaveLength(1));
    expect(shadowQuery(".a3s-markers").querySelectorAll(".a3s-marker-action")).toHaveLength(1);
    expect((shadowQuery(".a3s-marker") as HTMLElement).style.cssText).toContain("width: 100px");

    firstRect = DOMRect.fromRect({ x: 10, y: -130, width: 40, height: 20 });
    secondRect = DOMRect.fromRect({ x: 70, y: -130, width: 40, height: 20 });
    fireEvent.scroll(document.querySelector("#scroll-container")!);
    await waitFor(() => expect(shadowQuery(".a3s-marker").style.top).toBe("-130px"));

    fireEvent.click(shadowButton("Edit"));
    fireEvent.change(shadowQuery("textarea"), { target: { value: "Align both primary actions" } });
    fireEvent.click(shadowButton("Save changes"));
    await waitFor(() => expect(shadowQuery(".a3s-list").textContent).toContain("Align both primary actions"));

    fireEvent.click(shadowButton("Hide marker"));
    expect(shadowQuery(".a3s-markers").children).toHaveLength(0);
    fireEvent.click(shadowButton("Reopen marker"));
    await waitFor(() => expect(shadowQuery(".a3s-markers").children).toHaveLength(1));
  });

  it("hides markers while the review overlay is closed", async () => {
    render(<A3STestKit enabled page={{ id: "closed-markers" }} repairStorage="memory"><button id="closed-marker-target">Target</button><A3SReviewOverlay enabled defaultOpen /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());
    const target = document.querySelector<HTMLElement>("#closed-marker-target")!;
    setRect(target, { x: 20, y: 80, width: 120, height: 32 });

    fireEvent.click(shadowButton("Element"));
    target.dispatchEvent(pointerEventWithPath(target, 30, 90));
    fireEvent.change(await waitFor(() => shadowQuery(".a3s-editor textarea")), { target: { value: "Keep the marker scoped to the open overlay" } });
    fireEvent.click(shadowButton("Add draft"));
    await waitFor(() => expect(shadowQuery(".a3s-markers").children).toHaveLength(1));

    fireEvent.click(shadowQuery(".a3s-launch"));
    await waitFor(() => expect(shadowQuery(".a3s-markers").children).toHaveLength(0));
  });

  it("refreshes marker geometry after the page scrolls", async () => {
    render(<A3STestKit enabled page={{ id: "scrolling-markers" }} repairStorage="memory"><button id="scrolling-marker-target">Target</button><A3SReviewOverlay enabled defaultOpen /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());
    const target = document.querySelector<HTMLElement>("#scrolling-marker-target")!;
    let rect = DOMRect.fromRect({ x: 24, y: 180, width: 140, height: 36 });
    Object.defineProperty(target, "getBoundingClientRect", { configurable: true, value: () => rect });
    Object.defineProperty(target, "getClientRects", { configurable: true, value: () => [rect] });

    fireEvent.click(shadowButton("Element"));
    target.dispatchEvent(pointerEvent("pointermove", target, 40, 190));
    await waitFor(() => expect(shadowQuery(".a3s-root").querySelector(".a3s-highlight:not(.is-candidate)")).toBeTruthy());
    target.dispatchEvent(pointerEventWithPath(target, 40, 190));
    await waitFor(() => expect(shadowQuery(".a3s-highlight.is-candidate").style.top).toBe("180px"));
    expect(shadowQuery(".a3s-root").querySelector(".a3s-highlight:not(.is-candidate)")).toBeNull();

    rect = DOMRect.fromRect({ x: 24, y: 60, width: 140, height: 36 });
    fireEvent.scroll(window);
    await waitFor(() => expect(shadowQuery(".a3s-highlight.is-candidate").style.top).toBe("60px"));

    fireEvent.change(await waitFor(() => shadowQuery(".a3s-editor textarea")), { target: { value: "Keep this marker aligned while scrolling" } });
    fireEvent.click(shadowButton("Add draft"));
    await waitFor(() => expect(shadowQuery(".a3s-marker").style.top).toBe("60px"));

    rect = DOMRect.fromRect({ x: 24, y: 24, width: 140, height: 36 });
    fireEvent.scroll(window);
    await waitFor(() => expect(shadowQuery(".a3s-marker").style.top).toBe("24px"));
    expect(shadowQuery(".a3s-marker-action").dataset.tooltipAlign).toBe("start");
    expect(shadowQuery(".a3s-marker-action").dataset.tooltipSide).toBe("bottom");

    rect = DOMRect.fromRect({ x: 900, y: 760, width: 80, height: 30 });
    fireEvent.scroll(window);
    await waitFor(() => expect(shadowQuery(".a3s-marker").style.left).toBe("900px"));
    expect(shadowQuery(".a3s-marker-action").dataset.tooltipAlign).toBe("end");
    expect(shadowQuery(".a3s-marker-action").dataset.tooltipSide).toBe("top");
  });

  it("restores page-local drafts and semantic targets after a React reload", async () => {
    const first = render(<A3STestKit enabled page={{ id: "restored-review" }} repairStorage="memory"><main><button data-testid="restored-target">Restored target</button></main><A3SReviewOverlay enabled defaultOpen /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());
    const firstTarget = document.querySelector<HTMLElement>("[data-testid=restored-target]")!;
    setRect(firstTarget, { x: 20, y: 40, width: 140, height: 32 });
    fireEvent.click(shadowButton("Element"));
    firstTarget.dispatchEvent(pointerEventWithPath(firstTarget, 30, 50));
    fireEvent.change(await waitFor(() => shadowQuery(".a3s-editor textarea")), { target: { value: "Keep this draft across reload" } });
    fireEvent.click(shadowButton("Add draft"));
    await waitFor(() => expect(window.localStorage.length).toBe(1));
    first.unmount();

    render(<A3STestKit enabled page={{ id: "restored-review" }} repairStorage="memory"><main><p>New sibling changes private node ordering</p><button data-testid="restored-target">Restored target</button></main><A3SReviewOverlay enabled defaultOpen /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-list").textContent).toContain("Keep this draft across reload"));
    const restoredTarget = document.querySelector<HTMLElement>("[data-testid=restored-target]")!;
    setRect(restoredTarget, { x: 60, y: 90, width: 160, height: 36 });
    await waitFor(() => expect(shadowQuery(".a3s-markers").children).toHaveLength(1));
  });

  it("switches persisted drafts when an SPA route changes and restores them on return", async () => {
    window.history.replaceState(null, "", "/profile");
    render(<A3STestKit enabled page={{ id: "spa-review" }} repairStorage="memory"><button data-testid="spa-target">SPA target</button><A3SReviewOverlay enabled defaultOpen /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());
    const target = document.querySelector<HTMLElement>("[data-testid=spa-target]")!;
    setRect(target, { x: 30, y: 40, width: 120, height: 30 });
    fireEvent.click(shadowButton("Element"));
    target.dispatchEvent(pointerEventWithPath(target, 40, 50));
    fireEvent.change(await waitFor(() => shadowQuery(".a3s-editor textarea")), { target: { value: "Profile-only draft" } });
    fireEvent.click(shadowButton("Add draft"));
    await waitFor(() => expect(shadowQuery(".a3s-list").textContent).toContain("Profile-only draft"));

    window.history.pushState(null, "", "/security");
    await waitFor(() => expect(shadowButton("New feedback").getAttribute("aria-pressed")).toBe("true"));
    fireEvent.click(shadowButton("Findings"));
    expect(shadowQuery(".a3s-list").textContent).not.toContain("Profile-only draft");
    window.history.pushState(null, "", "/profile");
    await waitFor(() => expect(shadowQuery(".a3s-list").textContent).toContain("Profile-only draft"));
  });

  it("captures a bounded freehand finding and cycles manual themes", async () => {
    render(<A3STestKit enabled page={{ id: "draw" }} repairStorage="memory"><button id="target">Target</button><A3SReviewOverlay enabled defaultOpen /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());
    setRect(document.querySelector("#target")!, { x: 20, y: 20, width: 80, height: 40 });

    fireEvent.click(shadowQuery("[aria-label='Review preferences']"));
    fireEvent.change(shadowQuery("[aria-label='Overlay theme']"), { target: { value: "light" } });
    expect(shadowQuery(".a3s-root").dataset.theme).toBe("light");
    fireEvent.change(shadowQuery("[aria-label='Overlay theme']"), { target: { value: "dark" } });
    expect(shadowQuery(".a3s-root").dataset.theme).toBe("dark");

    fireEvent.click(shadowButton("New feedback"));
    fireEvent.click(shadowButton("Draw"));
    document.body.dispatchEvent(pointerEvent("pointerdown", document.body, 10, 10));
    document.body.dispatchEvent(pointerEvent("pointermove", document.body, 50, 25));
    await waitFor(() => expect(shadowQuery(".a3s-drawing")).toBeTruthy());
    document.body.dispatchEvent(pointerEvent("pointerup", document.body, 110, 70));
    await waitFor(() => expect(shadowQuery(".a3s-editor")).toBeTruthy());
    fireEvent.change(shadowQuery("textarea"), { target: { value: "Fix this drawn region" } });
    fireEvent.click(shadowButton("Send and auto-fix"));
    await waitFor(() => {
      const repair = getPageContextBridge()?.listRepairs()[0];
      expect(repair?.target.kind).toBe("drawing");
      expect(repair?.target.drawing?.length).toBeGreaterThanOrEqual(3);
      expect(repair?.target.region).toEqual({ x: 10, y: 10, width: 100, height: 60 });
    });
  });

  it("creates typed layout placements and rearrangements with pointer and keyboard input", async () => {
    const onSubmitted = vi.fn();
    render(<A3STestKit enabled page={{ id: "layout-mode" }} repairStorage="memory"><section id="layout-hero" data-testid="layout-hero" tabIndex={-1}>Hero section</section><A3SReviewOverlay enabled defaultOpen onSubmitted={onSubmitted} /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());
    const hero = document.querySelector<HTMLElement>("#layout-hero")!;
    setRect(hero, { x: 20, y: 30, width: 600, height: 220 });

    fireEvent.click(shadowButton("Layout"));
    expect(shadowButton("Layout").getAttribute("aria-pressed")).toBe("true");
    fireEvent.change(shadowQuery("[aria-label='Layout purpose']"), { target: { value: "Developer tool landing page" } });
    fireEvent.change(shadowQuery("[aria-label='Layout canvas']"), { target: { value: "wireframe" } });
    fireEvent.change(shadowQuery("[aria-label='Layout component type']"), { target: { value: "Pricing section" } });
    expect(shadowQuery(".a3s-wireframe")).toBeTruthy();

    fireEvent.click(shadowButton("Draw placement"));
    document.body.dispatchEvent(pointerEvent("pointerdown", document.body, 80, 300));
    document.body.dispatchEvent(pointerEvent("pointermove", document.body, 720, 560));
    document.body.dispatchEvent(pointerEvent("pointerup", document.body, 720, 560));
    await waitFor(() => expect(shadowQuery(".a3s-editor textarea").getAttribute("value") ?? (shadowQuery(".a3s-editor textarea") as HTMLTextAreaElement).value).toContain("Pricing section"));
    fireEvent.click(shadowButton("Add draft"));
    await waitFor(() => expect(shadowQuery(".a3s-list").textContent).toContain("Place Pricing section"));

    fireEvent.click(shadowButton("New feedback"));
    hero.focus();
    hero.dispatchEvent(new FocusEvent("focusin", { bubbles: true, composed: true }));
    fireEvent.click(shadowButton("Select section on page"));
    await waitFor(() => expect(document.activeElement).toBe(hero));
    fireEvent.keyDown(hero, { key: "Enter" });
    await waitFor(() => expect(shadowQuery(".a3s-layout-source").textContent).toContain("Hero section"));
    for (const [label, value] of [["Layout x", "40"], ["Layout y", "80"], ["Layout width", "600"], ["Layout height", "220"]] as const) {
      fireEvent.change(shadowQuery(`[aria-label='${label}']`), { target: { value } });
    }
    fireEvent.click(shadowButton("Create rearrange draft"));
    await waitFor(() => expect((shadowQuery(".a3s-editor textarea") as HTMLTextAreaElement).value).toContain("Move Hero section"));
    fireEvent.click(shadowButton("Add draft"));
    await waitFor(() => expect(shadowButton("Send selected (2)")).toBeTruthy());
    fireEvent.click(shadowButton("Send selected (2)"));

    await waitFor(() => expect(onSubmitted).toHaveBeenCalledTimes(1));
    const submitted = onSubmitted.mock.calls[0]![0];
    expect(submitted.map((repair: { target: { layout?: { kind: string } } }) => repair.target.layout?.kind)).toEqual(["placement", "rearrange"]);
    expect(submitted[0].target).toMatchObject({
      kind: "region",
      nodeIds: [],
      region: { x: 80, y: 300, width: 640, height: 260 },
      layout: { kind: "placement", componentType: "Pricing section", canvas: "wireframe", purpose: "Developer tool landing page" },
    });
    expect(submitted[1].target).toMatchObject({
      kind: "node",
      region: { x: 40, y: 80, width: 600, height: 220 },
      layout: { kind: "rearrange", originalRegion: { x: 20, y: 30, width: 600, height: 220 }, purpose: "Developer tool landing page" },
    });
    expect(hero.getAttribute("style")).toBeNull();
  });

  it("filters a categorized component catalog and retains free-form Layout input", async () => {
    render(<A3STestKit enabled page={{ id: "layout-catalog" }} repairStorage="memory"><A3SReviewOverlay enabled defaultOpen /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());

    fireEvent.click(shadowButton("Layout"));
    const catalog = shadowQuery(".a3s-catalog");
    expect(Number(catalog.dataset.componentCount)).toBeGreaterThanOrEqual(65);
    const catalogToggle = shadowButton("Component catalog · 90");
    expect(catalogToggle.querySelector(".a3s-catalog-icon svg")).toBeTruthy();
    expect(catalogToggle.getAttribute("aria-label")).toBe("Component catalog · 90");
    expect(catalogToggle.getAttribute("aria-expanded")).toBe("false");
    fireEvent.click(catalogToggle);
    expect(catalogToggle.getAttribute("aria-expanded")).toBe("true");
    expect(shadowQuery(".a3s-catalog-search-control svg")).toBeTruthy();
    fireEvent.change(shadowQuery("[aria-label='Search component catalog']"), { target: { value: "checkout" } });
    expect(shadowQuery(".a3s-catalog-results").textContent).toContain("Checkout Form");
    expect(shadowQuery(".a3s-catalog-results").textContent).not.toContain("Breadcrumbs");
    fireEvent.click(shadowButton("Checkout Form"));
    expect((shadowQuery("[aria-label='Layout component type']") as HTMLInputElement).value).toBe("Checkout Form");

    fireEvent.change(shadowQuery("[aria-label='Search component catalog']"), { target: { value: "no-such-component" } });
    expect(shadowQuery(".a3s-catalog-empty").textContent).toContain("No catalog matches");
    fireEvent.change(shadowQuery("[aria-label='Layout component type']"), { target: { value: "Custom orbit panel" } });
    expect((shadowQuery("[aria-label='Layout component type']") as HTMLInputElement).value).toBe("Custom orbit panel");
    expect(shadowButton("Draw placement").disabled).toBe(false);
  });

  it("copies bounded Markdown and JSON repair exports", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText } });
    render(<A3STestKit enabled page={{ id: "copy" }} repairStorage="memory"><button id="copy-target">Copy target</button><A3SReviewOverlay enabled defaultOpen /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());
    const target = document.querySelector<HTMLElement>("#copy-target")!;
    setRect(target, { x: 10, y: 10, width: 40, height: 20 });
    fireEvent.click(shadowButton("Element"));
    target.dispatchEvent(pointerEvent("pointerup", target, 20, 15));
    await waitFor(() => expect(shadowQuery("textarea")).toBeTruthy());
    fireEvent.change(shadowQuery("textarea"), { target: { value: "Copy this finding" } });
    fireEvent.click(shadowButton("Add draft"));
    fireEvent.click(shadowButton("Copy Markdown"));
    await waitFor(() => expect(writeText).toHaveBeenCalledTimes(1));
    const markdown = writeText.mock.calls[0]![0];
    expect(markdown).toContain("# A3S Test repair findings");
    expect(markdown).toContain("Copy this finding");
    expect(markdown).toContain("untrusted evidence");
    fireEvent.click(shadowButton("Copy JSON"));
    await waitFor(() => expect(writeText).toHaveBeenCalledTimes(2));
    const copied = JSON.parse(writeText.mock.calls[1]![0]);
    expect(copied).toMatchObject({
      protocol: "a3s.test.repair/1",
      page: { id: "copy" },
      findings: [{ instruction: "Copy this finding", context: { untrusted: true } }],
    });
  });

  it("provides guarded global shortcuts for review controls, copy, and clear", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText } });
    render(<A3STestKit enabled page={{ id: "shortcuts" }} repairStorage="memory"><input aria-label="Application input" /><div contentEditable aria-label="Application editor" /><div role="textbox" tabIndex={0} aria-label="ARIA editor" /><button id="shortcut-target">Shortcut target</button><A3SReviewOverlay enabled /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-launch")).toBeTruthy());
    const input = document.querySelector<HTMLInputElement>("[aria-label='Application input']")!;
    const target = document.querySelector<HTMLElement>("#shortcut-target")!;
    setRect(target, { x: 20, y: 20, width: 120, height: 32 });

    fireEvent.keyDown(input, { key: "F", metaKey: true, shiftKey: true });
    expect(document.querySelector<HTMLElement>("[data-a3s-testkit-overlay]")!.shadowRoot!.querySelector(".a3s-panel")).toBeNull();
    fireEvent.keyDown(document, { key: "F", metaKey: true, shiftKey: true });
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());

    fireEvent.keyDown(document, { key: "e" });
    expect(shadowButton("Element").getAttribute("aria-pressed")).toBe("true");
    target.dispatchEvent(pointerEventWithPath(target, 30, 30));
    fireEvent.change(await waitFor(() => shadowQuery(".a3s-editor textarea")), { target: { value: "Shortcut draft" } });
    fireEvent.click(shadowButton("Add draft"));
    await waitFor(() => expect(shadowQuery(".a3s-markers").children).toHaveLength(1));

    const editableTargets = [
      input,
      document.querySelector<HTMLElement>("[aria-label='Application editor']")!,
      document.querySelector<HTMLElement>("[aria-label='ARIA editor']")!,
    ];
    for (const editable of editableTargets) {
      for (const key of ["e", "m", "t", "a", "d", "l", "p", "h", "c", "x"]) fireEvent.keyDown(editable, { key });
    }
    expect(shadowQuery(".a3s-list").textContent).toContain("Shortcut draft");
    fireEvent.click(shadowButton("New feedback"));
    expect(shadowButton("Layout").getAttribute("aria-pressed")).toBe("false");
    expect(getPageContextBridge()?.animationsPaused()).toBe(false);
    expect(shadowQuery(".a3s-markers").children).toHaveLength(1);
    expect(writeText).not.toHaveBeenCalled();

    fireEvent.keyDown(document, { key: "l" });
    expect(shadowButton("Layout").getAttribute("aria-pressed")).toBe("true");
    fireEvent.keyDown(document, { key: "p" });
    expect(getPageContextBridge()?.animationsPaused()).toBe(true);
    fireEvent.keyDown(document, { key: "h" });
    fireEvent.click(shadowQuery("[aria-label='Review preferences']"));
    expect(shadowButton("Show markers").getAttribute("aria-pressed")).toBe("false");
    expect(shadowQuery(".a3s-markers").children).toHaveLength(0);
    fireEvent.keyDown(document, { key: "c" });
    await waitFor(() => expect(writeText).toHaveBeenCalledTimes(1));
    fireEvent.keyDown(document, { key: "x" });
    fireEvent.click(shadowButton("Findings"));
    await waitFor(() => expect(shadowQuery(".a3s-list").textContent).not.toContain("Shortcut draft"));
    expect(window.localStorage.length).toBe(0);
    fireEvent.keyDown(document, { key: "Escape" });
    await waitFor(() => expect(document.querySelector<HTMLElement>("[data-a3s-testkit-overlay]")!.shadowRoot!.querySelector(".a3s-panel")).toBeNull());
  });

  it("reserves editable Escape for active marking without closing an idle review panel", async () => {
    render(<A3STestKit enabled page={{ id: "editable-escape" }} repairStorage="memory"><input aria-label="Editable escape target" /><A3SReviewOverlay enabled defaultOpen /></A3STestKit>);
    const panel = await waitFor(() => shadowQuery(".a3s-panel"));
    const input = document.querySelector<HTMLInputElement>("[aria-label='Editable escape target']")!;

    input.focus();
    input.dispatchEvent(new FocusEvent("focusin", { bubbles: true, composed: true }));
    fireEvent.keyDown(input, { key: "Escape" });
    expect(document.querySelector<HTMLElement>("[data-a3s-testkit-overlay]")!.shadowRoot!.querySelector(".a3s-panel")).toBe(panel);

    fireEvent.click(shadowButton("Element"));
    await waitFor(() => expect(document.activeElement).toBe(input));
    fireEvent.keyDown(input, { key: "Escape" });
    await waitFor(() => expect(document.querySelector<HTMLElement>("[data-a3s-testkit-overlay]")!.shadowRoot!.querySelector(".a3s-hint")).toBeNull());
    expect(document.activeElement).toBe(input);
    expect(document.querySelector<HTMLElement>("[data-a3s-testkit-overlay]")!.shadowRoot!.querySelector(".a3s-panel")).toBe(panel);
  });

  it("cancels a finding editor when Escape originates in its textarea", async () => {
    render(<A3STestKit enabled page={{ id: "editor-escape" }} repairStorage="memory"><button id="editor-escape-target">Editor escape target</button><A3SReviewOverlay enabled defaultOpen /></A3STestKit>);
    const panel = await waitFor(() => shadowQuery(".a3s-panel"));
    const target = document.querySelector<HTMLElement>("#editor-escape-target")!;
    setRect(target, { x: 20, y: 20, width: 140, height: 32 });
    fireEvent.click(shadowButton("Element"));
    target.dispatchEvent(pointerEventWithPath(target, 40, 30));
    const editor = await waitFor(() => shadowQuery(".a3s-editor textarea"));
    expect(document.querySelector<HTMLElement>("[data-a3s-testkit-overlay]")!.shadowRoot!.activeElement).toBe(editor);

    fireEvent.keyDown(editor, { key: "Escape", composed: true });
    await waitFor(() => expect(document.querySelector<HTMLElement>("[data-a3s-testkit-overlay]")!.shadowRoot!.querySelector(".a3s-editor")).toBeNull());
    expect(document.querySelector<HTMLElement>("[data-a3s-testkit-overlay]")!.shadowRoot!.activeElement).toBe(panel);
  });

  it("exposes spatial draft editing and isolates typed host integration callbacks", async () => {
    const onDraftAdded = vi.fn(() => { throw new Error("host add failure"); });
    const onDraftUpdated = vi.fn(() => { throw new Error("host update failure"); });
    const onDraftDeleted = vi.fn(() => { throw new Error("host delete failure"); });
    const onDraftsCleared = vi.fn(() => { throw new Error("host clear failure"); });
    const onCopied = vi.fn(() => { throw new Error("host copy failure"); });
    const onSubmitted = vi.fn(() => { throw new Error("host submit failure"); });
    const copyToClipboard = vi.fn().mockResolvedValue(undefined);
    render(<A3STestKit enabled page={{ id: "host-callbacks" }} repairStorage="memory"><button id="callback-target">Callback target</button><A3SReviewOverlay
      enabled
      defaultOpen
      copyToClipboard={copyToClipboard}
      onCopied={onCopied}
      onDraftAdded={onDraftAdded}
      onDraftUpdated={onDraftUpdated}
      onDraftDeleted={onDraftDeleted}
      onDraftsCleared={onDraftsCleared}
      onSubmitted={onSubmitted}
    /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());
    const target = document.querySelector<HTMLElement>("#callback-target")!;
    setRect(target, { x: 40, y: 60, width: 140, height: 36 });

    await addElementDraft(target, "Callback draft");
    expect(onDraftAdded).toHaveBeenCalledWith(expect.objectContaining({ instruction: "Callback draft" }));
    const markerEdit = shadowQuery("[aria-label='Edit draft marker: Callback draft']");
    fireEvent.click(markerEdit);
    fireEvent.change(shadowQuery(".a3s-editor textarea"), { target: { value: "Updated callback draft" } });
    fireEvent.click(shadowButton("Save changes"));
    expect(onDraftUpdated).toHaveBeenCalledWith(expect.objectContaining({ instruction: "Updated callback draft" }));

    fireEvent.click(shadowQuery("[aria-label='Edit draft marker: Updated callback draft']"));
    fireEvent.click(shadowButton("Delete draft"));
    expect(onDraftDeleted).toHaveBeenCalledWith(expect.objectContaining({ instruction: "Updated callback draft" }));
    expect(shadowQuery(".a3s-list").textContent).not.toContain("Updated callback draft");

    await addElementDraft(target, "Copy callback draft");
    fireEvent.click(shadowButton("Copy Markdown"));
    await waitFor(() => expect(copyToClipboard).toHaveBeenCalledTimes(1));
    expect(onCopied).toHaveBeenCalledWith(expect.objectContaining({
      format: "markdown",
      drafts: [expect.objectContaining({ instruction: "Copy callback draft" })],
      text: expect.stringContaining("Copy callback draft"),
    }));
    await addElementDraft(target, "Clear callback draft");
    fireEvent.click(shadowButton("Clear drafts"));
    expect(onDraftsCleared).toHaveBeenCalledWith(expect.arrayContaining([
      expect.objectContaining({ instruction: "Copy callback draft" }),
      expect.objectContaining({ instruction: "Clear callback draft" }),
    ]));

    await addElementDraft(target, "Submit callback draft");
    fireEvent.click(shadowButton("Send and auto-fix"));
    await waitFor(() => expect(onSubmitted).toHaveBeenCalledTimes(1));
    expect(shadowQuery(".a3s-list").textContent).toContain("Submit callback draft");
  });

  it("persists bounded presentation preferences and clears drafts after a successful copy", async () => {
    const copyToClipboard = vi.fn()
      .mockRejectedValueOnce(new Error("clipboard unavailable"))
      .mockResolvedValue(undefined);
    const first = render(<A3STestKit enabled page={{ id: "preferences" }} repairStorage="memory"><button id="preference-target">Preference target</button><A3SReviewOverlay enabled defaultOpen copyToClipboard={copyToClipboard} /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());
    const target = document.querySelector<HTMLElement>("#preference-target")!;
    setRect(target, { x: 30, y: 50, width: 150, height: 34 });

    const preferencesToggle = shadowQuery("[aria-label='Review preferences']");
    expect(preferencesToggle.getAttribute("aria-label")).toBe("Review preferences");
    expect(preferencesToggle.getAttribute("aria-pressed")).toBe("false");
    fireEvent.click(preferencesToggle);
    expect(preferencesToggle.getAttribute("aria-pressed")).toBe("true");
    fireEvent.change(shadowQuery("[aria-label='Overlay theme']"), { target: { value: "dark" } });
    fireEvent.change(shadowQuery("[aria-label='Marker color']"), { target: { value: "#2563eb" } });
    fireEvent.click(shadowQuery("[aria-label='Clear drafts after copy']"));
    fireEvent.change(shadowQuery("[aria-label='Panel dock']"), { target: { value: "left" } });
    fireEvent.change(shadowQuery("[aria-label='Wireframe page fade']"), { target: { value: "0.42" } });
    await waitFor(() => expect(shadowQuery(".a3s-root").dataset.theme).toBe("dark"));
    expect(shadowQuery(".a3s-root").dataset.dock).toBe("left");
    expect(shadowQuery(".a3s-root").style.getPropertyValue("--a3s-marker-color")).toBe("#2563eb");
    expect(shadowQuery(".a3s-root").style.getPropertyValue("--a3s-wireframe-fade")).toBe("0.42");
    expect(shadowQuery("[aria-label='Clear drafts after copy']").getAttribute("role")).toBe("switch");
    expect((shadowQuery("[aria-label='Wireframe page fade']") as HTMLElement).style.getPropertyValue("--a3s-range-value")).toBe("52.5%");

    await addElementDraft(target, "Clear after copy");
    fireEvent.click(shadowButton("Copy Markdown"));
    await waitFor(() => expect(copyToClipboard).toHaveBeenCalledTimes(1));
    expect(shadowQuery(".a3s-list").textContent).toContain("Clear after copy");
    fireEvent.click(shadowButton("Copy Markdown"));
    await waitFor(() => expect(copyToClipboard).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(shadowQuery(".a3s-list").textContent).not.toContain("Clear after copy"));
    first.unmount();

    render(<A3STestKit enabled page={{ id: "preferences" }} repairStorage="memory"><button>Reloaded</button><A3SReviewOverlay enabled defaultOpen /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-root").dataset.theme).toBe("dark"));
    expect(shadowQuery(".a3s-root").dataset.dock).toBe("left");
    fireEvent.click(shadowQuery("[aria-label='Review preferences']"));
    expect((shadowQuery("[aria-label='Clear drafts after copy']") as HTMLInputElement).checked).toBe(true);
    expect(shadowButton("Auto-send · off")).toBeTruthy();
    expect(shadowButton("Pause")).toBeTruthy();
  });

  it("blocks host pointer input explicitly and can hide the overlay until tab restart", async () => {
    const hostClick = vi.fn();
    const first = render(<A3STestKit enabled page={{ id: "interaction-policy" }} repairStorage="memory"><button id="host-action" onClick={hostClick}>Host action</button><A3SReviewOverlay enabled defaultOpen /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());
    const action = document.querySelector<HTMLElement>("#host-action")!;
    action.focus();
    expect(document.activeElement).toBe(action);
    const restoredFocus = vi.spyOn(action, "focus");
    fireEvent.click(action);
    expect(hostClick).toHaveBeenCalledTimes(1);
    fireEvent.click(shadowQuery("[aria-label='Review preferences']"));
    fireEvent.click(shadowQuery("[aria-label='Block page pointer input']"));
    fireEvent.click(action);
    expect(hostClick).toHaveBeenCalledTimes(1);
    fireEvent.click(shadowButton("Pause"));
    fireEvent.click(shadowButton("New feedback"));
    fireEvent.click(shadowButton("Element"));
    fireEvent.click(shadowQuery("[aria-label='Review preferences']"));
    const hideUntilRestart = shadowButton("Hide until tab restart");
    hideUntilRestart.focus();
    expect(document.querySelector<HTMLElement>("[data-a3s-testkit-overlay]")!.shadowRoot!.activeElement).toBe(hideUntilRestart);
    fireEvent.click(hideUntilRestart);
    await waitFor(() => expect(document.querySelector("[data-a3s-testkit-overlay]")).toBeNull());
    await waitFor(() => expect(document.activeElement).toBe(action));
    expect(restoredFocus).toHaveBeenCalledWith({ preventScroll: true });
    expect(getPageContextBridge()?.animationsPaused()).toBe(false);
    fireEvent.click(action);
    expect(hostClick).toHaveBeenCalledTimes(2);
    first.unmount();

    const hidden = render(<A3STestKit enabled page={{ id: "interaction-policy" }} repairStorage="memory"><A3SReviewOverlay enabled defaultOpen /></A3STestKit>);
    await waitFor(() => expect(getPageContextBridge()).not.toBeNull());
    expect(document.querySelector("[data-a3s-testkit-overlay]")).toBeNull();
    hidden.unmount();
    window.sessionStorage.clear();
    render(<A3STestKit enabled page={{ id: "interaction-policy" }} repairStorage="memory"><A3SReviewOverlay enabled defaultOpen /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());
  });

  it("marks a focused application element with Enter and restores focus on Escape", async () => {
    render(<A3STestKit enabled page={{ id: "keyboard" }} repairStorage="memory"><button id="keyboard-target">Keyboard target</button><A3SReviewOverlay enabled defaultOpen /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());
    const target = document.querySelector<HTMLButtonElement>("#keyboard-target")!;
    setRect(target, { x: 20, y: 20, width: 120, height: 32 });
    target.focus();
    target.dispatchEvent(new FocusEvent("focusin", { bubbles: true, composed: true }));
    fireEvent.click(shadowButton("Element"));
    await waitFor(() => expect(document.activeElement).toBe(target));
    fireEvent.keyDown(target, { key: "Enter" });
    await waitFor(() => expect(shadowQuery(".a3s-editor").textContent).toContain("Keyboard target"));
    fireEvent.click(shadowButton("Cancel"));

    target.focus();
    target.dispatchEvent(new FocusEvent("focusin", { bubbles: true, composed: true }));
    fireEvent.click(shadowButton("Element"));
    fireEvent.keyDown(target, { key: "Escape" });
    await waitFor(() => expect(document.activeElement).toBe(target));
    expect(document.querySelector<HTMLElement>("[data-a3s-testkit-overlay]")!.shadowRoot!.querySelector(".a3s-hint")).toBeNull();
  });

  it("keeps keyboard multi-select in the application until explicit completion", async () => {
    const onSubmitted = vi.fn();
    render(<A3STestKit enabled page={{ id: "keyboard-multi" }} repairStorage="memory"><button id="keyboard-multi-one">One</button><button id="keyboard-multi-two">Two</button><A3SReviewOverlay enabled defaultOpen onSubmitted={onSubmitted} /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());
    const first = document.querySelector<HTMLButtonElement>("#keyboard-multi-one")!;
    const second = document.querySelector<HTMLButtonElement>("#keyboard-multi-two")!;
    const shadow = document.querySelector<HTMLElement>("[data-a3s-testkit-overlay]")!.shadowRoot!;
    setRect(first, { x: 20, y: 20, width: 80, height: 32 });
    setRect(second, { x: 120, y: 20, width: 80, height: 32 });

    first.focus();
    first.dispatchEvent(new FocusEvent("focusin", { bubbles: true, composed: true }));
    fireEvent.click(shadowButton("Multi"));
    await waitFor(() => expect(document.activeElement).toBe(first));
    expect(shadow.querySelector(".a3s-editor")).toBeNull();

    fireEvent.keyDown(first, { key: "Enter" });
    await waitFor(() => expect(document.activeElement).toBe(first));
    expect(shadow.querySelector(".a3s-editor")).toBeNull();

    second.focus();
    second.dispatchEvent(new FocusEvent("focusin", { bubbles: true, composed: true }));
    fireEvent.keyDown(second, { key: "Enter" });
    expect(shadow.querySelector(".a3s-editor")).toBeNull();
    fireEvent.keyDown(second, { key: "Enter", shiftKey: true });

    await waitFor(() => expect(shadowQuery(".a3s-editor").textContent).toContain("2 selected elements"));
    fireEvent.change(shadowQuery(".a3s-editor textarea"), { target: { value: "Align both keyboard actions" } });
    fireEvent.click(shadowButton("Send and auto-fix"));
    await waitFor(() => expect(onSubmitted).toHaveBeenCalledTimes(1));
    expect(onSubmitted.mock.calls[0]![0][0].target.nodeIds).toHaveLength(2);
  });

  it("removes an incomplete multi-select candidate when marking is cancelled", async () => {
    render(<A3STestKit enabled page={{ id: "cancel-multi" }} repairStorage="memory"><button id="cancel-multi-target">Target</button><A3SReviewOverlay enabled defaultOpen /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());
    const target = document.querySelector<HTMLButtonElement>("#cancel-multi-target")!;
    const shadow = document.querySelector<HTMLElement>("[data-a3s-testkit-overlay]")!.shadowRoot!;
    setRect(target, { x: 20, y: 20, width: 120, height: 32 });
    target.focus();
    target.dispatchEvent(new FocusEvent("focusin", { bubbles: true, composed: true }));

    fireEvent.click(shadowButton("Multi"));
    expect(shadowQuery(".a3s-mobile-marking-bar")).toBeTruthy();
    fireEvent.keyDown(target, { key: "Enter" });
    fireEvent.keyDown(target, { key: "Escape" });
    await waitFor(() => expect(document.activeElement).toBe(target));
    expect(shadow.querySelector(".a3s-editor")).toBeNull();
    expect(shadow.querySelector(".a3s-hint")).toBeNull();

    target.focus();
    target.dispatchEvent(new FocusEvent("focusin", { bubbles: true, composed: true }));
    fireEvent.click(shadowButton("Multi"));
    target.dispatchEvent(pointerEventWithPath(target, 40, 30));
    await waitFor(() => expect(shadow.querySelector(".a3s-editor")).not.toBeNull());
    fireEvent.click(shadowButton("Finish selection"));
    await waitFor(() => expect(shadowQuery(".a3s-panel").classList.contains("is-marking")).toBe(false));
    fireEvent.click(shadowButton("Cancel"));

    await waitFor(() => expect(shadow.activeElement).toBe(shadowQuery(".a3s-panel")));
    expect(shadow.querySelector(".a3s-editor")).toBeNull();
    expect(shadow.querySelector(".a3s-hint")).toBeNull();

    target.focus();
    target.dispatchEvent(new FocusEvent("focusin", { bubbles: true, composed: true }));
    fireEvent.click(shadowButton("Multi"));
    target.dispatchEvent(pointerEventWithPath(target, 40, 30));
    await waitFor(() => expect(shadow.querySelector(".a3s-editor")).not.toBeNull());
    fireEvent.click(shadowQuery(".a3s-launch"));
    await waitFor(() => expect(shadow.querySelector(".a3s-panel")).toBeNull());
    fireEvent.click(shadowQuery(".a3s-launch"));
    await waitFor(() => expect(shadow.querySelector(".a3s-panel")).not.toBeNull());
    expect(shadow.querySelector(".a3s-editor")).toBeNull();
    expect(shadow.querySelector(".a3s-hint")).toBeNull();
  });

  it("exposes one named side region with stable controls and focused status announcements", async () => {
    render(<A3STestKit enabled page={{ id: "accessible-overlay" }} repairStorage="memory"><button id="accessible-target">Accessible target</button><A3SReviewOverlay enabled defaultOpen /></A3STestKit>);
    const panel = await waitFor(() => shadowQuery(".a3s-panel"));
    const title = shadowQuery(".a3s-panel-title");
    const description = shadowQuery(".a3s-panel-description");
    expect(panel.getAttribute("role")).toBe("region");
    expect(panel.getAttribute("aria-labelledby")).toBe(title.id);
    expect(panel.getAttribute("aria-describedby")).toBe(description.id);
    expect(panel.querySelector("[role='dialog']")).toBeNull();
    expect(panel.querySelector(".a3s-tool-tray")).toBeNull();
    expect(panel.querySelector(".a3s-editor-popover")).toBeNull();
    expect(panel.classList.contains("task-pane")).toBe(true);
    expect(panel.querySelectorAll(".a3s-panel-tabs > button")).toHaveLength(2);
    expect(panel.querySelector(".a3s-panel-header [aria-label='Review preferences']")).toBeTruthy();
    expect(shadowQuery(".a3s-more-tools").getAttribute("aria-expanded")).toBe("false");
    expect(shadowQuery(".a3s-secondary-tools").hasAttribute("hidden")).toBe(true);
    fireEvent.click(shadowButton("More tools"));
    expect(shadowQuery(".a3s-secondary-tools").hasAttribute("hidden")).toBe(false);
    expect(shadowButton("Layout").getAttribute("aria-label")).toBe("Layout");
    expect(document.querySelector<HTMLElement>("[data-a3s-testkit-overlay]")!.shadowRoot!.querySelector("style")!.textContent).toContain("--a3s-control-height");
    expect(document.querySelector<HTMLElement>("[data-a3s-testkit-overlay]")!.shadowRoot!.querySelector("style")!.textContent).toContain("font: 13px/1.55 var(--a3s-ui-font)");
    expect(shadowQuery(".a3s-announcer").getAttribute("aria-atomic")).toBe("true");
    fireEvent.click(shadowButton("Findings"));
    expect(shadowQuery(".a3s-list").hasAttribute("aria-live")).toBe(false);
    expect((shadowQuery(".a3s-list") as HTMLElement).tabIndex).toBe(0);
    fireEvent.click(shadowButton("New feedback"));

    const launcher = shadowQuery(".a3s-launch");
    expect(launcher.getAttribute("aria-keyshortcuts")).toBe("Control+Shift+F Meta+Shift+F");
    expect(panel.getAttribute("aria-keyshortcuts")).toBe("Escape");
    expect(shadowButton("Element").getAttribute("aria-keyshortcuts")).toBe("E");
    expect(shadowButton("Element").getAttribute("aria-label")).toBe("Mark element");
    expect(shadowButton("Multi").getAttribute("aria-keyshortcuts")).toBe("M");
    expect(shadowButton("Text").getAttribute("aria-keyshortcuts")).toBe("T");
    expect(shadowButton("Area").getAttribute("aria-keyshortcuts")).toBe("A");
    expect(shadowButton("Draw").getAttribute("aria-keyshortcuts")).toBe("D");
    expect(shadowButton("Layout").getAttribute("aria-keyshortcuts")).toBe("L");
    fireEvent.click(shadowQuery("[aria-label='Review preferences']"));
    expect(shadowButton("Pause").getAttribute("aria-label")).toBe("Pause page animations");
    expect(shadowButton("Auto-send · off").getAttribute("aria-label")).toBe("Turn auto-send on");
    expect(shadowQuery("[aria-label='Overlay theme']")).toBeTruthy();
    expect(shadowButton("Pause").getAttribute("aria-keyshortcuts")).toBe("P");
    expect(shadowButton("Hide markers").getAttribute("aria-keyshortcuts")).toBe("H");
    const shortcutHelp = shadowQuery(".a3s-shortcuts");
    expect(shortcutHelp.getAttribute("aria-labelledby")).toBe(shadowQuery(".a3s-shortcuts-title").id);
    expect(shortcutHelp.textContent).toContain("Toggle review");
    expect(shortcutHelp.textContent).toContain("Copy selected drafts");
    expect(shortcutHelp.textContent).toContain("Letter shortcuts and panel toggle are ignored while typing");
    expect(shortcutHelp.textContent).toContain("Escape still cancels active marking or an open finding editor");

    fireEvent.click(shadowQuery(".a3s-panel-header .a3s-close"));
    await waitFor(() => expect(document.querySelector<HTMLElement>("[data-a3s-testkit-overlay]")!.shadowRoot!.activeElement).toBe(launcher));
    fireEvent.click(launcher);
    await waitFor(() => expect(document.querySelector<HTMLElement>("[data-a3s-testkit-overlay]")!.shadowRoot!.activeElement).toBe(shadowQuery(".a3s-panel")));

    const target = document.querySelector<HTMLElement>("#accessible-target")!;
    setRect(target, { x: 20, y: 20, width: 120, height: 32 });
    fireEvent.click(shadowButton("New feedback"));
    fireEvent.click(shadowButton("Element"));
    target.dispatchEvent(pointerEventWithPath(target, 40, 30));
    fireEvent.change(await waitFor(() => shadowQuery(".a3s-editor textarea")), { target: { value: "Name every finding action" } });
    fireEvent.click(shadowButton("Add draft"));
    await waitFor(() => expect(shadowQuery(".a3s-announcer").textContent).toBe("Draft added: Name every finding action"));
    expect(shadowButton("Clear drafts").getAttribute("aria-keyshortcuts")).toBe("X");
    expect(shadowButton("Copy Markdown").getAttribute("aria-keyshortcuts")).toBe("C");
    expect(shadowButton("Edit").getAttribute("aria-label")).toBe("Edit draft: Name every finding action");
    expect(shadowButton("Delete").getAttribute("aria-label")).toBe("Delete draft: Name every finding action");
    fireEvent.click(shadowButton("Send and auto-fix"));
    await waitFor(() => expect(shadowQuery(".a3s-announcer").textContent).toBe("1 finding sent for repair"));
    expect(document.querySelector<HTMLElement>("[data-a3s-testkit-overlay]")!.shadowRoot!.activeElement).toBe(shadowQuery(".a3s-panel"));
  });

  it("does not mount the overlay without an explicitly enabled compatible bridge", async () => {
    const disabled = render(<A3STestKit enabled={false} page={{ id: "disabled" }} repairStorage="memory"><A3SReviewOverlay enabled defaultOpen /></A3STestKit>);
    await waitFor(() => expect(getPageContextBridge()).toBeNull());
    expect(document.querySelector("[data-a3s-testkit-overlay]")).toBeNull();
    disabled.unmount();

    const headless = render(<A3STestKit enabled page={{ id: "headless" }} repairStorage="memory"><A3SReviewOverlay defaultOpen /></A3STestKit>);
    await waitFor(() => expect(getPageContextBridge()).not.toBeNull());
    expect(document.querySelector("[data-a3s-testkit-overlay]")).toBeNull();
    headless.unmount();
  });

  it("can attach an explicitly enabled overlay to a framework-neutral bridge", async () => {
    const bridge = installTestKit({
      enabled: true,
      page: { id: "framework-neutral" },
      repairStorage: "memory",
    });
    const view = render(<A3SReviewOverlay enabled defaultOpen />);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());
    view.unmount();
    bridge.dispose();
  });

  it("renders agent clarification messages in the submitted finding thread", async () => {
    const onSubmitted = vi.fn();
    render(<A3STestKit enabled page={{ id: "thread" }} repairStorage="memory"><button id="thread-target">Thread target</button><A3SReviewOverlay enabled defaultOpen onSubmitted={onSubmitted} /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());
    const target = document.querySelector<HTMLElement>("#thread-target")!;
    setRect(target, { x: 10, y: 10, width: 80, height: 30 });
    fireEvent.click(shadowButton("Element"));
    target.dispatchEvent(pointerEventWithPath(target, 20, 15));
    await waitFor(() => expect(shadowQuery("textarea")).toBeTruthy());
    fireEvent.change(shadowQuery("textarea"), { target: { value: "Fix with clarification" } });
    fireEvent.click(shadowButton("Send and auto-fix"));
    await waitFor(() => expect(onSubmitted).toHaveBeenCalledTimes(1));
    const findingId = onSubmitted.mock.calls[0]![0][0].id;
    getPageContextBridge()?.applyRepairEvent({
      requestId: "clarify-1",
      findingId,
      sequence: 1,
      status: "claimed",
      actor: "agent",
      timestamp: new Date().toISOString(),
    });
    getPageContextBridge()?.applyRepairEvent({
      requestId: "clarify-2",
      findingId,
      sequence: 2,
      status: "needs_input",
      actor: "agent",
      timestamp: new Date().toISOString(),
      message: "Should this retain its current label?",
    });
    await waitFor(() => expect(shadowQuery(".a3s-thread").textContent).toContain("Should this retain its current label?"));
    await waitFor(() => expect(shadowQuery(".a3s-announcer").textContent).toBe("Repair needs input: Fix with clarification"));
    expect(shadowQuery(".a3s-status").hasAttribute("role")).toBe(false);
    fireEvent.click(shadowButton("Reply"));
    fireEvent.change(shadowQuery("[aria-label='Reply to coding agent about: Fix with clarification']"), { target: { value: "Keep the label." } });
    fireEvent.click(shadowButton("Send reply"));
    await waitFor(() => expect(getPageContextBridge()?.takeRepairActions()[0]).toMatchObject({ action: "reply", findingId, message: "Keep the label." }));
    await waitFor(() => expect(shadowQuery(".a3s-announcer").textContent).toBe("Reply sent to the coding agent"));
    await waitFor(() => expect(document.querySelector<HTMLElement>("[data-a3s-testkit-overlay]")!.shadowRoot!.activeElement).toBe(shadowButton("Reply")));
  });

  it("lets a human accept or reject only review-ready repairs", async () => {
    const onSubmitted = vi.fn();
    render(<A3STestKit enabled page={{ id: "human-review" }} repairStorage="memory"><button id="review-target">Review target</button><A3SReviewOverlay enabled defaultOpen onSubmitted={onSubmitted} /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());
    const target = document.querySelector<HTMLElement>("#review-target")!;
    setRect(target, { x: 10, y: 10, width: 80, height: 30 });
    fireEvent.click(shadowButton("Element"));
    target.dispatchEvent(pointerEventWithPath(target, 20, 15));
    fireEvent.change(await waitFor(() => shadowQuery("textarea")), { target: { value: "Fix for review" } });
    fireEvent.click(shadowButton("Send and auto-fix"));
    await waitFor(() => expect(onSubmitted).toHaveBeenCalledTimes(1));
    const findingId = onSubmitted.mock.calls[0]![0][0].id;
    const statuses = ["claimed", "repairing", "verifying", "review_ready"] as const;
    statuses.forEach((status, index) => getPageContextBridge()?.applyRepairEvent({ requestId: `status-${index}`, findingId, sequence: index + 1, status, actor: status === "review_ready" ? "a3s-test" : "agent", timestamp: new Date().toISOString() }));
    await waitFor(() => expect(shadowButton("Accept repair")).toBeTruthy());
    fireEvent.click(shadowButton("Accept repair"));
    await waitFor(() => expect(getPageContextBridge()?.takeRepairActions()[0]).toMatchObject({ action: "accept", findingId }));
  });
});

function pointerEventWithPath(target: Element, clientX: number, clientY: number): Event {
  const event = new MouseEvent("pointerup", { bubbles: true, composed: true, button: 0, clientX, clientY });
  Object.defineProperty(event, "composedPath", { value: () => [target, document.body, document.documentElement, document, window] });
  return event;
}

function pointerEvent(type: string, target: Element, clientX: number, clientY: number): Event {
  const event = new MouseEvent(type, { bubbles: true, composed: true, button: 0, clientX, clientY });
  Object.defineProperty(event, "composedPath", { value: () => [target, document.body, document.documentElement, document, window] });
  return event;
}

function capturePointerEvent(type: string, target: Element, clientX: number, clientY: number, pointerId: number): Event {
  const event = pointerEvent(type, target, clientX, clientY);
  Object.defineProperty(event, "pointerId", { value: pointerId });
  return event;
}

async function addElementDraft(target: HTMLElement, instruction: string): Promise<void> {
  fireEvent.click(shadowButton("New feedback"));
  fireEvent.click(shadowButton("Element"));
  target.dispatchEvent(pointerEventWithPath(target, 50, 70));
  fireEvent.change(await waitFor(() => shadowQuery(".a3s-editor textarea")), { target: { value: instruction } });
  fireEvent.click(shadowButton("Add draft"));
  await waitFor(() => expect(shadowQuery(".a3s-list").textContent).toContain(instruction));
}
