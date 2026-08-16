import { useState, type ReactNode } from "react";
import type { RepairIntent, RepairSeverity, Rect } from "./types";
import { type LayoutCanvas, type LayoutSource, type OverlayTheme, type SelectionMode } from "./review-model";
import { validLayoutRect } from "./review-utils";
import { ComponentCatalogView } from "./component-catalog-view";
import { REVIEW_KEY_SHORTCUTS } from "./review-input-policy";
import { reviewModeLabel, useReviewI18n } from "./review-locale";

export type ReviewMarkingToolbarProps = {
  marking: boolean;
  mode: SelectionMode;
  layoutMode: boolean;
  paused: boolean;
  markersVisible: boolean;
  autoSendEnabled: boolean;
  theme: OverlayTheme;
  findingCount: number;
  workspaceOpen: boolean;
  settings: ReactNode;
  onStartMarking(value: SelectionMode): void;
  onToggleLayout(): void;
  onTogglePause(): void;
  onToggleMarkers(): void;
  onToggleAutoSend(): void;
  onCycleTheme(): void;
  onToggleWorkspace(): void;
  onCancelMarking(): void;
};

export function ReviewMarkingToolbar(props: ReviewMarkingToolbarProps) {
  const [moreOpen, setMoreOpen] = useState(false);
  const { t } = useReviewI18n();
  const themeLabel = t(
    props.theme === "system"
      ? "themeSystem"
      : props.theme === "light"
        ? "themeLight"
        : "themeDark",
  );
  const themeActionLabel = themeLabel.toLocaleLowerCase("en-US");
  const beginMarking = (value: SelectionMode) => {
    setMoreOpen(false);
    props.onStartMarking(value);
  };
  const toggleWorkspace = () => {
    setMoreOpen(false);
    props.onToggleWorkspace();
  };
  const toggleMore = () => {
    const next = !moreOpen;
    setMoreOpen(next);
    if (next && props.workspaceOpen) props.onToggleWorkspace();
  };

  return <section className="a3s-tools" aria-label={t("markPage")}>
    <div className="a3s-toolbar-core">
      <div className="a3s-tool-group a3s-tool-group-primary">
        {(["element", "multi", "text"] as const).map((value) => { const label = reviewModeLabel(t, value); const actionLabel = label.toLocaleLowerCase("en-US"); return <ToolButton key={value} label={label} ariaLabel={t("markAction", { mode: actionLabel })} icon={value} pressed={props.marking && props.mode === value} keyShortcut={REVIEW_KEY_SHORTCUTS[value]} title={t("markActionWithShortcut", { mode: actionLabel, shortcut: REVIEW_KEY_SHORTCUTS[value] })} onClick={() => beginMarking(value)} />; })}
      </div>
      <span className="a3s-tool-divider" aria-hidden="true" />
      <button type="button" className="a3s-workspace-toggle" data-tooltip={t("findings")} aria-label={t(props.workspaceOpen ? "hideReviewWorkspace" : "openReviewWorkspace")} aria-expanded={props.workspaceOpen} onClick={toggleWorkspace}>
        <ToolGlyph name="inbox" />
        <span className="a3s-sr-only">{t("findings")}</span>
        {props.findingCount > 0 && <span className="a3s-tool-count" aria-hidden="true">{props.findingCount}</span>}
      </button>
      <ToolButton label={t("moreTools")} ariaLabel={t("moreReviewTools")} icon="more" expanded={moreOpen} onClick={toggleMore} />
      {props.marking && <ToolButton label={t("cancel")} ariaLabel={t("cancelMarking")} icon="close" className="danger" onClick={props.onCancelMarking} />}
    </div>
    <div className="a3s-tool-tray" hidden={!moreOpen} role="group" aria-label={t("moreReviewTools")}>
      <div className="a3s-tool-tray-copy">
        <strong>{t("reviewTools")}</strong>
        <span>{props.marking ? t("modeActive", { mode: reviewModeLabel(t, props.mode) }) : t("markInspectSend")}</span>
      </div>
      <div className="a3s-tool-group">
        {(["area", "draw"] as const).map((value) => { const label = reviewModeLabel(t, value); const actionLabel = label.toLocaleLowerCase("en-US"); return <ToolButton key={value} label={label} ariaLabel={t("markAction", { mode: actionLabel })} icon={value} pressed={props.marking && props.mode === value} keyShortcut={REVIEW_KEY_SHORTCUTS[value]} title={t("markActionWithShortcut", { mode: actionLabel, shortcut: REVIEW_KEY_SHORTCUTS[value] })} onClick={() => beginMarking(value)} />; })}
        <ToolButton label={t("layout")} ariaLabel={t("layout")} icon="layout" pressed={props.layoutMode} title={t("toggleLayoutMode")} keyShortcut={REVIEW_KEY_SHORTCUTS.layout} onClick={() => { setMoreOpen(false); props.onToggleLayout(); }} />
        <ToolButton label={t(props.paused ? "resume" : "pause")} ariaLabel={t(props.paused ? "resumePageAnimations" : "pausePageAnimations")} icon={props.paused ? "play" : "pause"} pressed={props.paused} title={t("pauseOrResumeMotion")} keyShortcut={REVIEW_KEY_SHORTCUTS.pause} onClick={props.onTogglePause} />
        <ToolButton label={t(props.markersVisible ? "hideMarkers" : "showMarkers")} ariaLabel={t(props.markersVisible ? "hideMarkers" : "showMarkers")} icon={props.markersVisible ? "eye" : "eye-off"} pressed={props.markersVisible} title={t("showOrHideMarkers")} keyShortcut={REVIEW_KEY_SHORTCUTS.markers} onClick={props.onToggleMarkers} />
        <ToolButton label={t(props.autoSendEnabled ? "autoSendOn" : "autoSendOff")} ariaLabel={t(props.autoSendEnabled ? "turnAutoSendOff" : "turnAutoSendOn")} icon="send" pressed={props.autoSendEnabled} onClick={props.onToggleAutoSend} />
        <ToolButton label={t("themeCurrent", { theme: themeLabel })} ariaLabel={t("changeTheme", { theme: themeActionLabel })} icon="theme" onClick={props.onCycleTheme} />
        {props.settings}
      </div>
    </div>
  </section>;
}

type ToolIcon = SelectionMode | "layout" | "play" | "pause" | "eye" | "eye-off" | "send" | "theme" | "inbox" | "close" | "settings" | "more";

function ToolButton({ label, ariaLabel, icon, pressed, expanded, title, keyShortcut, className = "", onClick }: { label: string; ariaLabel: string; icon: ToolIcon; pressed?: boolean; expanded?: boolean; title?: string; keyShortcut?: string; className?: string; onClick(): void }) {
  return <button type="button" className={`${pressed || expanded ? "selected " : ""}${className}`.trim()} data-tooltip={label} title={title ?? label} aria-label={ariaLabel} {...(pressed === undefined ? {} : { "aria-pressed": pressed })} {...(expanded === undefined ? {} : { "aria-expanded": expanded })} {...(keyShortcut ? { "aria-keyshortcuts": keyShortcut } : {})} onClick={onClick}>
    <ToolGlyph name={icon} />
    <span className="a3s-sr-only">{label}</span>
  </button>;
}

export function ToolGlyph({ name }: { name: ToolIcon }) {
  const common = { viewBox: "0 0 20 20", "aria-hidden": true } as const;
  if (name === "element") return <svg {...common}><path d="M3.5 7V3.5H7M13 3.5h3.5V7M16.5 13v3.5H13M7 16.5H3.5V13" /><circle cx="10" cy="10" r="2.25" /></svg>;
  if (name === "multi") return <svg {...common}><rect x="3.5" y="5.5" width="9" height="9" rx="1.5" /><rect x="7.5" y="3.5" width="9" height="9" rx="1.5" /></svg>;
  if (name === "area") return <svg {...common}><rect x="3.5" y="4" width="13" height="12" rx="1.5" strokeDasharray="2.5 2.5" /></svg>;
  if (name === "text") return <svg {...common}><path d="M4 5h12M10 5v10M7.5 15h5" /></svg>;
  if (name === "draw") return <svg {...common}><path d="m4 14.8 2.8-.6 8.1-8.1a1.6 1.6 0 0 0-2.3-2.3l-8.1 8.1-.5 2.9Z" /><path d="m11.5 4.9 2.3 2.3" /></svg>;
  if (name === "layout") return <svg {...common}><rect x="3.5" y="3.5" width="13" height="13" rx="1.5" /><path d="M8 3.5v13M8 9h8.5" /></svg>;
  if (name === "play") return <svg {...common}><path d="m7 5.5 7 4.5-7 4.5Z" /></svg>;
  if (name === "pause") return <svg {...common}><path d="M7 5v10M13 5v10" /></svg>;
  if (name === "eye" || name === "eye-off") return <svg {...common}><path d="M2.8 10s2.5-4 7.2-4 7.2 4 7.2 4-2.5 4-7.2 4-7.2-4-7.2-4Z" /><circle cx="10" cy="10" r="1.8" />{name === "eye-off" && <path d="m4 4 12 12" />}</svg>;
  if (name === "send") return <svg {...common}><path d="m3.5 4 13 6-13 6 2.2-5.1L12 10l-6.3-.9Z" /></svg>;
  if (name === "theme") return <svg {...common}><path d="M10 2.8a7.2 7.2 0 1 0 7.2 7.2A5.8 5.8 0 0 1 10 2.8Z" /></svg>;
  if (name === "inbox") return <svg {...common}><path d="M3.5 5.5h13v10h-13Z" /><path d="M3.5 11h3l1.4 2h4.2l1.4-2h3" /></svg>;
  if (name === "settings") return <svg {...common}><circle cx="10" cy="10" r="2.4" /><path d="M10 3.3v1.3M10 15.4v1.3M3.3 10h1.3M15.4 10h1.3M5.3 5.3l.9.9M13.8 13.8l.9.9M14.7 5.3l-.9.9M6.2 13.8l-.9.9" /></svg>;
  if (name === "more") return <svg {...common}><circle cx="4.5" cy="10" r="1" /><circle cx="10" cy="10" r="1" /><circle cx="15.5" cy="10" r="1" /></svg>;
  return <svg {...common}><path d="m5 5 10 10M15 5 5 15" /></svg>;
}

export type FindingEditorProps = {
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
  onDelete?(): void;
  onSave(): void;
  onSend(): void;
};

export function FindingEditor(props: FindingEditorProps) {
  const { t } = useReviewI18n();
  const [detailsOpen, setDetailsOpen] = useState(() => Boolean(
    props.successCriteria.trim()
      || props.intent !== "fix"
      || props.severity !== "important"
      || props.conflictOptions.some((option) => option.checked),
  ));

  return <section className="a3s-editor">
    <header className="a3s-editor-header">
      <span className="a3s-editor-index" aria-hidden="true"><ToolGlyph name="element" /></span>
      <span><strong>{t(props.editing ? "editFinding" : "newFinding")}</strong><small className="a3s-editor-target" title={props.label}>{props.label}</small></span>
    </header>
    <div className="a3s-editor-scroll">
      <label className="a3s-editor-request">{t("requestedFix")}<textarea autoFocus maxLength={8192} value={props.instruction} onChange={(event) => props.onInstruction(event.target.value)} placeholder={t("describeChange")} /></label>
      <button type="button" className="a3s-editor-details" aria-expanded={detailsOpen} onClick={() => setDetailsOpen((current) => !current)}><span>{t("details")}</span><small>{t("detailsSummary")}</small><i aria-hidden="true" /></button>
      {detailsOpen && <div className="a3s-editor-options">
        <label>{t("successCriteria")} <span>{t("optional")}</span><textarea maxLength={4096} value={props.successCriteria} onChange={(event) => props.onSuccessCriteria(event.target.value)} placeholder={t("successCriteriaPlaceholder")} /></label>
        <div className="a3s-fields"><label>{t("severity")}<select value={props.severity} onChange={(event) => props.onSeverity(event.target.value as RepairSeverity)}><option value="blocking">{t("severityBlocking")}</option><option value="important">{t("severityImportant")}</option><option value="suggestion">{t("severitySuggestion")}</option></select></label><label>{t("intent")}<select value={props.intent} onChange={(event) => props.onIntent(event.target.value as RepairIntent)}><option value="fix">{t("intentFix")}</option><option value="change">{t("intentChange")}</option><option value="question">{t("intentQuestion")}</option><option value="approve">{t("intentApprove")}</option></select></label></div>
        {props.conflictOptions.length > 0 && <fieldset className="a3s-conflicts"><legend>{t("conflictsWithDraft")} <span>{t("optional")}</span></legend><small>{t("conflictHelp")}</small>{props.conflictOptions.map((option) => <label key={option.id}><input type="checkbox" checked={option.checked} onChange={(event) => props.onConflict(option.id, event.target.checked)} /><span>{option.label}</span></label>)}</fieldset>}
      </div>}
    </div>
    <div className="a3s-actions">{props.onDelete && <button type="button" className="danger" onClick={props.onDelete}>{t("deleteDraft")}</button>}<button type="button" className="quiet" onClick={props.onCancel}>{t("cancel")}</button><button type="button" className="a3s-save-draft" disabled={!props.instruction.trim()} onClick={props.onSave}>{t(props.editing ? "saveChanges" : "addDraft")}</button><button type="button" className="a3s-send-now" disabled={!props.instruction.trim()} onClick={props.onSend}>{t("sendAndAutoFix")}</button></div>
  </section>;
}

export type LayoutComposerProps = {
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

export function LayoutComposer(props: LayoutComposerProps) {
  const { t } = useReviewI18n();
  const helpId = `${props.idPrefix}-layout-help`;
  const updateNumber = (field: keyof Rect, value: number) => {
    if (Number.isFinite(value)) props.onTarget(field, value);
  };
  return <section className="a3s-layout" aria-label={t("layoutRepairIntent")}>
    <p id={helpId}>{t("layoutHelp")}</p>
    <label>{t("purpose")} <span>{t("optional")}</span><input type="text" aria-label={t("layoutPurpose")} aria-describedby={helpId} maxLength={512} value={props.purpose} onChange={(event) => props.onPurpose(event.target.value)} placeholder={t("layoutPurposePlaceholder")} /></label>
    <div className="a3s-layout-fields">
      <label>{t("canvas")}<select aria-label={t("layoutCanvas")} value={props.canvas} onChange={(event) => props.onCanvas(event.target.value as LayoutCanvas)}><option value="page">{t("currentPage")}</option><option value="wireframe">{t("wireframe")}</option></select></label>
      <label>{t("component")}<input type="text" aria-label={t("layoutComponentType")} maxLength={256} value={props.componentType} onChange={(event) => props.onComponentType(event.target.value)} placeholder={t("componentPlaceholder")} /></label>
    </div>
    <ComponentCatalogView selected={props.componentType} onSelect={props.onComponentType} />
    <button type="button" aria-pressed={props.markingMode === "layout_place"} className={props.markingMode === "layout_place" ? "selected" : ""} disabled={!props.componentType.trim()} onClick={props.onPlace}>{t("drawPlacement")}</button>
    <div className="a3s-layout-source">
      <span>{t("sectionToRearrange")}</span>
      <strong>{props.source?.label ?? t("noSectionSelected")}</strong>
      <button type="button" aria-pressed={props.markingMode === "layout_source"} className={props.markingMode === "layout_source" ? "selected" : ""} onClick={props.onSelectSource}>{t("selectSectionOnPage")}</button>
    </div>
    <div className="a3s-layout-fields" aria-label={t("layoutDestinationPixels")}>
      <label>X<input type="number" aria-label={t("layoutX")} step="1" value={props.target.x} onChange={(event) => updateNumber("x", event.currentTarget.valueAsNumber)} /></label>
      <label>Y<input type="number" aria-label={t("layoutY")} step="1" value={props.target.y} onChange={(event) => updateNumber("y", event.currentTarget.valueAsNumber)} /></label>
      <label>{t("width")}<input type="number" aria-label={t("layoutWidth")} min="8" step="1" value={props.target.width} onChange={(event) => updateNumber("width", event.currentTarget.valueAsNumber)} /></label>
      <label>{t("height")}<input type="number" aria-label={t("layoutHeight")} min="8" step="1" value={props.target.height} onChange={(event) => updateNumber("height", event.currentTarget.valueAsNumber)} /></label>
    </div>
    <div className="a3s-actions">
      <button type="button" className={props.markingMode === "layout_destination" ? "selected" : ""} aria-pressed={props.markingMode === "layout_destination"} disabled={!props.source} onClick={props.onDrawDestination}>{t("drawDestination")}</button>
      <button type="button" disabled={!props.source || !validLayoutRect(props.target)} onClick={props.onCreateRearrange}>{t("createRearrangeDraft")}</button>
    </div>
  </section>;
}
