import { useId } from "react";
import { REVIEW_KEY_SHORTCUTS, REVIEW_SHORTCUT_HELP } from "./review-input-policy";
import type { ReviewPreferences } from "./review-preferences";
import { ToolGlyph } from "./review-components";
import { reviewShortcutLabel, useReviewI18n } from "./review-locale";

export type ReviewSettingsProps = {
  preferences: ReviewPreferences;
  paused: boolean;
  markersVisible: boolean;
  autoSendEnabled: boolean;
  onChange(value: ReviewPreferences): void;
  onTogglePause(): void;
  onToggleMarkers(): void;
  onToggleAutoSend(): void;
  onHideUntilRestart(): void;
};

export function ReviewSettings({
  preferences,
  paused,
  markersVisible,
  autoSendEnabled,
  onChange,
  onTogglePause,
  onToggleMarkers,
  onToggleAutoSend,
  onHideUntilRestart,
}: ReviewSettingsProps) {
  const { t } = useReviewI18n();
  const idPrefix = useId().replace(/:/g, "");
  const shortcutsTitleId = `${idPrefix}-review-shortcuts-title`;
  const update = <Key extends keyof ReviewPreferences>(
    key: Key,
    value: ReviewPreferences[Key],
  ) => onChange({ ...preferences, [key]: value });

  return <section className="a3s-settings-content" aria-label={t("reviewPreferences")}>
      <header className="a3s-settings-heading"><strong>{t("reviewPreferences")}</strong><small>{t("settingsHelp")}</small></header>
      <div className="a3s-settings-actions" aria-label={t("pageControls")}>
        <button type="button" aria-pressed={paused} aria-label={t(paused ? "resumePageAnimations" : "pausePageAnimations")} aria-keyshortcuts={REVIEW_KEY_SHORTCUTS.pause} title={t("pauseOrResumeMotion")} onClick={onTogglePause}><ToolGlyph name={paused ? "play" : "pause"} /><span>{t(paused ? "resume" : "pause")}</span></button>
        <button type="button" aria-pressed={markersVisible} aria-label={t(markersVisible ? "hideMarkers" : "showMarkers")} aria-keyshortcuts={REVIEW_KEY_SHORTCUTS.markers} title={t("showOrHideMarkers")} onClick={onToggleMarkers}><ToolGlyph name={markersVisible ? "eye" : "eye-off"} /><span>{t(markersVisible ? "hideMarkers" : "showMarkers")}</span></button>
        <button type="button" aria-pressed={autoSendEnabled} aria-label={t(autoSendEnabled ? "turnAutoSendOff" : "turnAutoSendOn")} onClick={onToggleAutoSend}><ToolGlyph name="send" /><span>{t(autoSendEnabled ? "autoSendOn" : "autoSendOff")}</span></button>
      </div>
      <div className="a3s-settings-grid">
      <label>{t("theme")}<select aria-label={t("overlayTheme")} value={preferences.theme} onChange={(event) => update("theme", event.target.value as ReviewPreferences["theme"])}><option value="system">{t("themeSystem")}</option><option value="light">{t("themeLight")}</option><option value="dark">{t("themeDark")}</option></select></label>
      <label>{t("panelDock")}<select aria-label={t("panelDock")} value={preferences.dock} onChange={(event) => update("dock", event.target.value as ReviewPreferences["dock"])}><option value="right">{t("dockRight")}</option><option value="left">{t("dockLeft")}</option></select></label>
      <label>{t("markerColor")}<input type="color" aria-label={t("markerColor")} value={preferences.markerColor} onChange={(event) => update("markerColor", event.target.value)} /></label>
      <label>{t("wireframePageFade")} <output>{Math.round(preferences.wireframeFade * 100)}%</output><input type="range" aria-label={t("wireframePageFade")} min="0" max="0.8" step="0.01" value={preferences.wireframeFade} onChange={(event) => update("wireframeFade", event.currentTarget.valueAsNumber)} /></label>
      </div>
      <label className="a3s-setting-toggle"><input type="checkbox" aria-label={t("clearDraftsAfterCopy")} checked={preferences.clearOnCopy} onChange={(event) => update("clearOnCopy", event.target.checked)} /><span>{t("clearDraftsAfterCopyHelp")}</span></label>
      <label className="a3s-setting-toggle"><input type="checkbox" aria-label={t("blockPagePointerInput")} checked={preferences.blockInteractions} onChange={(event) => update("blockInteractions", event.target.checked)} /><span>{t("blockPagePointerInputHelp")}</span></label>
      <section className="a3s-shortcuts" aria-labelledby={shortcutsTitleId}>
        <h3 id={shortcutsTitleId} className="a3s-shortcuts-title">{t("keyboardShortcuts")}</h3>
        <dl>{REVIEW_SHORTCUT_HELP.map((shortcut) => <div key={shortcut.action}><dt>{reviewShortcutLabel(t, shortcut.action)}</dt><dd><kbd>{shortcut.keys}</kbd></dd></div>)}</dl>
        <p>{t("shortcutTypingHelp")}</p>
      </section>
      <button type="button" className="quiet" onClick={onHideUntilRestart}>{t("hideUntilTabRestart")}</button>
  </section>;
}
