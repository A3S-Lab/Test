import { useState, type ReactNode } from "react";
import type { RepairIntent, RepairSeverity, Rect } from "./types";
import { MODE_LABEL, type LayoutCanvas, type LayoutSource, type OverlayTheme, type SelectionMode } from "./review-model";
import { validLayoutRect } from "./review-utils";
import { ComponentCatalogView } from "./component-catalog-view";
import { REVIEW_KEY_SHORTCUTS } from "./review-input-policy";

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
  const beginMarking = (value: SelectionMode) => {
    setMoreOpen(false);
    props.onStartMarking(value);
  };

  return <section className="a3s-tools" aria-label="Mark page">
    <div className="a3s-toolbar-core">
      <div className="a3s-tool-group a3s-tool-group-primary">
        {(["element", "multi", "text"] as const).map((value) => <ToolButton key={value} label={MODE_LABEL[value]} ariaLabel={`Mark ${MODE_LABEL[value].toLowerCase()}`} icon={value} pressed={props.marking && props.mode === value} keyShortcut={REVIEW_KEY_SHORTCUTS[value]} title={`Mark ${MODE_LABEL[value].toLowerCase()} (${REVIEW_KEY_SHORTCUTS[value]})`} onClick={() => beginMarking(value)} />)}
      </div>
      <span className="a3s-tool-divider" aria-hidden="true" />
      <button type="button" className="a3s-workspace-toggle" data-tooltip="Findings" aria-label={props.workspaceOpen ? "Hide review workspace" : "Open review workspace"} aria-expanded={props.workspaceOpen} onClick={props.onToggleWorkspace}>
        <ToolGlyph name="inbox" />
        <span className="a3s-sr-only">Findings</span>
        {props.findingCount > 0 && <span className="a3s-tool-count" aria-hidden="true">{props.findingCount}</span>}
      </button>
      <ToolButton label="More tools" ariaLabel="More review tools" icon="more" expanded={moreOpen} onClick={() => setMoreOpen((current) => !current)} />
      {props.marking && <ToolButton label="Cancel" ariaLabel="Cancel marking" icon="close" className="danger" onClick={props.onCancelMarking} />}
    </div>
    <div className="a3s-tool-tray" hidden={!moreOpen} role="group" aria-label="More review tools">
      <div className="a3s-tool-tray-copy">
        <strong>Review tools</strong>
        <span>{props.marking ? `${MODE_LABEL[props.mode]} mode active` : "Mark, inspect, and send"}</span>
      </div>
      <div className="a3s-tool-group">
        {(["area", "draw"] as const).map((value) => <ToolButton key={value} label={MODE_LABEL[value]} ariaLabel={`Mark ${MODE_LABEL[value].toLowerCase()}`} icon={value} pressed={props.marking && props.mode === value} keyShortcut={REVIEW_KEY_SHORTCUTS[value]} title={`Mark ${MODE_LABEL[value].toLowerCase()} (${REVIEW_KEY_SHORTCUTS[value]})`} onClick={() => beginMarking(value)} />)}
        <ToolButton label="Layout" ariaLabel="Layout" icon="layout" pressed={props.layoutMode} title="Toggle Layout Mode (L)" keyShortcut={REVIEW_KEY_SHORTCUTS.layout} onClick={() => { setMoreOpen(false); props.onToggleLayout(); }} />
        <ToolButton label={props.paused ? "Resume" : "Pause"} ariaLabel={props.paused ? "Resume page animations" : "Pause page animations"} icon={props.paused ? "play" : "pause"} pressed={props.paused} title="Pause or resume page motion (P)" keyShortcut={REVIEW_KEY_SHORTCUTS.pause} onClick={props.onTogglePause} />
        <ToolButton label={props.markersVisible ? "Hide markers" : "Show markers"} ariaLabel={props.markersVisible ? "Hide markers" : "Show markers"} icon={props.markersVisible ? "eye" : "eye-off"} pressed={props.markersVisible} title="Show or hide finding markers (H)" keyShortcut={REVIEW_KEY_SHORTCUTS.markers} onClick={props.onToggleMarkers} />
        <ToolButton label={`Auto-send · ${props.autoSendEnabled ? "on" : "off"}`} ariaLabel={`Turn auto-send ${props.autoSendEnabled ? "off" : "on"}`} icon="send" pressed={props.autoSendEnabled} onClick={props.onToggleAutoSend} />
        <ToolButton label={`Theme · ${props.theme}`} ariaLabel={`Change overlay theme; current theme is ${props.theme}`} icon="theme" onClick={props.onCycleTheme} />
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
  const [detailsOpen, setDetailsOpen] = useState(() => Boolean(
    props.successCriteria.trim()
      || props.intent !== "fix"
      || props.severity !== "important"
      || props.conflictOptions.some((option) => option.checked),
  ));

  return <section className="a3s-editor">
    <header className="a3s-editor-header">
      <span className="a3s-editor-index" aria-hidden="true">01</span>
      <span><strong>{props.editing ? "Edit finding" : "New finding"}</strong><small className="a3s-editor-target" title={props.label}>{props.label}</small></span>
    </header>
    <div className="a3s-editor-scroll">
      <label className="a3s-editor-request">Requested fix<textarea autoFocus maxLength={8192} value={props.instruction} onChange={(event) => props.onInstruction(event.target.value)} placeholder="Describe what should change" /></label>
      <button type="button" className="a3s-editor-details" aria-expanded={detailsOpen} onClick={() => setDetailsOpen((current) => !current)}><span>Details</span><small>criteria, severity, intent</small><i aria-hidden="true" /></button>
      {detailsOpen && <div className="a3s-editor-options">
        <label>Success criteria <span>optional</span><textarea maxLength={4096} value={props.successCriteria} onChange={(event) => props.onSuccessCriteria(event.target.value)} placeholder="What should be visibly true after the fix?" /></label>
        <div className="a3s-fields"><label>Severity<select value={props.severity} onChange={(event) => props.onSeverity(event.target.value as RepairSeverity)}><option value="blocking">Blocking</option><option value="important">Important</option><option value="suggestion">Suggestion</option></select></label><label>Intent<select value={props.intent} onChange={(event) => props.onIntent(event.target.value as RepairIntent)}><option value="fix">Fix</option><option value="change">Change</option><option value="question">Question</option><option value="approve">Approve</option></select></label></div>
        {props.conflictOptions.length > 0 && <fieldset className="a3s-conflicts"><legend>Conflicts with another draft <span>optional</span></legend><small>Select requests that cannot both be satisfied. A3S Test will ask for clarification without interpreting their wording.</small>{props.conflictOptions.map((option) => <label key={option.id}><input type="checkbox" checked={option.checked} onChange={(event) => props.onConflict(option.id, event.target.checked)} /><span>{option.label}</span></label>)}</fieldset>}
      </div>}
    </div>
    <div className="a3s-actions">{props.onDelete && <button type="button" className="danger" onClick={props.onDelete}>Delete draft</button>}<button type="button" className="quiet" onClick={props.onCancel}>Cancel</button><button type="button" className="a3s-save-draft" disabled={!props.instruction.trim()} onClick={props.onSave}>{props.editing ? "Save changes" : "Add draft"}</button><button type="button" className="a3s-send-now" disabled={!props.instruction.trim()} onClick={props.onSend}>Send and auto-fix</button></div>
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
    <ComponentCatalogView selected={props.componentType} onSelect={props.onComponentType} />
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
