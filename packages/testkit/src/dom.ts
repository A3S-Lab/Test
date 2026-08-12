import type {
  ContextDetail,
  ContextNode,
  LocatorCandidate,
  NodeGeometry,
  Rect,
} from "./types";
import { truncateUtf8 } from "./sanitize";

const SKIPPED_TAGS = new Set(["script", "style", "noscript", "template", "meta", "link"]);
const INTERACTIVE_TAGS = new Set(["a", "button", "input", "select", "textarea", "summary"]);
const ROLE_BY_TAG: Record<string, string> = {
  a: "link",
  button: "button",
  h1: "heading",
  h2: "heading",
  h3: "heading",
  h4: "heading",
  h5: "heading",
  h6: "heading",
  img: "img",
  nav: "navigation",
  main: "main",
  form: "form",
  table: "table",
  textarea: "textbox",
  select: "combobox",
};

export type NodeIdentity = {
  idFor(element: Element): string;
};

function escapeCss(value: string): string {
  if (typeof CSS !== "undefined" && typeof CSS.escape === "function") return CSS.escape(value);
  return value.replace(/[^a-zA-Z0-9_-]/g, (character) => `\\${character.codePointAt(0)?.toString(16)} `);
}

export function walkElements(root: Document | ShadowRoot | Element): Element[] {
  const result: Element[] = [];
  const visit = (element: Element) => {
    const tag = element.tagName.toLowerCase();
    if (SKIPPED_TAGS.has(tag) || element.hasAttribute("data-a3s-testkit-overlay")) return;
    result.push(element);
    for (const child of element.children) visit(child);
    if (element.shadowRoot) {
      for (const child of element.shadowRoot.children) visit(child);
    }
  };
  if (root instanceof Element) visit(root);
  else for (const child of root.children) visit(child);
  return result;
}

export function isRedacted(element: Element, selectors: readonly string[]): boolean {
  if (element instanceof HTMLInputElement && ["password", "hidden"].includes(element.type)) return true;
  let current: Element | null = element;
  while (current) {
    for (const selector of selectors) {
      try {
        if (current.matches(selector)) return true;
      } catch {
        // Invalid application configuration must not break context capture.
      }
    }
    current = composedParent(current);
  }
  return false;
}

function roleFor(element: Element): string | undefined {
  const explicit = element.getAttribute("role")?.trim();
  if (explicit) return explicit.split(/\s+/)[0];
  const tag = element.tagName.toLowerCase();
  if (tag === "input" && element instanceof HTMLInputElement) {
    if (["button", "submit", "reset"].includes(element.type)) return "button";
    if (element.type === "checkbox") return "checkbox";
    if (element.type === "radio") return "radio";
    if (element.type === "range") return "slider";
    return "textbox";
  }
  return ROLE_BY_TAG[tag];
}

function labelText(element: Element, redactSelectors: readonly string[]): string | undefined {
  const ariaLabel = element.getAttribute("aria-label")?.trim();
  if (ariaLabel) return ariaLabel;
  const labelledBy = element.getAttribute("aria-labelledby");
  if (labelledBy) {
    const labels = labelledBy
      .split(/\s+/)
      .map((id) => element.ownerDocument.getElementById(id))
      .filter((label): label is HTMLElement => label !== null)
      .filter((label) => !isRedacted(label, redactSelectors))
      .map((label) => visibleText(label, redactSelectors))
      .filter((value): value is string => Boolean(value));
    if (labels.length > 0) return labels.join(" ");
  }
  if (element instanceof HTMLInputElement || element instanceof HTMLSelectElement || element instanceof HTMLTextAreaElement) {
    const labels = Array.from(element.labels ?? [])
      .filter((label) => !isRedacted(label, redactSelectors))
      .map((label) => visibleText(label, redactSelectors))
      .filter(Boolean);
    if (labels.length > 0) return labels.join(" ");
    if ((element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement) && element.placeholder.trim()) {
      return element.placeholder.trim();
    }
  }
  if (element instanceof HTMLImageElement && element.alt.trim()) return element.alt.trim();
  const text = visibleText(element, redactSelectors);
  return text || undefined;
}

function visibleText(element: Element, redactSelectors: readonly string[]): string {
  if (isInsideOverlay(element)) return "";
  const walker = element.ownerDocument.createTreeWalker(element, NodeFilter.SHOW_TEXT);
  const values: string[] = [];
  let current = walker.nextNode();
  while (current) {
    const parent = current.parentElement;
    if (parent && !isInsideOverlay(parent) && !isRedacted(parent, redactSelectors) && !isHiddenFromContext(parent)) {
      values.push(current.nodeValue ?? "");
    }
    current = walker.nextNode();
  }
  return values.join(" ").replace(/\s+/g, " ").trim();
}

function cssPath(element: Element): string {
  if (element.id) return `#${escapeCss(element.id)}`;
  const testId = element.getAttribute("data-testid") ?? element.getAttribute("data-test-id");
  if (testId) return `[data-testid="${escapeCss(testId)}"]`;
  const parts: string[] = [];
  let current: Element | null = element;
  while (current && parts.length < 5) {
    let part = current.tagName.toLowerCase();
    const parent: Element | null = current.parentElement;
    if (parent) {
      const peers = Array.from(parent.children).filter((child) => child.tagName === current?.tagName);
      if (peers.length > 1) part += `:nth-of-type(${peers.indexOf(current) + 1})`;
    }
    parts.unshift(part);
    const root: Node = current.getRootNode();
    current = parent ?? (root instanceof ShadowRoot ? root.host : null);
  }
  return parts.join(" > ");
}

function locatorsFor(
  element: Element,
  role: string | undefined,
  name: string | undefined,
  redactSelectors: readonly string[],
): LocatorCandidate[] {
  const result: LocatorCandidate[] = [];
  const testId = element.getAttribute("data-testid") ?? element.getAttribute("data-test-id");
  if (testId) result.push({ type: "test_id", value: testId });
  if (role && name) result.push({ type: "role", role, name });
  if (element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement || element instanceof HTMLSelectElement) {
    const labelElement = Array.from(element.labels ?? []).find((candidate) => !isRedacted(candidate, redactSelectors));
    const label = labelElement ? visibleText(labelElement, redactSelectors) : "";
    if (label) result.push({ type: "label", value: label });
    if ("placeholder" in element && element.placeholder) {
      result.push({ type: "placeholder", value: element.placeholder });
    }
  }
  const text = visibleText(element, redactSelectors);
  if (text && text.length <= 120) result.push({ type: "text", value: text, exact: true });
  result.push({ type: "css", value: cssPath(element) });
  return result.slice(0, 5);
}

function rectValue(rect: DOMRect | DOMRectReadOnly): Rect {
  return { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
}

function intersectionRatio(rect: DOMRect): number {
  if (rect.width <= 0 || rect.height <= 0) return 0;
  const width = Math.max(0, Math.min(rect.right, window.innerWidth) - Math.max(rect.left, 0));
  const height = Math.max(0, Math.min(rect.bottom, window.innerHeight) - Math.max(rect.top, 0));
  return Math.max(0, Math.min(1, (width * height) / (rect.width * rect.height)));
}

function isOccluded(element: Element, rect: DOMRect): boolean {
  if (rect.width <= 0 || rect.height <= 0 || typeof document.elementsFromPoint !== "function") return false;
  const x = Math.max(0, Math.min(window.innerWidth - 1, rect.left + rect.width / 2));
  const y = Math.max(0, Math.min(window.innerHeight - 1, rect.top + rect.height / 2));
  const top = document.elementsFromPoint(x, y)[0];
  return Boolean(top && top !== element && !element.contains(top) && !top.contains(element));
}

function geometryFor(element: Element, identity: NodeIdentity): NodeGeometry | undefined {
  const rect = element.getBoundingClientRect();
  if (rect.width <= 0 && rect.height <= 0) return undefined;
  const computed = getComputedStyle(element);
  const declaredPosition = (element as HTMLElement).style.position;
  const position = declaredPosition || computed.position;
  let scrollContainer: Element | null = element.parentElement;
  while (scrollContainer) {
    const style = getComputedStyle(scrollContainer);
    if (/(auto|scroll|overlay)/.test(`${style.overflow} ${style.overflowX} ${style.overflowY}`)) break;
    scrollContainer = scrollContainer.parentElement;
  }
  return {
    viewport: rectValue(rect),
    document: { x: rect.x + window.scrollX, y: rect.y + window.scrollY, width: rect.width, height: rect.height },
    normalized: {
      x: window.innerWidth ? rect.x / window.innerWidth : 0,
      y: window.innerHeight ? rect.y / window.innerHeight : 0,
      width: window.innerWidth ? rect.width / window.innerWidth : 0,
      height: window.innerHeight ? rect.height / window.innerHeight : 0,
    },
    visibleRatio: intersectionRatio(rect),
    occluded: isOccluded(element, rect),
    position: (["static", "relative", "absolute", "fixed", "sticky"].includes(position)
      ? position
      : "static") as NodeGeometry["position"],
    transformed: computed.transform !== "none",
    ...(scrollContainer ? { scrollContainerNodeId: identity.idFor(scrollContainer) } : {}),
  };
}

function shouldInclude(
  element: Element,
  role: string | undefined,
  detail: ContextDetail,
  redactSelectors: readonly string[],
): boolean {
  if (detail === "forensic") return true;
  const tag = element.tagName.toLowerCase();
  if (role || INTERACTIVE_TAGS.has(tag)) return true;
  if (["main", "nav", "header", "footer", "section", "article", "dialog"].includes(tag)) return true;
  const text = visibleText(element, redactSelectors);
  return Boolean(text && text.length <= 500 && !Array.from(element.children).some((child) => visibleText(child, redactSelectors) === text));
}

export function describeElement(
  element: Element,
  identity: NodeIdentity,
  detail: ContextDetail,
  maxStringBytes: number,
  redactSelectors: readonly string[],
): ContextNode | null {
  const role = roleFor(element);
  if (!shouldInclude(element, role, detail, redactSelectors)) return null;
  const redacted = isRedacted(element, redactSelectors);
  const geometry = geometryFor(element, identity);
  const name = redacted ? "[redacted]" : labelText(element, redactSelectors);
  const parent = element.parentElement ?? (element.getRootNode() instanceof ShadowRoot ? (element.getRootNode() as ShadowRoot).host : null);
  const state: ContextNode["state"] = {
    visible: Boolean(geometry && geometry.visibleRatio > 0 && getComputedStyle(element).visibility !== "hidden"),
    focused: document.activeElement === element,
  };
  if (element instanceof HTMLButtonElement || element instanceof HTMLInputElement || element instanceof HTMLSelectElement || element instanceof HTMLTextAreaElement) {
    state.disabled = element.disabled;
  }
  if (element instanceof HTMLInputElement) {
    if (["checkbox", "radio"].includes(element.type)) state.checked = element.checked;
    state.readonly = element.readOnly;
    state.required = element.required;
  }
  for (const [attribute, key] of [["aria-expanded", "expanded"], ["aria-selected", "selected"], ["aria-invalid", "invalid"]] as const) {
    const value = element.getAttribute(attribute);
    if (value !== null) state[key] = value === "true";
  }
  const text = redacted ? "[redacted]" : visibleText(element, redactSelectors);
  const testId = element.getAttribute("data-testid") ?? element.getAttribute("data-test-id");
  const node: ContextNode = {
    id: identity.idFor(element),
    ...(parent ? { parentId: identity.idFor(parent) } : {}),
    tag: element.tagName.toLowerCase(),
    ...(role ? { role: truncateUtf8(role, maxStringBytes) } : {}),
    ...(name ? { name: truncateUtf8(name, maxStringBytes) } : {}),
    ...(text ? { text: truncateUtf8(text, maxStringBytes) } : {}),
    ...(testId ? { testId: truncateUtf8(testId, maxStringBytes) } : {}),
    ...(geometry ? { geometry } : {}),
    state,
    locators: redacted ? [] : locatorsFor(element, role, name, redactSelectors).map((locator) => {
      if ("value" in locator) return { ...locator, value: truncateUtf8(locator.value, maxStringBytes) };
      return {
        ...locator,
        role: truncateUtf8(locator.role, maxStringBytes),
        name: truncateUtf8(locator.name, maxStringBytes),
      };
    }),
  };
  if (detail === "forensic") {
    node.classes = Array.from(element.classList).slice(0, 20).map((value) => truncateUtf8(value, 256));
    node.attributes = Object.fromEntries(
      Array.from(element.attributes)
        .filter((attribute) => !/^(value|srcdoc|style)$/i.test(attribute.name) && !/token|secret|password/i.test(attribute.name))
        .slice(0, 30)
        .map((attribute) => [truncateUtf8(attribute.name, 256), redacted ? "[redacted]" : truncateUtf8(attribute.value, maxStringBytes)]),
    );
    const style = getComputedStyle(element);
    node.computedStyles = Object.fromEntries(
      ["display", "position", "color", "background-color", "font-size", "font-weight", "margin", "padding", "gap", "z-index"]
        .map((property) => [property, style.getPropertyValue(property)])
        .filter(([, value]) => value),
    );
  }
  return node;
}

function composedParent(element: Element): Element | null {
  if (element.parentElement) return element.parentElement;
  const root = element.getRootNode();
  return root instanceof ShadowRoot ? root.host : null;
}

function isHiddenFromContext(element: Element): boolean {
  let current: Element | null = element;
  while (current) {
    if (current.hasAttribute("hidden") || current.getAttribute("aria-hidden") === "true") return true;
    const tag = current.tagName.toLowerCase();
    if (SKIPPED_TAGS.has(tag)) return true;
    current = composedParent(current);
  }
  return false;
}

function isInsideOverlay(element: Element): boolean {
  let current: Element | null = element;
  while (current) {
    if (current.hasAttribute("data-a3s-testkit-overlay")) return true;
    current = composedParent(current);
  }
  return false;
}

export function overlaps(rect: Rect, region: Rect): boolean {
  return rect.x < region.x + region.width && rect.x + rect.width > region.x && rect.y < region.y + region.height && rect.y + rect.height > region.y;
}
