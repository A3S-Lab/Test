import { truncateUtf8 } from "./sanitize";
import {
  UI_UNDERSTANDING_PROTOCOL,
  type ContextNode,
  type ContextScope,
  type PageViewport,
  type Rect,
  type UIAnimationProfile,
  type UIComponentCluster,
  type UIEvidenceSourceKind,
  type UILayoutEdge,
  type UILayoutNode,
  type UIMotionProfile,
  type UIObservedToken,
  type UIResponsiveCondition,
  type UIStateDiff,
  type UIStyleProfile,
  type UITypographyToken,
  type UIUnderstandingSnapshot,
  type UITruncationReason,
} from "./types";
import { canObserveUIState, UIStateTracker } from "./ui-understanding-state";
import {
  finalizeUIUnderstandingSnapshot,
  uiFingerprint,
} from "./ui-understanding-budget";

const COLOR_PROPERTIES = [
  "color",
  "background-color",
  "border-top-color",
  "border-right-color",
  "border-bottom-color",
  "border-left-color",
  "outline-color",
  "text-decoration-color",
  "fill",
  "stroke",
] as const;
const SPACING_PROPERTIES = [
  "margin-top",
  "margin-right",
  "margin-bottom",
  "margin-left",
  "padding-top",
  "padding-right",
  "padding-bottom",
  "padding-left",
  "gap",
  "row-gap",
  "column-gap",
] as const;
const RADIUS_PROPERTIES = [
  "border-top-left-radius",
  "border-top-right-radius",
  "border-bottom-right-radius",
  "border-bottom-left-radius",
] as const;
const SAFE_CUSTOM_PROPERTY_NAME =
  /(?:brand|color|font|type|space|spacing|gap|radius|shadow|size|width|height|duration|motion|ease|layer|z-index|breakpoint)/i;
const SENSITIVE_CUSTOM_PROPERTY_NAME =
  /(?:^|[-_])(?:token|secret|password|auth|session|cookie|api[-_]?key|key)(?:[-_]|$)/i;
const MAX_TOKEN_VALUES = 64;
const MAX_TOKEN_NODE_IDS = 8;
const MAX_COMPONENT_CLUSTERS = 64;
const MAX_CLUSTER_MEMBERS = 32;

export type UIUnderstandingCaptureLimits = {
  nodes: number;
  stateSamples: number;
  stringBytes: number;
  encodedBytes: number;
  durationMs: number;
};

export type UIUnderstandingIdentity = {
  idFor(element: Element): string;
};

export type UIUnderstandingCapture = {
  elements: Element[];
  nodes: ContextNode[];
  identity: UIUnderstandingIdentity;
  pageRevision: number;
  viewport: PageViewport;
  scope: ContextScope;
  limits: UIUnderstandingCaptureLimits;
  stateTracker: UIStateTracker;
};

type TokenAccumulatorValue = {
  properties: Set<string>;
  count: number;
  nodeIds: Set<string>;
};

class TokenAccumulator {
  readonly #values = new Map<string, TokenAccumulatorValue>();

  add(value: string, property: string, nodeId: string): void {
    const normalized = normalizeCss(value);
    if (!normalized || isUninformativeCssValue(normalized)) return;
    const current = this.#values.get(normalized) ?? {
      properties: new Set<string>(),
      count: 0,
      nodeIds: new Set<string>(),
    };
    current.properties.add(property);
    current.count += 1;
    if (current.nodeIds.size < MAX_TOKEN_NODE_IDS) current.nodeIds.add(nodeId);
    this.#values.set(normalized, current);
  }

  values(maxStringBytes: number): UIObservedToken[] {
    return Array.from(this.#values.entries())
      .sort(
        ([leftValue, left], [rightValue, right]) =>
          right.count - left.count || leftValue.localeCompare(rightValue),
      )
      .slice(0, MAX_TOKEN_VALUES)
      .map(([value, observation]) => ({
        value: truncateUtf8(value, maxStringBytes),
        properties: Array.from(observation.properties).sort(),
        count: observation.count,
        nodeIds: Array.from(observation.nodeIds),
        confidence: 1,
      }));
  }
}

type Sample = {
  element: Element;
  nodeId: string;
  node: ContextNode | undefined;
  style: CSSStyleDeclaration;
};

type StylesheetEvidence = {
  responsiveConditions: UIResponsiveCondition[];
  keyframeNames: string[];
  inaccessibleStyleSheets: number;
  inspected: boolean;
};

export function captureUIUnderstanding(
  input: UIUnderstandingCapture,
): UIUnderstandingSnapshot {
  const started = monotonicNow();
  const deadline = started + input.limits.durationMs;
  const nodeById = new Map(input.nodes.map((node) => [node.id, node]));
  const candidates = input.elements.filter(isUICandidate);
  const samples: Sample[] = [];
  const stateDiffs: UIStateDiff[] = [];
  const reasons = new Set<UITruncationReason>();
  const sourceKinds = new Set<UIEvidenceSourceKind>();
  let stateSamples = 0;
  let stateCandidates = 0;

  for (const element of candidates) {
    if (samples.length >= input.limits.nodes) {
      reasons.add("node_limit");
      break;
    }
    if (monotonicNow() >= deadline) {
      reasons.add("time_limit");
      break;
    }
    let style: CSSStyleDeclaration;
    try {
      style = getComputedStyle(element);
    } catch {
      continue;
    }
    if (styleValue(element, style, "display") === "none") continue;
    const nodeId = input.identity.idFor(element);
    const node = nodeById.get(nodeId);
    samples.push({ element, nodeId, node, style });
    if (canObserveUIState(element)) {
      stateCandidates += 1;
      if (stateSamples < input.limits.stateSamples) {
        stateSamples += 1;
        stateDiffs.push(
          ...input.stateTracker.observe(
            element,
            nodeId,
            style,
            node,
            input.limits.stringBytes,
          ),
        );
      }
    }
  }
  if (stateCandidates > stateSamples) reasons.add("state_sample_limit");
  if (samples.length < candidates.length && !reasons.has("time_limit"))
    reasons.add("node_limit");

  if (samples.length > 0) {
    sourceKinds.add("computed_style");
    sourceKinds.add("dom_structure");
    sourceKinds.add("layout_geometry");
  }
  if (stateSamples > 0) sourceKinds.add("accessibility_state");

  const style = styleProfile(samples, input.limits.stringBytes);
  const layout = layoutGraph(samples, input.identity, input.limits.stringBytes);
  const components = componentClusters(samples, input.limits.stringBytes);
  const motion = motionProfile(samples, input.limits.stringBytes);
  const stylesheet = stylesheetEvidence(deadline, input.limits.stringBytes);
  style.responsiveConditions = stylesheet.responsiveConditions;
  motion.keyframeNames = stylesheet.keyframeNames;
  if (stylesheet.inspected || stylesheet.inaccessibleStyleSheets > 0)
    sourceKinds.add("css_stylesheet");
  if (
    motion.animations.some((animation) =>
      animation.sources.includes("web_animations"),
    )
  )
    sourceKinds.add("web_animations");
  if (monotonicNow() >= deadline) reasons.add("time_limit");

  const durationMs = Math.max(0, Math.ceil(monotonicNow() - started));
  const snapshot: UIUnderstandingSnapshot = {
    protocol: UI_UNDERSTANDING_PROTOCOL,
    observationId: `ui-${input.pageRevision}-0000000000000000`,
    pageRevision: input.pageRevision,
    viewport: structuredClone(input.viewport),
    scope: structuredClone(input.scope),
    budget: {
      limits: { ...input.limits },
      used: {
        nodes: samples.length,
        stateSamples,
        encodedBytes: 0,
        durationMs,
      },
      truncated: reasons.size > 0,
      reasons: Array.from(reasons),
    },
    evidence: {
      sourceKinds: Array.from(sourceKinds),
      sampledNodeIds: samples.slice(0, 64).map((sample) => sample.nodeId),
      totalCandidateNodes: candidates.length,
      omittedNodes: Math.max(0, candidates.length - samples.length),
      inaccessibleStyleSheets: stylesheet.inaccessibleStyleSheets,
    },
    style,
    layout,
    components,
    stateDiffs: stateDiffs.slice(0, input.limits.stateSamples),
    motion,
  };
  finalizeUIUnderstandingSnapshot(snapshot, input.limits.encodedBytes);
  return snapshot;
}

function styleProfile(
  samples: Sample[],
  maxStringBytes: number,
): UIStyleProfile {
  const colors = new TokenAccumulator();
  const spacing = new TokenAccumulator();
  const radii = new TokenAccumulator();
  const shadows = new TokenAccumulator();
  const zIndices = new TokenAccumulator();
  const typography = new Map<
    string,
    {
      value: Omit<UITypographyToken, "count" | "nodeIds" | "confidence">;
      count: number;
      nodeIds: Set<string>;
    }
  >();
  for (const sample of samples) {
    for (const property of COLOR_PROPERTIES)
      colors.add(
        styleValue(sample.element, sample.style, property),
        property,
        sample.nodeId,
      );
    for (const property of SPACING_PROPERTIES)
      spacing.add(
        styleValue(sample.element, sample.style, property),
        property,
        sample.nodeId,
      );
    for (const property of RADIUS_PROPERTIES)
      radii.add(
        styleValue(sample.element, sample.style, property),
        property,
        sample.nodeId,
      );
    for (const property of ["box-shadow", "text-shadow"])
      shadows.add(
        styleValue(sample.element, sample.style, property),
        property,
        sample.nodeId,
      );
    zIndices.add(
      styleValue(sample.element, sample.style, "z-index"),
      "z-index",
      sample.nodeId,
    );
    const value = {
      family: normalizeCss(
        styleValue(sample.element, sample.style, "font-family"),
      ),
      size: normalizeCss(styleValue(sample.element, sample.style, "font-size")),
      weight: normalizeCss(
        styleValue(sample.element, sample.style, "font-weight"),
      ),
      lineHeight: normalizeCss(
        styleValue(sample.element, sample.style, "line-height"),
      ),
      letterSpacing: normalizeCss(
        styleValue(sample.element, sample.style, "letter-spacing"),
      ),
    };
    if (value.family || value.size || value.weight) {
      const key = JSON.stringify(value);
      const current = typography.get(key) ?? {
        value,
        count: 0,
        nodeIds: new Set<string>(),
      };
      current.count += 1;
      if (current.nodeIds.size < MAX_TOKEN_NODE_IDS)
        current.nodeIds.add(sample.nodeId);
      typography.set(key, current);
    }
  }
  return {
    colors: colors.values(maxStringBytes),
    typography: Array.from(typography.values())
      .sort(
        (left, right) =>
          right.count - left.count ||
          JSON.stringify(left.value).localeCompare(JSON.stringify(right.value)),
      )
      .slice(0, 32)
      .map((entry) => ({
        family: truncateUtf8(entry.value.family, maxStringBytes),
        size: truncateUtf8(entry.value.size, maxStringBytes),
        weight: truncateUtf8(entry.value.weight, maxStringBytes),
        lineHeight: truncateUtf8(entry.value.lineHeight, maxStringBytes),
        letterSpacing: truncateUtf8(entry.value.letterSpacing, maxStringBytes),
        count: entry.count,
        nodeIds: Array.from(entry.nodeIds),
        confidence: 1,
      })),
    spacing: spacing.values(maxStringBytes),
    radii: radii.values(maxStringBytes),
    shadows: shadows.values(maxStringBytes),
    zIndices: zIndices.values(maxStringBytes),
    customProperties: customProperties(maxStringBytes),
    responsiveConditions: [],
  };
}

function customProperties(
  maxStringBytes: number,
): UIStyleProfile["customProperties"] {
  const root = document.documentElement;
  const names = new Set<string>();
  for (const declaration of [root.style, getComputedStyle(root)]) {
    for (let index = 0; index < declaration.length; index += 1) {
      const name = declaration.item(index);
      if (name.startsWith("--")) names.add(name);
    }
  }
  return Array.from(names)
    .sort()
    .filter(
      (name) =>
        SAFE_CUSTOM_PROPERTY_NAME.test(name) &&
        !SENSITIVE_CUSTOM_PROPERTY_NAME.test(name),
    )
    .slice(0, 64)
    .flatMap((name) => {
      const value = normalizeCss(getComputedStyle(root).getPropertyValue(name));
      if (!safeCustomPropertyValue(name, value)) return [];
      return [
        {
          name: truncateUtf8(name, 256),
          value: truncateUtf8(value, maxStringBytes),
          source: "document_root" as const,
          confidence: 1 as const,
        },
      ];
    });
}

function layoutGraph(
  samples: Sample[],
  identity: UIUnderstandingIdentity,
  maxStringBytes: number,
): { nodes: UILayoutNode[]; edges: UILayoutEdge[] } {
  const nodes: UILayoutNode[] = [];
  const edges: UILayoutEdge[] = [];
  const edgeKeys = new Set<string>();
  for (const sample of samples) {
    const { element, nodeId, style } = sample;
    const parent = composedParent(element);
    const parentNodeId = parent ? identity.idFor(parent) : undefined;
    const display =
      boundedStyleValue(element, style, "display", maxStringBytes) || "inline";
    const position =
      boundedStyleValue(element, style, "position", maxStringBytes) || "static";
    const rect = element.getBoundingClientRect();
    const node: UILayoutNode = {
      nodeId,
      ...(parentNodeId ? { parentNodeId } : {}),
      display,
      position,
      ...(rect.width > 0 || rect.height > 0 ? { rect: rectValue(rect) } : {}),
      overflowX:
        boundedStyleValue(element, style, "overflow-x", maxStringBytes) ||
        "visible",
      overflowY:
        boundedStyleValue(element, style, "overflow-y", maxStringBytes) ||
        "visible",
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
    if (scrollContainer)
      addEdge(
        edges,
        edgeKeys,
        identity.idFor(scrollContainer),
        nodeId,
        "scroll_container",
      );
    if (
      element instanceof HTMLElement &&
      element.offsetParent instanceof Element
    ) {
      const offsetParentId = identity.idFor(element.offsetParent);
      if (offsetParentId !== parentNodeId)
        addEdge(edges, edgeKeys, offsetParentId, nodeId, "offset_parent");
    }
  }
  return { nodes, edges };
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

function componentClusters(
  samples: Sample[],
  maxStringBytes: number,
): UIComponentCluster[] {
  const byElement = new Map(samples.map((sample) => [sample.element, sample]));
  const clusters = new Map<string, { signature: string; nodeIds: string[] }>();
  for (const sample of samples) {
    const signature = structureSignature(sample.element, byElement, 0);
    const fingerprint = uiFingerprint(signature);
    const current = clusters.get(fingerprint) ?? { signature, nodeIds: [] };
    current.nodeIds.push(sample.nodeId);
    clusters.set(fingerprint, current);
  }
  return Array.from(clusters.entries())
    .filter(([, cluster]) => cluster.nodeIds.length >= 2)
    .sort(
      ([leftHash, left], [rightHash, right]) =>
        right.nodeIds.length - left.nodeIds.length ||
        leftHash.localeCompare(rightHash),
    )
    .slice(0, MAX_COMPONENT_CLUSTERS)
    .map(([fingerprint, cluster]) => ({
      id: `cluster-${fingerprint}`,
      fingerprint,
      signature: truncateUtf8(
        cluster.signature,
        Math.min(maxStringBytes, 1_024),
      ),
      representativeNodeId: cluster.nodeIds[0]!,
      memberNodeIds: cluster.nodeIds.slice(0, MAX_CLUSTER_MEMBERS),
      memberCount: cluster.nodeIds.length,
      confidence: 1,
    }));
}

function structureSignature(
  element: Element,
  samples: Map<Element, Sample>,
  depth: number,
): string {
  const sample = samples.get(element);
  const role =
    sample?.node?.role ?? element.getAttribute("role") ?? implicitRole(element);
  const style = sample?.style;
  const styleSignature = style
    ? [
        styleValue(element, style, "display"),
        styleValue(element, style, "position"),
        styleValue(element, style, "font-size"),
        styleValue(element, style, "font-weight"),
        styleValue(element, style, "border-radius"),
      ].join("|")
    : "";
  const semantic = [
    element.tagName.toLowerCase(),
    role,
    element instanceof HTMLInputElement ? element.type : "",
    element.hasAttribute("aria-expanded") ? "expandable" : "",
    element.hasAttribute("aria-checked") ? "checkable" : "",
    styleSignature,
  ].join(":");
  if (depth >= 3) return semantic;
  const children = composedChildren(element)
    .slice(0, 8)
    .map((child) => structureSignature(child, samples, depth + 1));
  return `${semantic}[${children.join(",")}]`;
}

function motionProfile(
  samples: Sample[],
  maxStringBytes: number,
): UIMotionProfile {
  const transitions: UIMotionProfile["transitions"] = [];
  const animations: UIAnimationProfile[] = [];
  const stickyNodeIds: string[] = [];
  const scrollContainerNodeIds: string[] = [];
  const canvasNodeIds: string[] = [];
  const mediaNodeIds: string[] = [];
  for (const sample of samples) {
    const { element, nodeId, style } = sample;
    const position = styleValue(element, style, "position");
    if (position === "sticky") stickyNodeIds.push(nodeId);
    if (isScrollContainer(element, style)) scrollContainerNodeIds.push(nodeId);
    if (element instanceof HTMLCanvasElement) canvasNodeIds.push(nodeId);
    if (element instanceof HTMLMediaElement) mediaNodeIds.push(nodeId);

    const transitionShorthand = styleValue(element, style, "transition");
    const transitionProperty = styleValue(
      element,
      style,
      "transition-property",
    );
    const transitionDuration = styleValue(
      element,
      style,
      "transition-duration",
    );
    if (
      (transitionShorthand && transitionShorthand !== "none") ||
      hasNonZeroTime(transitionDuration)
    ) {
      transitions.push({
        nodeId,
        properties: boundedCssList(
          transitionProperty || transitionShorthand,
          maxStringBytes,
        ),
        durations: splitCssList(
          transitionDuration ||
            timesFromShorthand(transitionShorthand)[0] ||
            "0s",
        ),
        delays: splitCssList(
          styleValue(element, style, "transition-delay") ||
            timesFromShorthand(transitionShorthand)[1] ||
            "0s",
        ),
        timingFunctions: boundedCssList(
          styleValue(element, style, "transition-timing-function") || "ease",
          maxStringBytes,
        ),
      });
    }

    const animationName = styleValue(element, style, "animation-name");
    const animationDuration = styleValue(element, style, "animation-duration");
    const webAnimations = safeAnimations(element);
    if (
      (animationName && animationName !== "none") ||
      hasNonZeroTime(animationDuration) ||
      webAnimations.length > 0
    ) {
      const sources: UIAnimationProfile["sources"] = [];
      if (
        (animationName && animationName !== "none") ||
        hasNonZeroTime(animationDuration)
      )
        sources.push("css");
      if (webAnimations.length > 0) sources.push("web_animations");
      animations.push({
        nodeId,
        names: boundedCssList(
          animationName || "(web-animation)",
          maxStringBytes,
        ),
        durations: splitCssList(animationDuration || "0s"),
        delays: splitCssList(
          styleValue(element, style, "animation-delay") || "0s",
        ),
        iterationCounts: boundedCssList(
          styleValue(element, style, "animation-iteration-count") || "1",
          maxStringBytes,
        ),
        playStates: Array.from(
          new Set([
            ...splitCssList(
              styleValue(element, style, "animation-play-state") || "running",
            ),
            ...webAnimations.map((animation) => animation.playState),
          ]),
        ).map((value) => truncateUtf8(value, maxStringBytes)),
        sources,
      });
    }
  }
  return {
    prefersReducedMotion:
      typeof matchMedia === "function" &&
      matchMedia("(prefers-reduced-motion: reduce)").matches,
    transitions: transitions.slice(0, 64),
    animations: animations.slice(0, 64),
    keyframeNames: [],
    stickyNodeIds: stickyNodeIds.slice(0, 64),
    scrollContainerNodeIds: scrollContainerNodeIds.slice(0, 64),
    canvasNodeIds: canvasNodeIds.slice(0, 64),
    mediaNodeIds: mediaNodeIds.slice(0, 64),
  };
}

function stylesheetEvidence(
  deadline: number,
  maxStringBytes: number,
): StylesheetEvidence {
  const responsiveConditions: UIResponsiveCondition[] = [];
  const keyframeNames = new Set<string>();
  let inaccessibleStyleSheets = 0;
  let inspected = false;
  const visit = (rules: CSSRuleList): void => {
    for (const rule of Array.from(rules)) {
      if (monotonicNow() >= deadline) return;
      if (rule.type === 4 && "conditionText" in rule && "cssRules" in rule) {
        const condition = truncateUtf8(
          String((rule as CSSMediaRule).conditionText),
          maxStringBytes,
        );
        if (
          condition &&
          responsiveConditions.length < 64 &&
          typeof matchMedia === "function"
        ) {
          responsiveConditions.push({
            condition,
            matches: matchMedia(condition).matches,
            source: "stylesheet",
            confidence: 1,
          });
        }
        visit((rule as CSSMediaRule).cssRules);
      } else if (rule.type === 7 && "name" in rule) {
        keyframeNames.add(
          truncateUtf8(String((rule as CSSKeyframesRule).name), maxStringBytes),
        );
      } else if ("cssRules" in rule) {
        visit((rule as CSSGroupingRule).cssRules);
      }
    }
  };
  for (const sheet of Array.from(document.styleSheets)) {
    if (monotonicNow() >= deadline) break;
    try {
      inspected = true;
      visit(sheet.cssRules);
    } catch {
      inaccessibleStyleSheets += 1;
    }
  }
  return {
    responsiveConditions,
    keyframeNames: Array.from(keyframeNames).sort().slice(0, 64),
    inaccessibleStyleSheets,
    inspected,
  };
}

function isUICandidate(element: Element): boolean {
  if (element.hasAttribute("data-a3s-testkit-overlay")) return false;
  const tag = element.tagName.toLowerCase();
  if (
    [
      "head",
      "link",
      "meta",
      "noscript",
      "script",
      "style",
      "template",
    ].includes(tag)
  )
    return false;
  const rect = element.getBoundingClientRect();
  return (
    element === document.documentElement ||
    element === document.body ||
    rect.width > 0 ||
    rect.height > 0
  );
}

function normalizeCss(value: string): string {
  return value.trim().replace(/\s+/g, " ");
}

function isUninformativeCssValue(value: string): boolean {
  const compact = value.toLowerCase().replace(/\s/g, "");
  return (
    [
      "none",
      "normal",
      "auto",
      "transparent",
      "currentcolor",
      "0",
      "0px",
    ].includes(compact) || /^rgba?\(0,?0,?0,?0(?:\.0+)?\)$/.test(compact)
  );
}

function styleValue(
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

function boundedStyleValue(
  element: Element,
  style: CSSStyleDeclaration,
  property: string,
  maxStringBytes: number,
): string {
  return truncateUtf8(styleValue(element, style, property), maxStringBytes);
}

function safeCustomPropertyValue(name: string, value: string): boolean {
  if (!value || value.length > 1_024) return false;
  if (/url\s*\(|javascript:|data:|[\r\n]/i.test(value)) return false;
  if (/(?:brand|color)/i.test(name))
    return /^(?:#[\da-f]{3,8}|(?:rgb|hsl|hwb|lab|lch|oklab|oklch|color)\()/i.test(
      value,
    );
  if (
    /(?:space|spacing|gap|radius|size|width|height|duration|layer|z-index|breakpoint)/i.test(
      name,
    )
  )
    return /^(?:-?(?:\d+|\d*\.\d+)(?:px|r?em|vh|vw|vmin|vmax|ch|ex|ms|s|%)?)(?:(?:\s+|,\s*)-?(?:\d+|\d*\.\d+)(?:px|r?em|vh|vw|vmin|vmax|ch|ex|ms|s|%)?)*$/i.test(
      value,
    );
  if (/(?:motion|ease)/i.test(name))
    return /^(?:linear|ease(?:-in|-out|-in-out)?|cubic-bezier\([\d.,\s-]+\)|steps\([\d\w,\s-]+\))$/i.test(
      value,
    );
  if (/shadow/i.test(name))
    return (
      /\d/.test(value) &&
      /(?:#[\da-f]{3,8}|(?:rgb|hsl|oklch|oklab)\()/i.test(value)
    );
  return false;
}

function rectValue(rect: DOMRect | DOMRectReadOnly): Rect {
  return {
    x: rect.x,
    y: rect.y,
    width: rect.width,
    height: rect.height,
  };
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

function composedParent(element: Element): Element | null {
  if (element.parentElement) return element.parentElement;
  const root = element.getRootNode();
  return root instanceof ShadowRoot ? root.host : null;
}

function composedChildren(element: Element): Element[] {
  return [
    ...Array.from(element.children),
    ...(element.shadowRoot ? Array.from(element.shadowRoot.children) : []),
  ];
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

function isScrollContainer(
  element: Element,
  style: CSSStyleDeclaration,
): boolean {
  return /(auto|scroll|overlay|hidden)/.test(
    `${styleValue(element, style, "overflow-x")} ${styleValue(element, style, "overflow-y")}`,
  );
}

function implicitRole(element: Element): string {
  const tag = element.tagName.toLowerCase();
  if (tag === "a" && element.hasAttribute("href")) return "link";
  if (tag === "button") return "button";
  if (/^h[1-6]$/.test(tag)) return "heading";
  if (tag === "input") return "textbox";
  if (tag === "nav") return "navigation";
  if (tag === "main") return "main";
  return "";
}

function splitCssList(value: string): string[] {
  return value.split(",").map(normalizeCss).filter(Boolean).slice(0, 16);
}

function boundedCssList(value: string, maxStringBytes: number): string[] {
  return splitCssList(value).map((part) => truncateUtf8(part, maxStringBytes));
}

function hasNonZeroTime(value: string): boolean {
  return splitCssList(value).some((part) => {
    const match = part.match(/^(-?[\d.]+)(ms|s)$/);
    return match ? Number(match[1]) !== 0 : false;
  });
}

function timesFromShorthand(
  value: string,
): [string | undefined, string | undefined] {
  const times = value.match(/-?[\d.]+m?s/g) ?? [];
  return [times[0], times[1]];
}

function safeAnimations(element: Element): Animation[] {
  if (typeof element.getAnimations !== "function") return [];
  try {
    return element.getAnimations({ subtree: false }).slice(0, 16);
  } catch {
    return [];
  }
}

function monotonicNow(): number {
  return globalThis.performance?.now?.() ?? Date.now();
}
