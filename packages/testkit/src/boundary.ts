import { safeCallback } from "./sanitize";
import type { BoundaryRegistration } from "./types";

export function composedContains(root: Element, candidate: Element): boolean {
  return composedDistance(root, candidate) !== null;
}

export function composedDistance(
  root: Element,
  candidate: Element,
): number | null {
  let distance = 0;
  let current: Node | null = candidate;
  while (current) {
    if (current === root) return distance;
    const parent: Node | null = current.parentNode;
    current =
      parent ??
      (current.getRootNode() instanceof ShadowRoot
        ? (current.getRootNode() as ShadowRoot).host
        : null);
    distance += 1;
  }
  return null;
}

export function boundaryDepth(boundary: BoundaryRegistration): number {
  return Math.max(0, ...boundaryElements(boundary).map(elementDepth));
}

export function boundaryElements(boundary: BoundaryRegistration): Element[] {
  const elements = safeCallback(boundary.elements, [] as readonly Element[]);
  return Array.from(
    new Set(
      elements.filter(
        (element): element is Element =>
          element instanceof Element && element.isConnected,
      ),
    ),
  );
}

function elementDepth(element: Element): number {
  let value = 0;
  let current: Node | null = element;
  while (current) {
    value += 1;
    const parent: Node | null = current.parentNode;
    current =
      parent ??
      (current.getRootNode() instanceof ShadowRoot
        ? (current.getRootNode() as ShadowRoot).host
        : null);
  }
  return value;
}
