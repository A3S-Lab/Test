import { useEffect, useRef } from "react";
import { isOverlayElement, isOverlayEvent } from "./review-dom";
import { isEditableEvent } from "./review-integration";

const REVIEW_EVENT_KEYS = {
  toggle: "f",
  escape: "Escape",
  layout: "l",
  pause: "p",
  markers: "h",
  copy: "c",
  clear: "x",
} as const;

export const REVIEW_KEY_SHORTCUTS = {
  toggle: "Control+Shift+F Meta+Shift+F",
  escape: REVIEW_EVENT_KEYS.escape,
  layout: REVIEW_EVENT_KEYS.layout.toUpperCase(),
  pause: REVIEW_EVENT_KEYS.pause.toUpperCase(),
  markers: REVIEW_EVENT_KEYS.markers.toUpperCase(),
  copy: REVIEW_EVENT_KEYS.copy.toUpperCase(),
  clear: REVIEW_EVENT_KEYS.clear.toUpperCase(),
} as const;

export const REVIEW_SHORTCUT_HELP = [
  { action: "Toggle review", keys: "Ctrl/Command + Shift + F" },
  { action: "Cancel or close", keys: "Esc" },
  { action: "Toggle Layout Mode", keys: REVIEW_KEY_SHORTCUTS.layout },
  { action: "Pause or resume motion", keys: REVIEW_KEY_SHORTCUTS.pause },
  { action: "Show or hide markers", keys: REVIEW_KEY_SHORTCUTS.markers },
  { action: "Copy selected drafts", keys: REVIEW_KEY_SHORTCUTS.copy },
  { action: "Clear local drafts", keys: REVIEW_KEY_SHORTCUTS.clear },
] as const;

export function useLastApplicationFocus(enabled: boolean, host: HTMLElement | null) {
  const lastApplicationFocus = useRef<HTMLElement | null>(null);
  useEffect(() => {
    if (!enabled) return;
    const rememberApplicationFocus = (event: FocusEvent) => {
      const target = event.composedPath()[0];
      if (target instanceof HTMLElement && !isOverlayElement(target, host)) {
        lastApplicationFocus.current = target;
      }
    };
    document.addEventListener("focusin", rememberApplicationFocus, true);
    return () => document.removeEventListener("focusin", rememberApplicationFocus, true);
  }, [enabled, host]);
  return lastApplicationFocus;
}

export function useHostPointerBlocking(active: boolean, host: HTMLElement | null) {
  useEffect(() => {
    if (!active || !host) return;
    const blockHostPointerInput = (event: Event) => {
      if (isOverlayEvent(event, host)) return;
      if (event.cancelable) event.preventDefault();
      event.stopPropagation();
    };
    const eventTypes = [
      "pointerdown",
      "pointermove",
      "pointerup",
      "mousedown",
      "mouseup",
      "click",
      "dblclick",
      "auxclick",
      "contextmenu",
      "touchstart",
      "touchmove",
      "touchend",
      "wheel",
    ];
    const options: AddEventListenerOptions = { capture: true, passive: false };
    for (const type of eventTypes) {
      document.addEventListener(type, blockHostPointerInput, options);
    }
    return () => {
      for (const type of eventTypes) {
        document.removeEventListener(type, blockHostPointerInput, options);
      }
    };
  }, [active, host]);
}

export type GlobalReviewShortcutsOptions = {
  active: boolean;
  open: boolean;
  marking: boolean;
  candidate: boolean;
  hasDrafts: boolean;
  onToggleOverlay(): void;
  onCancelMarking(restoreFocus: boolean): void;
  onCancelCandidate(): void;
  onCloseOverlay(): void;
  onToggleLayout(): void;
  onTogglePause(): void;
  onToggleMarkers(): void;
  onCopyDrafts(): void;
  onClearDrafts(): void;
};

export function useGlobalReviewShortcuts(options: GlobalReviewShortcutsOptions) {
  const latest = useRef(options);
  latest.current = options;
  useEffect(() => {
    if (!options.active) return;
    const onGlobalKeyDown = (event: KeyboardEvent) => {
      if (event.defaultPrevented || isEditableEvent(event)) return;
      const current = latest.current;
      const key = event.key.toLowerCase();
      if ((event.metaKey || event.ctrlKey) && event.shiftKey && key === REVIEW_EVENT_KEYS.toggle) {
        event.preventDefault();
        event.stopPropagation();
        if (current.marking) current.onCancelMarking(false);
        current.onToggleOverlay();
        return;
      }
      if (event.metaKey || event.ctrlKey || event.altKey || event.shiftKey) return;
      if (event.key === REVIEW_EVENT_KEYS.escape) {
        if (current.marking) current.onCancelMarking(true);
        else if (current.candidate) current.onCancelCandidate();
        else if (current.open) current.onCloseOverlay();
        else return;
        event.preventDefault();
        event.stopPropagation();
        return;
      }
      if (!current.open) return;
      if (key === REVIEW_EVENT_KEYS.layout) current.onToggleLayout();
      else if (key === REVIEW_EVENT_KEYS.pause) current.onTogglePause();
      else if (key === REVIEW_EVENT_KEYS.markers) current.onToggleMarkers();
      else if (key === REVIEW_EVENT_KEYS.copy && current.hasDrafts) current.onCopyDrafts();
      else if (key === REVIEW_EVENT_KEYS.clear && current.hasDrafts) current.onClearDrafts();
      else return;
      event.preventDefault();
      event.stopPropagation();
    };
    document.addEventListener("keydown", onGlobalKeyDown, true);
    return () => document.removeEventListener("keydown", onGlobalKeyDown, true);
  }, [options.active]);
}
