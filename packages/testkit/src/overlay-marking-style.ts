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
  border: 1.5px solid var(--a3s-marker-color);
  border-radius: 7px;
  background: color-mix(in srgb, var(--a3s-marker-color) 8%, transparent);
  box-shadow: 0 0 0 1px rgb(255 255 255 / 62%) inset, 0 8px 22px color-mix(in srgb, var(--a3s-marker-color) 13%, transparent);
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
  border: 1.5px solid color-mix(in srgb, var(--a3s-marker-color) 84%, transparent);
  border-radius: 7px;
  background: color-mix(in srgb, var(--a3s-marker-color) 6%, transparent);
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
  left: 0;
  z-index: 12;
  max-width: min(260px, calc(100vw - 24px));
  padding: 7px 9px;
  overflow: hidden;
  border: 1px solid var(--a3s-toolbar-line);
  border-radius: 9px;
  background: var(--a3s-toolbar);
  box-shadow: 0 12px 30px rgb(5 10 20 / 26%);
  color: var(--a3s-toolbar-text);
  content: attr(data-tooltip);
  font-size: 11px;
  font-weight: 550;
  opacity: 0;
  pointer-events: none;
  text-overflow: ellipsis;
  transform: translateY(-3px);
  transition: opacity 120ms ease, transform 120ms ease;
  white-space: nowrap;
}

.a3s-marker-action:hover::after,
.a3s-marker-action:focus-visible::after {
  opacity: 1;
  transform: translateY(0);
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

.a3s-editor-popover {
  position: fixed;
  z-index: 8;
  top: var(--a3s-editor-top, 80px);
  left: var(--a3s-editor-left, 80px);
  width: min(326px, calc(100vw - 24px));
  max-height: min(620px, calc(100vh - var(--a3s-editor-top, 80px) - 12px));
  overflow: hidden;
  border: 1px solid var(--a3s-line);
  border-radius: 16px;
  background: var(--a3s-panel);
  box-shadow: var(--a3s-shadow);
  pointer-events: auto;
  transform-origin: var(--a3s-editor-origin, left top);
  animation: a3s-editor-enter 220ms cubic-bezier(.16, 1, .3, 1) both;
}

@keyframes a3s-editor-enter {
  from {
    opacity: 0;
    transform: translateY(5px) scale(.97);
  }
  to {
    opacity: 1;
    transform: translateY(0) scale(1);
  }
}

.a3s-editor-popover::before {
  position: absolute;
  top: 22px;
  left: -6px;
  width: 10px;
  height: 10px;
  border-top: 1px solid var(--a3s-line);
  border-left: 1px solid var(--a3s-line);
  background: var(--a3s-panel);
  content: "";
  transform: rotate(-45deg);
}

.a3s-editor-popover[data-side="left"]::before {
  left: auto;
  right: -6px;
  transform: rotate(135deg);
}

.a3s-editor {
  display: grid;
  max-height: inherit;
  grid-template-rows: auto minmax(0, 1fr) auto;
}

.a3s-editor-header {
  display: grid;
  min-width: 0;
  min-height: 50px;
  padding: 9px 12px;
  border-bottom: 1px solid var(--a3s-line);
  background: var(--a3s-violet-soft);
  grid-template-columns: 25px minmax(0, 1fr);
  align-items: center;
  gap: 9px;
}

.a3s-editor-index {
  display: grid;
  width: 25px;
  height: 25px;
  border-radius: 999px;
  background: var(--a3s-violet);
  color: #ffffff;
  font: 750 9px/1 ui-monospace, SFMono-Regular, Menlo, monospace;
  place-items: center;
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
  font-size: 12px;
}

.a3s-editor-target {
  margin-top: 2px;
  color: var(--a3s-violet);
  font: 650 9px/1.35 ui-monospace, SFMono-Regular, Menlo, monospace;
}

.a3s-editor-scroll {
  display: flex;
  min-height: 0;
  padding: 12px;
  overflow: auto;
  overscroll-behavior: contain;
  scrollbar-color: var(--a3s-line-strong) transparent;
  flex-direction: column;
  gap: 9px;
}

.a3s-editor textarea,
.a3s-reply-label textarea {
  min-height: 64px;
  resize: vertical;
  line-height: 1.45;
}

.a3s-editor-request textarea {
  min-height: 84px;
}

.a3s-editor-details {
  position: relative;
  display: grid;
  min-height: 36px;
  padding: 6px 30px 6px 9px;
  border-color: var(--a3s-line);
  background: var(--a3s-bg);
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
  gap: 9px;
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
  padding: 9px 12px;
  border-top: 1px solid var(--a3s-line);
  background: var(--a3s-bg);
}

.a3s-editor > .a3s-actions button {
  min-height: 30px;
  padding: 0 8px;
  font-size: 10px;
}

.a3s-editor > .a3s-actions button.danger {
  margin-right: auto;
}

.a3s-editor > .a3s-actions .a3s-save-draft {
  border-color: var(--a3s-line-strong);
  background: var(--a3s-panel-raised);
  color: var(--a3s-text);
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
