import { OVERLAY_SHELL_CSS } from "./overlay-shell-style";
import { OVERLAY_MARKING_CSS } from "./overlay-marking-style";
import { DESIGN_BOARD_CSS } from "./design-board-style";

export const OVERLAY_CSS = `
${OVERLAY_SHELL_CSS}

.a3s-hint {
  margin: 0;
  padding: 10px 14px;
  border-bottom: 1px solid var(--a3s-line);
  background: var(--a3s-blue-soft);
  color: var(--a3s-blue-ink);
  font-size: 11px;
  animation: a3s-hint-enter 180ms cubic-bezier(.16, 1, .3, 1) both;
}

@keyframes a3s-hint-enter {
  from {
    opacity: 0;
    transform: translateY(-4px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.a3s-workspace {
  display: grid;
  height: 100%;
  min-height: 0;
  overflow: hidden;
  background: var(--a3s-panel);
  grid-template-rows: 44px minmax(0, 1fr) auto;
}

.a3s-workspace-header {
  display: flex;
  min-width: 0;
  min-height: 44px;
  padding: 0 9px 0 12px;
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
  font-size: 9.5px;
}

.a3s-workspace-scroll {
  min-height: 0;
  overflow: auto;
  scrollbar-color: var(--a3s-line-strong) transparent;
}

.a3s-workspace > footer {
  display: grid;
  padding: 8px 10px 10px;
  border-top: 1px solid var(--a3s-line);
  background: var(--a3s-bg);
  grid-template-columns: 1fr;
  gap: 6px;
}

.a3s-workspace > footer > div {
  display: flex;
  align-items: center;
  gap: 6px;
}

.a3s-workspace-send-actions {
  width: 100%;
  justify-content: flex-end;
}

.a3s-workspace > footer button {
  min-height: 32px;
  flex: 0 0 auto;
  padding: 0 9px;
  font-size: 11px;
}

.a3s-workspace > footer button:not(.quiet) {
  border-color: var(--a3s-blue-strong);
  background: var(--a3s-blue-strong);
  color: #ffffff;
}

.a3s-settings-content {
  display: flex;
  height: 100%;
  min-height: 0;
  padding: 14px;
  overflow: auto;
  overscroll-behavior: contain;
  background: var(--a3s-panel);
  color: var(--a3s-text);
  flex-direction: column;
  gap: 12px;
}

.a3s-settings-content > button.quiet {
  display: inline-flex;
  width: fit-content;
  height: auto;
  min-height: 34px;
  padding: 0 10px;
  border-color: var(--a3s-line-strong);
  border-radius: 8px;
  background: transparent;
  color: var(--a3s-muted);
  align-items: center;
  place-items: initial;
}

.a3s-settings-content > button.quiet:hover {
  border-color: var(--a3s-blue);
  background: var(--a3s-soft);
  color: var(--a3s-text);
}

.a3s-settings-heading strong,
.a3s-settings-heading small {
  display: block;
}

.a3s-settings-heading strong {
  font-size: 13px;
}

.a3s-settings-heading small {
  margin-top: 2px;
  color: var(--a3s-muted);
  font-size: 10px;
}

.a3s-settings-actions {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 6px;
}

.a3s-settings-actions button {
  display: flex;
  min-width: 0;
  min-height: 42px;
  padding: 5px 7px;
  color: var(--a3s-muted);
  align-items: center;
  justify-content: center;
  gap: 5px;
  font-size: 10px;
}

.a3s-settings-actions button[aria-pressed="true"] {
  border-color: var(--a3s-blue);
  background: var(--a3s-blue-soft);
  color: var(--a3s-blue-ink);
}

.a3s-settings-actions svg {
  width: 15px;
  height: 15px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.55;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.a3s-settings-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px 8px;
}

.a3s-settings-content label,
.a3s-layout label,
.a3s-editor label {
  display: flex;
  flex-direction: column;
  gap: 4px;
  color: var(--a3s-text);
  font-size: 11px;
  font-weight: 650;
}

.a3s-settings-content input,
.a3s-settings-content select,
.a3s-layout input,
.a3s-layout select,
.a3s-catalog input,
.a3s-editor textarea,
.a3s-editor select,
.a3s-reply-label textarea {
  width: 100%;
  min-height: 32px;
  padding: 6px 8px;
  border: 1px solid var(--a3s-line-strong);
  border-radius: 8px;
  background: var(--a3s-bg);
  color: var(--a3s-text);
  font-size: 13px;
}

.a3s-editor textarea::placeholder,
.a3s-reply-label textarea::placeholder,
.a3s-settings-content input::placeholder,
.a3s-layout input::placeholder,
.a3s-catalog input::placeholder {
  color: var(--a3s-faint);
  font-weight: 400;
  opacity: 1;
}

:is(
  .a3s-settings-content input:not([type="checkbox"]):not([type="color"]):not([type="range"]),
  .a3s-settings-content select,
  .a3s-layout input,
  .a3s-layout select,
  .a3s-catalog input,
  .a3s-editor textarea,
  .a3s-editor select,
  .a3s-reply-label textarea
):hover:not(:disabled) {
  border-color: color-mix(in srgb, var(--a3s-blue) 48%, var(--a3s-line));
  background: var(--a3s-soft);
}

:is(
  .a3s-settings-content input:not([type="checkbox"]):not([type="color"]):not([type="range"]),
  .a3s-settings-content select,
  .a3s-layout input,
  .a3s-layout select,
  .a3s-catalog input,
  .a3s-editor textarea,
  .a3s-editor select,
  .a3s-reply-label textarea
):focus-visible {
  border-color: var(--a3s-blue);
  outline: 0;
  box-shadow: var(--a3s-shadow-focus);
}

.a3s-editor-request textarea:focus-visible {
  border-color: var(--a3s-marker-color);
  outline: 0;
  box-shadow: 0 0 0 2px var(--a3s-panel), 0 0 0 4px color-mix(in srgb, var(--a3s-marker-color) 68%, transparent);
}

.a3s-setting-field {
  min-width: 0;
}

.a3s-setting-range {
  grid-column: 1 / -1;
}

.a3s-setting-label,
.a3s-color-control {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.a3s-color-control {
  min-height: 32px;
  padding: 2px 7px 2px 3px;
  border: 1px solid var(--a3s-line-strong);
  border-radius: 8px;
  background: var(--a3s-bg);
  transition: background-color var(--a3s-motion-normal) ease, border-color var(--a3s-motion-normal) ease, box-shadow var(--a3s-motion-normal) ease;
}

.a3s-color-control:hover {
  border-color: color-mix(in srgb, var(--a3s-blue) 48%, var(--a3s-line));
  background: var(--a3s-soft);
}

.a3s-color-control:focus-within {
  border-color: var(--a3s-blue);
  box-shadow: var(--a3s-shadow-focus);
}

.a3s-settings-content .a3s-color-control input[type="color"] {
  width: 30px;
  height: 26px;
  min-height: 0;
  padding: 1px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  cursor: pointer;
}

.a3s-color-control input[type="color"]:focus-visible {
  outline: 0;
}

.a3s-color-control output {
  overflow: hidden;
  color: var(--a3s-muted);
  font: 600 9.5px/1 ui-monospace, SFMono-Regular, Menlo, monospace;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.a3s-settings-content input[type="range"] {
  width: 100%;
  height: 24px;
  min-height: 24px;
  padding: 0;
  appearance: none;
  border: 0;
  background: transparent;
  cursor: pointer;
}

.a3s-settings-content input[type="range"]::-webkit-slider-runnable-track {
  height: 3px;
  border-radius: 999px;
  background: linear-gradient(to right, var(--a3s-blue) var(--a3s-range-value), var(--a3s-line-strong) var(--a3s-range-value));
}

.a3s-settings-content input[type="range"]::-moz-range-track {
  height: 3px;
  border-radius: 999px;
  background: linear-gradient(to right, var(--a3s-blue) var(--a3s-range-value), var(--a3s-line-strong) var(--a3s-range-value));
}

.a3s-settings-content input[type="range"]::-webkit-slider-thumb {
  width: 14px;
  height: 14px;
  margin-top: -5.5px;
  appearance: none;
  border: 2px solid var(--a3s-blue);
  border-radius: 999px;
  background: #ffffff;
  box-shadow: 0 1px 3px rgb(20 24 40 / 20%);
}

.a3s-settings-content input[type="range"]::-moz-range-thumb {
  width: 10px;
  height: 10px;
  border: 2px solid var(--a3s-blue);
  border-radius: 999px;
  background: #ffffff;
  box-shadow: 0 1px 3px rgb(20 24 40 / 20%);
}

.a3s-settings-content input[type="range"]:focus-visible {
  outline: 0;
}

.a3s-settings-content input[type="range"]:focus-visible::-webkit-slider-thumb {
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--a3s-blue) 20%, transparent), 0 1px 3px rgb(20 24 40 / 20%);
}

.a3s-settings-content input[type="range"]:focus-visible::-moz-range-thumb {
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--a3s-blue) 20%, transparent), 0 1px 3px rgb(20 24 40 / 20%);
}

.a3s-settings-content output,
.a3s-layout label span,
.a3s-editor label span,
.a3s-conflicts legend span {
  color: var(--a3s-faint);
  font-weight: 450;
}

.a3s-setting-toggles {
  display: flex;
  border-top: 1px solid var(--a3s-line);
  border-bottom: 1px solid var(--a3s-line);
  flex-direction: column;
}

.a3s-settings-content .a3s-setting-toggle {
  min-height: 52px;
  padding: 8px 2px;
  flex-direction: row;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  font-weight: 450;
}

.a3s-settings-content .a3s-setting-toggle + .a3s-setting-toggle {
  border-top: 1px solid var(--a3s-line);
}

.a3s-setting-toggle > span {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 1px;
}

.a3s-setting-toggle strong {
  color: var(--a3s-text);
  font-size: 11px;
  font-weight: 650;
}

.a3s-setting-toggle small {
  color: var(--a3s-faint);
  font-size: 9.5px;
  font-weight: 450;
  line-height: 1.35;
}

.a3s-settings-content .a3s-setting-switch {
  position: relative;
  width: 32px;
  height: 20px;
  min-height: 20px;
  padding: 0;
  appearance: none;
  border: 1px solid var(--a3s-line-strong);
  border-radius: 999px;
  background: var(--a3s-panel-strong);
  cursor: pointer;
  flex: 0 0 auto;
  transition: background-color var(--a3s-motion-normal) ease, border-color var(--a3s-motion-normal) ease, box-shadow var(--a3s-motion-normal) ease;
}

.a3s-settings-content .a3s-setting-switch::before {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 14px;
  height: 14px;
  border-radius: 999px;
  background: #ffffff;
  box-shadow: 0 1px 3px rgb(20 24 40 / 18%);
  content: "";
  transition: transform var(--a3s-motion-normal) ease;
}

.a3s-settings-content .a3s-setting-switch:checked {
  border-color: var(--a3s-blue);
  background: var(--a3s-blue);
}

.a3s-settings-content .a3s-setting-switch:checked::before {
  transform: translateX(12px);
}

.a3s-settings-content .a3s-setting-switch:focus-visible {
  outline: 0;
  box-shadow: var(--a3s-shadow-focus);
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

${OVERLAY_MARKING_CSS}
.a3s-layout {
  display: flex;
  padding: 10px;
  border-bottom: 1px solid var(--a3s-line);
  background: var(--a3s-soft);
  flex-direction: column;
  gap: 7px;
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
  padding: 6px;
  border: 1px solid var(--a3s-line);
  border-radius: 9px;
  background: var(--a3s-panel);
}

.a3s-catalog .a3s-disclosure {
  display: grid;
  width: 100%;
  min-height: 34px;
  padding: 0 7px 0 5px;
  border: 0;
  background: transparent;
  color: var(--a3s-text);
  grid-template-columns: 24px minmax(0, 1fr) 14px;
  align-items: center;
  gap: 7px;
  text-align: left;
}

.a3s-catalog-icon {
  display: grid;
  width: 24px;
  height: 24px;
  border-radius: 6px;
  background: var(--a3s-blue-soft);
  color: var(--a3s-blue-ink);
  place-items: center;
}

.a3s-catalog-icon svg,
.a3s-catalog-search-control > svg,
.a3s-catalog-empty > svg {
  width: 14px;
  height: 14px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.6;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.a3s-catalog .a3s-disclosure > span:nth-child(2) {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.a3s-catalog .a3s-disclosure > i {
  width: 7px;
  height: 7px;
  border-right: 1.5px solid currentColor;
  border-bottom: 1.5px solid currentColor;
  transform: rotate(45deg);
  transition: transform var(--a3s-motion-normal) ease;
}

.a3s-catalog .a3s-disclosure[aria-expanded="true"] > i {
  transform: rotate(225deg);
}

.a3s-catalog-content {
  display: flex;
  margin-top: 5px;
  padding: 9px 3px 3px;
  border-top: 1px solid var(--a3s-line);
  flex-direction: column;
  gap: 6px;
  animation: a3s-editor-options-enter 180ms cubic-bezier(.16, 1, .3, 1) both;
}

.a3s-catalog-search {
  display: flex;
  flex-direction: column;
  gap: 4px;
  color: var(--a3s-text);
  font-size: 10.5px;
  font-weight: 650;
}

.a3s-catalog-search-control {
  position: relative;
  display: block;
}

.a3s-catalog-search-control > svg {
  position: absolute;
  z-index: 1;
  top: 50%;
  left: 9px;
  color: var(--a3s-muted);
  pointer-events: none;
  transform: translateY(-50%);
}

.a3s-catalog .a3s-catalog-search-control > input {
  padding-left: 30px;
}

.a3s-catalog-count {
  color: var(--a3s-faint);
  font-size: 9.5px;
}

.a3s-catalog-results {
  display: flex;
  max-height: 190px;
  padding-right: 2px;
  overflow: auto;
  overscroll-behavior: contain;
  scrollbar-color: var(--a3s-line-strong) transparent;
  flex-direction: column;
  gap: 10px;
}

.a3s-catalog-results section {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.a3s-catalog-results section > div {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 5px;
}

.a3s-catalog-results button {
  display: block;
  min-width: 0;
  min-height: 30px;
  padding: 0 8px;
  overflow: hidden;
  border-color: transparent;
  background: var(--a3s-soft);
  font-size: 10px;
  font-weight: 550;
  text-align: left;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.a3s-catalog-results button.selected {
  border-color: var(--a3s-blue);
  background: var(--a3s-blue-soft);
  color: var(--a3s-blue-ink);
}

.a3s-catalog-empty {
  display: flex;
  min-height: 76px;
  padding: 12px;
  color: var(--a3s-faint);
  text-align: center;
  align-items: center;
  justify-content: center;
  flex-direction: column;
  gap: 6px;
}

.a3s-catalog-empty > p {
  max-width: 30ch;
  margin: 0;
  font-size: 10px;
}

.a3s-quality {
  display: flex;
  padding: 9px 10px 10px;
  border-bottom: 1px solid var(--a3s-line);
  flex-direction: column;
  gap: 6px;
}

.a3s-section-heading {
  display: flex;
  min-height: 24px;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
}

.a3s-section-heading small {
  color: var(--a3s-faint);
}

.a3s-quality-item {
  display: flex;
  min-width: 0;
  padding: 9px;
  border: 1px solid transparent;
  border-radius: 9px;
  background: color-mix(in srgb, var(--a3s-soft) 68%, var(--a3s-panel));
  flex-direction: column;
  gap: 4px;
  transition: background-color var(--a3s-motion-normal) ease, border-color var(--a3s-motion-normal) ease;
}

.a3s-quality-item:hover {
  border-color: var(--a3s-line);
  background: var(--a3s-soft);
}

.a3s-quality-item > header,
.a3s-quality-item > p,
.a3s-quality-item > small,
.a3s-quality-item > footer {
  min-width: 0;
  overflow-wrap: anywhere;
}

.a3s-quality-item > header {
  display: flex;
  align-items: flex-start;
  gap: 7px;
}

.a3s-quality-item > header > strong {
  min-width: 0;
  color: var(--a3s-text);
  line-height: 1.4;
}

.a3s-quality-item > p {
  margin: 2px 0;
  color: var(--a3s-muted);
  font-size: 11px;
}

.a3s-quality-item > small {
  color: var(--a3s-faint);
  font-size: 9.5px;
}

.a3s-quality-item > footer,
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
  font-size: 11px;
}

.a3s-list {
  display: flex;
  min-height: 72px;
  padding: 8px 10px 10px;
  flex-direction: column;
  gap: 7px;
}

.a3s-list.is-empty {
  min-height: 100%;
  justify-content: center;
}

.a3s-item {
  display: flex;
  padding: 9px;
  border: 1px solid var(--a3s-line);
  border-radius: 10px;
  background: var(--a3s-panel);
  flex-direction: column;
  gap: 5px;
  transition: border-color 150ms ease, background-color 150ms ease, transform 150ms ease;
}

.a3s-item:hover {
  border-color: var(--a3s-line-strong);
}

.a3s-item.submitted {
  background: color-mix(in srgb, var(--a3s-blue-soft) 46%, var(--a3s-panel));
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
  line-height: 1.35;
}

.a3s-root[lang="en"] .a3s-status,
.a3s-root[lang="en"] .a3s-thread span {
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
  color: var(--a3s-warning-ink);
}

.status-suggestion {
  background: var(--a3s-blue-soft);
  color: var(--a3s-blue-ink);
}

.status-resolved,
.status-review_ready {
  background: var(--a3s-green-soft);
  color: var(--a3s-green-ink);
}

.a3s-empty {
  display: flex;
  margin: auto;
  max-width: 280px;
  padding: 24px 12px;
  color: var(--a3s-faint);
  text-align: center;
  align-items: center;
  flex-direction: column;
  gap: 7px;
}

.a3s-empty-icon {
  display: grid;
  width: 38px;
  height: 38px;
  margin-bottom: 3px;
  border-radius: 10px;
  background: var(--a3s-blue-soft);
  color: var(--a3s-blue-ink);
  place-items: center;
}

.a3s-empty-icon svg {
  width: 19px;
  height: 19px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.55;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.a3s-empty > strong {
  color: var(--a3s-text);
  font-size: 13px;
}

.a3s-empty > p {
  margin: 0;
  font-size: 11px;
}

.a3s-empty > button {
  display: inline-flex;
  margin-top: 5px;
  border-color: var(--a3s-blue-strong);
  background: var(--a3s-blue-strong);
  color: #ffffff;
  align-items: center;
  gap: 6px;
}

.a3s-empty > button svg {
  width: 16px;
  height: 16px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.55;
  stroke-linecap: round;
  stroke-linejoin: round;
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
    bottom: max(12px, env(safe-area-inset-bottom));
    left: auto;
  }

  .a3s-panel,
  .a3s-root[data-dock="left"] .a3s-panel {
    top: 8px;
    right: 8px;
    bottom: max(8px, env(safe-area-inset-bottom));
    left: auto;
    width: min(390px, calc(100vw - 16px));
  }

  .a3s-root[data-dock="left"] .a3s-panel {
    right: auto;
    left: 8px;
  }

  .a3s-settings-content input,
  .a3s-settings-content select,
  .a3s-layout input,
  .a3s-layout select,
  .a3s-catalog input,
  .a3s-editor textarea,
  .a3s-editor select,
  .a3s-reply-label textarea {
    font-size: 16px;
  }

  .a3s-workspace button,
  .a3s-editor > .a3s-actions button,
  .a3s-settings-content button {
    min-height: 44px;
  }
}

@media (max-width: 420px) {
  .a3s-settings-actions {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .a3s-settings-actions button:last-child {
    grid-column: 1 / -1;
  }

  .a3s-settings-grid,
  .a3s-fields,
  .a3s-layout-fields {
    grid-template-columns: 1fr;
  }

  .a3s-editor > .a3s-actions button {
    flex: 1 1 0;
  }

  .a3s-editor > .a3s-actions button.danger {
    margin-right: 0;
  }

  .a3s-editor > .a3s-actions .a3s-send-now {
    flex: 1 0 100%;
  }

  .a3s-panel,
  .a3s-root[data-dock="left"] .a3s-panel {
    top: 0;
    right: 0;
    bottom: 0;
    left: 0;
    width: 100%;
    border: 0;
    border-radius: 0;
  }
}

@media (max-width: 340px) {
  .a3s-catalog-results section > div {
    grid-template-columns: 1fr;
  }
}

@media (hover: none), (pointer: coarse) {
  .a3s-close,
  .a3s-panel-tabs button,
  .a3s-selection-grid button,
  .a3s-workspace button,
  .a3s-editor > .a3s-actions button,
  .a3s-settings-content button {
    min-width: 44px;
    min-height: 44px;
  }
}

${DESIGN_BOARD_CSS}

@media (prefers-reduced-motion: reduce) {
  .a3s-root *,
  .a3s-root *::before,
  .a3s-root *::after {
    scroll-behavior: auto !important;
    animation-duration: .01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: .01ms !important;
  }
}
`;
