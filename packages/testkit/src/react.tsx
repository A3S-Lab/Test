import { useEffect, useId, useRef, useState, type CSSProperties } from "react";
import { createPortal } from "react-dom";
import { useBrowserLayoutEffect } from "./react-effect";
import { getPageContextBridge } from "./runtime";
import { DesignAuditCandidates, resolveDesignAuditCandidate, type DesignAuditCandidate, type DesignAuditSelection } from "./design-audit-candidates";
import { QualityCandidates, resolveQualityCandidate, type QualityCandidate, type QualitySelection } from "./quality-candidates";
import { loadReviewDrafts, reviewScope, saveReviewDrafts, type ReviewDraftItem } from "./review-storage";
import { FindingEditor, LayoutComposer, ReviewMarkingToolbar } from "./review-components";
import { ReviewDesignReferenceBoard, useReviewDesignReference } from "./review-design-reference";
import { ReviewSettings } from "./review-settings";
import { reviewEditorPlacement } from "./review-position";
import { useTestKitContext } from "./react-provider";
export { A3STestBoundary, A3STestKit, type A3STestBoundaryProps, type A3STestKitProps } from "./react-provider";
import { invokeCallback, useLatest, writeClipboard } from "./review-integration";
import { bridgeIsCompatible, deepActiveElement, isOverlayEvent, nodeForElement, selectionElement, targetElement } from "./review-dom";
import { REVIEW_KEY_SHORTCUTS, useGlobalReviewShortcuts, useHostPointerBlocking, useLastApplicationFocus } from "./review-input-policy";
import { useReviewOverlayHost } from "./review-overlay-host";
import type { A3SReviewOverlayProps } from "./review-overlay-types";
export type { A3SReviewCopyEvent, A3SReviewOverlayProps } from "./review-overlay-types";
export type { A3SReviewLocale, A3SReviewMessageKey, A3SReviewMessageOverrides } from "./review-locale";
import { DEFAULT_REVIEW_PREFERENCES, loadReviewPreferences, loadReviewTabHidden, saveReviewPreferences, saveReviewTabHidden, type ReviewPreferences } from "./review-preferences";
import { type LayoutCanvas, type LayoutSource, type SelectionMode } from "./review-model";
import { ReviewI18nProvider, reviewActorLabel, reviewModeHint, reviewRepairAnnouncement, reviewStatusLabel, reviewTargetSummary, useReviewI18nConfig } from "./review-locale";
import { appendDrawingPoint, drawingBounds, normalizedArea, rectStyle, rectValue, removeDraft, repairId, validLayoutRect } from "./review-utils";
import { ReviewMarkers } from "./review-markers";
import type { DesignAuditFinding, DesignAuditReportRecord, PageContextBridge, QualityFinding, QualityReportRecord, RepairDraft, RepairIntent, RepairSeverity, RepairTarget, Rect, SubmittedRepair } from "./types";
import { useLocalizedLayoutComponentType } from "./use-localized-layout-component-type";

type CandidateSource =
  | { kind: "quality"; selection: QualitySelection }
  | { kind: "design_audit"; selection: DesignAuditSelection };

export function A3SReviewOverlay({
  enabled = false,
  defaultOpen = false,
  autoSend = false,
  locale = "auto",
  messages,
  copyToClipboard,
  onCopied,
  onDraftAdded,
  onDraftUpdated,
  onDraftDeleted,
  onDraftsCleared,
  onSubmitted,
}: A3SReviewOverlayProps) {
  const reviewI18n = useReviewI18nConfig(locale, messages);
  const { t } = reviewI18n;
  const callbacks = useLatest({
    copyToClipboard,
    onCopied,
    onDraftAdded,
    onDraftUpdated,
    onDraftDeleted,
    onDraftsCleared,
    onSubmitted,
  });
  const context = useTestKitContext();
  const candidateBridge = context.providerConfigured
    ? context.bridge
    : getPageContextBridge();
  const bridge = bridgeIsCompatible(candidateBridge) ? candidateBridge : null;
  const [open, setOpen] = useState(defaultOpen);
  const [workspaceOpen, setWorkspaceOpen] = useState(false);
  const [marking, setMarking] = useState(false);
  const [paused, setPaused] = useState(false);
  const [markersVisible, setMarkersVisible] = useState(true);
  const [autoSendEnabled, setAutoSendEnabled] = useState(autoSend);
  const [mode, setMode] = useState<SelectionMode>("element");
  const [preferences, setPreferences] = useState<ReviewPreferences>(() => (
    typeof window === "undefined"
      ? { ...DEFAULT_REVIEW_PREFERENCES }
      : loadReviewPreferences()
  ));
  const [tabHidden, setTabHidden] = useState(() => (
    typeof window !== "undefined" && loadReviewTabHidden()
  ));
  const { host, mount } = useReviewOverlayHost(
    enabled && Boolean(bridge) && !tabHidden,
    bridge,
  );
  const [layoutMode, setLayoutMode] = useState(false);
  const [layoutPurpose, setLayoutPurpose] = useState("");
  const [layoutCanvas, setLayoutCanvas] = useState<LayoutCanvas>("page");
  const [layoutComponentType, setLayoutComponentType] = useLocalizedLayoutComponentType(reviewI18n.locale);
  const [layoutSource, setLayoutSource] = useState<LayoutSource | null>(null);
  const [layoutTarget, setLayoutTarget] = useState<Rect>({ x: 40, y: 120, width: 640, height: 240 });
  const [drafts, setDrafts] = useState<ReviewDraftItem[]>([]);
  const [repairs, setRepairs] = useState<SubmittedRepair[]>([]);
  const [qualityReports, setQualityReports] = useState<QualityReportRecord[]>([]);
  const [designAuditReports, setDesignAuditReports] = useState<DesignAuditReportRecord[]>([]);
  const [pendingCandidateSource, setPendingCandidateSource] = useState<CandidateSource | null>(null);
  const [candidateSource, setCandidateSource] = useState<CandidateSource | null>(null);
  const [candidate, setCandidate] = useState<RepairTarget | null>(null);
  const designReference = useReviewDesignReference();
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
  const restoredScopeRef = useRef<string | null>(null);
  const areaRef = useRef(area);
  const drawingRef = useRef(drawing);
  const launchRef = useRef<HTMLButtonElement | null>(null);
  const panelRef = useRef<HTMLElement | null>(null);
  const replyTriggerRefs = useRef(new Map<string, HTMLButtonElement>());
  const suppressHostClickRef = useRef<EventTarget | null>(null);
  const focusPanelOnOpenRef = useRef(false);
  const focusLauncherOnCloseRef = useRef(false);
  const restoreFocusRef = useRef<HTMLElement | null>(null);
  const hiddenFocusRef = useRef<HTMLElement | null>(null);
  const lastApplicationFocusRef = useLastApplicationFocus(enabled, host);
  const idPrefix = useId().replace(/:/g, "");

  areaRef.current = area;
  drawingRef.current = drawing;

  function closeOverlay() {
    focusLauncherOnCloseRef.current = true;
    setOpen(false);
  }

  function closeOverlayFromControl() {
    if (marking) cancelMarking(false);
    closeOverlay();
  }

  function openOverlay(focusPanel = false) {
    if (focusPanel) focusPanelOnOpenRef.current = true;
    setOpen(true);
  }

  function focusPanel() {
    queueMicrotask(() => panelRef.current?.focus());
  }

  function startMarking(value: SelectionMode, source: CandidateSource | null = null) {
    restoreFocusRef.current = lastApplicationFocusRef.current;
    setPendingCandidateSource(source);
    setCandidateSource(null);
    setEditingDraftId(null);
    setMode(value);
    setMarking(true);
    setKeyboardNodeIds([]);
    setConflictingDraftIds([]);
    designReference.clear();
    setCandidate(null);
    updateArea(null);
    updateDrawing(null);
    setHighlight(null);
    if (["element", "multi", "text", "layout_source"].includes(value)) {
      queueMicrotask(() => restoreFocusRef.current?.focus());
    }
  }

  function toggleLayoutMode() {
    const next = !layoutMode;
    if (marking) cancelMarking(false);
    if (!next) setLayoutSource(null);
    setLayoutMode(next);
    if (next) setWorkspaceOpen(true);
    announce(t(next ? "layoutModeEnabled" : "layoutModeDisabled"));
  }

  function togglePause() {
    const next = !paused;
    bridge?.setAnimationsPaused(next);
    setPaused(next);
    announce(t(next ? "animationsPaused" : "animationsResumed"));
  }

  function toggleMarkers() {
    const next = !markersVisible;
    setMarkersVisible(next);
    announce(t(next ? "markersShown" : "markersHidden"));
  }

  function changePreferences(value: ReviewPreferences) {
    setPreferences(value);
    if (typeof window !== "undefined") {
      saveReviewPreferences(value);
    }
  }

  function cycleTheme() {
    const theme = preferences.theme === "system"
      ? "light"
      : preferences.theme === "light" ? "dark" : "system";
    changePreferences({ ...preferences, theme });
  }

  function hideUntilTabRestart() {
    hiddenFocusRef.current = lastApplicationFocusRef.current;
    if (typeof window !== "undefined") {
      saveReviewTabHidden(true);
    }
    cancelMarking(false);
    bridge?.setAnimationsPaused(false);
    setPaused(false);
    setTabHidden(true);
  }

  useBrowserLayoutEffect(() => {
    if (!tabHidden) return;
    const applicationFocus = hiddenFocusRef.current;
    hiddenFocusRef.current = null;
    if (applicationFocus?.isConnected) {
      applicationFocus.focus({ preventScroll: true });
    }
  }, [tabHidden]);

  function stopMarking(restoreFocus = true) {
    setMarking(false);
    updateArea(null);
    updateDrawing(null);
    setHighlight(null);
    setKeyboardNodeIds([]);
    setPendingCandidateSource(null);
    if (restoreFocus) queueMicrotask(() => restoreFocusRef.current?.focus());
  }

  function clearCandidate() {
    setCandidate(null);
    setCandidateSource(null);
    designReference.clear();
    setEditingDraftId(null);
    setCandidateLabel("");
    setInstruction("");
    setSuccessCriteria("");
    setConflictingDraftIds([]);
  }

  function cancelMarking(restoreFocus = true) {
    stopMarking(restoreFocus);
    clearCandidate();
    if (marking) announce(t("markingCancelled"));
  }

  useEffect(() => {
    if (!bridge) return;
    const initialRepairs = bridge.listRepairs();
    const initialQualityReports = bridge.listQualityReports();
    const initialDesignAuditReports = bridge.listDesignAuditReports();
    setRepairs(initialRepairs);
    setQualityReports(initialQualityReports);
    setDesignAuditReports(initialDesignAuditReports);
    if (initialRepairs.length > 0 || initialQualityReports.length > 0 || initialDesignAuditReports.length > 0) setWorkspaceOpen(true);
    return bridge.subscribe((event) => {
      if (event.type === "context.revision" && restoredScopeRef.current !== null) {
        const scope = reviewScope(bridge);
        const encodedScope = `${scope.pageId}\u0000${scope.route}`;
        if (restoredScopeRef.current !== encodedScope) {
          restoredScopeRef.current = encodedScope;
          const restored = loadReviewDrafts(bridge);
          setCandidate(null);
          designReference.clear();
          setEditingDraftId(null);
          setMarking(false);
          setHighlight(null);
          setDrafts(restored);
          setWorkspaceOpen(restored.length > 0);
          announce(restored.length > 0
            ? t(restored.length === 1 ? "savedDraftRestoredForRouteOne" : "savedDraftRestoredForRouteMany", { count: restored.length })
            : t("draftsSwitchedRoute"));
        }
      }
      if (event.type === "repair.submitted" || event.type === "repair.updated") {
        setRepairs(bridge.listRepairs());
        setWorkspaceOpen(true);
      }
      if (event.type === "quality.reported") {
        setQualityReports(bridge.listQualityReports());
        setWorkspaceOpen(true);
        announce(event.report.findings.length > 0
          ? t(event.report.findings.length === 1 ? "contractFindingAvailableOne" : "contractFindingAvailableMany", { count: event.report.findings.length })
          : t("contractPassedCleared", { contract: event.report.contract }));
      }
      if (event.type === "quality.dismissed") {
        setQualityReports(bridge.listQualityReports());
      }
      if (event.type === "design_audit.reported") {
        setDesignAuditReports(bridge.listDesignAuditReports());
        setWorkspaceOpen(true);
        announce(event.report.findings.length > 0
          ? t(event.report.findings.length === 1 ? "designSuggestionAvailableOne" : "designSuggestionAvailableMany", { count: event.report.findings.length })
          : t("designSuggestionsCleared"));
      }
      if (event.type === "design_audit.dismissed") {
        setDesignAuditReports(bridge.listDesignAuditReports());
      }
      if (event.type === "repair.updated") announce(reviewRepairAnnouncement(t, event.repair));
    });
  }, [bridge, t]);

  useEffect(() => {
    if (!enabled || !bridge || !mount) return;
    const scope = reviewScope(bridge);
    const encodedScope = `${scope.pageId}\u0000${scope.route}`;
    if (restoredScopeRef.current === encodedScope) return;
    restoredScopeRef.current = encodedScope;
    const restored = loadReviewDrafts(bridge);
    setDrafts(restored);
    if (restored.length > 0) setWorkspaceOpen(true);
    if (restored.length > 0) {
      announce(t(restored.length === 1 ? "savedDraftRestoredOne" : "savedDraftRestoredMany", { count: restored.length }));
    }
  }, [bridge, enabled, mount, t]);

  useEffect(() => {
    if (!enabled || !bridge || !mount || restoredScopeRef.current === null) return;
    saveReviewDrafts(bridge, drafts);
  }, [bridge, drafts, enabled, mount]);

  useBrowserLayoutEffect(() => {
    if (!open || !focusPanelOnOpenRef.current) return;
    focusPanelOnOpenRef.current = false;
    panelRef.current?.focus();
  }, [mount, open]);

  useBrowserLayoutEffect(() => {
    if (open || !focusLauncherOnCloseRef.current) return;
    focusLauncherOnCloseRef.current = false;
    const focusLauncher = () => launchRef.current?.focus({ preventScroll: true });
    focusLauncher();
    const frame = window.requestAnimationFrame(focusLauncher);
    return () => window.cancelAnimationFrame(frame);
  }, [mount, open]);

  useBrowserLayoutEffect(() => {
    if (!restoreReplyFocusId || replyFindingId !== null) return;
    const trigger = replyTriggerRefs.current.get(restoreReplyFocusId);
    (trigger ?? panelRef.current)?.focus();
    setRestoreReplyFocusId(null);
  }, [repairs, replyFindingId, restoreReplyFocusId]);

  useHostPointerBlocking(
    enabled && Boolean(bridge) && !tabHidden && preferences.blockInteractions,
    host,
  );
  useEffect(() => {
    if (!enabled || !bridge || !mount) return;
    const suppressSelectedClick = (event: MouseEvent) => {
      const selectedTarget = suppressHostClickRef.current;
      if (!selectedTarget) return;
      suppressHostClickRef.current = null;
      if (isOverlayEvent(event, host) || !event.composedPath().includes(selectedTarget)) return;
      event.preventDefault();
      event.stopPropagation();
      event.stopImmediatePropagation();
    };
    document.addEventListener("click", suppressSelectedClick, true);
    return () => document.removeEventListener("click", suppressSelectedClick, true);
  }, [bridge, enabled, host, mount]);
  useGlobalReviewShortcuts({
    active: enabled && Boolean(bridge) && !tabHidden && !designReference.boardOpen,
    open,
    marking,
    candidate: candidate !== null,
    hasDrafts: drafts.length > 0,
    onToggleOverlay: () => open ? closeOverlay() : openOverlay(true),
    onCancelMarking: cancelMarking,
    onCancelCandidate: () => {
      clearCandidate();
      focusPanel();
    },
    onCloseOverlay: closeOverlay,
    onStartMarking: startMarking,
    onToggleLayout: toggleLayoutMode,
    onTogglePause: togglePause,
    onToggleMarkers: toggleMarkers,
    onCopyDrafts: () => void copyDrafts("markdown"),
    onClearDrafts: clearDrafts,
  });

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
        suppressHostClickRef.current = event.target;
        event.preventDefault();
        event.stopPropagation();
        const points = appendDrawingPoint(drawingRef.current, event.clientX, event.clientY).slice(0, 2_000);
        const region = drawingBounds(points);
        const snapshot = bridge.snapshot({ detail: "summary", scope: { kind: "region", space: "viewport", ...region } });
        stageCandidate({ kind: "drawing", nodeIds: snapshot.nodes.map((node) => node.id), region, drawing: points }, t("elementsNearDrawing", { count: snapshot.nodes.length }));
        updateDrawing(null);
        updateArea(null);
        setHighlight(null);
        setMarking(false);
        return;
      }
      if ((mode === "layout_place" || mode === "layout_destination") && areaRef.current) {
        suppressHostClickRef.current = event.target;
        event.preventDefault();
        event.stopPropagation();
        const region = normalizedArea(areaRef.current.startX, areaRef.current.startY, event.clientX, event.clientY);
        updateArea(null);
        setHighlight(null);
        if (region.width < 8 || region.height < 8) {
          announce(t("layoutRegionMinimum"));
          stopMarking(false);
          focusPanel();
          return;
        }
        if (mode === "layout_destination") {
          setLayoutTarget(region);
          announce(t("layoutDestinationUpdated"));
          stopMarking(false);
          focusPanel();
          return;
        }
        const componentType = layoutComponentType.trim();
        if (!componentType) {
          announce(t("chooseComponentBeforePlacement"));
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
          t("layoutPlacementLabel", { component: componentType }),
          {
            instruction: t("placeComponentInstruction", { component: componentType }),
            successCriteria: t("placeComponentCriteria", { component: componentType }),
            intent: "change",
          },
        );
        setMarking(false);
        return;
      }
      if ((mode === "area" || mode === "multi") && areaRef.current) {
        suppressHostClickRef.current = event.target;
        event.preventDefault();
        event.stopPropagation();
        const region = normalizedArea(areaRef.current.startX, areaRef.current.startY, event.clientX, event.clientY);
        const snapshot = bridge.snapshot({ detail: "summary", scope: { kind: "region", space: "viewport", ...region } });
        stageCandidate(
          mode === "multi"
            ? { kind: "node", nodeIds: snapshot.nodes.map((node) => node.id), region }
            : { kind: "region", nodeIds: snapshot.nodes.map((node) => node.id), region },
          t(mode === "multi" ? "elementsInSelection" : "elementsInArea", { count: snapshot.nodes.length }),
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
        suppressHostClickRef.current = event.target;
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
      suppressHostClickRef.current = element;
      if (mode === "layout_source") {
        selectLayoutSource(element, node);
        return;
      }
      if (mode === "multi") {
        setCandidate((current) => ({
          kind: "node",
          nodeIds: Array.from(new Set([...(current?.nodeIds ?? []), node.id])),
        }));
      } else {
        if (pendingCandidateSource) {
          stagePendingCandidate(pendingCandidateSource, node.id);
        } else {
          stageCandidate({ kind: "node", nodeIds: [node.id] }, node.name ?? node.text ?? `<${node.tag}>`);
        }
        setMarking(false);
      }
    };
    const onKeyDown = (event: KeyboardEvent) => {
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
          stageCandidate({ kind: "node", nodeIds: keyboardNodeIds }, t(keyboardNodeIds.length === 1 ? "selectedElementsOne" : "selectedElementsMany", { count: keyboardNodeIds.length }));
          stopMarking(false);
          return;
        }
        const next = Array.from(new Set([...keyboardNodeIds, node.id]));
        setKeyboardNodeIds(next);
        const selectedLabel = t(next.length === 1 ? "selectedElementsOne" : "selectedElementsMany", { count: next.length });
        setCandidateLabel(selectedLabel);
        announce(t("selectionFinish", { count: selectedLabel }));
      } else {
        if (pendingCandidateSource) {
          stagePendingCandidate(pendingCandidateSource, node.id);
        } else {
          stageCandidate({ kind: "node", nodeIds: [node.id] }, node.name ?? node.text ?? `<${node.tag}>`);
        }
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
  }, [bridge, host, keyboardNodeIds, layoutCanvas, layoutComponentType, layoutPurpose, marking, mode, pendingCandidateSource, t]);

  if (!enabled || !bridge || !mount || tabHidden) return null;

  const selectedCount = drafts.filter((item) => item.selected).length;
  const areaRect = area ? normalizedArea(area.startX, area.startY, area.currentX, area.currentY) : null;
  const drawingPath = drawing && drawing.length > 1 ? drawing.map((point, index) => `${index === 0 ? "M" : "L"}${point.x} ${point.y}`).join(" ") : null;
  function stageCandidate(
    target: RepairTarget,
    label: string,
    defaults: { instruction?: string; successCriteria?: string; intent?: RepairIntent } = {},
    source: CandidateSource | null = null,
  ) {
    setPendingCandidateSource(null);
    setCandidateSource(source);
    setEditingDraftId(null);
    setCandidate(target);
    designReference.clear();
    setCandidateLabel(label);
    setInstruction(defaults.instruction ?? "");
    setSuccessCriteria(defaults.successCriteria ?? "");
    if (defaults.intent) setIntent(defaults.intent);
    openOverlay();
  }

  function reviewQualityFinding(reportId: string, finding: QualityFinding) {
    if (!bridge) return;
    const selection = { reportId, finding };
    const resolved = resolveQualityCandidate(bridge, selection, undefined, t);
    if (!resolved) {
      announce(t("chooseContractTarget"));
      startMarking("element", { kind: "quality", selection });
      return;
    }
    stageQualityCandidate(resolved);
  }

  function stageQualityCandidate(resolved: QualityCandidate) {
    stageCandidate(
      resolved.target,
      resolved.label,
      {
        instruction: resolved.instruction,
        successCriteria: resolved.successCriteria,
        intent: resolved.intent,
      },
      { kind: "quality", selection: resolved.selection },
    );
    setSeverity(resolved.severity);
  }

  function reviewDesignAuditFinding(reportId: string, finding: DesignAuditFinding) {
    if (!bridge) return;
    const selection = { reportId, finding };
    const resolved = resolveDesignAuditCandidate(bridge, selection, undefined, t);
    if (!resolved) {
      announce(t("chooseDesignTarget"));
      startMarking("element", { kind: "design_audit", selection });
      return;
    }
    stageDesignAuditCandidate(resolved);
  }

  function stageDesignAuditCandidate(resolved: DesignAuditCandidate) {
    stageCandidate(
      resolved.target,
      resolved.label,
      {
        instruction: resolved.instruction,
        successCriteria: resolved.successCriteria,
        intent: resolved.intent,
      },
      { kind: "design_audit", selection: resolved.selection },
    );
    setSeverity(resolved.severity);
  }

  function stagePendingCandidate(source: CandidateSource, nodeId: string) {
    if (!bridge) return;
    if (source.kind === "quality") {
      const resolved = resolveQualityCandidate(bridge, source.selection, nodeId, t);
      if (resolved) stageQualityCandidate(resolved);
      return;
    }
    const resolved = resolveDesignAuditCandidate(bridge, source.selection, nodeId, t);
    if (resolved) stageDesignAuditCandidate(resolved);
  }

  function selectLayoutSource(element: Element, node: { id: string; name?: string; text?: string; tag: string }) {
    const originalRegion = rectValue(element.getBoundingClientRect());
    const label = node.name ?? node.text ?? `<${node.tag}>`;
    setLayoutSource({ nodeId: node.id, label, originalRegion });
    setLayoutTarget(originalRegion);
    setHighlight(null);
    stopMarking(false);
    announce(t("layoutSectionSelected", { label }));
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
      t("layoutRearrangementLabel", { label: layoutSource.label }),
      {
        instruction: t("moveSectionInstruction", { label: layoutSource.label }),
        successCriteria: t("moveSectionCriteria", { label: layoutSource.label }),
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
      ...(designReference.reference ? { designReference: designReference.reference } : {}),
      target: candidate,
      createdAt: new Date().toISOString(),
    };
    setCandidate(null);
    const source = candidateSource;
    setCandidateSource(null);
    setEditingDraftId(null);
    setMarking(false);
    setHighlight(null);
    setCandidateLabel("");
    setInstruction("");
    designReference.clear();
    setConflictingDraftIds([]);
    if (send || autoSendEnabled) {
      const submitted = submit([draft]);
      if (submitted.some((repair) => repair.id === draft.id)) dismissCandidateSource(source);
    } else {
      setWorkspaceOpen(true);
      setDrafts((current) => {
      const index = current.findIndex((item) => item.draft.id === draft.id);
      if (index < 0) return [...current, { draft, selected: true, hidden: false }];
      return current.map((item, itemIndex) => itemIndex === index ? { ...item, draft } : item);
      });
      invokeCallback(wasEditing ? callbacks.current.onDraftUpdated : callbacks.current.onDraftAdded, structuredClone(draft));
      dismissCandidateSource(source);
      announce(t(wasEditing ? "draftUpdated" : "draftAdded", { message: draft.instruction }));
      focusPanel();
    }
  }

  function deleteDraft(draft: RepairDraft) {
    setDrafts((current) => removeDraft(current, draft.id));
    if (editingDraftId === draft.id) {
      setCandidate(null);
      designReference.clear();
      setEditingDraftId(null);
      setConflictingDraftIds([]);
    }
    invokeCallback(callbacks.current.onDraftDeleted, structuredClone(draft));
    announce(t("draftDeleted", { message: draft.instruction }));
    focusPanel();
  }

  function clearDrafts() {
    if (drafts.length === 0) return;
    const cleared = drafts.map((item) => structuredClone(item.draft));
    setDrafts([]);
    setCandidate(null);
    designReference.clear();
    setEditingDraftId(null);
    setConflictingDraftIds([]);
    invokeCallback(callbacks.current.onDraftsCleared, cleared);
    announce(t(cleared.length === 1 ? "draftClearedOne" : "draftClearedMany", { count: cleared.length }));
    focusPanel();
  }

  function clearCopiedDrafts(copied: RepairDraft[]) {
    if (copied.length === 0) return;
    const copiedIds = new Set(copied.map((draft) => draft.id));
    setDrafts((current) => current.filter((item) => !copiedIds.has(item.draft.id)));
    if (editingDraftId && copiedIds.has(editingDraftId)) {
      setCandidate(null);
      designReference.clear();
      setEditingDraftId(null);
      setConflictingDraftIds([]);
    }
    invokeCallback(
      callbacks.current.onDraftsCleared,
      copied.map((draft) => structuredClone(draft)),
    );
    announce(t(copied.length === 1 ? "copiedDraftClearedOne" : "copiedDraftClearedMany", { count: copied.length }));
    focusPanel();
  }

  function editDraft(item: ReviewDraftItem) {
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
    designReference.load(item.draft.designReference ?? null);
    setCandidateLabel(reviewTargetSummary(t, item.draft.target));
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

  function submit(items: RepairDraft[]): SubmittedRepair[] {
    if (!bridge || items.length === 0) return [];
    const submitted = bridge.submitRepair({ findings: items });
    const submittedIds = new Set(submitted.map((repair) => repair.id));
    setDrafts((current) => current.filter((item) => !submittedIds.has(item.draft.id)));
    setRepairs(bridge.listRepairs());
    invokeCallback(callbacks.current.onSubmitted, structuredClone(submitted));
    if (submitted.length > 0) {
      setWorkspaceOpen(true);
      announce(t(submitted.length === 1 ? "findingSentOne" : "findingSentMany", { count: submitted.length }));
      focusPanel();
    }
    return submitted;
  }

  function dismissCandidateSource(source: CandidateSource | null) {
    if (!bridge || !source) return;
    if (source.kind === "quality") {
      bridge.dismissQualityFinding(source.selection.reportId, source.selection.finding.id);
      setQualityReports(bridge.listQualityReports());
      return;
    }
    bridge.dismissDesignAuditFinding(source.selection.reportId, source.selection.finding.id);
    setDesignAuditReports(bridge.listDesignAuditReports());
  }

  function submitHumanAction(
    findingId: string,
    action: "reply" | "accept" | "dismiss" | "reopen",
    message?: string,
  ) {
    if (!bridge) return;
    if (!bridge.submitRepairAction({ findingId, action, ...(message?.trim() ? { message: message.trim() } : {}) })) return;
    const localizedAction = action === "accept" ? t("actionAccept") : action === "dismiss" ? t("actionDismiss") : action === "reopen" ? t("actionReopen") : action;
    announce(action === "reply" ? t("replySent") : t("repairActionQueued", { action: localizedAction }));
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

  async function copyDrafts(format: "markdown" | "json" = "json") {
    if (!bridge || drafts.length === 0) return;
    const selected = drafts.filter((item) => item.selected).map((item) => item.draft);
    const copied = selected.length > 0 ? selected : drafts.map((item) => item.draft);
    const text = format === "markdown"
      ? bridge.exportRepairsMarkdown(copied)
      : JSON.stringify(bridge.exportRepairs(copied), null, 2);
    if (!await writeClipboard(text, callbacks.current.copyToClipboard)) return;
    invokeCallback(callbacks.current.onCopied, { format, text, drafts: structuredClone(copied) });
    if (preferences.clearOnCopy) clearCopiedDrafts(copied);
  }

  const rootStyle = {
    "--a3s-marker-color": preferences.markerColor,
    "--a3s-wireframe-fade": String(preferences.wireframeFade),
  } as CSSProperties;
  const editorPlacement = candidate && open
    ? reviewEditorPlacement(candidate, bridge)
    : null;
  const editorStyle = editorPlacement
    ? {
        "--a3s-editor-left": `${editorPlacement.left}px`,
        "--a3s-editor-top": `${editorPlacement.top}px`,
      } as CSSProperties
    : undefined;
  const findingCount = drafts.length
    + repairs.length
    + qualityReports.reduce((count, report) => count + report.findings.length, 0)
    + designAuditReports.reduce((count, report) => count + report.findings.length, 0);
  const content = (
    <ReviewI18nProvider value={reviewI18n}>
    <div className="a3s-root" data-a3s-testkit-overlay="" data-theme={preferences.theme} data-dock={preferences.dock} lang={reviewI18n.locale} style={rootStyle}>
      {layoutMode && layoutCanvas === "wireframe" && <div className="a3s-wireframe" aria-hidden="true" />}
      {(highlight || areaRect) && <div className="a3s-highlight" style={rectStyle(areaRect ?? highlight!)} aria-hidden="true"><span>01</span></div>}
      {editorPlacement && <div className="a3s-highlight is-candidate" style={rectStyle(editorPlacement.rect)} aria-hidden="true"><span>01</span></div>}
      {layoutMode && layoutSource && !candidate && <div className="a3s-layout-target-preview" style={rectStyle(layoutTarget)} aria-hidden="true" />}
      {drawingPath && <svg className="a3s-drawing" aria-hidden="true"><path d={drawingPath} /></svg>}
      <ReviewMarkers visible={markersVisible} bridge={bridge} drafts={drafts} repairs={repairs} qualityReports={qualityReports} designAuditReports={designAuditReports} onEditDraft={editDraft} />
      <button ref={launchRef} className={`a3s-launch${marking ? " is-active" : ""}${open ? " is-open" : ""}`} type="button" title={t("toggleReviewOverlay")} onClick={() => open ? closeOverlayFromControl() : openOverlay(true)} aria-expanded={open} aria-controls={`${idPrefix}-review-panel`} aria-keyshortcuts={REVIEW_KEY_SHORTCUTS.toggle}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="8.4" /><path d="m7.3 16 3.9-9.2c.3-.8 1.4-.8 1.8 0l3.8 9.2M9.2 12.5h5.7" /><path d="M4.8 15.4c3-2.5 6.3-2.8 9.7-.9 1.7.9 3.4.3 5.2-1.3-1 4.7-4 7.1-8.5 7.1" /></svg>
        <span className="a3s-sr-only">{t("reviewLauncher")}</span>
        {findingCount > 0 && <span className="a3s-launch-count" aria-hidden="true">{findingCount}</span>}
      </button>
      <div className="a3s-announcer" role="status" aria-live="polite" aria-atomic="true">{announcement}</div>
      <ReviewDesignReferenceBoard active={candidate !== null} design={designReference} idPrefix={idPrefix} theme={preferences.theme} onAnnounce={announce} />
      {open && !designReference.boardOpen && <>
        {candidate && <div className="a3s-editor-popover" data-side={editorPlacement?.side ?? "right"} style={editorStyle}><FindingEditor
          label={candidateLabel || t(candidate.nodeIds.length === 1 ? "selectedElementsOne" : "selectedElementsMany", { count: candidate.nodeIds.length })}
          instruction={instruction} successCriteria={successCriteria} severity={severity}
          intent={intent} designReference={designReference.reference}
          conflictOptions={drafts
            .filter((item) => item.draft.id !== editingDraftId)
            .map((item) => ({ id: item.draft.id, label: item.draft.instruction, checked: conflictingDraftIds.includes(item.draft.id) }))}
          onInstruction={setInstruction} onSuccessCriteria={setSuccessCriteria}
          onSeverity={setSeverity} onIntent={setIntent}
          onOpenDesignBoard={designReference.open}
          onRemoveDesignReference={() => { designReference.remove(); announce("Design reference removed"); }}
          onConflict={(findingId, checked) => setConflictingDraftIds((current) => checked
            ? [...new Set([...current, findingId])]
            : current.filter((candidate) => candidate !== findingId))}
          editing={Boolean(editingDraftId)}
          onCancel={() => { clearCandidate(); focusPanel(); }}
          {...(editingDraftId ? { onDelete: () => { const item = drafts.find((candidate) => candidate.draft.id === editingDraftId); if (item) deleteDraft(item.draft); } } : {})}
          onSave={() => saveDraft(false)} onSend={() => saveDraft(true)}
        /></div>}
        <aside ref={panelRef} id={`${idPrefix}-review-panel`} className="a3s-panel" aria-labelledby={`${idPrefix}-review-title`} aria-describedby={`${idPrefix}-review-description`} aria-keyshortcuts={REVIEW_KEY_SHORTCUTS.escape} role="dialog" aria-modal="false" tabIndex={-1}>
          <div className="a3s-command-bar">
            <header><span className="a3s-panel-mark" aria-hidden="true"><svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="8.4" /><path d="m7.3 16 3.9-9.2c.3-.8 1.4-.8 1.8 0l3.8 9.2M9.2 12.5h5.7" /><path d="M4.8 15.4c3-2.5 6.3-2.8 9.7-.9 1.7.9 3.4.3 5.2-1.3-1 4.7-4 7.1-8.5 7.1" /></svg></span><span><strong id={`${idPrefix}-review-title`} className="a3s-panel-title">{t("reviewTitle")}</strong><small id={`${idPrefix}-review-description`} className="a3s-panel-description">{t("reviewDescription")}</small></span><button type="button" className="a3s-close" onClick={closeOverlayFromControl} aria-label={t("closeReviewOverlay")}><svg viewBox="0 0 16 16" aria-hidden="true"><path d="M3.5 3.5 12.5 12.5M12.5 3.5 3.5 12.5" /></svg></button></header>
            <ReviewMarkingToolbar marking={marking} mode={mode} layoutMode={layoutMode} paused={paused} markersVisible={markersVisible} autoSendEnabled={autoSendEnabled} theme={preferences.theme} findingCount={findingCount} workspaceOpen={workspaceOpen} settings={<ReviewSettings preferences={preferences} onChange={changePreferences} onHideUntilRestart={hideUntilTabRestart} />} onStartMarking={startMarking} onToggleLayout={toggleLayoutMode} onTogglePause={togglePause} onToggleMarkers={toggleMarkers} onToggleAutoSend={() => setAutoSendEnabled((current) => !current)} onCycleTheme={cycleTheme} onToggleWorkspace={() => setWorkspaceOpen((current) => !current)} onCancelMarking={cancelMarking} />
          </div>
          {marking && <p className="a3s-hint" role="status">{reviewModeHint(t, mode)} {t("pressEscapeToCancel")}</p>}
          <section className="a3s-workspace" hidden={!workspaceOpen} aria-label={t("reviewWorkspace")}>
            <header className="a3s-workspace-header"><span><strong>{t("reviewWorkspace")}</strong><small>{findingCount > 0 ? t("inThisPage", { count: findingCount }) : t("noSavedFindings")}</small></span><button type="button" className="a3s-close" aria-label={t("closeFindings")} onClick={() => setWorkspaceOpen(false)}><svg viewBox="0 0 16 16" aria-hidden="true"><path d="M3.5 3.5 12.5 12.5M12.5 3.5 3.5 12.5" /></svg></button></header>
            <div className="a3s-workspace-scroll">
              {layoutMode && <LayoutComposer idPrefix={idPrefix} purpose={layoutPurpose} canvas={layoutCanvas}
                componentType={layoutComponentType} source={layoutSource} target={layoutTarget}
                markingMode={marking ? mode : null}
                onPurpose={setLayoutPurpose} onCanvas={setLayoutCanvas} onComponentType={setLayoutComponentType}
                onPlace={() => startMarking("layout_place")}
                onSelectSource={() => startMarking("layout_source")}
                onDrawDestination={() => startMarking("layout_destination")}
                onTarget={(field, value) => setLayoutTarget((current) => ({ ...current, [field]: value }))}
                onCreateRearrange={createRearrangeCandidate}
              />}
              <QualityCandidates reports={qualityReports} onReview={reviewQualityFinding} onDismiss={(reportId, findingId) => bridge.dismissQualityFinding(reportId, findingId)} />
              <DesignAuditCandidates reports={designAuditReports} onReview={reviewDesignAuditFinding} onDismiss={(reportId, findingId) => bridge.dismissDesignAuditFinding(reportId, findingId)} />
              <section className="a3s-list" aria-label={t("draftAndSubmittedFindings")} tabIndex={0}>
          {drafts.map((item) => <article key={item.draft.id} className={`a3s-item${item.hidden ? " is-hidden" : ""}`}>
            <label><input type="checkbox" aria-label={t("selectDraft", { message: item.draft.instruction })} checked={item.selected} onChange={(event) => setDrafts((current) => current.map((candidate) => candidate.draft.id === item.draft.id ? { ...candidate, selected: event.target.checked } : candidate))} /><span><strong>{item.draft.instruction}</strong><small>{reviewTargetSummary(t, item.draft.target)}{item.draft.designReference ? ` · ${item.draft.designReference.kind} reference` : ""} · {t("draft")}</small></span></label>
            <div><button type="button" aria-label={t("sendDraftAutoFix", { message: item.draft.instruction })} onClick={() => submit([item.draft])}>{t("sendAndAutoFix")}</button><button type="button" className="quiet" aria-label={t("editDraftAction", { message: item.draft.instruction })} onClick={() => editDraft(item)}>{t("edit")}</button><button type="button" className="quiet" aria-label={t(item.hidden ? "reopenMarkerForDraft" : "hideMarkerForDraft", { message: item.draft.instruction })} onClick={() => setDrafts((current) => current.map((candidate) => candidate.draft.id === item.draft.id ? { ...candidate, hidden: !candidate.hidden } : candidate))}>{t(item.hidden ? "reopenMarker" : "hideMarker")}</button><button type="button" className="quiet" aria-label={t("deleteDraftAction", { message: item.draft.instruction })} onClick={() => deleteDraft(item.draft)}>{t("delete")}</button></div>
          </article>)}
          {repairs.map((repair) => {
            const replies = bridge.listRepairReplies(repair.id);
            return <article key={repair.id} className="a3s-item submitted"><span className={`a3s-status status-${repair.status}`}>{reviewStatusLabel(t, repair.status)}</span><strong>{repair.instruction}</strong><small>{reviewTargetSummary(t, repair.target)}{repair.designReference ? ` · ${repair.designReference.kind} reference` : ""} · {t("revision", { revision: repair.contextRevision })}</small>{replies.length > 0 && <ol className="a3s-thread" aria-label={t("repairConversation", { message: repair.instruction })}>{replies.map((reply) => <li key={reply.requestId}><span>{reviewActorLabel(t, reply.actor)}</span><p>{reply.message}</p></li>)}</ol>}{repair.status === "needs_input" && <div className="a3s-human-actions">{replyFindingId === repair.id ? <><label className="a3s-reply-label">{t("replyToCodingAgent")}<textarea aria-label={t("replyToCodingAgentAbout", { message: repair.instruction })} autoFocus maxLength={8192} value={replyMessage} onChange={(event) => setReplyMessage(event.target.value)} /></label><button type="button" disabled={!replyMessage.trim()} onClick={() => submitHumanAction(repair.id, "reply", replyMessage)}>{t("sendReply")}</button><button type="button" className="quiet" onClick={() => { setRestoreReplyFocusId(repair.id); setReplyFindingId(null); setReplyMessage(""); }}>{t("cancelReply")}</button></> : <button ref={(element) => { if (element) replyTriggerRefs.current.set(repair.id, element); else replyTriggerRefs.current.delete(repair.id); }} type="button" aria-label={t("replyAboutRepair", { message: repair.instruction })} onClick={() => setReplyFindingId(repair.id)}>{t("reply")}</button>}</div>}{repair.status === "review_ready" && <div className="a3s-human-actions" aria-label={t("reviewRepair", { message: repair.instruction })}><button type="button" aria-label={t("acceptRepairAction", { message: repair.instruction })} onClick={() => submitHumanAction(repair.id, "accept")}>{t("acceptRepair")}</button><button type="button" className="quiet" aria-label={t("rejectRepairAction", { message: repair.instruction })} onClick={() => submitHumanAction(repair.id, "dismiss")}>{t("reject")}</button><button type="button" className="quiet" aria-label={t("reopenRepairAction", { message: repair.instruction })} onClick={() => submitHumanAction(repair.id, "reopen")}>{t("reopen")}</button></div>}{["resolved", "dismissed", "cancelled", "failed", "verification_failed"].includes(repair.status) && <div className="a3s-human-actions"><button type="button" className="quiet" aria-label={t("reopenRepairAction", { message: repair.instruction })} onClick={() => submitHumanAction(repair.id, "reopen")}>{t("reopen")}</button></div>}</article>;
          })}
          {drafts.length === 0 && repairs.length === 0 && qualityReports.length === 0 && designAuditReports.length === 0 && !candidate && <p className="a3s-empty">{t("emptyWorkspace")}</p>}
              </section>
            </div>
            {drafts.length > 0 && <footer>
              <div className="a3s-workspace-secondary-actions"><button type="button" className="quiet" title={t("clearDraftsTitle")} aria-keyshortcuts={REVIEW_KEY_SHORTCUTS.clear} onClick={clearDrafts}>{t("clearDrafts")}</button><button type="button" className="quiet" title={t("copyMarkdownTitle")} aria-keyshortcuts={REVIEW_KEY_SHORTCUTS.copy} onClick={() => void copyDrafts("markdown")}>{t("copyMarkdown")}</button><button type="button" className="quiet" onClick={() => void copyDrafts()}>{t("copyJson")}</button></div>
              <div className="a3s-workspace-send-actions"><button type="button" disabled={selectedCount === 0} onClick={() => submit(drafts.filter((item) => item.selected).map((item) => item.draft))}>{t("sendSelected", { count: selectedCount })}</button><button type="button" onClick={() => submit(drafts.map((item) => item.draft))}>{t("sendAll")}</button></div>
            </footer>}
          </section>
        </aside>
      </>}
    </div>
    </ReviewI18nProvider>
  );
  return createPortal(content, mount);
}
