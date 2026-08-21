export const OVERLAY_MARKING_CSS = `
.a3s-wireframe {
  position: fixed;
  inset: 0;
  border: 16px solid rgb(12 24 48 / var(--a3s-wireframe-fade));
  background-color: rgb(12 24 48 / var(--a3s-wireframe-fade));
  background-image: linear-gradient(90deg, color-mix(in srgb, var(--a3s-marker-color) 13%, transparent) 1px, transparent 1px), linear-gradient(color-mix(in srgb, var(--a3s-marker-color) 13%, transparent) 1px, transparent 1px);
  background-size: 24px 24px;
  pointer-events: none;
}

.a3s-highlight,
.a3s-layout-target-preview {
  position: fixed;
  pointer-events: none;
}

.a3s-highlight {
  z-index: 3;
  border: 2px solid var(--a3s-marker-color);
  border-radius: 6px;
  background: color-mix(in srgb, var(--a3s-marker-color) 6%, transparent);
  box-shadow: 0 0 0 1px rgb(255 255 255 / 68%) inset, 0 8px 22px color-mix(in srgb, var(--a3s-marker-color) 12%, transparent);
}

.a3s-highlight.is-candidate {
  border-color: var(--a3s-violet);
  background: color-mix(in srgb, var(--a3s-violet) 10%, transparent);
}

.a3s-highlight > span,
.a3s-marker-index {
  display: grid;
  min-width: 22px;
  height: 22px;
  padding: 0 5px;
  border: 2px solid #ffffff;
  border-radius: 999px;
  background: var(--a3s-marker-color);
  color: #ffffff;
  font: 750 9px/1 ui-monospace, SFMono-Regular, Menlo, monospace;
  place-items: center;
  box-shadow: 0 3px 9px rgb(27 20 56 / 24%);
}

.a3s-highlight > span {
  position: absolute;
  top: -13px;
  left: -13px;
}

.a3s-layout-target-preview {
  border: 2px dashed var(--a3s-blue);
  background: color-mix(in srgb, var(--a3s-blue) 9%, transparent);
  box-shadow: 0 0 0 1px rgb(255 255 255 / 72%) inset;
}

.a3s-markers {
  position: fixed;
  inset: 0;
  pointer-events: none;
}

.a3s-marker {
  position: fixed;
  border: 2px solid color-mix(in srgb, var(--a3s-marker-color) 88%, transparent);
  border-radius: 6px;
  background: color-mix(in srgb, var(--a3s-marker-color) 5%, transparent);
  transition: background-color 150ms ease, border-color 150ms ease;
}

.a3s-marker:hover {
  border-color: var(--a3s-marker-color);
  background: color-mix(in srgb, var(--a3s-marker-color) 10%, transparent);
}

.a3s-marker > .a3s-marker-index {
  position: absolute;
  top: -13px;
  left: -13px;
}

.a3s-marker-action {
  position: absolute;
  top: -13px;
  left: -13px;
  display: grid;
  width: 24px;
  height: 24px;
  min-height: 24px;
  padding: 0;
  border: 0;
  border-radius: 999px;
  background: transparent;
  pointer-events: auto;
  place-items: center;
}

.a3s-marker-action::after {
  position: absolute;
  top: calc(100% + 8px);
  left: 12px;
  z-index: 12;
  width: max-content;
  max-width: min(260px, calc(100vw - 24px));
  padding: 7px 9px;
  border: 1px solid var(--a3s-toolbar-line);
  border-radius: 9px;
  background: var(--a3s-toolbar);
  box-shadow: 0 12px 30px rgb(5 10 20 / 26%);
  color: var(--a3s-toolbar-text);
  content: attr(data-tooltip);
  font-size: 11px;
  font-weight: 550;
  opacity: 0;
  overflow-wrap: anywhere;
  pointer-events: none;
  transform: translateY(-3px);
  transition: opacity 120ms ease, transform 120ms ease;
  white-space: normal;
}

.a3s-marker-action[data-tooltip-align="end"]::after {
  right: 12px;
  left: auto;
}

.a3s-marker-action[data-tooltip-side="top"]::after {
  top: auto;
  bottom: calc(100% + 8px);
  transform: translateY(3px);
}

.a3s-marker-action:hover::after,
.a3s-marker-action:focus-visible::after {
  opacity: 1;
  transform: translateY(0);
}

.a3s-marker-action:focus-visible {
  outline: 2px solid #ffffff;
  outline-offset: 1px;
  box-shadow: 0 0 0 4px var(--a3s-marker-color);
}

.a3s-marker-action .a3s-marker-index {
  position: static;
}

.a3s-marker.status-quality {
  border-style: dashed;
  border-color: #d89016;
  background: rgb(216 144 22 / 8%);
}

.a3s-marker.status-design-audit {
  border-style: dashed;
  border-color: var(--a3s-blue);
  background: color-mix(in srgb, var(--a3s-blue) 8%, transparent);
}

.a3s-marker.status-review_ready,
.a3s-marker.status-resolved {
  border-color: var(--a3s-green);
  background: color-mix(in srgb, var(--a3s-green) 8%, transparent);
}

.a3s-marker.status-failed,
.a3s-marker.status-verification_failed {
  border-color: var(--a3s-danger);
  background: color-mix(in srgb, var(--a3s-danger) 8%, transparent);
}

.a3s-drawing {
  position: fixed;
  inset: 0;
  width: 100vw;
  height: 100vh;
  overflow: visible;
  pointer-events: none;
}

.a3s-drawing path {
  fill: none;
  stroke: var(--a3s-marker-color);
  stroke-width: 3;
  stroke-linecap: round;
  stroke-linejoin: round;
  filter: drop-shadow(0 1px 1px #ffffff);
}

.a3s-editor {
  display: grid;
  height: 100%;
  min-height: 0;
  grid-template-rows: auto minmax(0, 1fr) auto;
}

.a3s-editor-header {
  display: grid;
  min-width: 0;
  min-height: 46px;
  padding: 8px 11px;
  border-bottom: 1px solid var(--a3s-line);
  background: var(--a3s-panel);
  grid-template-columns: 25px minmax(0, 1fr);
  align-items: center;
  gap: 9px;
}

.a3s-editor-index {
  display: grid;
  width: 24px;
  height: 24px;
  border-radius: 999px;
  background: var(--a3s-violet);
  color: #ffffff;
  place-items: center;
}

.a3s-editor-index svg {
  width: 14px;
  height: 14px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.65;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.a3s-editor-header > span:last-child {
  min-width: 0;
}

.a3s-editor-header strong,
.a3s-editor-target {
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.a3s-editor-header strong {
  color: var(--a3s-text);
  font-size: 13px;
  letter-spacing: -.01em;
}

.a3s-editor-target {
  margin-top: 2px;
  color: var(--a3s-muted);
  font: 650 9px/1.35 ui-monospace, SFMono-Regular, Menlo, monospace;
}

.a3s-editor-scroll {
  display: flex;
  min-height: 0;
  padding: 11px;
  overflow: auto;
  overscroll-behavior: contain;
  scrollbar-color: var(--a3s-line-strong) transparent;
  flex-direction: column;
  gap: 7px;
}

.a3s-editor textarea,
.a3s-reply-label textarea {
  min-height: 72px;
  resize: vertical;
  line-height: 1.45;
}

.a3s-editor-request textarea {
  min-height: 82px;
}

.a3s-editor-details {
  position: relative;
  display: grid;
  min-height: 34px;
  padding: 6px 28px 6px 9px;
  border-color: var(--a3s-line);
  background: var(--a3s-panel);
  grid-template-columns: auto minmax(0, 1fr);
  align-items: baseline;
  gap: 7px;
  text-align: left;
}

.a3s-editor-details > span {
  color: var(--a3s-text);
  font-size: 11px;
}

.a3s-editor-details > small {
  overflow: hidden;
  color: var(--a3s-faint);
  font-size: 9px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.a3s-editor-details > i {
  position: absolute;
  top: 12px;
  right: 11px;
  width: 7px;
  height: 7px;
  border-right: 1.5px solid currentColor;
  border-bottom: 1.5px solid currentColor;
  color: var(--a3s-muted);
  transform: rotate(45deg);
  transition: transform 180ms cubic-bezier(.16, 1, .3, 1), top 180ms ease;
}

.a3s-editor-details[aria-expanded="true"] > i {
  top: 15px;
  transform: rotate(225deg);
}

.a3s-editor-options {
  display: flex;
  flex-direction: column;
  gap: 7px;
  animation: a3s-editor-options-enter 180ms cubic-bezier(.16, 1, .3, 1) both;
}

@keyframes a3s-editor-options-enter {
  from {
    opacity: 0;
    transform: translateY(-4px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.a3s-fields,
.a3s-layout-fields {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
}

.a3s-actions {
  display: flex;
  justify-content: flex-end;
  flex-wrap: wrap;
  gap: 6px;
}

.a3s-editor > .a3s-actions {
  padding: 9px 11px 10px;
  border-top: 1px solid var(--a3s-line);
  background: var(--a3s-panel);
}

.a3s-editor > .a3s-actions button {
  min-height: 32px;
  padding: 0 9px;
  font-size: 11px;
}

.a3s-editor > .a3s-actions .a3s-send-now {
  box-shadow: 0 7px 16px color-mix(in srgb, var(--a3s-blue-strong) 22%, transparent);
}

.a3s-editor > .a3s-actions button.danger {
  margin-right: auto;
}

.a3s-editor > .a3s-actions .a3s-save-draft {
  border-color: var(--a3s-line);
  background: var(--a3s-panel);
  color: var(--a3s-muted);
}

.a3s-actions button:not(.quiet):not(.danger),
.a3s-quality-item button:first-child,
.a3s-human-actions button:first-of-type,
.a3s-item > div button:first-child {
  border-color: var(--a3s-blue-strong);
  background: var(--a3s-blue-strong);
  color: #ffffff;
}

.a3s-conflicts {
  display: flex;
  margin: 0;
  padding: 9px;
  border: 1px solid var(--a3s-line);
  border-radius: 9px;
  flex-direction: column;
  gap: 7px;
}

.a3s-conflicts legend {
  padding: 0 4px;
  color: var(--a3s-text);
  font-weight: 650;
}

.a3s-conflicts label {
  display: flex;
  flex-direction: row;
  align-items: flex-start;
  gap: 7px;
  font-weight: 450;
}

.a3s-conflicts input,
.a3s-item input[type="checkbox"] {
  margin-top: 3px;
  accent-color: var(--a3s-blue);
}

`;
