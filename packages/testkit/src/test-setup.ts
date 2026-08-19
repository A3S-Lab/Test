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
  vi.restoreAllMocks();
  if (typeof window === "undefined") return;
  Object.defineProperty(globalThis, "ResizeObserver", { value: ResizeObserverStub, configurable: true });
  Object.defineProperty(window, "innerWidth", { value: 1000, configurable: true });
  Object.defineProperty(window, "innerHeight", { value: 800, configurable: true });
  Object.defineProperty(window, "devicePixelRatio", { value: 1, configurable: true });
  Object.defineProperty(window, "visualViewport", { value: undefined, configurable: true });
  Object.defineProperty(window, "scrollX", { value: 0, configurable: true });
  Object.defineProperty(window, "scrollY", { value: 0, configurable: true });
  Object.defineProperty(document, "elementsFromPoint", { value: () => [], configurable: true });
  Object.defineProperty(HTMLCanvasElement.prototype, "getContext", {
    configurable: true,
    value: () => ({
      beginPath() {},
      drawImage() {},
      fillRect() {},
      fillText() {},
      lineTo() {},
      moveTo() {},
      restore() {},
      save() {},
      scale() {},
      stroke() {},
      strokeRect() {},
    }) as unknown as CanvasRenderingContext2D,
  });
  Object.defineProperty(HTMLCanvasElement.prototype, "toBlob", {
    configurable: true,
    value: (callback: BlobCallback, mediaType = "image/png") => {
      callback(new Blob([new Uint8Array([1, 2, 3, 4])], { type: mediaType }));
    },
  });
  Object.defineProperty(HTMLCanvasElement.prototype, "toDataURL", {
    configurable: true,
    value: (mediaType = "image/png") => `data:${mediaType};base64,AQIDBA==`,
  });
  document.documentElement.innerHTML = "<head><title>Test page</title></head><body></body>";
  window.history.replaceState(null, "", "/test");
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
