import type {
  ContextNode,
  UIAccessibilityStateChange,
  UIInteractionState,
  UIStateDiff,
  UIStyleChange,
} from "./types";
import { truncateUtf8 } from "./sanitize";

const STATE_STYLE_PROPERTIES = [
  "color",
  "background-color",
  "border-top-color",
  "border-right-color",
  "border-bottom-color",
  "border-left-color",
  "border-top-width",
  "border-right-width",
  "border-bottom-width",
  "border-left-width",
  "border-radius",
  "box-shadow",
  "outline-color",
  "outline-style",
  "outline-width",
  "opacity",
  "transform",
  "filter",
  "text-decoration-color",
  "text-decoration-line",
  "cursor",
] as const;

const ACCESSIBILITY_STATES = [
  "disabled",
  "checked",
  "selected",
  "expanded",
  "focused",
  "readonly",
  "required",
  "invalid",
] as const;

type StyleObservation = Record<string, string>;
type AccessibilityObservation = Record<string, boolean | null>;
type DefaultObservation = {
  styles: StyleObservation;
  accessibility: AccessibilityObservation;
};

function normalized(value: string): string {
  return value.trim().replace(/\s+/g, " ");
}

function observeStyles(
  style: CSSStyleDeclaration,
  maxStringBytes: number,
): StyleObservation {
  return Object.fromEntries(
    STATE_STYLE_PROPERTIES.map((property) => [
      property,
      truncateUtf8(
        normalized(style.getPropertyValue(property)),
        maxStringBytes,
      ),
    ]),
  );
}

function observeAccessibility(
  element: Element,
  node: ContextNode | undefined,
): AccessibilityObservation {
  const state = node?.state;
  return Object.fromEntries(
    ACCESSIBILITY_STATES.map((key) => {
      if (key === "focused") {
        return [key, state?.focused ?? hasFocus(element)];
      }
      return [key, state?.[key] ?? null];
    }),
  );
}

function hasFocus(element: Element): boolean {
  if (safeMatches(element, ":focus")) return true;
  let active: Element | null = element.ownerDocument.activeElement;
  while (active?.shadowRoot?.activeElement)
    active = active.shadowRoot.activeElement;
  return active === element;
}

function safeMatches(element: Element, selector: string): boolean {
  try {
    return element.matches(selector);
  } catch {
    return false;
  }
}

function observedStates(
  element: Element,
  node: ContextNode | undefined,
): Array<Exclude<UIInteractionState, "default">> {
  const result: Array<Exclude<UIInteractionState, "default">> = [];
  if (hasFocus(element)) result.push("focus");
  if (safeMatches(element, ":focus-visible")) result.push("focus_visible");
  if (safeMatches(element, ":hover")) result.push("hover");
  const state = node?.state;
  const input = element instanceof HTMLInputElement ? element : null;
  const option = element instanceof HTMLOptionElement ? element : null;
  if (state?.checked === true || input?.checked === true)
    result.push("checked");
  if (
    state?.expanded === true ||
    element.getAttribute("aria-expanded") === "true"
  )
    result.push("expanded");
  if (state?.selected === true || option?.selected === true)
    result.push("selected");
  if (
    state?.disabled === true ||
    element.getAttribute("aria-disabled") === "true" ||
    ("disabled" in element && (element as HTMLButtonElement).disabled === true)
  )
    result.push("disabled");
  return Array.from(new Set(result));
}

function styleChanges(
  before: StyleObservation,
  after: StyleObservation,
): UIStyleChange[] {
  return STATE_STYLE_PROPERTIES.flatMap((property) =>
    before[property] !== after[property]
      ? [
          {
            property,
            before: before[property] ?? "",
            after: after[property] ?? "",
          },
        ]
      : [],
  ).slice(0, 24);
}

function accessibilityChanges(
  before: AccessibilityObservation,
  after: AccessibilityObservation,
): UIAccessibilityStateChange[] {
  return ACCESSIBILITY_STATES.flatMap((state) =>
    before[state] !== after[state]
      ? [
          {
            state,
            before: before[state] ?? null,
            after: after[state] ?? null,
          },
        ]
      : [],
  );
}

export function canObserveUIState(element: Element): boolean {
  const tag = element.tagName.toLowerCase();
  return (
    [
      "a",
      "button",
      "input",
      "option",
      "select",
      "summary",
      "textarea",
    ].includes(tag) ||
    element.hasAttribute("tabindex") ||
    element.hasAttribute("role") ||
    element.hasAttribute("aria-checked") ||
    element.hasAttribute("aria-expanded") ||
    element.hasAttribute("aria-selected")
  );
}

export class UIStateTracker {
  readonly #defaults = new Map<string, DefaultObservation>();

  observe(
    element: Element,
    nodeId: string,
    style: CSSStyleDeclaration,
    node: ContextNode | undefined,
    maxStringBytes: number,
  ): UIStateDiff[] {
    const current = {
      styles: observeStyles(style, maxStringBytes),
      accessibility: observeAccessibility(element, node),
    };
    const states = observedStates(element, node);
    if (states.length === 0) {
      if (this.#defaults.size >= 5_000 && !this.#defaults.has(nodeId)) {
        const oldest = this.#defaults.keys().next().value as string | undefined;
        if (oldest) this.#defaults.delete(oldest);
      }
      this.#defaults.set(nodeId, current);
      return [];
    }
    const baseline = this.#defaults.get(nodeId);
    if (!baseline) return [];
    const styles = styleChanges(baseline.styles, current.styles);
    const accessibility = accessibilityChanges(
      baseline.accessibility,
      current.accessibility,
    );
    if (styles.length === 0 && accessibility.length === 0) return [];
    return states.map((state) => ({
      nodeId,
      from: "default",
      to: state,
      styleChanges: styles,
      accessibilityChanges: accessibility,
      confidence: 1,
    }));
  }

  clear(): void {
    this.#defaults.clear();
  }
}
