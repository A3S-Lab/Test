export const OVERLAY_SHELL_CSS = `
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
  font: 13px/1.55 "PingFang SC", "Microsoft YaHei", "Noto Sans CJK SC", ui-sans-serif, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  --a3s-marker-color: #7157c9;
  --a3s-wireframe-fade: .16;
  --a3s-bg: #0b1426;
  --a3s-panel: #101c32;
  --a3s-panel-raised: #14233e;
  --a3s-soft: #172844;
  --a3s-line: #30415e;
  --a3s-line-strong: #465a7a;
  --a3s-text: #f4f7fc;
  --a3s-muted: #aab7ca;
  --a3s-faint: #8190a7;
  --a3s-blue: #4c87ff;
  --a3s-blue-strong: #1d60db;
  --a3s-blue-soft: #19376c;
  --a3s-blue-ink: #b8cdff;
  --a3s-violet: #9c87e8;
  --a3s-violet-soft: #2d2750;
  --a3s-green: #3bc89b;
  --a3s-green-soft: #113f36;
  --a3s-danger: #ff8c8c;
  --a3s-danger-soft: #471f29;
  --a3s-warning: #f6c45b;
  --a3s-warning-soft: #3c3015;
  --a3s-shadow: 0 28px 72px rgb(0 0 0 / 36%);
  --a3s-toolbar: #15171c;
  --a3s-toolbar-raised: #252830;
  --a3s-toolbar-line: rgb(255 255 255 / 12%);
  --a3s-toolbar-text: #f7f8fb;
  --a3s-toolbar-muted: #a7afbd;
}

.a3s-root[data-theme="light"] {
  --a3s-bg: #f8fbff;
  --a3s-panel: #ffffff;
  --a3s-panel-raised: #ffffff;
  --a3s-soft: #f1f5fb;
  --a3s-line: #dce4f0;
  --a3s-line-strong: #bfd0e8;
  --a3s-text: #101827;
  --a3s-muted: #56657b;
  --a3s-faint: #5f6f84;
  --a3s-blue: #1264ff;
  --a3s-blue-strong: #084ed0;
  --a3s-blue-soft: #eaf2ff;
  --a3s-blue-ink: #084ed0;
  --a3s-violet: #7157c9;
  --a3s-violet-soft: #f0edfb;
  --a3s-green: #087858;
  --a3s-green-soft: #e9f8f2;
  --a3s-danger: #c9343f;
  --a3s-danger-soft: #fff0f1;
  --a3s-warning: #8a5200;
  --a3s-warning-soft: #fff3d4;
  --a3s-shadow: 0 28px 72px rgb(36 76 137 / 18%);
}

@media (prefers-color-scheme: light) {
  .a3s-root[data-theme="system"] {
    --a3s-bg: #f8fbff;
    --a3s-panel: #ffffff;
    --a3s-panel-raised: #ffffff;
    --a3s-soft: #f1f5fb;
    --a3s-line: #dce4f0;
    --a3s-line-strong: #bfd0e8;
    --a3s-text: #101827;
    --a3s-muted: #56657b;
    --a3s-faint: #5f6f84;
    --a3s-blue: #1264ff;
    --a3s-blue-strong: #084ed0;
    --a3s-blue-soft: #eaf2ff;
    --a3s-blue-ink: #084ed0;
    --a3s-violet: #7157c9;
    --a3s-violet-soft: #f0edfb;
    --a3s-green: #087858;
    --a3s-green-soft: #e9f8f2;
    --a3s-danger: #c9343f;
    --a3s-danger-soft: #fff0f1;
    --a3s-warning: #8a5200;
    --a3s-warning-soft: #fff3d4;
    --a3s-shadow: 0 28px 72px rgb(36 76 137 / 18%);
  }
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
  position: fixed;
  right: 20px;
  bottom: 20px;
  width: min(410px, calc(100vw - 40px));
  overflow: visible;
  border: 0;
  background: transparent;
  pointer-events: none;
  transform-origin: right bottom;
  animation: a3s-panel-enter 260ms cubic-bezier(.16, 1, .3, 1);
}

@keyframes a3s-panel-enter {
  from {
    opacity: 0;
    transform: translateY(8px) scale(.94);
  }
  to {
    opacity: 1;
    transform: translateY(0) scale(1);
  }
}

.a3s-command-bar {
  display: flex;
  min-width: 0;
  height: 58px;
  padding: 6px;
  border: 1px solid var(--a3s-toolbar-line);
  border-radius: 16px;
  background: color-mix(in srgb, var(--a3s-toolbar) 97%, transparent);
  box-shadow: 0 20px 52px rgb(5 10 20 / 26%), 0 2px 8px rgb(5 10 20 / 18%);
  color: var(--a3s-toolbar-text);
  pointer-events: auto;
  align-items: center;
  gap: 7px;
  backdrop-filter: blur(14px);
}

.a3s-command-bar > header {
  display: grid;
  min-width: 116px;
  height: 46px;
  padding: 0 5px 0 1px;
  border-right: 1px solid var(--a3s-toolbar-line);
  grid-template-columns: 32px minmax(48px, 1fr) 30px;
  align-items: center;
  gap: 3px;
}

.a3s-panel-mark {
  display: grid;
  width: 30px;
  height: 30px;
  border-radius: 9px;
  background: #1264ff;
  color: #ffffff;
  font: 780 8px/1 ui-sans-serif, sans-serif;
  letter-spacing: -.03em;
  place-items: center;
  box-shadow: inset 0 1px 0 rgb(255 255 255 / 26%), 0 5px 12px rgb(18 100 255 / 22%);
}

.a3s-panel-mark > svg {
  width: 19px;
  height: 19px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.45;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.a3s-command-bar > header > span:nth-child(2) {
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
  color: var(--a3s-toolbar-text);
  font-size: 11.5px;
  line-height: 1.2;
}

.a3s-panel-description {
  margin-top: 2px;
  color: var(--a3s-toolbar-muted);
  font-size: 9px;
  line-height: 1.25;
}

.a3s-close {
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

.a3s-command-bar .a3s-close {
  color: var(--a3s-toolbar-muted);
}

.a3s-command-bar .a3s-close:hover {
  border-color: var(--a3s-toolbar-line);
  background: rgb(255 255 255 / 8%);
  color: var(--a3s-toolbar-text);
}

.a3s-close svg {
  width: 15px;
  height: 15px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.6;
  stroke-linecap: round;
}

.a3s-tools {
  position: relative;
  display: block;
  min-width: 0;
  flex: 1 1 auto;
}

.a3s-toolbar-core {
  display: flex;
  min-width: 0;
  align-items: center;
  justify-content: flex-start;
  gap: 4px;
}

.a3s-tool-group {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  gap: 3px;
}

.a3s-tool-group-primary {
  padding: 2px;
  border: 1px solid rgb(255 255 255 / 7%);
  border-radius: 10px;
  background: rgb(255 255 255 / 5%);
}

.a3s-tool-divider {
  width: 1px;
  height: 24px;
  flex: 0 0 auto;
  background: var(--a3s-toolbar-line);
}

.a3s-tools button,
.a3s-settings > .a3s-disclosure {
  position: relative;
  display: grid;
  width: 38px;
  height: 38px;
  min-height: 38px;
  padding: 0;
  border-color: transparent;
  border-radius: 8px;
  background: transparent;
  color: var(--a3s-toolbar-muted);
  place-items: center;
}

.a3s-tools button:hover,
.a3s-tools button.selected,
.a3s-settings > .a3s-disclosure:hover,
.a3s-settings > .a3s-disclosure[aria-expanded="true"] {
  border-color: rgb(255 255 255 / 9%);
  background: var(--a3s-toolbar-raised);
  color: var(--a3s-toolbar-text);
}

.a3s-tool-group-primary button.selected {
  border-color: rgb(255 255 255 / 19%);
  background: #7157c9;
  color: #ffffff;
  box-shadow: inset 0 1px 0 rgb(255 255 255 / 18%);
}

.a3s-tools button.danger {
  background: rgb(255 107 118 / 15%);
  color: #ff9ba3;
}

.a3s-tools button svg,
.a3s-settings > .a3s-disclosure svg {
  width: 17px;
  height: 17px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.55;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.a3s-tools button[data-tooltip]::after,
.a3s-settings > .a3s-disclosure[data-tooltip]::after {
  position: absolute;
  bottom: calc(100% + 9px);
  left: 50%;
  z-index: 12;
  max-width: min(260px, calc(100vw - 24px));
  padding: 6px 8px;
  border-radius: 6px;
  border: 1px solid rgb(255 255 255 / 9%);
  background: #0e1015;
  color: #f7f8fb;
  content: attr(data-tooltip);
  font-size: 10.5px;
  font-weight: 650;
  opacity: 0;
  pointer-events: none;
  transform: translate(-50%, 3px);
  transition: opacity 120ms ease, transform 120ms ease;
  text-align: center;
  white-space: nowrap;
}

.a3s-tools button[data-tooltip]:hover::after,
.a3s-tools button[data-tooltip]:focus-visible::after,
.a3s-settings > .a3s-disclosure[data-tooltip]:hover::after,
.a3s-settings > .a3s-disclosure[data-tooltip]:focus-visible::after {
  opacity: 1;
  transform: translate(-50%, 0);
}

.a3s-workspace-toggle {
  overflow: visible;
}

.a3s-tool-tray {
  position: absolute;
  right: 0;
  bottom: calc(100% + 12px);
  display: grid;
  width: min(416px, calc(100vw - 40px));
  min-height: 58px;
  padding: 8px;
  border: 1px solid var(--a3s-toolbar-line);
  border-radius: 14px;
  background: color-mix(in srgb, var(--a3s-toolbar) 97%, transparent);
  box-shadow: 0 18px 48px rgb(5 10 20 / 30%);
  color: var(--a3s-toolbar-text);
  grid-template-columns: minmax(112px, 1fr) auto;
  align-items: center;
  gap: 8px;
  animation: a3s-tool-tray-enter 180ms cubic-bezier(.16, 1, .3, 1);
}

.a3s-tool-tray[hidden] {
  display: none;
}

@keyframes a3s-tool-tray-enter {
  from {
    opacity: 0;
    transform: translateY(6px) scale(.97);
  }
  to {
    opacity: 1;
    transform: translateY(0) scale(1);
  }
}

.a3s-tool-tray-copy {
  min-width: 0;
  padding-left: 6px;
}

.a3s-tool-tray-copy strong,
.a3s-tool-tray-copy span {
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.a3s-tool-tray-copy strong {
  color: var(--a3s-toolbar-text);
  font-size: 11px;
}

.a3s-tool-tray-copy span {
  margin-top: 2px;
  color: var(--a3s-toolbar-muted);
  font-size: 9px;
}

.a3s-tool-count {
  position: absolute;
  top: -4px;
  right: -4px;
  display: grid;
  min-width: 17px;
  height: 17px;
  padding: 0 4px;
  border: 2px solid var(--a3s-toolbar);
  border-radius: 999px;
  background: var(--a3s-violet);
  color: #ffffff;
  font: 750 8px/1 ui-monospace, SFMono-Regular, Menlo, monospace;
  place-items: center;
}

.a3s-root[data-dock="left"] .a3s-launch {
  right: auto;
  left: 20px;
}

.a3s-root[data-dock="left"] .a3s-panel {
  right: auto;
  left: 20px;
  transform-origin: left bottom;
}

.a3s-root[data-dock="left"] .a3s-tool-tray {
  right: auto;
  left: 0;
}
`;
