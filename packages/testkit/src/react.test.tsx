import { StrictMode } from "react";
import { fireEvent, render, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { getPageContextBridge, installTestKit } from "./runtime";
import { A3SReviewOverlay, A3STestBoundary, A3STestKit } from "./react";
import { setRect } from "./test-setup";

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

describe("React adapter and review overlay", () => {
  it("survives StrictMode and registers component context", async () => {
    const view = render(<StrictMode><A3STestKit enabled page={{ id: "react" }} repairStorage="memory"><A3STestBoundary id="card" name="Card" source={{ file: "src/Card.tsx" }}><button>Buy</button></A3STestBoundary></A3STestKit></StrictMode>);
    await waitFor(() => expect(getPageContextBridge()).not.toBeNull());
    const boundary = document.querySelector("div")!;
    const button = document.querySelector("button")!;
    setRect(boundary, { x: 10, y: 10, width: 200, height: 80 });
    setRect(button, { x: 20, y: 20, width: 70, height: 30 });
    await waitFor(() => expect(getPageContextBridge()?.snapshot().components).toHaveLength(1));
    expect(getPageContextBridge()?.snapshot().nodes.find((node) => node.role === "button")?.componentId).toBe("card");
    view.unmount();
    expect(getPageContextBridge()).toBeNull();
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
    fireEvent.click(send);
    await waitFor(() => expect(onSubmitted).toHaveBeenCalledTimes(1));
    const repair = onSubmitted.mock.calls[0]![0][0];
    expect(repair).toMatchObject({ instruction: "Make this action work", status: "queued", target: { kind: "node" }, context: { untrusted: true } });
    expect(getPageContextBridge()?.snapshot({ detail: "forensic" }).nodes.some((node) => node.text?.includes("Review & repair"))).toBe(false);
  });

  it("submits selected drafts in visible order as one batch", async () => {
    const onSubmitted = vi.fn();
    render(<A3STestKit enabled page={{ id: "batch" }} repairStorage="memory"><button id="one">One</button><button id="two">Two</button><A3SReviewOverlay enabled defaultOpen onSubmitted={onSubmitted} /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());
    for (const [selector, instruction] of [["#one", "Fix one"], ["#two", "Fix two"]] as const) {
      const target = document.querySelector<HTMLElement>(selector)!;
      setRect(target, { x: 10, y: 10, width: 40, height: 20 });
      fireEvent.click(shadowQuery(".a3s-tools button"));
      target.dispatchEvent(pointerEventWithPath(target, 20, 15));
      await waitFor(() => expect(shadowQuery("textarea")).toBeTruthy());
      fireEvent.change(shadowQuery("textarea"), { target: { value: instruction } });
      if (selector === "#two") {
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

  it("supports drag multi-select, persistent markers, draft editing, hide, and reopen", async () => {
    render(<A3STestKit enabled page={{ id: "multi" }} repairStorage="memory"><button id="one">One</button><button id="two">Two</button><A3SReviewOverlay enabled defaultOpen /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());
    setRect(document.querySelector("#one")!, { x: 10, y: 10, width: 40, height: 20 });
    setRect(document.querySelector("#two")!, { x: 70, y: 10, width: 40, height: 20 });

    fireEvent.click(shadowButton("Multi"));
    document.body.dispatchEvent(pointerEvent("pointerdown", document.body, 0, 0));
    document.body.dispatchEvent(pointerEvent("pointermove", document.body, 120, 40));
    document.body.dispatchEvent(pointerEvent("pointerup", document.body, 120, 40));
    await waitFor(() => expect(shadowQuery(".a3s-editor").textContent).toContain("2 elements"));
    fireEvent.change(shadowQuery("textarea"), { target: { value: "Align both actions" } });
    fireEvent.click(shadowButton("Add draft"));
    await waitFor(() => expect(shadowQuery(".a3s-markers").children).toHaveLength(2));

    fireEvent.click(shadowButton("Edit"));
    fireEvent.change(shadowQuery("textarea"), { target: { value: "Align both primary actions" } });
    fireEvent.click(shadowButton("Save changes"));
    await waitFor(() => expect(shadowQuery(".a3s-list").textContent).toContain("Align both primary actions"));

    fireEvent.click(shadowButton("Hide marker"));
    expect(shadowQuery(".a3s-markers").children).toHaveLength(0);
    fireEvent.click(shadowButton("Reopen marker"));
    await waitFor(() => expect(shadowQuery(".a3s-markers").children).toHaveLength(2));
  });

  it("captures a bounded freehand finding and cycles manual themes", async () => {
    render(<A3STestKit enabled page={{ id: "draw" }} repairStorage="memory"><button id="target">Target</button><A3SReviewOverlay enabled defaultOpen /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());
    setRect(document.querySelector("#target")!, { x: 20, y: 20, width: 80, height: 40 });

    const theme = shadowButton("Theme · system");
    fireEvent.click(theme);
    expect(shadowQuery(".a3s-root").dataset.theme).toBe("light");
    fireEvent.click(shadowButton("Theme · light"));
    expect(shadowQuery(".a3s-root").dataset.theme).toBe("dark");

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
    fireEvent.click(shadowButton("Reply"));
    fireEvent.change(shadowQuery("[aria-label='Reply to coding agent']"), { target: { value: "Keep the label." } });
    fireEvent.click(shadowButton("Send reply"));
    await waitFor(() => expect(getPageContextBridge()?.takeRepairActions()[0]).toMatchObject({ action: "reply", findingId, message: "Keep the label." }));
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
