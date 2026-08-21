import { A3S_UI_SHADOW_CSS } from "./a3s-ui-style";

export const OVERLAY_SHELL_CSS = `
${A3S_UI_SHADOW_CSS}

:host {
  all: initial;
  color-scheme: light dark;
}

*, *::before, *::after {
  box-sizing: border-box;
}

button, input, textarea, select {
  font: inherit;
}

.a3s-root {
  position: fixed;
  z-index: 2147483646;
  inset: 0;
  pointer-events: none;
  color: var(--a3s-text);
  font: 13px/1.55 var(--a3s-ui-font);
  --a3s-marker-color: #7157c9;
  --a3s-wireframe-fade: .16;
  --a3s-panel-raised: var(--a3s-panel);
  --a3s-soft: var(--a3s-panel-soft);
  --a3s-text: var(--a3s-ink);
  --a3s-faint: color-mix(in srgb, var(--a3s-muted) 88%, var(--a3s-text));
  --a3s-blue-strong: color-mix(in srgb, var(--a3s-blue) 84%, #000000);
  --a3s-blue-soft: color-mix(in srgb, var(--a3s-blue) 10%, var(--a3s-panel));
  --a3s-blue-ink: color-mix(in srgb, var(--a3s-blue) 94%, var(--a3s-text));
  --a3s-violet: var(--a3s-purple);
  --a3s-violet-soft: color-mix(in srgb, var(--a3s-purple) 10%, var(--a3s-panel));
  --a3s-green-ink: color-mix(in srgb, var(--a3s-green) 94%, var(--a3s-text));
  --a3s-green-soft: color-mix(in srgb, var(--a3s-green) 10%, var(--a3s-panel));
  --a3s-danger: color-mix(in srgb, var(--a3s-red) 94%, var(--a3s-text));
  --a3s-danger-soft: color-mix(in srgb, var(--a3s-red) 10%, var(--a3s-panel));
  --a3s-warning-ink: color-mix(in srgb, var(--a3s-warning) 94%, var(--a3s-text));
  --a3s-warning-soft: color-mix(in srgb, var(--a3s-warning) 12%, var(--a3s-panel));
  --a3s-toolbar: #15171c;
  --a3s-toolbar-raised: #252830;
  --a3s-toolbar-line: rgb(255 255 255 / 12%);
  --a3s-toolbar-text: #f7f8fb;
  --a3s-toolbar-muted: #a7afbd;
}

.a3s-sr-only,
.a3s-announcer {
  position: fixed;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  border: 0;
  clip: rect(0 0 0 0);
  clip-path: inset(50%);
  white-space: nowrap;
}

button {
  min-height: 32px;
  padding: 0 10px;
  border: 1px solid var(--a3s-line-strong);
  border-radius: 8px;
  background: var(--a3s-panel-raised);
  color: var(--a3s-text);
  cursor: pointer;
  font-weight: 650;
  transition: background-color 150ms ease, border-color 150ms ease, color 150ms ease, transform 150ms ease;
}

button:hover {
  border-color: var(--a3s-blue);
}

button:active {
  transform: translateY(1px);
}

button:disabled {
  opacity: .42;
  cursor: not-allowed;
}

button:focus-visible,
input:focus-visible,
textarea:focus-visible,
select:focus-visible,
.a3s-panel:focus-visible,
.a3s-list:focus-visible {
  outline: 3px solid var(--a3s-blue);
  outline-offset: 2px;
}

button.quiet {
  border-color: transparent;
  background: transparent;
  color: var(--a3s-muted);
}

button.danger {
  color: var(--a3s-danger);
}

.a3s-launch {
  position: fixed;
  right: 20px;
  bottom: 20px;
  display: grid;
  width: 44px;
  height: 44px;
  min-height: 44px;
  padding: 0;
  border: 1px solid var(--a3s-toolbar-line);
  border-radius: 13px;
  background: var(--a3s-toolbar);
  color: #ffffff;
  box-shadow: 0 14px 36px rgb(5 10 20 / 28%);
  pointer-events: auto;
  place-items: center;
  transform-origin: center;
  transition: opacity 180ms ease, transform 220ms cubic-bezier(.16, 1, .3, 1), visibility 180ms ease, background-color 150ms ease;
}

.a3s-launch:hover {
  border-color: rgb(255 255 255 / 26%);
  background: var(--a3s-toolbar-raised);
  transform: translateY(-2px);
}

.a3s-launch.is-active {
  box-shadow: 0 0 0 4px color-mix(in srgb, var(--a3s-blue) 22%, transparent), 0 12px 32px rgb(5 10 20 / 30%);
}

.a3s-launch.is-open {
  visibility: hidden;
  opacity: 0;
  pointer-events: none;
  transform: scale(.72) rotate(-12deg);
}

.a3s-launch > svg {
  width: 22px;
  height: 22px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.55;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.a3s-launch-count {
  position: absolute;
  top: -6px;
  right: -6px;
  display: grid;
  min-width: 20px;
  height: 20px;
  padding: 0 5px;
  border: 2px solid var(--a3s-panel);
  border-radius: 999px;
  background: var(--a3s-violet);
  color: #ffffff;
  font: 750 10px/1 ui-monospace, SFMono-Regular, Menlo, monospace;
  place-items: center;
}

.a3s-panel {
  --task-pane-width: 390px;
  position: fixed;
  z-index: 16;
  top: 12px;
  right: 12px;
  bottom: 12px;
  width: min(390px, calc(100vw - 24px));
  min-height: 0;
  overflow: hidden;
  border: 1px solid var(--a3s-line-strong);
  border-radius: 14px;
  background: var(--a3s-panel);
  box-shadow: 0 26px 68px rgb(5 10 20 / 26%), 0 4px 16px rgb(5 10 20 / 12%);
  pointer-events: auto;
  transform-origin: right center;
  animation: a3s-panel-enter 260ms cubic-bezier(.16, 1, .3, 1);
  transition: opacity 160ms ease, transform 220ms cubic-bezier(.16, 1, .3, 1), visibility 0s;
}

@keyframes a3s-panel-enter {
  from {
    opacity: .72;
    clip-path: inset(0 0 0 10% round 14px);
    transform: translateX(32px);
  }
  to {
    opacity: 1;
    clip-path: inset(0 round 14px);
    transform: translateX(0);
  }
}

.a3s-panel-header {
  display: grid;
  min-width: 0;
  min-height: 58px;
  padding: 9px 10px 9px 12px;
  border-bottom: 1px solid var(--a3s-line);
  background: var(--a3s-panel-raised);
  grid-template-columns: 32px minmax(0, 1fr) auto;
  align-items: center;
  gap: 9px;
}

.a3s-panel-mark {
  display: grid;
  width: 32px;
  height: 32px;
  border-radius: 9px;
  background: var(--a3s-blue);
  color: #ffffff;
  place-items: center;
}

.a3s-panel-mark > svg {
  width: 17px;
  height: 17px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.45;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.a3s-panel-header > span:nth-child(2) {
  min-width: 0;
  overflow: hidden;
}

.a3s-panel-title,
.a3s-panel-description {
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.a3s-panel-title {
  color: var(--a3s-text);
  font-size: 14px;
  line-height: 1.3;
  letter-spacing: -.01em;
}

.a3s-panel-description {
  margin-top: 1px;
  color: var(--a3s-muted);
  font-size: 10px;
}

.a3s-close {
  display: grid;
  width: 26px;
  height: 26px;
  min-height: 26px;
  padding: 0;
  border-color: transparent;
  background: transparent;
  color: var(--a3s-muted);
  place-items: center;
}

.a3s-close:hover {
  border-color: var(--a3s-line);
  background: var(--a3s-soft);
  color: var(--a3s-text);
}

.a3s-close svg {
  width: 15px;
  height: 15px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.6;
  stroke-linecap: round;
}

.a3s-panel-actions {
  display: flex;
  align-items: center;
  gap: 2px;
}

.a3s-header-settings {
  display: grid;
  width: 30px;
  height: 30px;
  min-height: 30px;
  padding: 0;
  border-color: transparent;
  background: transparent;
  color: var(--a3s-muted);
  place-items: center;
}

.a3s-header-settings:hover,
.a3s-header-settings.selected {
  border-color: var(--a3s-line);
  background: var(--a3s-soft);
  color: var(--a3s-blue-ink);
}

.a3s-header-settings svg {
  width: 16px;
  height: 16px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.55;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.a3s-panel-tabs {
  height: auto;
  min-height: 45px;
  padding: 5px 8px;
  overflow: visible;
  border-bottom: 1px solid var(--a3s-line);
  background: var(--a3s-bg);
  gap: 4px;
}

.a3s-panel-tabs button {
  position: relative;
  display: inline-flex;
  min-width: 0;
  min-height: 34px;
  padding: 0 8px;
  border-color: transparent;
  background: transparent;
  color: var(--a3s-muted);
  align-items: center;
  justify-content: center;
  gap: 6px;
  font-size: 11px;
  flex: 1 1 0;
}

.a3s-panel-tabs button:hover {
  background: var(--a3s-soft);
  color: var(--a3s-text);
}

.a3s-panel-tabs button.selected {
  border-color: var(--a3s-line);
  background: var(--a3s-panel-raised);
  color: var(--a3s-blue-ink);
}

.a3s-panel-tabs svg,
.a3s-tools button svg {
  width: 16px;
  height: 16px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.55;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.a3s-panel-tabs b {
  display: grid;
  min-width: 17px;
  height: 17px;
  padding: 0 4px;
  border-radius: 999px;
  background: var(--a3s-violet-soft);
  color: var(--a3s-violet);
  font-size: 9px;
  line-height: 1;
  place-items: center;
}

.a3s-panel-body {
  min-height: 0;
  overflow: hidden;
  flex: 1 1 auto;
}

.a3s-compose {
  display: flex;
  height: 100%;
  min-height: 0;
  overflow: auto;
  overscroll-behavior: contain;
  flex-direction: column;
}

.a3s-tools {
  padding: 16px 14px;
}

.a3s-tools-heading {
  display: flex;
  margin-bottom: 10px;
  align-items: flex-start;
  justify-content: space-between;
  gap: 10px;
}

.a3s-tools-heading strong,
.a3s-tools-heading small {
  display: block;
}

.a3s-tools-heading strong {
  color: var(--a3s-text);
  font-size: 12px;
}

.a3s-tools-heading small {
  margin-top: 2px;
  color: var(--a3s-muted);
  font-size: 10px;
}

.a3s-selection-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 6px;
}

.a3s-tools .a3s-selection-grid button {
  display: inline-flex;
  min-width: 0;
  min-height: 38px;
  padding: 0 10px;
  border-color: var(--a3s-line);
  background: var(--a3s-panel-raised);
  color: var(--a3s-muted);
  align-items: center;
  justify-content: flex-start;
  gap: 7px;
  font-size: 11px;
}

.a3s-tools .a3s-selection-grid button:hover {
  border-color: var(--a3s-blue);
  color: var(--a3s-text);
}

.a3s-tools .a3s-selection-grid button.selected {
  border-color: var(--a3s-blue);
  background: var(--a3s-blue-soft);
  color: var(--a3s-blue-ink);
}

.a3s-primary-tools > button:first-child:not(.selected) {
  border-color: var(--a3s-blue);
  background: var(--a3s-blue);
  color: #ffffff;
}

.a3s-primary-tools > button:first-child:not(.selected):hover {
  border-color: var(--a3s-blue-strong);
  background: var(--a3s-blue-strong);
  color: #ffffff;
}

.a3s-more-tools {
  position: relative;
  display: flex;
  width: 100%;
  min-height: 32px;
  margin-top: 8px;
  padding: 0 25px 0 6px;
  align-items: center;
  justify-content: flex-start;
  gap: 6px;
  font-size: 10px;
}

.a3s-more-tools > i {
  position: absolute;
  top: 10px;
  right: 8px;
  width: 7px;
  height: 7px;
  border-right: 1.5px solid currentColor;
  border-bottom: 1.5px solid currentColor;
  transform: rotate(45deg);
  transition: transform 150ms ease, top 150ms ease;
}

.a3s-more-tools[aria-expanded="true"] > i {
  top: 13px;
  transform: rotate(225deg);
}

.a3s-secondary-tools {
  margin-top: 6px;
  animation: a3s-secondary-tools-enter 160ms cubic-bezier(.16, 1, .3, 1);
}

.a3s-secondary-tools[hidden] {
  display: none;
}

@keyframes a3s-secondary-tools-enter {
  from {
    opacity: 0;
    transform: translateY(-4px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.a3s-cancel-marking {
  display: inline-flex;
  min-height: 30px;
  padding: 0 7px;
  align-items: center;
  gap: 4px;
  font-size: 10px;
}

.a3s-panel.is-marking {
  visibility: hidden;
  opacity: 0;
  pointer-events: none;
  transform: translateX(calc(100% + 24px));
  animation: none;
  transition: opacity 160ms ease, transform 220ms cubic-bezier(.16, 1, .3, 1), visibility 0s linear 220ms;
}

.a3s-root[data-dock="left"] .a3s-panel.is-marking {
  transform: translateX(calc(-100% - 24px));
}

.a3s-mobile-marking-bar {
  position: fixed;
  z-index: 18;
  top: max(8px, env(safe-area-inset-top));
  left: 50%;
  display: flex;
  width: min(560px, calc(100vw - 16px));
  min-height: 48px;
  padding: 7px 8px 7px 12px;
  border: 1px solid var(--a3s-line-strong);
  border-radius: 12px;
  background: var(--a3s-panel-raised);
  box-shadow: 0 16px 42px rgb(5 10 20 / 24%);
  pointer-events: none;
  transform: translateX(-50%);
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.a3s-mobile-marking-bar > span {
  overflow: hidden;
  color: var(--a3s-text);
  font-size: 12px;
  font-weight: 650;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.a3s-mobile-marking-actions {
  display: flex;
  flex: 0 0 auto;
  pointer-events: auto;
  align-items: center;
  gap: 5px;
}

.a3s-mobile-marking-actions button {
  display: inline-flex;
  min-height: 34px;
  flex: 0 0 auto;
  align-items: center;
  gap: 5px;
}

.a3s-mobile-marking-bar svg {
  width: 15px;
  height: 15px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.55;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.a3s-root[data-dock="left"] .a3s-launch {
  right: auto;
  left: 20px;
}

.a3s-root[data-dock="left"] .a3s-panel {
  right: auto;
  left: 12px;
  transform-origin: left center;
}

@media (max-width: 720px) {
  .a3s-panel.is-marking,
  .a3s-root[data-dock="left"] .a3s-panel.is-marking {
    display: none;
  }

  .a3s-mobile-marking-bar {
    right: 8px;
    left: 8px;
    width: auto;
    transform: none;
  }
}
`;
