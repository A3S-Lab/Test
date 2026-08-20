import { DESIGN_REFERENCE_CSS } from "./design-reference-style";

export const DESIGN_BOARD_CSS = `
${DESIGN_REFERENCE_CSS}

.a3s-design-layer {
  position: fixed;
  z-index: 20;
  inset: 0;
  display: flex;
  padding: 12px;
  pointer-events: none;
  justify-content: flex-end;
}

.a3s-page-capture {
  position: fixed;
  z-index: 24;
  inset: 0;
  overflow: hidden;
  background: rgb(5 12 26 / 48%);
  cursor: crosshair;
  pointer-events: auto;
  touch-action: none;
  user-select: none;
}

.a3s-page-capture.has-selection {
  background: transparent;
}

.a3s-page-capture:focus-visible {
  outline: none;
}

.a3s-page-capture-bar {
  position: fixed;
  z-index: 3;
  top: max(14px, env(safe-area-inset-top));
  left: 50%;
  display: grid;
  width: min(510px, calc(100vw - 24px));
  min-height: 52px;
  padding: 7px 8px 7px 10px;
  border: 1px solid rgb(255 255 255 / 18%);
  border-radius: 12px;
  background: #0e1b35;
  box-shadow: 0 16px 42px rgb(3 8 18 / 34%), 0 3px 10px rgb(3 8 18 / 24%);
  color: #ffffff;
  cursor: default;
  grid-template-columns: 32px minmax(0, 1fr) 34px;
  align-items: center;
  gap: 9px;
  transform: translateX(-50%);
}

.a3s-page-capture-icon {
  display: grid;
  width: 30px;
  height: 30px;
  border-radius: 8px;
  background: var(--a3s-blue);
  color: #ffffff;
  place-items: center;
}

.a3s-page-capture-icon svg,
.a3s-page-capture-bar button svg {
  width: 16px;
  height: 16px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.7;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.a3s-page-capture-copy {
  display: flex;
  min-width: 0;
  flex-direction: column;
}

.a3s-page-capture-copy strong {
  overflow: hidden;
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.a3s-page-capture-copy small {
  overflow: hidden;
  color: #c7d6ee;
  font-size: 10px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.a3s-page-capture-bar button {
  display: grid;
  width: 34px;
  height: 34px;
  min-height: 34px;
  padding: 0;
  border-color: transparent;
  background: transparent;
  color: #d8e3f5;
  place-items: center;
}

.a3s-page-capture-bar button:hover {
  background: rgb(255 255 255 / 10%);
  color: #ffffff;
}

.a3s-page-capture-selection {
  position: fixed;
  z-index: 1;
  border: 2px solid #ffffff;
  background: transparent;
  box-shadow: 0 0 0 1px var(--a3s-blue), 0 0 0 9999px rgb(5 12 26 / 58%);
  pointer-events: none;
}

.a3s-page-capture-selection::before,
.a3s-page-capture-selection::after {
  position: absolute;
  width: 8px;
  height: 8px;
  border: 2px solid #ffffff;
  background: var(--a3s-blue);
  content: "";
}

.a3s-page-capture-selection::before {
  top: -5px;
  left: -5px;
}

.a3s-page-capture-selection::after {
  right: -5px;
  bottom: -5px;
}

.a3s-page-capture-size {
  position: fixed;
  z-index: 2;
  display: grid;
  width: 88px;
  height: 26px;
  border-radius: 7px;
  background: #0e1b35;
  box-shadow: 0 5px 16px rgb(3 8 18 / 28%);
  color: #ffffff;
  font: 700 10px/1 ui-monospace, SFMono-Regular, Menlo, monospace;
  pointer-events: none;
  place-items: center;
}

.a3s-page-capture[aria-busy="true"] {
  cursor: progress;
}

.a3s-design-board {
  display: flex;
  width: min(880px, calc(100vw - 24px));
  height: calc(100dvh - 24px);
  min-height: 420px;
  overflow: hidden;
  border: 1px solid var(--a3s-line-strong);
  border-radius: 14px;
  background: var(--a3s-panel);
  box-shadow: 0 28px 72px rgb(5 10 20 / 30%), 0 4px 18px rgb(5 10 20 / 14%);
  color: var(--a3s-text);
  pointer-events: auto;
  flex-direction: column;
  animation: a3s-design-drawer-enter 260ms cubic-bezier(.16, 1, .3, 1);
  container-type: inline-size;
}

@keyframes a3s-design-drawer-enter {
  from {
    opacity: .72;
    clip-path: inset(0 0 0 10% round 14px);
    transform: translateX(40px);
  }
  to {
    opacity: 1;
    clip-path: inset(0 round 14px);
    transform: translateX(0);
  }
}

.a3s-design-header {
  display: grid;
  min-height: 58px;
  padding: 8px 10px 8px 12px;
  border-bottom: 1px solid var(--a3s-line);
  background: var(--a3s-panel-raised);
  grid-template-columns: 34px minmax(0, 1fr) 32px;
  align-items: center;
  gap: 9px;
}

.a3s-design-header-icon {
  display: grid;
  width: 32px;
  height: 32px;
  border-radius: 9px;
  background: var(--a3s-blue);
  color: #ffffff;
  place-items: center;
}

.a3s-design-header-icon svg,
.a3s-design-header .a3s-close svg {
  width: 17px;
  height: 17px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.65;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.a3s-design-heading {
  display: flex;
  min-width: 0;
  flex-direction: column;
}

.a3s-design-heading strong {
  font-size: 14px;
  letter-spacing: -.01em;
}

.a3s-design-heading small {
  overflow: hidden;
  color: var(--a3s-muted);
  font-size: 10px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.a3s-design-body {
  display: flex;
  min-height: 0;
  padding: 10px 12px 12px;
  overflow: hidden;
  flex: 1 1 auto;
  flex-direction: column;
  gap: 10px;
}

.a3s-design-toolbar {
  height: auto;
  min-height: 48px;
  padding: 6px;
  overflow: visible;
  border: 1px solid var(--a3s-line);
  border-radius: 10px;
  background: var(--a3s-panel-raised);
  align-items: center;
  flex: 0 0 auto;
  flex-wrap: wrap;
  gap: 5px;
}

.a3s-design-tool-group,
.a3s-design-history,
.a3s-design-style {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  gap: 4px;
}

.a3s-design-toolbar button {
  display: inline-flex;
  min-height: 34px;
  padding: 0 8px;
  align-items: center;
  justify-content: center;
  gap: 5px;
  font-size: 10.5px;
  white-space: nowrap;
}

.a3s-design-toolbar button svg,
.a3s-design-attach svg,
.a3s-design-empty svg {
  width: 17px;
  height: 17px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.65;
  stroke-linecap: round;
  stroke-linejoin: round;
  flex: 0 0 auto;
}

.a3s-design-toolbar button.selected {
  border-color: var(--a3s-blue);
  background: var(--a3s-blue-soft);
  color: var(--a3s-blue-ink);
}

.a3s-design-history button {
  width: 34px;
  padding: 0;
}

.a3s-design-divider {
  width: 1px;
  height: 24px;
  background: var(--a3s-line);
  flex: 0 0 auto;
}

.a3s-design-style {
  width: 100%;
  margin-left: 0;
  padding-top: 6px;
  border-top: 1px solid var(--a3s-line);
  justify-content: flex-end;
}

.a3s-design-style label {
  display: flex;
  color: var(--a3s-muted);
  align-items: center;
  gap: 4px;
  font-size: 9.5px;
  font-weight: 650;
}

.a3s-design-style input[type="color"] {
  width: 30px;
  height: 30px;
  padding: 2px;
  border: 1px solid var(--a3s-line-strong);
  border-radius: 7px;
  background: var(--a3s-bg);
}

.a3s-design-style select {
  width: auto;
  min-width: 52px;
  min-height: 30px;
  padding: 3px 5px;
  border: 1px solid var(--a3s-line-strong);
  border-radius: 7px;
  background: var(--a3s-bg);
  color: var(--a3s-text);
  font-size: 10px;
}

.a3s-design-style select:disabled,
.a3s-design-style input:disabled {
  opacity: .42;
}

.a3s-design-stage {
  display: flex;
  min-height: 280px;
  overflow: hidden;
  border: 1px solid var(--a3s-line-strong);
  border-radius: 10px;
  background: #d9e1ec;
  flex: 1 1 auto;
}

.a3s-design-canvas {
  position: relative;
  display: grid;
  width: 100%;
  min-height: 0;
  padding: 12px;
  overflow: auto;
  place-items: center;
}

.a3s-design-canvas-surface {
  display: block;
  width: 100%;
  max-width: 960px;
  max-height: 100%;
  overflow: visible;
  background: #ffffff;
  box-shadow: 0 3px 14px rgb(15 23 42 / 18%);
  aspect-ratio: 8 / 5;
  touch-action: none;
  user-select: none;
  cursor: crosshair;
}

.a3s-design-canvas-surface[data-tool="select"] {
  cursor: default;
}

.a3s-design-canvas-surface:focus-visible {
  outline: 3px solid var(--a3s-blue);
  outline-offset: 2px;
}

.a3s-design-canvas-background {
  fill: #ffffff;
}

.a3s-design-empty {
  position: absolute;
  z-index: 1;
  top: 50%;
  left: 50%;
  display: flex;
  width: min(320px, calc(100% - 48px));
  color: #5f6f84;
  pointer-events: none;
  text-align: center;
  align-items: center;
  flex-direction: column;
  transform: translate(-50%, -50%);
}

.a3s-design-empty > span {
  display: grid;
  width: 38px;
  height: 38px;
  margin-bottom: 8px;
  border: 1px solid #bfd0e8;
  border-radius: 10px;
  background: #eaf2ff;
  color: #1264ff;
  place-items: center;
}

.a3s-design-empty strong {
  color: #26364d;
  font-size: 12px;
}

.a3s-design-empty small {
  max-width: 34ch;
  margin-top: 3px;
  font-size: 10px;
}

.a3s-design-element {
  cursor: inherit;
}

.a3s-design-canvas-surface[data-tool="select"] .a3s-design-element {
  cursor: move;
}

.a3s-design-selection rect {
  fill: none;
  stroke: var(--a3s-blue);
  stroke-width: 2;
  stroke-dasharray: 6 4;
  pointer-events: none;
}

.a3s-design-selection circle {
  fill: #ffffff;
  stroke: var(--a3s-blue);
  stroke-width: 2;
  cursor: nwse-resize;
  pointer-events: all;
}

.a3s-design-text-editor {
  overflow: visible;
}

.a3s-design-text-editor input {
  width: 100%;
  height: 38px;
  padding: 7px 9px;
  border: 2px solid var(--a3s-blue);
  border-radius: 5px;
  background: #ffffff;
  color: #111827;
  font: 24px/1.2 ui-sans-serif, system-ui, sans-serif;
}

.a3s-design-footer {
  display: grid;
  min-height: 58px;
  padding: 8px 12px;
  border-top: 1px solid var(--a3s-line);
  background: var(--a3s-bg);
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: 12px;
}

.a3s-design-status {
  min-width: 0;
  margin: 0;
  overflow: hidden;
  color: var(--a3s-faint);
  font-size: 10px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.a3s-design-status.is-error {
  color: var(--a3s-danger);
  white-space: normal;
}

.a3s-design-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 7px;
}

.a3s-design-attach {
  display: inline-flex;
  min-height: 36px;
  border-color: var(--a3s-blue);
  background: var(--a3s-blue);
  color: #ffffff;
  align-items: center;
  gap: 5px;
}

.a3s-design-attach:hover {
  border-color: var(--a3s-blue-strong);
  background: var(--a3s-blue-strong);
}

@media (max-width: 600px) {
  .a3s-page-capture-bar {
    top: max(8px, env(safe-area-inset-top));
    width: calc(100vw - 16px);
    min-height: 58px;
  }

  .a3s-page-capture-bar button {
    width: 44px;
    height: 44px;
    min-height: 44px;
  }

  .a3s-design-layer {
    padding: 0;
  }

  .a3s-design-board {
    width: 100%;
    height: 100dvh;
    min-height: 0;
    border: 0;
    border-radius: 0;
  }

  .a3s-design-header {
    min-height: 60px;
    padding-top: max(8px, env(safe-area-inset-top));
  }

  .a3s-design-body {
    padding: 8px;
    gap: 8px;
  }

  .a3s-design-toolbar {
    max-height: 158px;
    padding: 5px;
    overflow-y: auto;
  }

  .a3s-design-toolbar button,
  .a3s-design-style input[type="color"],
  .a3s-design-style select {
    min-height: 44px;
  }

  .a3s-design-history button {
    width: 44px;
  }

  .a3s-design-style {
    justify-content: flex-start;
  }

  .a3s-design-stage {
    min-height: 220px;
  }

  .a3s-design-canvas {
    padding: 8px;
  }

  .a3s-design-footer {
    padding: 8px;
    padding-bottom: max(8px, env(safe-area-inset-bottom));
    grid-template-columns: 1fr;
    gap: 6px;
  }

  .a3s-design-status {
    min-height: 16px;
    white-space: normal;
  }

  .a3s-design-actions button {
    min-height: 44px;
    flex: 1 1 0;
  }
}

@media (max-width: 420px) {
  .a3s-design-media {
    flex: 1 1 auto;
  }

  .a3s-design-media button {
    flex: 1 1 auto;
  }
}
`;
