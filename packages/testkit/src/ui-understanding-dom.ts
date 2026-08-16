import { truncateUtf8 } from "./sanitize";
import type { ContextNode, Rect } from "./types";

export type UIUnderstandingIdentity = {
  idFor(element: Element): string;
};

export type UISample = {
  element: Element;
  nodeId: string;
  node: ContextNode | undefined;
  style: CSSStyleDeclaration;
};

export function normalizeCss(value: string): string {
  return value.trim().replace(/\s+/g, " ");
}

export function styleValue(
  element: Element,
  style: CSSStyleDeclaration,
  property: string,
): string {
  const computed = normalizeCss(style.getPropertyValue(property));
  if (computed) return computed;
  if (element instanceof HTMLElement || element instanceof SVGElement)
    return normalizeCss(element.style.getPropertyValue(property));
  return "";
}

export function boundedStyleValue(
  element: Element,
  style: CSSStyleDeclaration,
  property: string,
  maxStringBytes: number,
): string {
  return truncateUtf8(styleValue(element, style, property), maxStringBytes);
}

export function rectValue(rect: DOMRect | DOMRectReadOnly): Rect {
  return {
    x: rect.x,
    y: rect.y,
    width: rect.width,
    height: rect.height,
  };
}

export function composedParent(element: Element): Element | null {
  if (element.parentElement) return element.parentElement;
  const root = element.getRootNode();
  return root instanceof ShadowRoot ? root.host : null;
}

export function composedChildren(element: Element): Element[] {
  return [
    ...Array.from(element.children),
    ...(element.shadowRoot ? Array.from(element.shadowRoot.children) : []),
  ];
}

export function isScrollContainer(
  element: Element,
  style: CSSStyleDeclaration,
): boolean {
  return /(auto|scroll|overlay|hidden)/.test(
    `${styleValue(element, style, "overflow-x")} ${styleValue(element, style, "overflow-y")}`,
  );
}

export function monotonicNow(): number {
  return globalThis.performance?.now?.() ?? Date.now();
}
