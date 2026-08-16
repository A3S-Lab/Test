import { walkElements } from "./dom";

const TRANSIENT_BROWSER_INSTRUMENTATION_ATTRIBUTES = new Set([
  "data-__ab-ci",
  "data-agent-browser-located",
]);

export function installPageObserver(markChanged: () => void): () => void {
  const cleanup: Array<() => void> = [];
  const shadowRoots = new WeakSet<ShadowRoot>();
  const observeShadows = (root: ParentNode) => {
    for (const element of walkElements(
      root as Document | ShadowRoot | Element,
    )) {
      const shadow = element.shadowRoot;
      if (!shadow || shadowRoots.has(shadow)) continue;
      shadowRoots.add(shadow);
      mutation.observe(shadow, {
        subtree: true,
        childList: true,
        attributes: true,
        characterData: true,
      });
      observeShadows(shadow);
    }
  };
  const mutation = new MutationObserver((records) => {
    observeShadows(document);
    const hasPageChange = records.some((record) => {
      if (
        record.type === "attributes" &&
        record.attributeName &&
        TRANSIENT_BROWSER_INSTRUMENTATION_ATTRIBUTES.has(record.attributeName)
      )
        return false;
      return !insideOverlay(record.target);
    });
    if (hasPageChange) markChanged();
  });
  mutation.observe(document.documentElement, {
    subtree: true,
    childList: true,
    attributes: true,
    characterData: true,
  });
  observeShadows(document);
  cleanup.push(() => mutation.disconnect());

  if (typeof ResizeObserver !== "undefined") {
    const resize = new ResizeObserver(markChanged);
    resize.observe(document.documentElement);
    if (document.body) resize.observe(document.body);
    cleanup.push(() => resize.disconnect());
  }

  for (const event of ["resize", "scroll", "popstate", "hashchange"] as const) {
    window.addEventListener(event, markChanged, {
      capture: true,
      passive: true,
    });
    cleanup.push(() => window.removeEventListener(event, markChanged, true));
  }
  const stateChanged = (event: Event) => {
    if (
      event
        .composedPath()
        .some(
          (target) =>
            target instanceof Element &&
            target.hasAttribute("data-a3s-testkit-overlay"),
        )
    )
      return;
    markChanged();
  };
  for (const event of ["change", "input", "toggle"]) {
    window.addEventListener(event, stateChanged, true);
    cleanup.push(() => window.removeEventListener(event, stateChanged, true));
  }
  if (window.visualViewport) {
    for (const event of ["resize", "scroll"] as const) {
      window.visualViewport.addEventListener(event, markChanged, {
        passive: true,
      });
      cleanup.push(() =>
        window.visualViewport?.removeEventListener(event, markChanged),
      );
    }
  }

  for (const method of ["pushState", "replaceState"] as const) {
    const original = history[method];
    const replacement: History[typeof method] = (data, unused, url) => {
      const result = original.call(history, data, unused, url);
      markChanged();
      return result;
    };
    history[method] = replacement;
    cleanup.push(() => {
      history[method] = original;
    });
  }
  return () => {
    for (const dispose of cleanup.reverse()) dispose();
  };
}

function insideOverlay(node: Node): boolean {
  let current: Node | null = node;
  while (current) {
    if (
      current instanceof Element &&
      current.hasAttribute("data-a3s-testkit-overlay")
    )
      return true;
    const parent: Node | null = current.parentNode;
    const root = current.getRootNode();
    current = parent ?? (root instanceof ShadowRoot ? root.host : null);
  }
  return false;
}
