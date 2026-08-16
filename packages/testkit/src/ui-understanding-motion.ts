import { truncateUtf8 } from "./sanitize";
import type {
  UIAnimationProfile,
  UIAnimationTimeline,
  UIAnimationTimelineKind,
  UIMotionProfile,
} from "./types";
import {
  isScrollContainer,
  normalizeCss,
  styleValue,
  type UISample,
} from "./ui-understanding-dom";

export function captureUIMotionProfile(
  samples: UISample[],
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
        timelines: animationTimelines(
          element,
          style,
          webAnimations,
          maxStringBytes,
        ),
        rangeStarts: boundedCssList(
          styleValue(element, style, "animation-range-start") || "normal",
          maxStringBytes,
        ),
        rangeEnds: boundedCssList(
          styleValue(element, style, "animation-range-end") || "normal",
          maxStringBytes,
        ),
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

function animationTimelines(
  element: Element,
  style: CSSStyleDeclaration,
  webAnimations: Animation[],
  maxStringBytes: number,
): UIAnimationTimeline[] {
  const result: UIAnimationTimeline[] = boundedCssList(
    styleValue(element, style, "animation-timeline") || "auto",
    maxStringBytes,
  ).map((value) => ({
    value,
    kind: cssTimelineKind(value),
    source: "computed_style",
  }));
  for (const animation of webAnimations) {
    const kind = webAnimationTimelineKind(animation.timeline);
    result.push({
      value: timelineLabel(kind),
      kind,
      source: "web_animations",
    });
  }
  return Array.from(
    new Map(
      result.map((timeline) => [
        `${timeline.source}:${timeline.kind}:${timeline.value}`,
        timeline,
      ]),
    ).values(),
  ).slice(0, 32);
}

function cssTimelineKind(value: string): UIAnimationTimelineKind {
  const normalized = value.toLowerCase();
  if (normalized === "auto") return "document";
  if (normalized === "none") return "none";
  if (normalized.startsWith("scroll(")) return "scroll";
  if (normalized.startsWith("view(")) return "view";
  if (/^--[-_a-z0-9]+$/i.test(normalized)) return "named";
  return "unknown";
}

function webAnimationTimelineKind(
  timeline: AnimationTimeline | null,
): UIAnimationTimelineKind {
  if (timeline === null) return "none";
  const name = timeline.constructor?.name.toLowerCase() ?? "";
  if (name === "documenttimeline") return "document";
  if (name === "scrolltimeline") return "scroll";
  if (name === "viewtimeline") return "view";
  return "unknown";
}

function timelineLabel(kind: UIAnimationTimelineKind): string {
  switch (kind) {
    case "document":
      return "(document-timeline)";
    case "scroll":
      return "(scroll-timeline)";
    case "view":
      return "(view-timeline)";
    case "none":
      return "none";
    default:
      return "(custom-timeline)";
  }
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
