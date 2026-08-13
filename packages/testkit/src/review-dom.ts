import {
  PAGE_CONTEXT_PROTOCOL,
  type PageContextBridge,
} from "./types";

export function bridgeIsCompatible(bridge: PageContextBridge | null): bridge is PageContextBridge {
  if (!bridge) return false;
  try {
    return bridge.probe().protocol === PAGE_CONTEXT_PROTOCOL;
  } catch {
    return false;
  }
}

export function isOverlayEvent(event: Event, host: HTMLElement | null): boolean {
  return Boolean(host && event.composedPath().includes(host));
}

export function isOverlayElement(element: Element, host: HTMLElement | null): boolean {
  return Boolean(host && (element === host || element.getRootNode() === host.shadowRoot));
}

export function targetElement(event: PointerEvent, host: HTMLElement | null): Element | null {
  if (isOverlayEvent(event, host)) return null;
  return event.composedPath().find((item): item is Element => (
    item instanceof Element && !item.closest("[data-a3s-testkit-overlay]")
  )) ?? null;
}

export function selectionElement(selection: Selection | null): Element | null {
  const node = selection?.anchorNode;
  return node instanceof Element ? node : node?.parentElement ?? null;
}

export function deepActiveElement(): HTMLElement | null {
  let active: Element | null = document.activeElement;
  while (active?.shadowRoot?.activeElement) active = active.shadowRoot.activeElement;
  return active instanceof HTMLElement ? active : null;
}

export function nodeForElement(bridge: PageContextBridge, element: Element) {
  const snapshot = bridge.snapshot({ detail: "forensic", limits: { nodes: 5_000 } });
  return snapshot.nodes.find((node) => bridge.resolve(node.id) === element) ?? null;
}
