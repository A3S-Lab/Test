import { truncateUtf8 } from "./sanitize";
import {
  UI_UNDERSTANDING_PROTOCOL,
  type ContextNode,
  type ContextScope,
  type PageViewport,
  type UIComponentCluster,
  type UIEvidenceSourceKind,
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
  composedChildren,
  monotonicNow,
  normalizeCss,
  styleValue,
  type UISample,
  type UIUnderstandingIdentity,
} from "./ui-understanding-dom";
import { captureUILayoutGraph } from "./ui-understanding-layout";
import { captureUIMotionProfile } from "./ui-understanding-motion";
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

export type { UIUnderstandingIdentity } from "./ui-understanding-dom";

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
  const samples: UISample[] = [];
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
  const layout = captureUILayoutGraph(samples, input.limits.stringBytes);
  const components = componentClusters(samples, input.limits.stringBytes);
  const motion = captureUIMotionProfile(samples, input.limits.stringBytes);
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
  samples: UISample[],
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

function componentClusters(
  samples: UISample[],
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
  samples: Map<Element, UISample>,
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
