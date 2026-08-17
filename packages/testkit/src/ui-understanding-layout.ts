import type {
  UIBoxEdges,
  UIBoxModel,
  UILayoutEdge,
  UILayoutGraph,
  UILayoutNode,
  UIOverflowMetrics,
} from "./types";
import {
  boundedStyleValue,
  composedParent,
  isScrollContainer,
  rectValue,
  styleValue,
  type UISample,
} from "./ui-understanding-dom";

export function captureUILayoutGraph(
  samples: UISample[],
  maxStringBytes: number,
): UILayoutGraph {
  const nodes: UILayoutNode[] = [];
  const edges: UILayoutEdge[] = [];
  const edgeKeys = new Set<string>();
  const samplesByElement = new Map(
    samples.map((sample) => [sample.element, sample] as const),
  );
  for (const sample of samples) {
    const { element, nodeId, style } = sample;
    const parentNodeId = nearestSampledAncestor(
      element,
      samplesByElement,
    )?.nodeId;
    const display =
      boundedStyleValue(element, style, "display", maxStringBytes) || "inline";
    const position =
      boundedStyleValue(element, style, "position", maxStringBytes) || "static";
    const overflowX =
      boundedStyleValue(element, style, "overflow-x", maxStringBytes) ||
      "visible";
    const overflowY =
      boundedStyleValue(element, style, "overflow-y", maxStringBytes) ||
      "visible";
    const rect = element.getBoundingClientRect();
    const node: UILayoutNode = {
      nodeId,
      ...(parentNodeId ? { parentNodeId } : {}),
      display,
      position,
      ...(rect.width > 0 || rect.height > 0 ? { rect: rectValue(rect) } : {}),
      overflowX,
      overflowY,
      overflowMetrics: captureOverflowMetrics(element, overflowX, overflowY),
      boxModel: captureBoxModel(element, style, maxStringBytes),
      order: boundedStyleValue(element, style, "order", maxStringBytes) || "0",
      stackingContextReasons: stackingContextReasons(element, style),
    };
    if (display.includes("flex")) {
      node.flex = {
        direction:
          boundedStyleValue(element, style, "flex-direction", maxStringBytes) ||
          "row",
        wrap:
          boundedStyleValue(element, style, "flex-wrap", maxStringBytes) ||
          "nowrap",
        justifyContent:
          boundedStyleValue(
            element,
            style,
            "justify-content",
            maxStringBytes,
          ) || "normal",
        alignItems:
          boundedStyleValue(element, style, "align-items", maxStringBytes) ||
          "normal",
        alignContent:
          boundedStyleValue(element, style, "align-content", maxStringBytes) ||
          "normal",
        gap:
          boundedStyleValue(element, style, "gap", maxStringBytes) || "normal",
      };
    }
    if (display.includes("grid")) {
      node.grid = {
        templateColumns:
          boundedStyleValue(
            element,
            style,
            "grid-template-columns",
            maxStringBytes,
          ) || "none",
        templateRows:
          boundedStyleValue(
            element,
            style,
            "grid-template-rows",
            maxStringBytes,
          ) || "none",
        autoFlow:
          boundedStyleValue(element, style, "grid-auto-flow", maxStringBytes) ||
          "row",
        justifyItems:
          boundedStyleValue(element, style, "justify-items", maxStringBytes) ||
          "normal",
        alignItems:
          boundedStyleValue(element, style, "align-items", maxStringBytes) ||
          "normal",
        gap:
          boundedStyleValue(element, style, "gap", maxStringBytes) || "normal",
      };
    }
    nodes.push(node);
    if (parentNodeId)
      addEdge(edges, edgeKeys, parentNodeId, nodeId, "contains");
    const scrollContainer = nearestScrollContainer(element);
    const scrollContainerId = scrollContainer
      ? samplesByElement.get(scrollContainer)?.nodeId
      : undefined;
    if (scrollContainerId)
      addEdge(edges, edgeKeys, scrollContainerId, nodeId, "scroll_container");
    if (
      element instanceof HTMLElement &&
      element.offsetParent instanceof Element
    ) {
      const offsetParentId = samplesByElement.get(element.offsetParent)?.nodeId;
      if (offsetParentId && offsetParentId !== parentNodeId)
        addEdge(edges, edgeKeys, offsetParentId, nodeId, "offset_parent");
    }
  }
  return { nodes, edges };
}

function nearestSampledAncestor(
  element: Element,
  samplesByElement: ReadonlyMap<Element, UISample>,
): UISample | undefined {
  let current = composedParent(element);
  while (current) {
    const sample = samplesByElement.get(current);
    if (sample) return sample;
    current = composedParent(current);
  }
  return undefined;
}

function captureBoxModel(
  element: Element,
  style: CSSStyleDeclaration,
  maxStringBytes: number,
): UIBoxModel {
  return {
    boxSizing: boxSizing(
      boundedStyleValue(element, style, "box-sizing", maxStringBytes),
    ),
    writingMode: writingMode(
      boundedStyleValue(element, style, "writing-mode", maxStringBytes),
    ),
    direction: textDirection(
      boundedStyleValue(element, style, "direction", maxStringBytes),
    ),
    margin: captureBoxEdges(element, style, "margin-", "", maxStringBytes),
    borderWidth: captureBoxEdges(
      element,
      style,
      "border-",
      "-width",
      maxStringBytes,
    ),
    padding: captureBoxEdges(element, style, "padding-", "", maxStringBytes),
  };
}

function captureBoxEdges(
  element: Element,
  style: CSSStyleDeclaration,
  prefix: string,
  suffix: string,
  maxStringBytes: number,
): UIBoxEdges {
  const value = (side: keyof UIBoxEdges): string =>
    boundedStyleValue(
      element,
      style,
      `${prefix}${side}${suffix}`,
      maxStringBytes,
    ) || "0px";
  return {
    top: value("top"),
    right: value("right"),
    bottom: value("bottom"),
    left: value("left"),
  };
}

function boxSizing(value: string): UIBoxModel["boxSizing"] {
  if (!value) return "content-box";
  if (value === "content-box" || value === "border-box") return value;
  return "unknown";
}

function writingMode(value: string): UIBoxModel["writingMode"] {
  if (!value) return "horizontal-tb";
  if (
    value === "horizontal-tb" ||
    value === "vertical-rl" ||
    value === "vertical-lr" ||
    value === "sideways-rl" ||
    value === "sideways-lr"
  )
    return value;
  return "unknown";
}

function textDirection(value: string): UIBoxModel["direction"] {
  if (!value) return "ltr";
  if (value === "ltr" || value === "rtl") return value;
  return "unknown";
}

function captureOverflowMetrics(
  element: Element,
  overflowX: string,
  overflowY: string,
): UIOverflowMetrics {
  const clientWidth = nonNegativeFinite(element.clientWidth);
  const clientHeight = nonNegativeFinite(element.clientHeight);
  const scrollWidth = Math.max(
    clientWidth,
    nonNegativeFinite(element.scrollWidth),
  );
  const scrollHeight = Math.max(
    clientHeight,
    nonNegativeFinite(element.scrollHeight),
  );
  const overflowingX = scrollWidth > clientWidth;
  const overflowingY = scrollHeight > clientHeight;
  return {
    clientWidth,
    clientHeight,
    scrollWidth,
    scrollHeight,
    scrollLeft: finiteOrZero(element.scrollLeft),
    scrollTop: finiteOrZero(element.scrollTop),
    overflowingX,
    overflowingY,
    clipsX: overflowingX && overflowX !== "visible",
    clipsY: overflowingY && overflowY !== "visible",
  };
}

function finiteOrZero(value: number): number {
  return Number.isFinite(value) ? value : 0;
}

function nonNegativeFinite(value: number): number {
  return Math.max(0, finiteOrZero(value));
}

function addEdge(
  edges: UILayoutEdge[],
  keys: Set<string>,
  fromNodeId: string,
  toNodeId: string,
  relation: UILayoutEdge["relation"],
): void {
  const key = `${relation}:${fromNodeId}:${toNodeId}`;
  if (keys.has(key)) return;
  keys.add(key);
  edges.push({ fromNodeId, toNodeId, relation });
}

function stackingContextReasons(
  element: Element,
  style: CSSStyleDeclaration,
): string[] {
  const result: string[] = [];
  if (element === document.documentElement) result.push("root");
  const position = styleValue(element, style, "position");
  const zIndex = styleValue(element, style, "z-index");
  if (["fixed", "sticky"].includes(position)) result.push(position);
  if (
    ["absolute", "relative"].includes(position) &&
    zIndex &&
    zIndex !== "auto"
  )
    result.push("positioned_z_index");
  const opacity = Number.parseFloat(styleValue(element, style, "opacity"));
  if (Number.isFinite(opacity) && opacity < 1) result.push("opacity");
  if (!isNone(styleValue(element, style, "transform")))
    result.push("transform");
  if (!isNone(styleValue(element, style, "filter"))) result.push("filter");
  if (styleValue(element, style, "isolation") === "isolate")
    result.push("isolation");
  if (!isNone(styleValue(element, style, "mix-blend-mode"), "normal"))
    result.push("blend_mode");
  if (
    /transform|opacity|filter/.test(styleValue(element, style, "will-change"))
  )
    result.push("will_change");
  return result;
}

function isNone(value: string, defaultValue = "none"): boolean {
  return !value || value === defaultValue;
}

function nearestScrollContainer(element: Element): Element | null {
  let current = composedParent(element);
  while (current) {
    const style = getComputedStyle(current);
    if (isScrollContainer(current, style)) return current;
    current = composedParent(current);
  }
  return null;
}
