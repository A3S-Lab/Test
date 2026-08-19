import { OVERLAY_SHELL_CSS } from "./overlay-shell-style";
import { OVERLAY_MARKING_CSS } from "./overlay-marking-style";
import { DESIGN_BOARD_CSS } from "./design-board-style";

export const OVERLAY_CSS = `
${OVERLAY_SHELL_CSS}

.a3s-hint {
  position: absolute;
  right: 0;
  bottom: calc(100% + 8px);
  width: min(330px, calc(100vw - 24px));
  margin: 0;
  padding: 8px 10px;
  border: 1px solid var(--a3s-toolbar-line);
  border-radius: 10px;
  background: var(--a3s-toolbar);
  box-shadow: 0 14px 36px rgb(5 10 20 / 28%);
  color: var(--a3s-toolbar-text);
  pointer-events: auto;
  font-size: 11px;
  animation: a3s-hint-enter 180ms cubic-bezier(.16, 1, .3, 1) both;
}

@keyframes a3s-hint-enter {
  from {
    opacity: 0;
    transform: translateY(5px) scale(.98);
  }
  to {
    opacity: 1;
    transform: translateY(0) scale(1);
  }
}

.a3s-root[data-dock="left"] .a3s-hint {
  right: auto;
  left: 0;
}

.a3s-workspace {
  position: absolute;
  right: 0;
  bottom: calc(100% + 8px);
  display: grid;
  width: min(400px, calc(100vw - 24px));
  max-height: min(600px, calc(100vh - 82px));
  overflow: hidden;
  border: 0;
  border-radius: 14px;
  background: var(--a3s-panel);
  box-shadow: var(--a3s-shadow);
  pointer-events: auto;
  grid-template-rows: 44px minmax(0, 1fr) auto;
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

.a3s-settings {
  position: relative;
  flex: 0 0 auto;
}

.a3s-settings-content {
  position: absolute;
  right: 0;
  bottom: calc(100% + 8px);
  z-index: 14;
  display: flex;
  width: min(348px, calc(100vw - 24px));
  max-height: min(540px, calc(100vh - 150px));
  padding: 12px;
  overflow: auto;
  overscroll-behavior: contain;
  border: 0;
  border-radius: 14px;
  background: var(--a3s-panel);
  box-shadow: var(--a3s-shadow);
  color: var(--a3s-text);
  flex-direction: column;
  gap: 9px;
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

.a3s-root[data-dock="left"] .a3s-settings-content {
  right: auto;
  left: 0;
}

.a3s-settings-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
}

.a3s-settings label,
.a3s-layout label,
.a3s-editor label {
  display: flex;
  flex-direction: column;
  gap: 4px;
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
.a3s-settings input::placeholder,
.a3s-layout input::placeholder,
.a3s-catalog input::placeholder {
  color: var(--a3s-faint);
  opacity: 1;
}

.a3s-editor-request textarea:focus-visible {
  border-color: var(--a3s-marker-color);
  outline: 2px solid color-mix(in srgb, var(--a3s-marker-color) 28%, transparent);
  outline-offset: 1px;
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
  max-height: 170px;
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
  color: var(--a3s-blue-ink);
}

.a3s-catalog-empty {
  margin: 0;
  color: var(--a3s-faint);
}

.a3s-quality {
  padding: 8px 10px 0;
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
  padding: 8px 0;
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
  font-size: 11px;
}

.a3s-list {
  display: flex;
  min-height: 72px;
  padding: 8px 10px 10px;
  flex-direction: column;
  gap: 7px;
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
  color: var(--a3s-warning);
}

.status-suggestion {
  background: var(--a3s-blue-soft);
  color: var(--a3s-blue-ink);
}

.status-resolved,
.status-review_ready {
  background: var(--a3s-green-soft);
  color: var(--a3s-green);
}

.a3s-empty {
  margin: 0;
  padding: 14px 8px;
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
    right: 8px;
    bottom: max(8px, env(safe-area-inset-bottom));
    left: 8px;
    width: auto;
  }

  .a3s-command-bar {
    height: 58px;
    min-height: 58px;
    align-items: center;
  }

  .a3s-command-bar > header {
    min-width: 122px;
    height: 46px;
    align-self: center;
    grid-template-columns: 28px minmax(44px, 1fr) 44px;
  }

  .a3s-toolbar-core {
    flex-wrap: nowrap;
  }

  .a3s-tools button,
  .a3s-settings > .a3s-disclosure {
    width: 44px;
    height: 44px;
    min-height: 44px;
  }

  .a3s-close {
    width: 44px;
    height: 44px;
    min-width: 44px;
    min-height: 44px;
  }

  .a3s-tool-tray,
  .a3s-root[data-dock="left"] .a3s-tool-tray {
    right: 0;
    left: auto;
    width: min(360px, calc(100vw - 16px));
    grid-template-columns: 1fr;
  }

  .a3s-tool-tray-copy {
    display: none;
  }

  .a3s-tool-tray > .a3s-tool-group {
    flex-wrap: wrap;
    justify-content: center;
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
    bottom: calc(78px + env(safe-area-inset-bottom));
    left: 8px;
    width: auto;
    max-height: calc(100vh - 110px - env(safe-area-inset-bottom));
    max-height: calc(100dvh - 110px - env(safe-area-inset-bottom));
  }

  .a3s-editor-popover {
    top: auto;
    right: 8px;
    bottom: calc(78px + env(safe-area-inset-bottom));
    left: 8px;
    width: auto;
    max-height: calc(100% - 80px);
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

  .a3s-settings input,
  .a3s-settings select,
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
  .a3s-command-bar {
    padding: 2px;
    gap: 1px;
  }

  .a3s-settings-grid,
  .a3s-fields,
  .a3s-layout-fields {
    grid-template-columns: 1fr;
  }

  .a3s-command-bar > header {
    min-width: 70px;
    padding-right: 2px;
    padding-left: 0;
    grid-template-columns: 24px 44px;
    gap: 1px;
  }

  .a3s-panel-mark {
    width: 24px;
    height: 24px;
  }

  .a3s-command-bar > header > span:nth-child(2) {
    position: fixed;
    width: 1px;
    height: 1px;
    clip-path: inset(50%);
  }

  .a3s-tools button,
  .a3s-settings > .a3s-disclosure {
    width: 44px;
    height: 44px;
    min-height: 44px;
    flex: 0 0 44px;
  }

  .a3s-toolbar-core,
  .a3s-tool-group {
    gap: 1px;
  }
}

@media (max-width: 340px) {
  .a3s-command-bar > header {
    min-width: 44px;
    padding: 0;
    grid-template-columns: 44px;
    gap: 0;
  }

  .a3s-panel-mark {
    display: none;
  }
}

@media (hover: none), (pointer: coarse) {
  .a3s-panel {
    width: min(410px, calc(100vw - 32px));
  }

  .a3s-command-bar {
    height: 58px;
    min-height: 58px;
    padding: 6px;
  }

  .a3s-command-bar > header {
    height: 46px;
  }

  .a3s-tools button,
  .a3s-settings > .a3s-disclosure,
  .a3s-close,
  .a3s-workspace button,
  .a3s-editor > .a3s-actions button,
  .a3s-settings-content button {
    min-width: 44px;
    min-height: 44px;
  }

  .a3s-tools button[data-tooltip]::after,
  .a3s-settings > .a3s-disclosure[data-tooltip]::after {
    display: none;
  }
}

@media (max-width: 720px) and (hover: none),
  (max-width: 720px) and (pointer: coarse) {
  .a3s-panel,
  .a3s-root[data-dock="left"] .a3s-panel {
    width: auto;
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
