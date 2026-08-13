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
    await waitFor(() => expect(shadowQuery(".a3s-list").textContent).not.toContain("Profile-only draft"));
    window.history.pushState(null, "", "/profile");
    await waitFor(() => expect(shadowQuery(".a3s-list").textContent).toContain("Profile-only draft"));
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
    expect(catalogToggle.getAttribute("aria-label")).toBe("Component catalog · 90");
    expect(catalogToggle.getAttribute("aria-expanded")).toBe("false");
    fireEvent.click(catalogToggle);
    expect(catalogToggle.getAttribute("aria-expanded")).toBe("true");
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

    fireEvent.click(shadowButton("Element"));
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
      for (const key of ["l", "p", "h", "c", "x"]) fireEvent.keyDown(editable, { key });
    }
    expect(shadowButton("Layout").getAttribute("aria-pressed")).toBe("false");
    expect(getPageContextBridge()?.animationsPaused()).toBe(false);
    expect(shadowQuery(".a3s-markers").children).toHaveLength(1);
    expect(writeText).not.toHaveBeenCalled();
    expect(shadowQuery(".a3s-list").textContent).toContain("Shortcut draft");

    fireEvent.keyDown(document, { key: "l" });
    expect(shadowButton("Layout").getAttribute("aria-pressed")).toBe("true");
    fireEvent.keyDown(document, { key: "p" });
    expect(getPageContextBridge()?.animationsPaused()).toBe(true);
    fireEvent.keyDown(document, { key: "h" });
    expect(shadowButton("Show markers").getAttribute("aria-pressed")).toBe("false");
    expect(shadowQuery(".a3s-markers").children).toHaveLength(0);
    fireEvent.keyDown(document, { key: "c" });
    await waitFor(() => expect(writeText).toHaveBeenCalledTimes(1));
    fireEvent.keyDown(document, { key: "x" });
    await waitFor(() => expect(shadowQuery(".a3s-list").textContent).not.toContain("Shortcut draft"));
    expect(window.localStorage.length).toBe(0);
    fireEvent.keyDown(document, { key: "Escape" });
    await waitFor(() => expect(document.querySelector<HTMLElement>("[data-a3s-testkit-overlay]")!.shadowRoot!.querySelector(".a3s-panel")).toBeNull());
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

    const preferencesToggle = shadowButton("Review preferences");
    expect(preferencesToggle.getAttribute("aria-label")).toBe("Review preferences");
    expect(preferencesToggle.getAttribute("aria-expanded")).toBe("false");
    fireEvent.click(preferencesToggle);
    expect(preferencesToggle.getAttribute("aria-expanded")).toBe("true");
    fireEvent.change(shadowQuery("[aria-label='Overlay theme']"), { target: { value: "dark" } });
    fireEvent.change(shadowQuery("[aria-label='Marker color']"), { target: { value: "#2563eb" } });
    fireEvent.click(shadowQuery("[aria-label='Clear drafts after copy']"));
    fireEvent.change(shadowQuery("[aria-label='Panel dock']"), { target: { value: "left" } });
    fireEvent.change(shadowQuery("[aria-label='Wireframe page fade']"), { target: { value: "0.42" } });
    await waitFor(() => expect(shadowQuery(".a3s-root").dataset.theme).toBe("dark"));
    expect(shadowQuery(".a3s-root").dataset.dock).toBe("left");
    expect(shadowQuery(".a3s-root").style.getPropertyValue("--a3s-marker-color")).toBe("#2563eb");
    expect(shadowQuery(".a3s-root").style.getPropertyValue("--a3s-wireframe-fade")).toBe("0.42");

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
    fireEvent.click(shadowButton("Review preferences"));
    expect((shadowQuery("[aria-label='Clear drafts after copy']") as HTMLInputElement).checked).toBe(true);
    expect(shadowButton("Auto-send · off")).toBeTruthy();
    expect(shadowButton("Pause")).toBeTruthy();
  });

  it("blocks host pointer input explicitly and can hide the overlay until tab restart", async () => {
    const hostClick = vi.fn();
    const first = render(<A3STestKit enabled page={{ id: "interaction-policy" }} repairStorage="memory"><button id="host-action" onClick={hostClick}>Host action</button><A3SReviewOverlay enabled defaultOpen /></A3STestKit>);
    await waitFor(() => expect(shadowQuery(".a3s-panel")).toBeTruthy());
    const action = document.querySelector<HTMLElement>("#host-action")!;
    fireEvent.click(action);
    expect(hostClick).toHaveBeenCalledTimes(1);
    fireEvent.click(shadowButton("Review preferences"));
    fireEvent.click(shadowQuery("[aria-label='Block page pointer input']"));
    fireEvent.click(action);
    expect(hostClick).toHaveBeenCalledTimes(1);
    fireEvent.click(shadowButton("Pause"));
    fireEvent.click(shadowButton("Element"));
    fireEvent.click(shadowButton("Hide until tab restart"));
    await waitFor(() => expect(document.querySelector("[data-a3s-testkit-overlay]")).toBeNull());
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

  it("exposes a named dialog, stable control names, and focused status announcements", async () => {
    render(<A3STestKit enabled page={{ id: "accessible-overlay" }} repairStorage="memory"><button id="accessible-target">Accessible target</button><A3SReviewOverlay enabled defaultOpen /></A3STestKit>);
    const panel = await waitFor(() => shadowQuery(".a3s-panel"));
    const title = shadowQuery(".a3s-panel-title");
    const description = shadowQuery(".a3s-panel-description");
    expect(panel.getAttribute("role")).toBe("dialog");
    expect(panel.getAttribute("aria-labelledby")).toBe(title.id);
    expect(panel.getAttribute("aria-describedby")).toBe(description.id);
    expect(shadowButton("Pause").getAttribute("aria-label")).toBe("Pause page animations");
    expect(shadowButton("Auto-send · off").getAttribute("aria-label")).toBe("Turn auto-send on");
    expect(shadowButton("Theme · system").getAttribute("aria-label")).toBe("Change overlay theme; current theme is system");
    expect(shadowQuery(".a3s-announcer").getAttribute("aria-atomic")).toBe("true");
    expect(shadowQuery(".a3s-list").hasAttribute("aria-live")).toBe(false);

    const launcher = shadowQuery(".a3s-launch");
    fireEvent.click(shadowQuery("header button"));
    await waitFor(() => expect(document.querySelector<HTMLElement>("[data-a3s-testkit-overlay]")!.shadowRoot!.activeElement).toBe(launcher));
    fireEvent.click(launcher);
    await waitFor(() => expect(document.querySelector<HTMLElement>("[data-a3s-testkit-overlay]")!.shadowRoot!.activeElement).toBe(shadowQuery(".a3s-panel")));

    const target = document.querySelector<HTMLElement>("#accessible-target")!;
    setRect(target, { x: 20, y: 20, width: 120, height: 32 });
    fireEvent.click(shadowButton("Element"));
    target.dispatchEvent(pointerEventWithPath(target, 40, 30));
    fireEvent.change(await waitFor(() => shadowQuery(".a3s-editor textarea")), { target: { value: "Name every finding action" } });
    fireEvent.click(shadowButton("Add draft"));
    await waitFor(() => expect(shadowQuery(".a3s-announcer").textContent).toBe("Draft added: Name every finding action"));
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

async function addElementDraft(target: HTMLElement, instruction: string): Promise<void> {
  fireEvent.click(shadowButton("Element"));
  target.dispatchEvent(pointerEventWithPath(target, 50, 70));
  fireEvent.change(await waitFor(() => shadowQuery(".a3s-editor textarea")), { target: { value: instruction } });
  fireEvent.click(shadowButton("Add draft"));
  await waitFor(() => expect(shadowQuery(".a3s-list").textContent).toContain(instruction));
}
