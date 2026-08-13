import type { ReviewPreferences } from "./review-preferences";

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
  const update = <Key extends keyof ReviewPreferences>(
    key: Key,
    value: ReviewPreferences[Key],
  ) => onChange({ ...preferences, [key]: value });

  return <details className="a3s-settings">
    <summary>Review preferences</summary>
    <div className="a3s-settings-grid">
      <label>Theme<select aria-label="Overlay theme" value={preferences.theme} onChange={(event) => update("theme", event.target.value as ReviewPreferences["theme"])}><option value="system">System</option><option value="light">Light</option><option value="dark">Dark</option></select></label>
      <label>Panel dock<select aria-label="Panel dock" value={preferences.dock} onChange={(event) => update("dock", event.target.value as ReviewPreferences["dock"])}><option value="right">Right</option><option value="left">Left</option></select></label>
      <label>Marker color<input type="color" aria-label="Marker color" value={preferences.markerColor} onChange={(event) => update("markerColor", event.target.value)} /></label>
      <label>Wireframe page fade <output>{Math.round(preferences.wireframeFade * 100)}%</output><input type="range" aria-label="Wireframe page fade" min="0" max="0.8" step="0.01" value={preferences.wireframeFade} onChange={(event) => update("wireframeFade", event.currentTarget.valueAsNumber)} /></label>
    </div>
    <label className="a3s-setting-toggle"><input type="checkbox" aria-label="Clear drafts after copy" checked={preferences.clearOnCopy} onChange={(event) => update("clearOnCopy", event.target.checked)} /><span>Clear copied drafts after a successful copy</span></label>
    <label className="a3s-setting-toggle"><input type="checkbox" aria-label="Block page pointer input" checked={preferences.blockInteractions} onChange={(event) => update("blockInteractions", event.target.checked)} /><span>Block page pointer input while the overlay is available</span></label>
    <button type="button" className="quiet" onClick={onHideUntilRestart}>Hide until tab restart</button>
  </details>;
}
