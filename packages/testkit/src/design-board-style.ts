export const DESIGN_BOARD_CSS = `
.a3s-design-reference {
  display: grid;
  padding: 9px;
  border: 1px solid var(--a3s-line);
  border-radius: 10px;
  background: var(--a3s-soft);
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: 7px 9px;
}

.a3s-design-reference.has-reference {
  grid-template-columns: 64px minmax(0, 1fr) auto;
}

.a3s-design-reference > div {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 1px;
}

.a3s-design-reference strong {
  color: var(--a3s-text);
  font-size: 11px;
}

.a3s-design-reference small {
  color: var(--a3s-faint);
  font-size: 9.5px;
}

.a3s-design-reference img {
  width: 64px;
  height: 44px;
  border: 1px solid var(--a3s-line-strong);
  border-radius: 7px;
  background: #ffffff;
  object-fit: cover;
  grid-row: 1 / span 2;
}

.a3s-design-reference img + div {
  grid-column: 2;
}

.a3s-design-reference img ~ button {
  min-height: 28px;
  padding: 0 7px;
  font-size: 10px;
  grid-row: 2;
}

.a3s-design-reference img ~ button:first-of-type {
  grid-column: 2;
  justify-self: start;
}

.a3s-design-reference img ~ button:last-of-type {
  grid-column: 3;
}

.a3s-design-reference > button {
  white-space: nowrap;
}

.a3s-design-scrim {
  position: fixed;
  z-index: 20;
  inset: 0;
  display: grid;
  padding: 20px;
  background: rgb(3 8 18 / 76%);
  pointer-events: auto;
  place-items: center;
}

.a3s-design-board {
  display: flex;
  width: min(1040px, 100%);
  height: min(820px, calc(100dvh - 40px));
  max-height: 100%;
  overflow: hidden;
  border: 1px solid var(--a3s-line-strong);
  border-radius: 16px;
  background: var(--a3s-panel);
  box-shadow: 0 34px 100px rgb(0 0 0 / 52%);
  color: var(--a3s-text);
  flex-direction: column;
}

.a3s-design-header {
  display: flex;
  min-height: 54px;
  padding: 8px 10px 8px 14px;
  border-bottom: 1px solid var(--a3s-line);
  background: var(--a3s-panel-raised);
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}

.a3s-design-header > div {
  display: flex;
  min-width: 0;
  flex-direction: column;
}

.a3s-design-header strong {
  font-size: 14px;
  letter-spacing: -.01em;
}

.a3s-design-header small {
  color: var(--a3s-muted);
  font-size: 10px;
}

.a3s-design-header .a3s-close svg {
  width: 16px;
  height: 16px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.5;
  stroke-linecap: round;
}

.a3s-design-body {
  display: flex;
  min-height: 0;
  padding: 12px 14px;
  overflow: hidden;
  flex: 1 1 auto;
  flex-direction: column;
}

.a3s-design-import {
  display: flex;
  padding-bottom: 10px;
  border-bottom: 1px solid var(--a3s-line);
  align-items: center;
  flex-wrap: wrap;
  gap: 7px;
}

.a3s-design-import small {
  color: var(--a3s-faint);
  flex: 1 1 260px;
  font-size: 10px;
}

.a3s-design-history {
  display: flex;
  gap: 7px;
}

.a3s-design-stage {
  display: flex;
  min-height: 320px;
  margin-top: 12px;
  overflow: hidden;
  border: 1px solid var(--a3s-line-strong);
  border-radius: 10px;
  background: var(--a3s-soft);
  flex: 1 1 420px;
  flex-direction: column;
}

.a3s-design-toolbar {
  display: flex;
  min-height: 48px;
  padding: 7px 8px;
  border-bottom: 1px solid var(--a3s-line);
  background: var(--a3s-panel-raised);
  align-items: center;
  flex-wrap: wrap;
  gap: 6px;
}

.a3s-design-toolbar button {
  min-height: 30px;
  padding: 0 8px;
  font-size: 11px;
}

.a3s-design-toolbar button.selected {
  border-color: var(--a3s-blue);
  background: var(--a3s-blue-soft);
  color: var(--a3s-blue-ink);
}

.a3s-design-toolbar label {
  display: flex;
  color: var(--a3s-muted);
  align-items: center;
  gap: 4px;
  font-size: 10px;
  font-weight: 650;
}

.a3s-design-toolbar input[type="color"] {
  width: 32px;
  height: 28px;
  padding: 2px;
  border: 1px solid var(--a3s-line-strong);
  border-radius: 6px;
  background: var(--a3s-bg);
}

.a3s-design-toolbar select {
  width: auto;
  min-width: 54px;
  min-height: 28px;
  padding: 4px 6px;
  border: 1px solid var(--a3s-line-strong);
  border-radius: 6px;
  background: var(--a3s-bg);
  color: var(--a3s-text);
}

.a3s-design-toolbar output {
  margin-left: auto;
  color: var(--a3s-faint);
  font: 10px/1.4 ui-monospace, SFMono-Regular, Menlo, monospace;
}

.a3s-design-canvas {
  display: grid;
  min-height: 0;
  padding: 10px;
  overflow: auto;
  background: #d9e0ea;
  flex: 1 1 auto;
  place-items: center;
}

.a3s-design-canvas-surface {
  display: block;
  width: 100%;
  max-width: 960px;
  max-height: 100%;
  overflow: visible;
  background: #ffffff;
  box-shadow: 0 2px 12px rgb(15 23 42 / 20%);
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

.a3s-design-element {
  cursor: inherit;
}

.a3s-design-canvas-surface[data-tool="select"] .a3s-design-element {
  cursor: move;
}

.a3s-design-selection rect {
  fill: none;
  stroke: #1264ff;
  stroke-width: 2;
  stroke-dasharray: 6 4;
  pointer-events: none;
}

.a3s-design-selection circle {
  fill: #ffffff;
  stroke: #1264ff;
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
  border: 2px solid #1264ff;
  border-radius: 5px;
  background: #ffffff;
  color: #111827;
  font: 24px/1.2 ui-sans-serif, system-ui, sans-serif;
}

.a3s-design-status {
  min-height: 20px;
  margin: 7px 0 0;
  color: var(--a3s-faint);
  flex: 0 0 auto;
  font-size: 10px;
}

.a3s-design-status.is-error {
  color: var(--a3s-danger);
}

.a3s-design-actions {
  display: flex;
  min-height: 54px;
  padding: 9px 14px;
  border-top: 1px solid var(--a3s-line);
  background: var(--a3s-bg);
  align-items: center;
  justify-content: flex-end;
  gap: 7px;
}

@media (max-width: 600px) {
  .a3s-design-scrim {
    padding: 8px;
  }

  .a3s-design-board {
    width: 100%;
    height: calc(100dvh - 16px);
    border-radius: 12px;
  }

  .a3s-design-body {
    padding: 10px;
  }

  .a3s-design-import > button {
    min-height: 40px;
    flex: 1 1 128px;
  }

  .a3s-design-history {
    width: 100%;
  }

  .a3s-design-history button {
    min-height: 40px;
    flex: 1 1 auto;
  }

  .a3s-design-stage {
    min-height: 280px;
  }

  .a3s-design-toolbar {
    max-height: 100px;
    overflow: auto;
  }

  .a3s-design-toolbar output {
    margin-left: 0;
  }

  .a3s-design-actions button {
    min-height: 44px;
  }
}
`;
