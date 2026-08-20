import { useId, useState } from "react";
import type { RepairDesignReference, RepairIntent, RepairSeverity, Rect } from "./types";
import { type LayoutCanvas, type LayoutSource, type SelectionMode } from "./review-model";
import { validLayoutRect } from "./review-utils";
import { ComponentCatalogView } from "./component-catalog-view";
import { useDesignBoardI18n } from "./design-board-i18n";
import { DesignGlyph } from "./design-icons";
import { REVIEW_KEY_SHORTCUTS } from "./review-input-policy";
import { reviewModeLabel, useReviewI18n } from "./review-locale";

export type ReviewPanelView = "compose" | "findings" | "settings";

export function ReviewPanelHeader({ idPrefix, view, findingCount, onClose, onView }: {
  idPrefix: string;
  view: ReviewPanelView;
  findingCount: number;
  onClose(): void;
  onView(view: ReviewPanelView): void;
}) {
  const { t } = useReviewI18n();
  return <>
    <header className="a3s-panel-header">
      <span className="a3s-panel-mark" aria-hidden="true"><svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="8.4" /><path d="m7.3 16 3.9-9.2c.3-.8 1.4-.8 1.8 0l3.8 9.2M9.2 12.5h5.7" /><path d="M4.8 15.4c3-2.5 6.3-2.8 9.7-.9 1.7.9 3.4.3 5.2-1.3-1 4.7-4 7.1-8.5 7.1" /></svg></span>
      <span><strong id={`${idPrefix}-review-title`} className="a3s-panel-title">{t("reviewTitle")}</strong><small id={`${idPrefix}-review-description`} className="a3s-panel-description">{t("reviewDescription")}</small></span>
      <div className="a3s-panel-actions">
        <button type="button" className={`a3s-header-settings${view === "settings" ? " selected" : ""}`} aria-label={t("reviewPreferences")} aria-pressed={view === "settings"} title={t("reviewPreferences")} onClick={() => onView("settings")}><ToolGlyph name="settings" /><span className="a3s-sr-only">{t("preferences")}</span></button>
        <button type="button" className="a3s-close" onClick={onClose} aria-label={t("closeReviewOverlay")}><svg viewBox="0 0 16 16" aria-hidden="true"><path d="M3.5 3.5 12.5 12.5M12.5 3.5 3.5 12.5" /></svg></button>
      </div>
    </header>
    <nav className="a3s-panel-tabs toolbar" aria-label={t("reviewViews")}>
      <button type="button" aria-pressed={view === "compose"} className={view === "compose" ? "selected" : ""} onClick={() => onView("compose")}><ToolGlyph name="element" /><span>{t("newFeedback")}</span></button>
      <button type="button" aria-pressed={view === "findings"} className={view === "findings" ? "selected" : ""} onClick={() => onView("findings")}><ToolGlyph name="inbox" /><span>{t("findings")}</span>{findingCount > 0 && <b aria-label={t("inThisPage", { count: findingCount })}>{findingCount}</b>}</button>
    </nav>
  </>;
}

export type ReviewMarkingToolbarProps = {
  marking: boolean;
  mode: SelectionMode;
  layoutMode: boolean;
  onStartMarking(value: SelectionMode): void;
  onToggleLayout(): void;
  onCancelMarking(): void;
};

export function ReviewMarkingToolbar(props: ReviewMarkingToolbarProps) {
  const { t } = useReviewI18n();
  const [moreOpen, setMoreOpen] = useState(false);
  const secondaryToolsId = useId();
  const primaryModes = ["element", "area"] as const;
  const secondaryModes = ["text", "multi", "draw"] as const;
  const secondaryActive = props.layoutMode || (props.marking && secondaryModes.includes(props.mode as typeof secondaryModes[number]));
  const showSecondary = moreOpen || secondaryActive;
  const renderMode = (value: typeof primaryModes[number] | typeof secondaryModes[number]) => {
    const label = reviewModeLabel(t, value);
    const actionLabel = label.toLocaleLowerCase("en-US");
    return <ToolButton
      key={value}
      label={label}
      ariaLabel={t("markAction", { mode: actionLabel })}
      icon={value}
      pressed={props.marking && props.mode === value}
      keyShortcut={REVIEW_KEY_SHORTCUTS[value]}
      title={t("markActionWithShortcut", { mode: actionLabel, shortcut: REVIEW_KEY_SHORTCUTS[value] })}
      showLabel
      onClick={() => props.onStartMarking(value)}
    />;
  };

  return <section className="a3s-tools" aria-label={t("markPage")}>
    <div className="a3s-tools-heading">
      <span><strong>{t("chooseTarget")}</strong><small>{t("chooseTargetHelp")}</small></span>
      {props.marking && <button type="button" className="quiet danger a3s-cancel-marking" onClick={props.onCancelMarking}><ToolGlyph name="close" /><span>{t("cancel")}</span></button>}
    </div>
    <div className="a3s-selection-grid a3s-primary-tools">
      {primaryModes.map(renderMode)}
    </div>
    <button type="button" className="quiet a3s-more-tools" aria-expanded={showSecondary} aria-controls={secondaryToolsId} aria-label={t("moreReviewTools")} onClick={() => setMoreOpen((current) => !current)}><ToolGlyph name="more" /><span>{t("moreTools")}</span><i aria-hidden="true" /></button>
    <div id={secondaryToolsId} className="a3s-selection-grid a3s-secondary-tools" hidden={!showSecondary}>
      {secondaryModes.map(renderMode)}
      <ToolButton
          label={t("layout")}
          ariaLabel={t("layout")}
          icon="layout"
          pressed={props.layoutMode}
          title={t("toggleLayoutMode")}
          keyShortcut={REVIEW_KEY_SHORTCUTS.layout}
          showLabel
          onClick={props.onToggleLayout}
        />
    </div>
  </section>;
}

type ToolIcon = SelectionMode | "layout" | "play" | "pause" | "eye" | "eye-off" | "send" | "theme" | "inbox" | "close" | "settings" | "more";

function ToolButton({ label, ariaLabel, icon, pressed, title, keyShortcut, className = "", showLabel = false, onClick }: { label: string; ariaLabel: string; icon: ToolIcon; pressed?: boolean; title?: string; keyShortcut?: string; className?: string; showLabel?: boolean; onClick(): void }) {
  return <button type="button" className={`${pressed ? "selected " : ""}${showLabel ? "with-label " : ""}${className}`.trim()} title={title ?? label} aria-label={ariaLabel} {...(pressed === undefined ? {} : { "aria-pressed": pressed })} {...(keyShortcut ? { "aria-keyshortcuts": keyShortcut } : {})} onClick={onClick}>
    <ToolGlyph name={icon} />
    <span className={showLabel ? undefined : "a3s-sr-only"}>{label}</span>
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
  designReference: RepairDesignReference | null;
  conflictOptions: Array<{ id: string; label: string; checked: boolean }>;
  editing: boolean;
  onInstruction(value: string): void;
  onSuccessCriteria(value: string): void;
  onSeverity(value: RepairSeverity): void;
  onIntent(value: RepairIntent): void;
  onOpenDesignBoard(): void;
  onRemoveDesignReference(): void;
  onConflict(findingId: string, checked: boolean): void;
  onCancel(): void;
  onDelete?(): void;
  onSave(): void;
  onSend(): void;
};

export function FindingEditor(props: FindingEditorProps) {
  const { t } = useReviewI18n();
  const { t: designT } = useDesignBoardI18n();
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
      <div className={`a3s-design-reference${props.designReference ? " has-reference" : ""}`}>
        {props.designReference ? <>
          {props.designReference.image.kind === "inline" && <img src={props.designReference.image.dataUrl} alt={designT(props.designReference.kind === "sketch" ? "referenceSketchAlt" : "referenceScreenshotAlt")} />}
          <div><strong>{designT(props.designReference.kind === "sketch" ? "referenceSketchAttached" : "referenceScreenshotAttached")}</strong><small>{props.designReference.width} × {props.designReference.height} · {designT("referenceStored")}</small></div>
          <button type="button" className="quiet a3s-design-reference-action" onClick={props.onOpenDesignBoard}><DesignGlyph name="draw" /><span>{designT("editReference")}</span></button>
          <button type="button" className="quiet danger a3s-design-reference-action" onClick={props.onRemoveDesignReference}><DesignGlyph name="trash" /><span>{designT("removeReference")}</span></button>
        </> : <>
          <span className="a3s-design-reference-icon" aria-hidden="true"><DesignGlyph name="image" /></span>
          <div><strong>{designT("referencePromptTitle")}</strong><small>{designT("referencePromptDescription")}</small></div>
          <button type="button" className="a3s-design-reference-open" onClick={props.onOpenDesignBoard}><DesignGlyph name="draw" /><span>{designT("openBoard")}</span></button>
        </>}
      </div>
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
