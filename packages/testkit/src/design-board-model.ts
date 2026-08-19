import {
  DESIGN_BOARD_HEIGHT,
  DESIGN_BOARD_WIDTH,
} from "./design-reference";
import type { RepairDesignReference } from "./types";

export type DesignTool = "select" | "draw" | "rectangle" | "text";

export type DesignPoint = {
  x: number;
  y: number;
};

type DesignElementBase = {
  id: string;
};

export type DesignDrawElement = DesignElementBase & {
  kind: "draw";
  points: DesignPoint[];
  color: string;
  strokeWidth: number;
};

export type DesignRectangleElement = DesignElementBase & {
  kind: "rectangle";
  x: number;
  y: number;
  width: number;
  height: number;
  color: string;
  fill: string;
  strokeWidth: number;
};

export type DesignTextElement = DesignElementBase & {
  kind: "text";
  x: number;
  y: number;
  text: string;
  color: string;
  fontSize: number;
};

export type DesignImageElement = DesignElementBase & {
  kind: "image";
  x: number;
  y: number;
  width: number;
  height: number;
  src: string;
  mediaType: "image/png" | "image/jpeg";
  referenceKind: RepairDesignReference["kind"];
  background: true;
};

export type DesignElement =
  | DesignDrawElement
  | DesignRectangleElement
  | DesignTextElement
  | DesignImageElement;

export type DesignBoardSummary = {
  kind: RepairDesignReference["kind"] | null;
  label: string;
  hasImage: boolean;
};

export type DesignHistory = {
  past: DesignElement[][];
  present: DesignElement[];
  future: DesignElement[][];
};

export type ElementBounds = {
  x: number;
  y: number;
  width: number;
  height: number;
};

export const MAX_DESIGN_ELEMENTS = 250;
const MAX_HISTORY_ENTRIES = 100;
const MIN_ELEMENT_SIZE = 12;
const DEFAULT_TEXT_WIDTH = 80;
let nextElementId = 0;

export function createDesignElementId(): string {
  nextElementId += 1;
  return `design-${Date.now().toString(36)}-${nextElementId.toString(36)}`;
}

export function createDesignHistory(elements: DesignElement[] = []): DesignHistory {
  return { past: [], present: elements, future: [] };
}

export function commitDesignHistory(
  history: DesignHistory,
  elements: DesignElement[],
): DesignHistory {
  if (elements.length > MAX_DESIGN_ELEMENTS) return history;
  if (elements === history.present || elementsEqual(elements, history.present)) return history;
  return {
    past: [...history.past.slice(-(MAX_HISTORY_ENTRIES - 1)), history.present],
    present: elements,
    future: [],
  };
}

export function undoDesignHistory(history: DesignHistory): DesignHistory {
  const previous = history.past.at(-1);
  if (!previous) return history;
  return {
    past: history.past.slice(0, -1),
    present: previous,
    future: [history.present, ...history.future].slice(0, MAX_HISTORY_ENTRIES),
  };
}

export function redoDesignHistory(history: DesignHistory): DesignHistory {
  const next = history.future[0];
  if (!next) return history;
  return {
    past: [...history.past.slice(-(MAX_HISTORY_ENTRIES - 1)), history.present],
    present: next,
    future: history.future.slice(1),
  };
}

export function summarizeBoard(elements: DesignElement[]): DesignBoardSummary {
  if (elements.length === 0) {
    return { kind: null, label: "Blank board", hasImage: false };
  }
  const images = elements.filter((element): element is DesignImageElement => element.kind === "image");
  const hasImage = images.length > 0;
  const hasAuthoredElement = elements.some((element) => element.kind !== "image");
  const hasRestoredSketch = images.some((element) => element.referenceKind === "sketch");
  const kind = hasAuthoredElement || hasRestoredSketch ? "sketch" : "screenshot";
  const label = hasAuthoredElement && hasImage
    ? "Screenshot with sketch annotations"
    : hasAuthoredElement
      ? "UI sketch"
      : hasRestoredSketch
        ? "Existing sketch"
        : "Screenshot";
  return { kind, label, hasImage };
}

export function elementBounds(element: DesignElement): ElementBounds {
  if (element.kind === "draw") {
    const xs = element.points.map((point) => point.x);
    const ys = element.points.map((point) => point.y);
    const padding = Math.max(2, element.strokeWidth / 2);
    const minX = Math.min(...xs);
    const minY = Math.min(...ys);
    const maxX = Math.max(...xs);
    const maxY = Math.max(...ys);
    return {
      x: minX - padding,
      y: minY - padding,
      width: Math.max(MIN_ELEMENT_SIZE, maxX - minX + padding * 2),
      height: Math.max(MIN_ELEMENT_SIZE, maxY - minY + padding * 2),
    };
  }
  if (element.kind === "text") {
    const lines = element.text.split("\n");
    const longest = Math.max(...lines.map((line) => line.length), 1);
    return {
      x: element.x,
      y: element.y,
      width: Math.max(DEFAULT_TEXT_WIDTH, longest * element.fontSize * 0.62),
      height: Math.max(element.fontSize * 1.3, lines.length * element.fontSize * 1.3),
    };
  }
  return {
    x: element.x,
    y: element.y,
    width: element.width,
    height: element.height,
  };
}

export function moveDesignElement(
  element: DesignElement,
  deltaX: number,
  deltaY: number,
): DesignElement {
  const bounds = elementBounds(element);
  const boundedDeltaX = clamp(deltaX, -bounds.x, DESIGN_BOARD_WIDTH - bounds.x - bounds.width);
  const boundedDeltaY = clamp(deltaY, -bounds.y, DESIGN_BOARD_HEIGHT - bounds.y - bounds.height);
  if (element.kind === "draw") {
    return {
      ...element,
      points: element.points.map((point) => ({
        x: point.x + boundedDeltaX,
        y: point.y + boundedDeltaY,
      })),
    };
  }
  return {
    ...element,
    x: element.x + boundedDeltaX,
    y: element.y + boundedDeltaY,
  };
}

export function resizeDesignElement(
  element: DesignElement,
  point: DesignPoint,
): DesignElement {
  const bounds = elementBounds(element);
  const width = clamp(point.x - bounds.x, MIN_ELEMENT_SIZE, DESIGN_BOARD_WIDTH - bounds.x);
  const height = clamp(point.y - bounds.y, MIN_ELEMENT_SIZE, DESIGN_BOARD_HEIGHT - bounds.y);
  if (element.kind === "draw") {
    const scaleX = width / Math.max(bounds.width, 1);
    const scaleY = height / Math.max(bounds.height, 1);
    return {
      ...element,
      points: element.points.map((candidate) => ({
        x: bounds.x + (candidate.x - bounds.x) * scaleX,
        y: bounds.y + (candidate.y - bounds.y) * scaleY,
      })),
    };
  }
  if (element.kind === "text") {
    const scale = height / Math.max(bounds.height, 1);
    return {
      ...element,
      fontSize: clamp(Math.round(element.fontSize * scale), 12, 72),
    };
  }
  return { ...element, width, height };
}

export function replaceDesignElement(
  elements: DesignElement[],
  replacement: DesignElement,
): DesignElement[] {
  return elements.map((element) => element.id === replacement.id ? replacement : element);
}

export function normalizeRectangle(start: DesignPoint, end: DesignPoint): ElementBounds {
  return {
    x: Math.min(start.x, end.x),
    y: Math.min(start.y, end.y),
    width: Math.abs(end.x - start.x),
    height: Math.abs(end.y - start.y),
  };
}

export function clampDesignPoint(point: DesignPoint): DesignPoint {
  return {
    x: clamp(point.x, 0, DESIGN_BOARD_WIDTH),
    y: clamp(point.y, 0, DESIGN_BOARD_HEIGHT),
  };
}

function elementsEqual(left: DesignElement[], right: DesignElement[]): boolean {
  if (left.length !== right.length) return false;
  return left.every((element, index) => element === right[index]);
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(Math.max(value, minimum), Math.max(minimum, maximum));
}
