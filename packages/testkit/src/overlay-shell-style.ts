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
  z-index: 16;
  top: 12px;
  right: 12px;
  bottom: 12px;
  display: grid;
  width: min(390px, calc(100vw - 24px));
  min-height: 0;
  overflow: hidden;
  border: 1px solid var(--a3s-line-strong);
  border-radius: 14px;
  background: var(--a3s-panel);
  box-shadow: 0 26px 68px rgb(5 10 20 / 26%), 0 4px 16px rgb(5 10 20 / 12%);
  pointer-events: auto;
  grid-template-rows: auto auto minmax(0, 1fr);
  transform-origin: right center;
  animation: a3s-panel-enter 260ms cubic-bezier(.16, 1, .3, 1);
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
  grid-template-columns: 32px minmax(0, 1fr) 32px;
  align-items: center;
  gap: 9px;
}

.a3s-panel-mark {
  display: grid;
  width: 32px;
  height: 32px;
  border-radius: 9px;
  background: #1264ff;
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

.a3s-panel-tabs {
  display: grid;
  min-height: 45px;
  padding: 5px 8px;
  border-bottom: 1px solid var(--a3s-line);
  background: var(--a3s-bg);
  grid-template-columns: repeat(3, 1fr);
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
.a3s-tools button svg,
.a3s-compose-empty svg {
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
  padding: 14px;
  border-bottom: 1px solid var(--a3s-line);
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

.a3s-cancel-marking {
  display: inline-flex;
  min-height: 30px;
  padding: 0 7px;
  align-items: center;
  gap: 4px;
  font-size: 10px;
}

.a3s-compose-empty {
  display: flex;
  min-height: 180px;
  padding: 28px 24px;
  color: var(--a3s-muted);
  text-align: center;
  align-items: center;
  justify-content: center;
  flex-direction: column;
}

.a3s-compose-empty > span {
  display: grid;
  width: 40px;
  height: 40px;
  margin-bottom: 10px;
  border-radius: 11px;
  background: var(--a3s-blue-soft);
  color: var(--a3s-blue-ink);
  place-items: center;
}

.a3s-compose-empty strong {
  color: var(--a3s-text);
  font-size: 13px;
}

.a3s-compose-empty p {
  max-width: 32ch;
  margin: 4px 0 0;
  font-size: 11px;
}

.a3s-mobile-marking-bar {
  display: none;
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
    position: fixed;
    z-index: 18;
    top: max(8px, env(safe-area-inset-top));
    right: 8px;
    left: 8px;
    display: flex;
    min-height: 48px;
    padding: 7px 8px 7px 12px;
    border: 1px solid var(--a3s-line-strong);
    border-radius: 12px;
    background: var(--a3s-panel-raised);
    box-shadow: 0 16px 42px rgb(5 10 20 / 24%);
    pointer-events: auto;
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
}
`;
