import { useId, useState } from "react";
import { REVIEW_SHORTCUT_HELP } from "./review-input-policy";
import type { ReviewPreferences } from "./review-preferences";
import { ToolGlyph } from "./review-components";

export type ReviewSettingsProps = {
  preferences: ReviewPreferences;
  onChange(value: ReviewPreferences): void;
  onHideUntilRestart(): void;
};

export function ReviewSettings({
  preferences,
  onChange,
  onHideUntilRestart,
}: ReviewSettingsProps) {
  const [open, setOpen] = useState(false);
  const idPrefix = useId().replace(/:/g, "");
  const panelId = `${idPrefix}-review-preferences`;
  const shortcutsTitleId = `${idPrefix}-review-shortcuts-title`;
  const update = <Key extends keyof ReviewPreferences>(
    key: Key,
    value: ReviewPreferences[Key],
  ) => onChange({ ...preferences, [key]: value });

  return <section className="a3s-settings">
    <button type="button" className="a3s-disclosure" data-tooltip="Preferences" title="Review preferences" aria-label="Review preferences" aria-expanded={open} aria-controls={panelId} onClick={() => setOpen((current) => !current)}><ToolGlyph name="settings" /><span className="a3s-sr-only">Review preferences</span></button>
    {open && <div id={panelId} className="a3s-settings-content">
      <div className="a3s-settings-grid">
      <label>Theme<select aria-label="Overlay theme" value={preferences.theme} onChange={(event) => update("theme", event.target.value as ReviewPreferences["theme"])}><option value="system">System</option><option value="light">Light</option><option value="dark">Dark</option></select></label>
      <label>Panel dock<select aria-label="Panel dock" value={preferences.dock} onChange={(event) => update("dock", event.target.value as ReviewPreferences["dock"])}><option value="right">Right</option><option value="left">Left</option></select></label>
      <label>Marker color<input type="color" aria-label="Marker color" value={preferences.markerColor} onChange={(event) => update("markerColor", event.target.value)} /></label>
      <label>Wireframe page fade <output>{Math.round(preferences.wireframeFade * 100)}%</output><input type="range" aria-label="Wireframe page fade" min="0" max="0.8" step="0.01" value={preferences.wireframeFade} onChange={(event) => update("wireframeFade", event.currentTarget.valueAsNumber)} /></label>
      </div>
      <label className="a3s-setting-toggle"><input type="checkbox" aria-label="Clear drafts after copy" checked={preferences.clearOnCopy} onChange={(event) => update("clearOnCopy", event.target.checked)} /><span>Clear copied drafts after a successful copy</span></label>
      <label className="a3s-setting-toggle"><input type="checkbox" aria-label="Block page pointer input" checked={preferences.blockInteractions} onChange={(event) => update("blockInteractions", event.target.checked)} /><span>Block page pointer input while the overlay is available</span></label>
      <section className="a3s-shortcuts" aria-labelledby={shortcutsTitleId}>
        <h3 id={shortcutsTitleId} className="a3s-shortcuts-title">Keyboard shortcuts</h3>
        <dl>{REVIEW_SHORTCUT_HELP.map((shortcut) => <div key={shortcut.action}><dt>{shortcut.action}</dt><dd><kbd>{shortcut.keys}</kbd></dd></div>)}</dl>
        <p>Letter shortcuts and panel toggle are ignored while typing. Escape still cancels active marking or an open finding editor.</p>
      </section>
      <button type="button" className="quiet" onClick={onHideUntilRestart}>Hide until tab restart</button>
    </div>}
  </section>;
}
