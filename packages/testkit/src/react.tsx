import {
  createContext,
  useContext,
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type PropsWithChildren,
} from "react";
import { createPortal } from "react-dom";
import {
  getPageContextBridge,
  installTestKit,
} from "./runtime";
import { PAGE_CONTEXT_PROTOCOL } from "./types";
import { OVERLAY_CSS } from "./overlay-style";
import type {
  PageContextBridge,
  RepairDraft,
  RepairIntent,
  RepairSeverity,
  RepairStatus,
  RepairTarget,
  Rect,
  SubmittedRepair,
  TestKitOptions,
  TestKitRuntime,
} from "./types";

type TestKitContextValue = {
  bridge: PageContextBridge | null;
  providerConfigured: boolean;
};

const TestKitContext = createContext<TestKitContextValue>({
  bridge: null,
  providerConfigured: false,
});

export type A3STestKitProps = PropsWithChildren<
  Omit<TestKitOptions, "enabled"> & { enabled: boolean }
>;

export function A3STestKit({ children, ...options }: A3STestKitProps) {
  const latest = useLatest(options);
  const [bridge, setBridge] = useState<PageContextBridge | null>(null);

  useEffect(() => {
    if (options.enabled !== true) {
      setBridge(null);
      return;
    }
    const installed = installTestKit({
      ...options,
      enabled: true,
      ready: () => latest.current.ready?.() ?? document.readyState !== "loading",
      facts: () => latest.current.facts?.() ?? {},
    });
    setBridge(installed);
    return () => {
      installed.dispose();
      setBridge((current) => current === installed ? null : current);
    };
  }, [options.enabled, options.maxEncodedBytes, options.maxNodes, options.maxStringBytes, options.page.id, options.repairEndpoint, options.repairStorage, stableList(options.redact)]);

  const value = useMemo(
    () => ({ bridge, providerConfigured: true }),
    [bridge],
  );
  return <TestKitContext.Provider value={value}>{children}</TestKitContext.Provider>;
}

export type A3STestBoundaryProps = PropsWithChildren<{
  id: string;
  name: string;
  source?: { file: string; line?: number; column?: number };
  ready?: () => boolean;
  facts?: () => Record<string, unknown>;
  roots?: () => readonly Element[];
  as?: "div" | "section" | "main" | "nav" | "article" | "aside" | "header" | "footer" | "span";
  className?: string;
  style?: CSSProperties;
}>;

export function A3STestBoundary({
  id,
  name,
  source,
  ready,
  facts,
  roots,
  as: Tag = "div",
  children,
  className,
  style,
}: A3STestBoundaryProps) {
  const { bridge } = useContext(TestKitContext);
  const ref = useRef<HTMLElement | null>(null);
  const latest = useLatest({ ready, facts, roots });
  useLayoutEffect(() => {
    const element = ref.current;
    if (!element || !bridge) return;
    if (!("registerBoundary" in bridge)) return;
    return (bridge as TestKitRuntime).registerBoundary({
      id,
      name,
      elements: () => [element, ...(latest.current.roots?.() ?? [])],
      ...(source ? { source } : {}),
      ready: () => latest.current.ready?.() ?? true,
      facts: () => latest.current.facts?.() ?? {},
    });
  }, [bridge, id, name, source?.file, source?.line, source?.column]);
  return <Tag ref={ref as never} className={className} style={style}>{children}</Tag>;
}

type DraftItem = { draft: RepairDraft; selected: boolean; hidden: boolean };
type SelectionMode = "element" | "text" | "multi" | "area" | "draw" | "layout_place" | "layout_source" | "layout_destination";
type OverlayTheme = "system" | "light" | "dark";
type LayoutCanvas = "page" | "wireframe";
type LayoutSource = { nodeId: string; label: string; originalRegion: Rect };

export type A3SReviewOverlayProps = {
  enabled?: boolean;
  defaultOpen?: boolean;
  autoSend?: boolean;
  onSubmitted?: (repairs: SubmittedRepair[]) => void;
};

export function A3SReviewOverlay({
  enabled = false,
  defaultOpen = false,
  autoSend = false,
  onSubmitted,
}: A3SReviewOverlayProps) {
  const context = useContext(TestKitContext);
  const candidateBridge = context.providerConfigured
    ? context.bridge
    : getPageContextBridge();
  const bridge = bridgeIsCompatible(candidateBridge) ? candidateBridge : null;
  const [host, setHost] = useState<HTMLElement | null>(null);
  const [mount, setMount] = useState<HTMLElement | null>(null);
  const [open, setOpen] = useState(defaultOpen);
  const [marking, setMarking] = useState(false);
  const [paused, setPaused] = useState(false);
  const [autoSendEnabled, setAutoSendEnabled] = useState(autoSend);
  const [mode, setMode] = useState<SelectionMode>("element");
  const [theme, setTheme] = useState<OverlayTheme>("system");
  const [layoutMode, setLayoutMode] = useState(false);
  const [layoutPurpose, setLayoutPurpose] = useState("");
  const [layoutCanvas, setLayoutCanvas] = useState<LayoutCanvas>("page");
  const [layoutComponentType, setLayoutComponentType] = useState("Section");
  const [layoutSource, setLayoutSource] = useState<LayoutSource | null>(null);
  const [layoutTarget, setLayoutTarget] = useState<Rect>({ x: 40, y: 120, width: 640, height: 240 });
  const [drafts, setDrafts] = useState<DraftItem[]>([]);
  const [repairs, setRepairs] = useState<SubmittedRepair[]>([]);
  const [candidate, setCandidate] = useState<RepairTarget | null>(null);
  const [editingDraftId, setEditingDraftId] = useState<string | null>(null);
  const [candidateLabel, setCandidateLabel] = useState("");
  const [instruction, setInstruction] = useState("");
  const [successCriteria, setSuccessCriteria] = useState("");
  const [severity, setSeverity] = useState<RepairSeverity>("important");
  const [intent, setIntent] = useState<RepairIntent>("fix");
  const [conflictingDraftIds, setConflictingDraftIds] = useState<string[]>([]);
  const [replyFindingId, setReplyFindingId] = useState<string | null>(null);
  const [restoreReplyFocusId, setRestoreReplyFocusId] = useState<string | null>(null);
  const [replyMessage, setReplyMessage] = useState("");
  const [announcement, setAnnouncement] = useState("");
  const [keyboardNodeIds, setKeyboardNodeIds] = useState<string[]>([]);
  const [highlight, setHighlight] = useState<DOMRect | null>(null);
  const [area, setArea] = useState<{ startX: number; startY: number; currentX: number; currentY: number } | null>(null);
  const [drawing, setDrawing] = useState<Array<{ x: number; y: number }> | null>(null);
  const areaRef = useRef(area);
  const drawingRef = useRef(drawing);
  const launchRef = useRef<HTMLButtonElement | null>(null);
  const panelRef = useRef<HTMLElement | null>(null);
  const replyTriggerRefs = useRef(new Map<string, HTMLButtonElement>());
  const focusPanelOnOpenRef = useRef(false);
  const restoreFocusRef = useRef<HTMLElement | null>(null);
  const lastApplicationFocusRef = useRef<HTMLElement | null>(null);
  const idPrefix = useId().replace(/:/g, "");

  areaRef.current = area;
  drawingRef.current = drawing;

  function closeOverlay() {
    setOpen(false);
    queueMicrotask(() => launchRef.current?.focus());
  }

  function openOverlay(focusPanel = false) {
    if (focusPanel) focusPanelOnOpenRef.current = true;
    setOpen(true);
  }

  function focusPanel() {
    queueMicrotask(() => panelRef.current?.focus());
  }

  function startMarking(value: SelectionMode) {
    restoreFocusRef.current = lastApplicationFocusRef.current;
    setEditingDraftId(null);
    setMode(value);
    setMarking(true);
    setKeyboardNodeIds([]);
    setConflictingDraftIds([]);
    setCandidate(value === "multi" ? { kind: "node", nodeIds: [] } : null);
    updateArea(null);
    updateDrawing(null);
    setHighlight(null);
    if (["element", "multi", "text", "layout_source"].includes(value)) {
      queueMicrotask(() => restoreFocusRef.current?.focus());
    }
  }

  function toggleLayoutMode() {
    const next = !layoutMode;
    if (marking) stopMarking(false);
    if (!next) setLayoutSource(null);
    setLayoutMode(next);
    announce(next ? "Layout mode enabled" : "Layout mode disabled");
  }

  function stopMarking(restoreFocus = true) {
    setMarking(false);
    updateArea(null);
    updateDrawing(null);
    setHighlight(null);
    setKeyboardNodeIds([]);
    if (restoreFocus) queueMicrotask(() => restoreFocusRef.current?.focus());
  }

  useEffect(() => {
    if (!enabled || !bridge || !document.body) return;
    const element = document.createElement("div");
    element.dataset.a3sTestkitOverlay = "";
    element.setAttribute("aria-label", "A3S Test review overlay");
    const shadow = element.attachShadow({ mode: "open" });
    const style = document.createElement("style");
    style.textContent = OVERLAY_CSS;
    const root = document.createElement("div");
    root.dataset.a3sTestkitOverlay = "";
    shadow.append(style, root);
    document.body.append(element);
    setHost(element);
    setMount(root);
    return () => {
      setMount(null);
      setHost(null);
      element.remove();
    };
  }, [bridge, enabled]);

  useEffect(() => {
    if (!bridge) return;
    setRepairs(bridge.listRepairs());
    return bridge.subscribe((event) => {
      if (event.type === "repair.submitted" || event.type === "repair.updated") {
        setRepairs(bridge.listRepairs());
      }
      if (event.type === "repair.updated") announce(repairAnnouncement(event.repair));
    });
  }, [bridge]);

  useLayoutEffect(() => {
    if (!open || !focusPanelOnOpenRef.current) return;
    focusPanelOnOpenRef.current = false;
    panelRef.current?.focus();
  }, [mount, open]);

  useLayoutEffect(() => {
    if (!restoreReplyFocusId || replyFindingId !== null) return;
    const trigger = replyTriggerRefs.current.get(restoreReplyFocusId);
    (trigger ?? panelRef.current)?.focus();
    setRestoreReplyFocusId(null);
  }, [repairs, replyFindingId, restoreReplyFocusId]);

  useEffect(() => {
    if (!enabled) return;
    const rememberApplicationFocus = (event: FocusEvent) => {
      const target = event.composedPath()[0];
      if (target instanceof HTMLElement && !isOverlayElement(target, host)) {
        lastApplicationFocusRef.current = target;
      }
    };
    document.addEventListener("focusin", rememberApplicationFocus, true);
    return () => document.removeEventListener("focusin", rememberApplicationFocus, true);
  }, [enabled, host]);

  useEffect(() => {
    if (!marking || !bridge) return;
    const onPointerMove = (event: PointerEvent) => {
      if (mode === "draw" && drawingRef.current) {
        updateDrawing(appendDrawingPoint(drawingRef.current, event.clientX, event.clientY));
        return;
      }
      if (["area", "multi", "layout_place", "layout_destination"].includes(mode) && areaRef.current) {
        updateArea({ ...areaRef.current, currentX: event.clientX, currentY: event.clientY });
        return;
      }
      if (!["element", "layout_source"].includes(mode)) return;
      const element = targetElement(event, host);
      setHighlight(element?.getBoundingClientRect() ?? null);
    };
    const onPointerDown = (event: PointerEvent) => {
      if (!(["area", "multi", "draw", "layout_place", "layout_destination"] as SelectionMode[]).includes(mode) || event.button !== 0 || isOverlayEvent(event, host)) return;
      event.preventDefault();
      event.stopPropagation();
      if (mode === "draw") {
        updateDrawing([{ x: event.clientX, y: event.clientY }]);
        return;
      }
      updateArea({ startX: event.clientX, startY: event.clientY, currentX: event.clientX, currentY: event.clientY });
    };
    const onPointerUp = (event: PointerEvent) => {
      if (event.button !== 0 || isOverlayEvent(event, host)) return;
      if (mode === "draw" && drawingRef.current) {
        event.preventDefault();
        event.stopPropagation();
        const points = appendDrawingPoint(drawingRef.current, event.clientX, event.clientY).slice(0, 2_000);
        const region = drawingBounds(points);
        const snapshot = bridge.snapshot({ detail: "summary", scope: { kind: "region", space: "viewport", ...region } });
        stageCandidate({ kind: "drawing", nodeIds: snapshot.nodes.map((node) => node.id), region, drawing: points }, `${snapshot.nodes.length} elements near drawing`);
        updateDrawing(null);
        updateArea(null);
        setHighlight(null);
        setMarking(false);
        return;
      }
      if ((mode === "layout_place" || mode === "layout_destination") && areaRef.current) {
        event.preventDefault();
        event.stopPropagation();
        const region = normalizedArea(areaRef.current.startX, areaRef.current.startY, event.clientX, event.clientY);
        updateArea(null);
        setHighlight(null);
        if (region.width < 8 || region.height < 8) {
          announce("Layout regions must be at least 8 by 8 CSS pixels");
          stopMarking(false);
          focusPanel();
          return;
        }
        if (mode === "layout_destination") {
          setLayoutTarget(region);
          announce("Layout destination updated");
          stopMarking(false);
          focusPanel();
          return;
        }
        const componentType = layoutComponentType.trim();
        if (!componentType) {
          announce("Choose a component type before drawing its placement");
          stopMarking(false);
          focusPanel();
          return;
        }
        stageCandidate(
          {
            kind: "region",
            nodeIds: [],
            region,
            layout: {
              kind: "placement",
              componentType,
              canvas: layoutCanvas,
              ...(layoutPurpose.trim() ? { purpose: layoutPurpose.trim() } : {}),
            },
          },
          `${componentType} placement`,
          {
            instruction: `Place ${componentType} in the selected layout region`,
            successCriteria: `${componentType} is visibly present within the requested layout region`,
            intent: "change",
          },
        );
        setMarking(false);
        return;
      }
      if ((mode === "area" || mode === "multi") && areaRef.current) {
        event.preventDefault();
        event.stopPropagation();
        const region = normalizedArea(areaRef.current.startX, areaRef.current.startY, event.clientX, event.clientY);
        const snapshot = bridge.snapshot({ detail: "summary", scope: { kind: "region", space: "viewport", ...region } });
        stageCandidate(
          mode === "multi"
            ? { kind: "node", nodeIds: snapshot.nodes.map((node) => node.id), region }
            : { kind: "region", nodeIds: snapshot.nodes.map((node) => node.id), region },
          `${snapshot.nodes.length} elements in ${mode === "multi" ? "selection" : "area"}`,
        );
        updateArea(null);
        setHighlight(null);
        setMarking(false);
        return;
      }
      if (mode === "text") {
        const selection = window.getSelection();
        const text = selection?.toString().trim() ?? "";
        const element = selectionElement(selection);
        if (!text || !element) return;
        event.preventDefault();
        event.stopPropagation();
        const node = nodeForElement(bridge, element);
        if (node) stageCandidate({ kind: "text", nodeIds: [node.id], selectedText: text.slice(0, 4_096) }, text.slice(0, 80));
        setMarking(false);
        return;
      }
      const element = targetElement(event, host);
      if (!element) return;
      event.preventDefault();
      event.stopPropagation();
      const node = nodeForElement(bridge, element);
      if (!node) return;
      if (mode === "layout_source") {
        selectLayoutSource(element, node);
        return;
      }
      if (mode === "multi") {
        setCandidate((current) => ({
          kind: "node",
          nodeIds: Array.from(new Set([...(current?.nodeIds ?? []), node.id])),
        }));
        setCandidateLabel((current) => current ? `${Number.parseInt(current, 10) + 1 || 2} selected elements` : "1 selected element");
      } else {
        stageCandidate({ kind: "node", nodeIds: [node.id] }, node.name ?? node.text ?? `<${node.tag}>`);
        setMarking(false);
      }
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        stopMarking();
        return;
      }
      if (!marking || !["element", "multi", "text", "layout_source"].includes(mode) || event.key !== "Enter") return;
      if (mode === "text") {
        const selection = window.getSelection();
        const text = selection?.toString().trim() ?? "";
        const element = selectionElement(selection);
        if (!text || !element) return;
        const node = nodeForElement(bridge, element);
        if (!node) return;
        event.preventDefault();
        event.stopPropagation();
        stageCandidate({ kind: "text", nodeIds: [node.id], selectedText: text.slice(0, 4_096) }, text.slice(0, 80));
        stopMarking(false);
        return;
      }
      const focused = deepActiveElement();
      if (!(focused instanceof Element) || isOverlayEvent(event, host)) return;
      const node = nodeForElement(bridge, focused);
      if (!node) return;
      event.preventDefault();
      event.stopPropagation();
      if (mode === "layout_source") {
        selectLayoutSource(focused, node);
      } else if (mode === "multi") {
        if (event.shiftKey && keyboardNodeIds.length > 0) {
          event.preventDefault();
          event.stopPropagation();
          stageCandidate({ kind: "node", nodeIds: keyboardNodeIds }, `${keyboardNodeIds.length} selected elements`);
          stopMarking(false);
          return;
        }
        const next = Array.from(new Set([...keyboardNodeIds, node.id]));
        setKeyboardNodeIds(next);
        setCandidate({ kind: "node", nodeIds: next });
        setCandidateLabel(`${next.length} selected element${next.length === 1 ? "" : "s"}`);
      } else {
        stageCandidate({ kind: "node", nodeIds: [node.id] }, node.name ?? node.text ?? `<${node.tag}>`);
        setMarking(false);
      }
    };
    document.addEventListener("pointermove", onPointerMove, true);
    document.addEventListener("pointerdown", onPointerDown, true);
    document.addEventListener("pointerup", onPointerUp, true);
    document.addEventListener("keydown", onKeyDown, true);
    return () => {
      document.removeEventListener("pointermove", onPointerMove, true);
      document.removeEventListener("pointerdown", onPointerDown, true);
      document.removeEventListener("pointerup", onPointerUp, true);
      document.removeEventListener("keydown", onKeyDown, true);
    };
  }, [bridge, host, keyboardNodeIds, layoutCanvas, layoutComponentType, layoutPurpose, marking, mode]);

  if (!enabled || !bridge || !mount) return null;

  const selectedCount = drafts.filter((item) => item.selected).length;
  const areaRect = area ? normalizedArea(area.startX, area.startY, area.currentX, area.currentY) : null;
  const drawingPath = drawing && drawing.length > 1 ? drawing.map((point, index) => `${index === 0 ? "M" : "L"}${point.x} ${point.y}`).join(" ") : null;
  const markers = [
    ...drafts.filter((item) => !item.hidden).map((item) => ({ id: item.draft.id, target: item.draft.target, status: "draft" as const })),
    ...repairs.map((repair) => ({ id: repair.id, target: repair.target, status: repair.status })),
  ];

  function stageCandidate(
    target: RepairTarget,
    label: string,
    defaults: { instruction?: string; successCriteria?: string; intent?: RepairIntent } = {},
  ) {
    setEditingDraftId(null);
    setCandidate(target);
    setCandidateLabel(label);
    setInstruction(defaults.instruction ?? "");
    setSuccessCriteria(defaults.successCriteria ?? "");
    if (defaults.intent) setIntent(defaults.intent);
    openOverlay();
  }

  function selectLayoutSource(element: Element, node: { id: string; name?: string; text?: string; tag: string }) {
    const originalRegion = rectValue(element.getBoundingClientRect());
    const label = node.name ?? node.text ?? `<${node.tag}>`;
    setLayoutSource({ nodeId: node.id, label, originalRegion });
    setLayoutTarget(originalRegion);
    setHighlight(null);
    stopMarking(false);
    announce(`Layout section selected: ${label}`);
    focusPanel();
  }

  function createRearrangeCandidate() {
    if (!layoutSource || !validLayoutRect(layoutTarget)) return;
    stageCandidate(
      {
        kind: "node",
        nodeIds: [layoutSource.nodeId],
        region: layoutTarget,
        layout: {
          kind: "rearrange",
          originalRegion: layoutSource.originalRegion,
          ...(layoutPurpose.trim() ? { purpose: layoutPurpose.trim() } : {}),
        },
      },
      `${layoutSource.label} rearrangement`,
      {
        instruction: `Move ${layoutSource.label} to the selected layout region`,
        successCriteria: `${layoutSource.label} appears within the requested layout region`,
        intent: "change",
      },
    );
  }

  function updateArea(value: typeof area) {
    areaRef.current = value;
    setArea(value);
  }

  function updateDrawing(value: typeof drawing) {
    drawingRef.current = value;
    setDrawing(value);
  }

  function saveDraft(send = false) {
    if (!candidate || !instruction.trim()) return;
    const wasEditing = editingDraftId !== null;
    const draft: RepairDraft = {
      id: editingDraftId ?? repairId(idPrefix),
      instruction: instruction.trim(),
      ...(successCriteria.trim() ? { successCriteria: successCriteria.trim() } : {}),
      intent,
      severity,
      ...(conflictingDraftIds.length > 0 ? {
        relations: conflictingDraftIds.map((findingId) => ({ kind: "conflicts_with" as const, findingId })),
      } : {}),
      target: candidate,
      createdAt: new Date().toISOString(),
    };
    setCandidate(null);
    setEditingDraftId(null);
    setMarking(false);
    setHighlight(null);
    setCandidateLabel("");
    setInstruction("");
    setConflictingDraftIds([]);
    if (send || autoSendEnabled) {
      submit([draft]);
    } else {
      setDrafts((current) => {
      const index = current.findIndex((item) => item.draft.id === draft.id);
      if (index < 0) return [...current, { draft, selected: true, hidden: false }];
      return current.map((item, itemIndex) => itemIndex === index ? { ...item, draft } : item);
      });
      announce(`${wasEditing ? "Draft updated" : "Draft added"}: ${draft.instruction}`);
      focusPanel();
    }
  }

  function editDraft(item: DraftItem) {
    const layout = item.draft.target.layout;
    if (layout) {
      setLayoutMode(true);
      setLayoutPurpose(layout.purpose ?? "");
      if (item.draft.target.region) setLayoutTarget(item.draft.target.region);
      if (layout.kind === "placement") {
        setLayoutCanvas(layout.canvas);
        setLayoutComponentType(layout.componentType);
        setLayoutSource(null);
      } else {
        const nodeId = item.draft.target.nodeIds[0];
        const node = nodeId
          ? bridge?.snapshot({ detail: "summary", scope: { kind: "node", nodeId } }).nodes.find((candidate) => candidate.id === nodeId)
          : undefined;
        if (nodeId) {
          setLayoutSource({
            nodeId,
            label: node?.name ?? node?.text ?? `<${node?.tag ?? "element"}>`,
            originalRegion: layout.originalRegion,
          });
        }
      }
    }
    setEditingDraftId(item.draft.id);
    setCandidate(item.draft.target);
    setCandidateLabel(targetSummary(item.draft.target));
    setInstruction(item.draft.instruction);
    setSuccessCriteria(item.draft.successCriteria ?? "");
    setSeverity(item.draft.severity);
    setIntent(item.draft.intent);
    setConflictingDraftIds(
      item.draft.relations
        ?.filter((relation) => relation.kind === "conflicts_with")
        .map((relation) => relation.findingId) ?? [],
    );
    openOverlay();
  }

  function submit(items: RepairDraft[]) {
    if (!bridge || items.length === 0) return;
    const submitted = bridge.submitRepair({ findings: items });
    const submittedIds = new Set(submitted.map((repair) => repair.id));
    setDrafts((current) => current.filter((item) => !submittedIds.has(item.draft.id)));
    setRepairs(bridge.listRepairs());
    onSubmitted?.(submitted);
    if (submitted.length > 0) {
      announce(`${submitted.length} finding${submitted.length === 1 ? "" : "s"} sent for repair`);
      focusPanel();
    }
  }

  function submitHumanAction(
    findingId: string,
    action: "reply" | "accept" | "dismiss" | "reopen",
    message?: string,
  ) {
    if (!bridge) return;
    if (!bridge.submitRepairAction({ findingId, action, ...(message?.trim() ? { message: message.trim() } : {}) })) return;
    announce(action === "reply" ? "Reply sent to the coding agent" : `Repair ${action} action queued`);
    if (action === "reply") {
      setRestoreReplyFocusId(findingId);
      setReplyFindingId(null);
      setReplyMessage("");
    }
  }

  function announce(message: string) {
    setAnnouncement("");
    queueMicrotask(() => setAnnouncement(message));
  }

  async function copyDrafts() {
    if (!bridge || drafts.length === 0) return;
    const selected = drafts.filter((item) => item.selected).map((item) => item.draft);
    const exported = bridge.exportRepairs(selected.length > 0 ? selected : drafts.map((item) => item.draft));
    await navigator.clipboard?.writeText(JSON.stringify(exported, null, 2));
  }

  async function copyDraftsMarkdown() {
    if (!bridge || drafts.length === 0) return;
    const selected = drafts.filter((item) => item.selected).map((item) => item.draft);
    await navigator.clipboard?.writeText(
      bridge.exportRepairsMarkdown(selected.length > 0 ? selected : drafts.map((item) => item.draft)),
    );
  }

  const content = (
    <div className="a3s-root" data-a3s-testkit-overlay="" data-theme={theme}>
      {layoutMode && layoutCanvas === "wireframe" && <div className="a3s-wireframe" aria-hidden="true" />}
      {(highlight || areaRect) && <div className="a3s-highlight" style={rectStyle(areaRect ?? highlight!)} aria-hidden="true" />}
      {layoutMode && layoutSource && !candidate && <div className="a3s-layout-target-preview" style={rectStyle(layoutTarget)} aria-hidden="true" />}
      {drawingPath && <svg className="a3s-drawing" aria-hidden="true"><path d={drawingPath} /></svg>}
      <div className="a3s-markers" aria-hidden="true">{markers.flatMap((marker) => markerRects(marker.target, bridge).map((rect, index) => <span key={`${marker.id}-${index}`} className={`a3s-marker status-${marker.status}`} style={rectStyle(rect)} />))}</div>
      <button ref={launchRef} className={`a3s-launch ${marking ? "is-active" : ""}`} type="button" onClick={() => open ? closeOverlay() : openOverlay(true)} aria-expanded={open} aria-controls={`${idPrefix}-review-panel`}>
        A3S Review{drafts.length + repairs.length > 0 ? ` · ${drafts.length + repairs.length}` : ""}
      </button>
      <div className="a3s-announcer" role="status" aria-live="polite" aria-atomic="true">{announcement}</div>
      {open && <aside ref={panelRef} id={`${idPrefix}-review-panel`} className="a3s-panel" aria-labelledby={`${idPrefix}-review-title`} aria-describedby={`${idPrefix}-review-description`} role="dialog" aria-modal="false" tabIndex={-1}>
        <header><div><strong id={`${idPrefix}-review-title`} className="a3s-panel-title">Review & repair</strong><small id={`${idPrefix}-review-description`} className="a3s-panel-description">Send bounded findings to the active A3S Test agent</small></div><button type="button" onClick={closeOverlay} aria-label="Close review overlay">×</button></header>
        <section className="a3s-tools" aria-label="Mark page">
          {(["element", "text", "multi", "area", "draw"] as SelectionMode[]).map((value) => <button key={value} type="button" aria-label={`Mark ${MODE_LABEL[value].toLowerCase()}`} aria-pressed={marking && mode === value} className={marking && mode === value ? "selected" : ""} onClick={() => startMarking(value)}>{MODE_LABEL[value]}</button>)}
          <button type="button" aria-pressed={layoutMode} className={layoutMode ? "selected" : ""} onClick={toggleLayoutMode}>Layout</button>
          <button type="button" aria-label={paused ? "Resume page animations" : "Pause page animations"} aria-pressed={paused} className={paused ? "selected" : ""} onClick={() => { const next = !paused; bridge?.setAnimationsPaused(next); setPaused(next); announce(next ? "Page animations paused" : "Page animations resumed"); }}>{paused ? "Resume" : "Pause"}</button>
          <button type="button" aria-label={`Turn auto-send ${autoSendEnabled ? "off" : "on"}`} aria-pressed={autoSendEnabled} className={autoSendEnabled ? "selected" : ""} onClick={() => setAutoSendEnabled((current) => !current)}>Auto-send · {autoSendEnabled ? "on" : "off"}</button>
          <button type="button" aria-label={`Change overlay theme; current theme is ${theme}`} onClick={() => setTheme((current) => current === "system" ? "light" : current === "light" ? "dark" : "system")}>Theme · {theme}</button>
          {marking && <button type="button" className="danger" onClick={() => stopMarking()}>Cancel</button>}
        </section>
        {layoutMode && <LayoutComposer
          idPrefix={idPrefix}
          purpose={layoutPurpose}
          canvas={layoutCanvas}
          componentType={layoutComponentType}
          source={layoutSource}
          target={layoutTarget}
          markingMode={marking ? mode : null}
          onPurpose={setLayoutPurpose}
          onCanvas={setLayoutCanvas}
          onComponentType={setLayoutComponentType}
          onPlace={() => startMarking("layout_place")}
          onSelectSource={() => startMarking("layout_source")}
          onDrawDestination={() => startMarking("layout_destination")}
          onTarget={(field, value) => setLayoutTarget((current) => ({ ...current, [field]: value }))}
          onCreateRearrange={createRearrangeCandidate}
        />}
        {marking && <p className="a3s-hint" role="status">{MODE_HINT[mode]} Press Esc to cancel.</p>}
        {candidate && <FindingEditor
          label={candidateLabel || `${candidate.nodeIds.length} selected elements`}
          instruction={instruction}
          successCriteria={successCriteria}
          severity={severity}
          intent={intent}
          conflictOptions={drafts
            .filter((item) => item.draft.id !== editingDraftId)
            .map((item) => ({
              id: item.draft.id,
              label: item.draft.instruction,
              checked: conflictingDraftIds.includes(item.draft.id),
            }))}
          onInstruction={setInstruction}
          onSuccessCriteria={setSuccessCriteria}
          onSeverity={setSeverity}
          onIntent={setIntent}
          onConflict={(findingId, checked) => setConflictingDraftIds((current) => checked
            ? [...new Set([...current, findingId])]
            : current.filter((candidate) => candidate !== findingId))}
          editing={Boolean(editingDraftId)}
          onCancel={() => { setCandidate(null); setEditingDraftId(null); setConflictingDraftIds([]); }}
          onSave={() => saveDraft(false)}
          onSend={() => saveDraft(true)}
        />}
        <section className="a3s-list" aria-label="Draft and submitted findings">
          {drafts.map((item) => <article key={item.draft.id} className={`a3s-item${item.hidden ? " is-hidden" : ""}`}>
            <label><input type="checkbox" aria-label={`Select draft: ${item.draft.instruction}`} checked={item.selected} onChange={(event) => setDrafts((current) => current.map((candidate) => candidate.draft.id === item.draft.id ? { ...candidate, selected: event.target.checked } : candidate))} /><span><strong>{item.draft.instruction}</strong><small>{targetSummary(item.draft.target)} · draft</small></span></label>
            <div><button type="button" aria-label={`Send draft for auto-fix: ${item.draft.instruction}`} onClick={() => submit([item.draft])}>Send and auto-fix</button><button type="button" className="quiet" aria-label={`Edit draft: ${item.draft.instruction}`} onClick={() => editDraft(item)}>Edit</button><button type="button" className="quiet" aria-label={`${item.hidden ? "Reopen" : "Hide"} marker for draft: ${item.draft.instruction}`} onClick={() => setDrafts((current) => current.map((candidate) => candidate.draft.id === item.draft.id ? { ...candidate, hidden: !candidate.hidden } : candidate))}>{item.hidden ? "Reopen marker" : "Hide marker"}</button><button type="button" className="quiet" aria-label={`Delete draft: ${item.draft.instruction}`} onClick={() => { setDrafts((current) => removeDraft(current, item.draft.id)); announce(`Draft deleted: ${item.draft.instruction}`); focusPanel(); }}>Delete</button></div>
          </article>)}
          {repairs.map((repair) => {
            const replies = bridge.listRepairReplies(repair.id);
            return <article key={repair.id} className="a3s-item submitted"><span className={`a3s-status status-${repair.status}`}>{statusLabel(repair.status)}</span><strong>{repair.instruction}</strong><small>{targetSummary(repair.target)} · revision {repair.contextRevision}</small>{replies.length > 0 && <ol className="a3s-thread" aria-label={`Repair conversation: ${repair.instruction}`}>{replies.map((reply) => <li key={reply.requestId}><span>{reply.actor}</span><p>{reply.message}</p></li>)}</ol>}{repair.status === "needs_input" && <div className="a3s-human-actions">{replyFindingId === repair.id ? <><label className="a3s-reply-label">Reply to the coding agent<textarea aria-label={`Reply to coding agent about: ${repair.instruction}`} autoFocus maxLength={8192} value={replyMessage} onChange={(event) => setReplyMessage(event.target.value)} /></label><button type="button" disabled={!replyMessage.trim()} onClick={() => submitHumanAction(repair.id, "reply", replyMessage)}>Send reply</button><button type="button" className="quiet" onClick={() => { setRestoreReplyFocusId(repair.id); setReplyFindingId(null); setReplyMessage(""); }}>Cancel reply</button></> : <button ref={(element) => { if (element) replyTriggerRefs.current.set(repair.id, element); else replyTriggerRefs.current.delete(repair.id); }} type="button" aria-label={`Reply about repair: ${repair.instruction}`} onClick={() => setReplyFindingId(repair.id)}>Reply</button>}</div>}{repair.status === "review_ready" && <div className="a3s-human-actions" aria-label={`Review repair: ${repair.instruction}`}><button type="button" aria-label={`Accept repair: ${repair.instruction}`} onClick={() => submitHumanAction(repair.id, "accept")}>Accept repair</button><button type="button" className="quiet" aria-label={`Reject repair: ${repair.instruction}`} onClick={() => submitHumanAction(repair.id, "dismiss")}>Reject</button><button type="button" className="quiet" aria-label={`Reopen repair: ${repair.instruction}`} onClick={() => submitHumanAction(repair.id, "reopen")}>Reopen</button></div>}{["resolved", "dismissed", "cancelled", "failed", "verification_failed"].includes(repair.status) && <div className="a3s-human-actions"><button type="button" className="quiet" aria-label={`Reopen repair: ${repair.instruction}`} onClick={() => submitHumanAction(repair.id, "reopen")}>Reopen</button></div>}</article>;
          })}
          {drafts.length === 0 && repairs.length === 0 && !candidate && <p className="a3s-empty">Choose a marking mode, select the page context, then describe the desired fix.</p>}
        </section>
        {drafts.length > 0 && <footer><button type="button" className="quiet" onClick={() => void copyDraftsMarkdown()}>Copy Markdown</button><button type="button" className="quiet" onClick={() => void copyDrafts()}>Copy JSON</button><button type="button" disabled={selectedCount === 0} onClick={() => submit(drafts.filter((item) => item.selected).map((item) => item.draft))}>Send selected ({selectedCount})</button><button type="button" onClick={() => submit(drafts.map((item) => item.draft))}>Send all</button></footer>}
      </aside>}
    </div>
  );
  return createPortal(content, mount);
}

type FindingEditorProps = {
  label: string;
  instruction: string;
  successCriteria: string;
  severity: RepairSeverity;
  intent: RepairIntent;
  conflictOptions: Array<{ id: string; label: string; checked: boolean }>;
  editing: boolean;
  onInstruction(value: string): void;
  onSuccessCriteria(value: string): void;
  onSeverity(value: RepairSeverity): void;
  onIntent(value: RepairIntent): void;
  onConflict(findingId: string, checked: boolean): void;
  onCancel(): void;
  onSave(): void;
  onSend(): void;
};

function FindingEditor(props: FindingEditorProps) {
  return <section className="a3s-editor">
    <small>Target · {props.label}</small>
    <label>Requested fix<textarea autoFocus maxLength={8192} value={props.instruction} onChange={(event) => props.onInstruction(event.target.value)} placeholder="Describe what should change" /></label>
    <label>Success criteria <span>optional</span><textarea maxLength={4096} value={props.successCriteria} onChange={(event) => props.onSuccessCriteria(event.target.value)} placeholder="What should be visibly true after the fix?" /></label>
    <div className="a3s-fields"><label>Severity<select value={props.severity} onChange={(event) => props.onSeverity(event.target.value as RepairSeverity)}><option value="blocking">Blocking</option><option value="important">Important</option><option value="suggestion">Suggestion</option></select></label><label>Intent<select value={props.intent} onChange={(event) => props.onIntent(event.target.value as RepairIntent)}><option value="fix">Fix</option><option value="change">Change</option><option value="question">Question</option><option value="approve">Approve</option></select></label></div>
    {props.conflictOptions.length > 0 && <fieldset className="a3s-conflicts"><legend>Conflicts with another draft <span>optional</span></legend><small>Select requests that cannot both be satisfied. A3S Test will ask for clarification without interpreting their wording.</small>{props.conflictOptions.map((option) => <label key={option.id}><input type="checkbox" checked={option.checked} onChange={(event) => props.onConflict(option.id, event.target.checked)} /><span>{option.label}</span></label>)}</fieldset>}
    <div className="a3s-actions"><button type="button" className="quiet" onClick={props.onCancel}>Cancel</button><button type="button" disabled={!props.instruction.trim()} onClick={props.onSave}>{props.editing ? "Save changes" : "Add draft"}</button><button type="button" disabled={!props.instruction.trim()} onClick={props.onSend}>Send and auto-fix</button></div>
  </section>;
}

type LayoutComposerProps = {
  idPrefix: string;
  purpose: string;
  canvas: LayoutCanvas;
  componentType: string;
  source: LayoutSource | null;
  target: Rect;
  markingMode: SelectionMode | null;
  onPurpose(value: string): void;
  onCanvas(value: LayoutCanvas): void;
  onComponentType(value: string): void;
  onPlace(): void;
  onSelectSource(): void;
  onDrawDestination(): void;
  onTarget(field: keyof Rect, value: number): void;
  onCreateRearrange(): void;
};

function LayoutComposer(props: LayoutComposerProps) {
  const helpId = `${props.idPrefix}-layout-help`;
  const updateNumber = (field: keyof Rect, value: number) => {
    if (Number.isFinite(value)) props.onTarget(field, value);
  };
  return <section className="a3s-layout" aria-label="Layout repair intent">
    <p id={helpId}>Describe placement and rearrangement as typed repair intent. The overlay previews coordinates without changing the page.</p>
    <label>Purpose <span>optional</span><input type="text" aria-label="Layout purpose" aria-describedby={helpId} maxLength={512} value={props.purpose} onChange={(event) => props.onPurpose(event.target.value)} placeholder="What should this layout help people do?" /></label>
    <div className="a3s-layout-fields">
      <label>Canvas<select aria-label="Layout canvas" value={props.canvas} onChange={(event) => props.onCanvas(event.target.value as LayoutCanvas)}><option value="page">Current page</option><option value="wireframe">Wireframe</option></select></label>
      <label>Component<input type="text" aria-label="Layout component type" maxLength={256} value={props.componentType} onChange={(event) => props.onComponentType(event.target.value)} placeholder="Section" /></label>
    </div>
    <button type="button" aria-pressed={props.markingMode === "layout_place"} className={props.markingMode === "layout_place" ? "selected" : ""} disabled={!props.componentType.trim()} onClick={props.onPlace}>Draw placement</button>
    <div className="a3s-layout-source">
      <span>Section to rearrange</span>
      <strong>{props.source?.label ?? "No section selected"}</strong>
      <button type="button" aria-pressed={props.markingMode === "layout_source"} className={props.markingMode === "layout_source" ? "selected" : ""} onClick={props.onSelectSource}>Select section on page</button>
    </div>
    <div className="a3s-layout-fields" aria-label="Layout destination in viewport CSS pixels">
      <label>X<input type="number" aria-label="Layout x" step="1" value={props.target.x} onChange={(event) => updateNumber("x", event.currentTarget.valueAsNumber)} /></label>
      <label>Y<input type="number" aria-label="Layout y" step="1" value={props.target.y} onChange={(event) => updateNumber("y", event.currentTarget.valueAsNumber)} /></label>
      <label>Width<input type="number" aria-label="Layout width" min="8" step="1" value={props.target.width} onChange={(event) => updateNumber("width", event.currentTarget.valueAsNumber)} /></label>
      <label>Height<input type="number" aria-label="Layout height" min="8" step="1" value={props.target.height} onChange={(event) => updateNumber("height", event.currentTarget.valueAsNumber)} /></label>
    </div>
    <div className="a3s-actions">
      <button type="button" className={props.markingMode === "layout_destination" ? "selected" : ""} aria-pressed={props.markingMode === "layout_destination"} disabled={!props.source} onClick={props.onDrawDestination}>Draw destination</button>
      <button type="button" disabled={!props.source || !validLayoutRect(props.target)} onClick={props.onCreateRearrange}>Create rearrange draft</button>
    </div>
  </section>;
}

function useLatest<T>(value: T) {
  const ref = useRef(value);
  ref.current = value;
  return ref;
}

function stableList(values: readonly string[] | undefined): string {
  return JSON.stringify(values ?? []);
}

function removeDraft(items: DraftItem[], findingId: string): DraftItem[] {
  return items
    .filter((item) => item.draft.id !== findingId)
    .map((item) => {
      const relations = item.draft.relations?.filter((relation) => relation.findingId !== findingId);
      if (relations?.length === item.draft.relations?.length) return item;
      const draft = { ...item.draft };
      if (relations?.length) draft.relations = relations;
      else delete draft.relations;
      return { ...item, draft };
    });
}

function bridgeIsCompatible(bridge: PageContextBridge | null): bridge is PageContextBridge {
  if (!bridge) return false;
  try {
    return bridge.probe().protocol === PAGE_CONTEXT_PROTOCOL;
  } catch {
    return false;
  }
}

function isOverlayEvent(event: Event, host: HTMLElement | null): boolean {
  return Boolean(host && event.composedPath().includes(host));
}

function isOverlayElement(element: Element, host: HTMLElement | null): boolean {
  return Boolean(host && (element === host || element.getRootNode() === host.shadowRoot));
}

function targetElement(event: PointerEvent, host: HTMLElement | null): Element | null {
  if (isOverlayEvent(event, host)) return null;
  return event.composedPath().find((item): item is Element => item instanceof Element && !item.closest("[data-a3s-testkit-overlay]")) ?? null;
}

function selectionElement(selection: Selection | null): Element | null {
  const node = selection?.anchorNode;
  return node instanceof Element ? node : node?.parentElement ?? null;
}

function deepActiveElement(): HTMLElement | null {
  let active: Element | null = document.activeElement;
  while (active?.shadowRoot?.activeElement) active = active.shadowRoot.activeElement;
  return active instanceof HTMLElement ? active : null;
}

function nodeForElement(bridge: PageContextBridge, element: Element) {
  const snapshot = bridge.snapshot({ detail: "forensic", limits: { nodes: 5_000 } });
  return snapshot.nodes.find((node) => bridge.resolve(node.id) === element) ?? null;
}

function normalizedArea(startX: number, startY: number, endX: number, endY: number) {
  return { x: Math.min(startX, endX), y: Math.min(startY, endY), width: Math.abs(endX - startX), height: Math.abs(endY - startY) };
}

function appendDrawingPoint(points: Array<{ x: number; y: number }>, x: number, y: number) {
  const previous = points.at(-1);
  if (previous && Math.hypot(previous.x - x, previous.y - y) < 2) return points;
  return [...points, { x, y }];
}

function drawingBounds(points: Array<{ x: number; y: number }>) {
  const xs = points.map((point) => point.x);
  const ys = points.map((point) => point.y);
  const left = Math.min(...xs);
  const top = Math.min(...ys);
  return { x: left, y: top, width: Math.max(1, Math.max(...xs) - left), height: Math.max(1, Math.max(...ys) - top) };
}

function rectStyle(rect: Pick<DOMRect, "x" | "y" | "width" | "height">): CSSProperties {
  return { left: rect.x, top: rect.y, width: rect.width, height: rect.height };
}

function rectValue(rect: Pick<DOMRect, "x" | "y" | "width" | "height">): Rect {
  return { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
}

function validLayoutRect(rect: Rect): boolean {
  return [rect.x, rect.y, rect.width, rect.height].every(Number.isFinite)
    && rect.width >= 8
    && rect.height >= 8;
}

function markerRects(target: RepairTarget, bridge: PageContextBridge | null): RectLike[] {
  if (!bridge) return [];
  if (target.layout && target.region) return [target.region];
  const nodeRects = target.nodeIds.flatMap((nodeId) => {
    const element = bridge.resolve(nodeId);
    return element ? [element.getBoundingClientRect()] : [];
  });
  return nodeRects.length > 0 ? nodeRects : target.region ? [target.region] : [];
}

type RectLike = Pick<DOMRect, "x" | "y" | "width" | "height">;

function repairId(prefix: string): string {
  return `finding-${prefix}-${globalThis.crypto?.randomUUID?.() ?? `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`}`;
}

function targetSummary(target: RepairTarget): string {
  if (target.layout?.kind === "placement") return `layout placement · ${target.layout.componentType} · ${target.layout.canvas}`;
  if (target.layout?.kind === "rearrange") return `layout rearrangement · ${target.nodeIds.length} element${target.nodeIds.length === 1 ? "" : "s"}`;
  if (target.kind === "text") return `text · ${target.selectedText?.slice(0, 36) ?? "selection"}`;
  if (target.kind === "region") return `area · ${target.nodeIds.length} elements`;
  if (target.kind === "drawing") return `drawing · ${target.nodeIds.length} elements`;
  return `${target.nodeIds.length} element${target.nodeIds.length === 1 ? "" : "s"}`;
}

function statusLabel(status: RepairStatus): string {
  return status.replaceAll("_", " ");
}

function repairAnnouncement(repair: SubmittedRepair): string {
  if (repair.status === "needs_input") return `Repair needs input: ${repair.instruction}`;
  if (repair.status === "review_ready") return `Repair ready for review: ${repair.instruction}`;
  return `Repair ${statusLabel(repair.status)}: ${repair.instruction}`;
}

const MODE_LABEL: Record<SelectionMode, string> = {
  element: "Element",
  text: "Text",
  multi: "Multi",
  area: "Area",
  draw: "Draw",
  layout_place: "Layout placement",
  layout_source: "Layout source",
  layout_destination: "Layout destination",
};
const MODE_HINT: Record<SelectionMode, string> = {
  element: "Click one element, or focus it and press Enter, to create a finding.",
  text: "Select text, then release the pointer.",
  multi: "Drag across elements, or focus each element and press Enter to add it; press Shift+Enter to finish.",
  area: "Optional pointer mode: drag a rectangle over the page.",
  draw: "Optional pointer mode: draw a freehand mark around the relevant page area.",
  layout_place: "Drag the intended component region in viewport CSS pixels.",
  layout_source: "Click a section, or focus it and press Enter, to choose what should move.",
  layout_destination: "Drag the intended destination region in viewport CSS pixels.",
};
