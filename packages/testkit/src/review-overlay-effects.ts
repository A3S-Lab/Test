import { useEffect, useState } from "react";
import { useBrowserLayoutEffect } from "./react-effect";

type ReviewOverlayFocusOptions = {
  mount: HTMLElement | null;
  open: boolean;
  panelRef: { current: HTMLElement | null };
  launchRef: { current: HTMLButtonElement | null };
  focusPanelOnOpenRef: { current: boolean };
  focusLauncherOnCloseRef: { current: boolean };
};

export function useReviewOverlayFocus(options: ReviewOverlayFocusOptions): void {
  useBrowserLayoutEffect(() => {
    if (!options.open || !options.focusPanelOnOpenRef.current) return;
    options.focusPanelOnOpenRef.current = false;
    options.panelRef.current?.focus();
  }, [options.mount, options.open]);

  useBrowserLayoutEffect(() => {
    if (options.open || !options.focusLauncherOnCloseRef.current) return;
    options.focusLauncherOnCloseRef.current = false;
    const focusLauncher = () => options.launchRef.current?.focus({ preventScroll: true });
    focusLauncher();
    const frame = window.requestAnimationFrame(focusLauncher);
    return () => window.cancelAnimationFrame(frame);
  }, [options.mount, options.open]);
}

export function useReviewGeometryRefresh(open: boolean): void {
  const [, setRevision] = useState(0);

  useEffect(() => {
    if (!open) return;
    let frame: number | null = null;
    const refreshGeometry = () => {
      if (frame !== null) return;
      frame = window.requestAnimationFrame(() => {
        frame = null;
        setRevision((current) => current + 1);
      });
    };
    const visualViewport = window.visualViewport;
    window.addEventListener("scroll", refreshGeometry, true);
    window.addEventListener("resize", refreshGeometry);
    visualViewport?.addEventListener("scroll", refreshGeometry);
    visualViewport?.addEventListener("resize", refreshGeometry);
    return () => {
      window.removeEventListener("scroll", refreshGeometry, true);
      window.removeEventListener("resize", refreshGeometry);
      visualViewport?.removeEventListener("scroll", refreshGeometry);
      visualViewport?.removeEventListener("resize", refreshGeometry);
      if (frame !== null) window.cancelAnimationFrame(frame);
    };
  }, [open]);
}

type InlineOverflowSnapshot = {
  element: HTMLElement;
  priority: string;
  value: string;
};

function lockDocumentScroll(): () => void {
  const snapshots: InlineOverflowSnapshot[] = [document.documentElement, document.body]
    .filter((element): element is HTMLElement => Boolean(element))
    .map((element) => ({
      element,
      priority: element.style.getPropertyPriority("overflow"),
      value: element.style.getPropertyValue("overflow"),
    }));

  for (const { element } of snapshots) {
    element.style.setProperty("overflow", "hidden", "important");
  }

  return () => {
    for (const { element, priority, value } of snapshots) {
      if (value) element.style.setProperty("overflow", value, priority);
      else element.style.removeProperty("overflow");
    }
  };
}

export function useReviewMobileScrollLock(active: boolean): void {
  useEffect(() => {
    if (!active || typeof window.matchMedia !== "function") return;
    const compactViewport = window.matchMedia("(max-width: 420px)");
    let unlock: (() => void) | null = null;

    const sync = () => {
      if (compactViewport.matches && !unlock) {
        unlock = lockDocumentScroll();
      } else if (!compactViewport.matches && unlock) {
        unlock();
        unlock = null;
      }
    };

    sync();
    compactViewport.addEventListener("change", sync);
    return () => {
      compactViewport.removeEventListener("change", sync);
      unlock?.();
    };
  }, [active]);
}
