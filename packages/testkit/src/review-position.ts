import { markerRects } from "./review-utils";
import type { PageContextBridge, Rect, RepairTarget } from "./types";

export type ReviewEditorPlacement = {
  rect: Rect;
  side: "left" | "right";
  left: number;
  top: number;
};

export function reviewEditorPlacement(
  target: RepairTarget,
  bridge: PageContextBridge,
): ReviewEditorPlacement | null {
  if (typeof window === "undefined") return null;
  const rect = targetRect(target, bridge);
  if (!rect) return null;

  const gutter = 12;
  const gap = 14;
  const width = Math.min(360, window.innerWidth - gutter * 2);
  const estimatedHeight = Math.min(430, window.innerHeight - gutter * 2);
  const roomOnRight = window.innerWidth - (rect.x + rect.width);
  const side = roomOnRight >= width + gap ? "right" : "left";
  const preferredLeft = side === "right"
    ? rect.x + rect.width + gap
    : rect.x - width - gap;
  const left = clamp(preferredLeft, gutter, window.innerWidth - width - gutter);
  const top = clamp(rect.y, gutter, window.innerHeight - estimatedHeight - gutter);

  return { rect, side, left, top };
}

function targetRect(
  target: RepairTarget,
  bridge: PageContextBridge,
): Rect | null {
  const rects = markerRects(target, bridge);
  if (rects.length === 0) return null;
  const left = Math.min(...rects.map((rect) => rect.x));
  const top = Math.min(...rects.map((rect) => rect.y));
  const right = Math.max(...rects.map((rect) => rect.x + rect.width));
  const bottom = Math.max(...rects.map((rect) => rect.y + rect.height));
  return { x: left, y: top, width: right - left, height: bottom - top };
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(Math.max(value, minimum), Math.max(minimum, maximum));
}
