import { OVERLAY_SHELL_CSS } from "./overlay-shell-style";

export const OVERLAY_CSS = `
${OVERLAY_SHELL_CSS}

.a3s-hint {
  position: absolute;
  right: 0;
  bottom: calc(100% + 10px);
  width: min(410px, calc(100vw - 24px));
  margin: 0;
  padding: 9px 11px;
  border: 1px solid color-mix(in srgb, var(--a3s-violet) 34%, var(--a3s-line));
  border-radius: 9px;
  background: var(--a3s-violet-soft);
  box-shadow: 0 12px 26px rgb(31 38 66 / 14%);
  color: var(--a3s-text);
  pointer-events: auto;
  font-size: 11px;
}

.a3s-root[data-dock="left"] .a3s-hint {
  right: auto;
  left: 0;
}

.a3s-workspace {
  position: absolute;
  right: 0;
  bottom: calc(100% + 10px);
  display: grid;
  width: min(420px, calc(100vw - 24px));
  max-height: min(680px, calc(100vh - 92px));
  overflow: hidden;
  border: 1px solid var(--a3s-line);
  border-radius: 14px;
  background: var(--a3s-panel);
  box-shadow: var(--a3s-shadow);
  pointer-events: auto;
  grid-template-rows: 48px minmax(0, 1fr) auto;
}

.a3s-workspace[hidden] {
  display: none;
}

.a3s-root[data-dock="left"] .a3s-workspace {
  right: auto;
  left: 0;
}

.a3s-workspace-header {
  display: flex;
  min-width: 0;
  padding: 0 10px 0 14px;
  border-bottom: 1px solid var(--a3s-line);
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.a3s-workspace-header > span {
  min-width: 0;
}

.a3s-workspace-header strong,
.a3s-workspace-header small {
  display: block;
}

.a3s-workspace-header strong {
  color: var(--a3s-text);
  font-size: 13px;
}

.a3s-workspace-header small {
  margin-top: 1px;
  color: var(--a3s-faint);
  font-size: 9px;
}

.a3s-workspace-scroll {
  min-height: 0;
  overflow: auto;
  scrollbar-color: var(--a3s-line-strong) transparent;
}

.a3s-workspace > footer {
  display: flex;
  padding: 9px 10px;
  border-top: 1px solid var(--a3s-line);
  background: var(--a3s-bg);
  overflow-x: auto;
  align-items: center;
  justify-content: flex-end;
  gap: 5px;
  scrollbar-width: none;
}

.a3s-workspace > footer button {
  min-height: 30px;
  flex: 0 0 auto;
  padding: 0 8px;
  font-size: 10px;
}

.a3s-workspace > footer button:not(.quiet) {
  border-color: var(--a3s-blue);
  background: var(--a3s-blue);
  color: #ffffff;
}

.a3s-settings {
  position: relative;
  flex: 0 0 auto;
}

.a3s-settings-content {
  position: absolute;
  right: 0;
  bottom: calc(100% + 12px);
  z-index: 14;
  display: flex;
  width: min(380px, calc(100vw - 24px));
  max-height: min(610px, calc(100vh - 92px));
  padding: 14px;
  overflow: auto;
  border: 1px solid var(--a3s-line);
  border-radius: 14px;
  background: var(--a3s-panel);
  box-shadow: var(--a3s-shadow);
  flex-direction: column;
  gap: 11px;
}

.a3s-root[data-dock="left"] .a3s-settings-content {
  right: auto;
  left: 0;
}

.a3s-settings-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px;
}

.a3s-settings label,
.a3s-layout label,
.a3s-editor label {
  display: flex;
  flex-direction: column;
  gap: 5px;
  color: var(--a3s-text);
  font-size: 11px;
  font-weight: 650;
}

.a3s-settings input,
.a3s-settings select,
.a3s-layout input,
.a3s-layout select,
.a3s-catalog input,
.a3s-editor textarea,
.a3s-editor select,
.a3s-reply-label textarea {
  width: 100%;
  min-height: 34px;
  padding: 7px 8px;
  border: 1px solid var(--a3s-line-strong);
  border-radius: 8px;
  background: var(--a3s-bg);
  color: var(--a3s-text);
}

.a3s-settings input[type="color"] {
  padding: 3px;
}

.a3s-settings output,
.a3s-layout label span,
.a3s-editor label span,
.a3s-conflicts legend span {
  color: var(--a3s-faint);
  font-weight: 450;
}

.a3s-settings .a3s-setting-toggle {
  flex-direction: row;
  align-items: flex-start;
  font-weight: 450;
}

.a3s-settings .a3s-setting-toggle input {
  width: auto;
  min-height: 0;
  margin-top: 3px;
  accent-color: var(--a3s-blue);
}

.a3s-shortcuts {
  padding-top: 10px;
  border-top: 1px solid var(--a3s-line);
}

.a3s-shortcuts h3,
.a3s-shortcuts p,
.a3s-shortcuts dl,
.a3s-shortcuts dd {
  margin: 0;
}

.a3s-shortcuts h3 {
  font-size: 11px;
}

.a3s-shortcuts dl {
  display: grid;
  margin-top: 7px;
  gap: 4px;
}

.a3s-shortcuts dl > div {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: baseline;
  gap: 8px;
}

.a3s-shortcuts dt {
  color: var(--a3s-muted);
  overflow-wrap: anywhere;
}

.a3s-shortcuts kbd {
  display: inline-block;
  padding: 1px 5px;
  border: 1px solid var(--a3s-line-strong);
  border-radius: 5px;
  background: var(--a3s-bg);
  color: var(--a3s-text);
  font: 10px/1.5 ui-monospace, SFMono-Regular, Menlo, monospace;
  white-space: nowrap;
}

.a3s-shortcuts p {
  margin-top: 8px;
  color: var(--a3s-faint);
  font-size: 10px;
}

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
  border-radius: 5px;
  background: color-mix(in srgb, var(--a3s-marker-color) 10%, transparent);
  box-shadow: 0 0 0 1px rgb(255 255 255 / 72%) inset, 0 7px 18px color-mix(in srgb, var(--a3s-marker-color) 14%, transparent);
}

.a3s-highlight.is-candidate {
  border-color: var(--a3s-violet);
  background: color-mix(in srgb, var(--a3s-violet) 10%, transparent);
}

.a3s-highlight > span,
.a3s-marker-index {
  display: grid;
  min-width: 24px;
  height: 24px;
  padding: 0 5px;
  border: 2px solid #ffffff;
  border-radius: 999px;
  background: var(--a3s-marker-color);
  color: #ffffff;
  font: 750 9px/1 ui-monospace, SFMono-Regular, Menlo, monospace;
  place-items: center;
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
  border: 2px solid var(--a3s-marker-color);
  border-radius: 5px;
  background: color-mix(in srgb, var(--a3s-marker-color) 7%, transparent);
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
  width: 26px;
  height: 26px;
  min-height: 26px;
  padding: 0;
  border: 0;
  border-radius: 999px;
  background: transparent;
  pointer-events: auto;
  place-items: center;
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
  width: min(376px, calc(100vw - 24px));
  max-height: min(720px, calc(100vh - var(--a3s-editor-top, 80px) - 12px));
  overflow: hidden;
  border: 1px solid var(--a3s-line);
  border-radius: 14px;
  background: var(--a3s-panel);
  box-shadow: var(--a3s-shadow);
  pointer-events: auto;
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

.a3s-editor-target {
  display: block;
  padding: 11px 14px;
  overflow: hidden;
  border-bottom: 1px solid var(--a3s-line);
  background: var(--a3s-violet-soft);
  color: var(--a3s-violet);
  font: 700 10px/1.35 ui-monospace, SFMono-Regular, Menlo, monospace;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.a3s-editor-scroll {
  display: flex;
  min-height: 0;
  padding: 14px;
  overflow: auto;
  overscroll-behavior: contain;
  scrollbar-color: var(--a3s-line-strong) transparent;
  flex-direction: column;
  gap: 10px;
}

.a3s-editor textarea,
.a3s-reply-label textarea {
  min-height: 64px;
  resize: vertical;
  line-height: 1.45;
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
  padding: 10px 14px;
  border-top: 1px solid var(--a3s-line);
  background: var(--a3s-bg);
}

.a3s-actions button:not(.quiet):not(.danger),
.a3s-quality-item button:first-child,
.a3s-human-actions button:first-of-type,
.a3s-item > div button:first-child {
  border-color: var(--a3s-blue);
  background: var(--a3s-blue);
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

.a3s-layout {
  display: flex;
  padding: 12px;
  border-bottom: 1px solid var(--a3s-line);
  background: var(--a3s-soft);
  flex-direction: column;
  gap: 9px;
}

.a3s-layout > p {
  margin: 0;
  color: var(--a3s-muted);
  font-size: 11px;
}

.a3s-layout-source {
  display: flex;
  padding-top: 8px;
  border-top: 1px solid var(--a3s-line);
  flex-direction: column;
  gap: 6px;
}

.a3s-layout-source > span {
  color: var(--a3s-faint);
}

.a3s-layout-source strong {
  overflow-wrap: anywhere;
}

.a3s-catalog {
  padding: 8px;
  border: 1px solid var(--a3s-line);
  border-radius: 9px;
  background: var(--a3s-panel);
}

.a3s-catalog .a3s-disclosure {
  position: relative;
  width: 100%;
  padding: 0 24px 0 2px;
  border: 0;
  background: transparent;
  color: var(--a3s-text);
  text-align: left;
}

.a3s-catalog .a3s-disclosure::after {
  position: absolute;
  top: 10px;
  right: 5px;
  width: 7px;
  height: 7px;
  border-right: 1.5px solid currentColor;
  border-bottom: 1.5px solid currentColor;
  content: "";
  transform: rotate(45deg);
}

.a3s-catalog .a3s-disclosure[aria-expanded="true"]::after {
  top: 13px;
  transform: rotate(225deg);
}

.a3s-catalog-content {
  display: flex;
  padding-top: 8px;
  flex-direction: column;
  gap: 8px;
}

.a3s-catalog-results {
  display: flex;
  max-height: 210px;
  overflow: auto;
  flex-direction: column;
  gap: 9px;
}

.a3s-catalog-results section {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.a3s-catalog-results section > div {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
}

.a3s-catalog-results button {
  min-height: 28px;
  padding: 0 7px;
  font-size: 10px;
}

.a3s-catalog-results button.selected {
  border-color: var(--a3s-blue);
  background: var(--a3s-blue-soft);
  color: var(--a3s-blue);
}

.a3s-catalog-empty {
  margin: 0;
  color: var(--a3s-faint);
}

.a3s-quality {
  padding: 10px 12px 0;
  border-bottom: 1px solid var(--a3s-line);
}

.a3s-section-heading {
  display: flex;
  padding-bottom: 8px;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
}

.a3s-section-heading small {
  color: var(--a3s-faint);
}

.a3s-quality-item {
  display: grid;
  padding: 10px 0;
  border-top: 1px solid var(--a3s-line);
  grid-template-columns: auto minmax(0, 1fr);
  gap: 3px 8px;
}

.a3s-quality-item > strong,
.a3s-quality-item > p,
.a3s-quality-item > small,
.a3s-quality-item > div {
  min-width: 0;
  grid-column: 2;
  overflow-wrap: anywhere;
}

.a3s-quality-item > p {
  margin: 2px 0;
  color: var(--a3s-muted);
  font-size: 11px;
}

.a3s-quality-item > .a3s-status {
  grid-column: 1;
  grid-row: 1 / span 2;
}

.a3s-quality-item > div,
.a3s-item > div,
.a3s-human-actions {
  display: flex;
  padding-top: 4px;
  flex-wrap: wrap;
  gap: 5px;
}

.a3s-quality-item button,
.a3s-item button,
.a3s-human-actions button {
  min-height: 28px;
  padding: 0 7px;
  font-size: 10px;
}

.a3s-list {
  display: flex;
  min-height: 88px;
  padding: 10px 12px 12px;
  flex-direction: column;
  gap: 8px;
}

.a3s-item {
  display: flex;
  padding: 10px;
  border: 1px solid var(--a3s-line);
  border-radius: 10px;
  background: var(--a3s-soft);
  flex-direction: column;
  gap: 7px;
}

.a3s-item.is-hidden {
  opacity: .6;
}

.a3s-item label {
  display: flex;
  align-items: flex-start;
  gap: 8px;
}

.a3s-item label > span,
.a3s-item.submitted {
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.a3s-item strong {
  color: var(--a3s-text);
  overflow-wrap: anywhere;
}

.a3s-item small {
  color: var(--a3s-faint);
  overflow-wrap: anywhere;
}

.a3s-status {
  align-self: flex-start;
  padding: 2px 7px;
  border-radius: 999px;
  background: var(--a3s-violet-soft);
  color: var(--a3s-violet);
  font-size: 10px;
  text-transform: capitalize;
}

.status-blocking,
.status-failed,
.status-verification_failed {
  background: var(--a3s-danger-soft);
  color: var(--a3s-danger);
}

.status-important {
  background: var(--a3s-warning-soft);
  color: var(--a3s-warning);
}

.status-suggestion {
  background: var(--a3s-blue-soft);
  color: var(--a3s-blue);
}

.status-resolved,
.status-review_ready {
  background: var(--a3s-green-soft);
  color: var(--a3s-green);
}

.a3s-empty {
  margin: 0;
  padding: 18px 10px;
  color: var(--a3s-faint);
  text-align: center;
}

.a3s-thread {
  display: flex;
  margin: 5px 0 0;
  padding: 8px;
  border-top: 1px solid var(--a3s-line);
  list-style: none;
  flex-direction: column;
  gap: 6px;
}

.a3s-thread li {
  display: grid;
  grid-template-columns: auto 1fr;
  align-items: start;
  gap: 8px;
}

.a3s-thread span {
  color: var(--a3s-violet);
  font-size: 10px;
  text-transform: capitalize;
}

.a3s-thread p {
  margin: 0;
  overflow-wrap: anywhere;
  white-space: pre-wrap;
}

.a3s-reply-label {
  display: flex;
  flex: 1 1 100%;
  flex-direction: column;
  gap: 4px;
  font-weight: 650;
}

@media (max-width: 720px) {
  .a3s-launch,
  .a3s-root[data-dock="left"] .a3s-launch {
    right: 12px;
    bottom: 12px;
    left: auto;
  }

  .a3s-panel,
  .a3s-root[data-dock="left"] .a3s-panel {
    right: 8px;
    bottom: 66px;
    left: 8px;
    width: auto;
  }

  .a3s-command-bar {
    height: auto;
    min-height: 50px;
    align-items: stretch;
  }

  .a3s-command-bar > header {
    min-width: 70px;
    height: auto;
    align-self: stretch;
    grid-template-columns: 27px 28px;
  }

  .a3s-tools {
    overflow: visible;
    flex-wrap: wrap;
    align-content: center;
    row-gap: 3px;
  }

  .a3s-command-bar > header > span:nth-child(2) {
    position: fixed;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
  }

  .a3s-workspace,
  .a3s-root[data-dock="left"] .a3s-workspace {
    right: 0;
    bottom: calc(100% + 8px);
    left: 0;
    width: auto;
    max-height: calc(100vh - 136px);
  }

  .a3s-settings-content,
  .a3s-root[data-dock="left"] .a3s-settings-content {
    position: fixed;
    right: 8px;
    bottom: 66px;
    left: 8px;
    width: auto;
    max-height: calc(100vh - 82px);
  }

  .a3s-editor-popover {
    top: auto;
    right: 8px;
    bottom: 66px;
    left: 8px;
    width: auto;
    max-height: calc(100vh - 82px);
  }

  .a3s-editor-popover::before {
    display: none;
  }

  .a3s-hint,
  .a3s-root[data-dock="left"] .a3s-hint {
    right: 0;
    bottom: calc(100% + 8px);
    left: 0;
    width: auto;
  }
}

@media (max-width: 420px) {
  .a3s-settings-grid,
  .a3s-fields,
  .a3s-layout-fields {
    grid-template-columns: 1fr;
  }

  .a3s-command-bar > header {
    min-width: 61px;
    padding-left: 1px;
    gap: 3px;
  }
}

@media (hover: none) {
  .a3s-tools button[data-tooltip]::after,
  .a3s-settings > .a3s-disclosure[data-tooltip]::after {
    display: none;
  }
}

@media (prefers-reduced-motion: reduce) {
  .a3s-root *,
  .a3s-root *::before,
  .a3s-root *::after {
    scroll-behavior: auto !important;
    transition-duration: .01ms !important;
  }
}
`;
