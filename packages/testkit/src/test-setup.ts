import { afterEach, beforeEach, vi } from "vitest";
import { cleanup } from "@testing-library/react";
import { getPageContextBridge } from "./runtime";

class ResizeObserverStub implements ResizeObserver {
  readonly #callback: ResizeObserverCallback;

  constructor(callback: ResizeObserverCallback) {
    this.#callback = callback;
  }

  disconnect(): void {}
  observe(): void {}
  unobserve(): void {}
  takeRecords(): ResizeObserverEntry[] { return []; }
}

beforeEach(() => {
  if (typeof window === "undefined") return;
  Object.defineProperty(globalThis, "ResizeObserver", { value: ResizeObserverStub, configurable: true });
  Object.defineProperty(window, "innerWidth", { value: 1000, configurable: true });
  Object.defineProperty(window, "innerHeight", { value: 800, configurable: true });
  Object.defineProperty(window, "devicePixelRatio", { value: 1, configurable: true });
  Object.defineProperty(window, "visualViewport", { value: undefined, configurable: true });
  Object.defineProperty(window, "scrollX", { value: 0, configurable: true });
  Object.defineProperty(window, "scrollY", { value: 0, configurable: true });
  Object.defineProperty(document, "elementsFromPoint", { value: () => [], configurable: true });
  document.documentElement.innerHTML = "<head><title>Test page</title></head><body></body>";
  window.history.replaceState(null, "", "/test");
  vi.restoreAllMocks();
});

afterEach(() => {
  if (typeof window === "undefined") return;
  cleanup();
  getPageContextBridge()?.dispose();
  document.querySelectorAll("[data-a3s-testkit-overlay]").forEach((element) => element.remove());
  window.sessionStorage.clear();
  window.localStorage.clear();
});

export function setRect(element: Element, rect: { x: number; y: number; width: number; height: number }): void {
  const value = DOMRect.fromRect(rect);
  Object.defineProperty(element, "getBoundingClientRect", { value: () => value, configurable: true });
  Object.defineProperty(element, "getClientRects", { value: () => [value], configurable: true });
}
