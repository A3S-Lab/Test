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
  onStartMarking(value: SelectionMode): void;
  onToggleLayout(): void;
  onTogglePause(): void;
  onToggleMarkers(): void;
  onToggleAutoSend(): void;
  onCycleTheme(): void;
  onCancelMarking(): void;
};

export function ReviewMarkingToolbar(props: ReviewMarkingToolbarProps) {
  return <section className="a3s-tools" aria-label="Mark page">
    {(["element", "text", "multi", "area", "draw"] as SelectionMode[]).map((value) => <button key={value} type="button" aria-label={`Mark ${MODE_LABEL[value].toLowerCase()}`} aria-pressed={props.marking && props.mode === value} className={props.marking && props.mode === value ? "selected" : ""} onClick={() => props.onStartMarking(value)}>{MODE_LABEL[value]}</button>)}
    <button type="button" title="Toggle Layout Mode (L)" aria-keyshortcuts={REVIEW_KEY_SHORTCUTS.layout} aria-pressed={props.layoutMode} className={props.layoutMode ? "selected" : ""} onClick={props.onToggleLayout}>Layout</button>
    <button type="button" title="Pause or resume page motion (P)" aria-label={props.paused ? "Resume page animations" : "Pause page animations"} aria-keyshortcuts={REVIEW_KEY_SHORTCUTS.pause} aria-pressed={props.paused} className={props.paused ? "selected" : ""} onClick={props.onTogglePause}>{props.paused ? "Resume" : "Pause"}</button>
    <button type="button" title="Show or hide finding markers (H)" aria-label={props.markersVisible ? "Hide markers" : "Show markers"} aria-keyshortcuts={REVIEW_KEY_SHORTCUTS.markers} aria-pressed={props.markersVisible} className={props.markersVisible ? "selected" : ""} onClick={props.onToggleMarkers}>{props.markersVisible ? "Hide markers" : "Show markers"}</button>
    <button type="button" aria-label={`Turn auto-send ${props.autoSendEnabled ? "off" : "on"}`} aria-pressed={props.autoSendEnabled} className={props.autoSendEnabled ? "selected" : ""} onClick={props.onToggleAutoSend}>Auto-send · {props.autoSendEnabled ? "on" : "off"}</button>
    <button type="button" aria-label={`Change overlay theme; current theme is ${props.theme}`} onClick={props.onCycleTheme}>Theme · {props.theme}</button>
    {props.marking && <button type="button" className="danger" onClick={props.onCancelMarking}>Cancel</button>}
  </section>;
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
  return <section className="a3s-editor">
    <small>Target · {props.label}</small>
    <label>Requested fix<textarea autoFocus maxLength={8192} value={props.instruction} onChange={(event) => props.onInstruction(event.target.value)} placeholder="Describe what should change" /></label>
    <label>Success criteria <span>optional</span><textarea maxLength={4096} value={props.successCriteria} onChange={(event) => props.onSuccessCriteria(event.target.value)} placeholder="What should be visibly true after the fix?" /></label>
    <div className="a3s-fields"><label>Severity<select value={props.severity} onChange={(event) => props.onSeverity(event.target.value as RepairSeverity)}><option value="blocking">Blocking</option><option value="important">Important</option><option value="suggestion">Suggestion</option></select></label><label>Intent<select value={props.intent} onChange={(event) => props.onIntent(event.target.value as RepairIntent)}><option value="fix">Fix</option><option value="change">Change</option><option value="question">Question</option><option value="approve">Approve</option></select></label></div>
    {props.conflictOptions.length > 0 && <fieldset className="a3s-conflicts"><legend>Conflicts with another draft <span>optional</span></legend><small>Select requests that cannot both be satisfied. A3S Test will ask for clarification without interpreting their wording.</small>{props.conflictOptions.map((option) => <label key={option.id}><input type="checkbox" checked={option.checked} onChange={(event) => props.onConflict(option.id, event.target.checked)} /><span>{option.label}</span></label>)}</fieldset>}
    <div className="a3s-actions">{props.onDelete && <button type="button" className="danger" onClick={props.onDelete}>Delete draft</button>}<button type="button" className="quiet" onClick={props.onCancel}>Cancel</button><button type="button" disabled={!props.instruction.trim()} onClick={props.onSave}>{props.editing ? "Save changes" : "Add draft"}</button><button type="button" disabled={!props.instruction.trim()} onClick={props.onSend}>Send and auto-fix</button></div>
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
