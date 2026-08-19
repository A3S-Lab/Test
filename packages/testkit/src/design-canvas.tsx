import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type ClipboardEvent,
  type DragEvent,
  type KeyboardEvent,
  type MouseEvent,
  type PointerEvent,
} from "react";
import {
  clampDesignPoint,
  createDesignElementId,
  MAX_DESIGN_ELEMENTS,
  moveDesignElement,
  normalizeRectangle,
  replaceDesignElement,
  resizeDesignElement,
  type DesignElement,
  type DesignPoint,
  type DesignTool,
} from "./design-board-model";
import { DesignCanvasElements } from "./design-canvas-elements";
import {
  DESIGN_BOARD_HEIGHT,
  DESIGN_BOARD_WIDTH,
} from "./design-reference";

type CanvasInteraction =
  | { kind: "draw"; pointerId: number; points: DesignPoint[] }
  | { kind: "rectangle"; pointerId: number; start: DesignPoint; current: DesignPoint }
  | {
    kind: "move" | "resize";
    pointerId: number;
    id: string;
    start: DesignPoint;
    current: DesignPoint;
    original: DesignElement;
  };

type TextDraft = {
  editingId: string | null;
  point: DesignPoint;
  value: string;
};

const TOOL_SHORTCUTS: Readonly<Partial<Record<string, DesignTool>>> = {
  v: "select",
  d: "draw",
  r: "rectangle",
  t: "text",
};

export type DesignCanvasProps = {
  elements: DesignElement[];
  selectedId: string | null;
  tool: DesignTool;
  color: string;
  fill: string;
  strokeWidth: number;
  disabled: boolean;
  canUndo: boolean;
  canRedo: boolean;
  describedBy: string;
  onSelect(id: string | null): void;
  onToolChange(tool: DesignTool): void;
  onCommit(elements: DesignElement[], selectedId?: string | null): void;
  onUndo(): void;
  onRedo(): void;
  onImportFile(file: File): void;
  onLimit(): void;
};

export function DesignCanvas({
  elements,
  selectedId,
  tool,
  color,
  fill,
  strokeWidth,
  disabled,
  canUndo,
  canRedo,
  describedBy,
  onSelect,
  onToolChange,
  onCommit,
  onUndo,
  onRedo,
  onImportFile,
  onLimit,
}: DesignCanvasProps) {
  const inputRef = useRef<HTMLInputElement | null>(null);
  const interactionRef = useRef<CanvasInteraction | null>(null);
  const [interaction, setInteraction] = useState<CanvasInteraction | null>(null);
  const [textDraft, setTextDraft] = useState<TextDraft | null>(null);
  const displayElements = useMemo(
    () => interactionElements(elements, interaction, color, fill, strokeWidth),
    [color, elements, fill, interaction, strokeWidth],
  );

  useEffect(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
  }, [textDraft?.editingId, textDraft?.point.x, textDraft?.point.y]);

  function pointerDown(event: PointerEvent<SVGSVGElement>) {
    if (disabled || event.button !== 0) return;
    event.currentTarget.focus();
    const point = eventPoint(event.currentTarget, event.clientX, event.clientY);
    const target = event.target instanceof Element ? event.target : null;
    const resizeId = target?.closest("[data-resize-id]")?.getAttribute("data-resize-id") ?? null;
    const elementId = target?.closest("[data-element-id]")?.getAttribute("data-element-id") ?? null;
    if (tool === "select") {
      const id = resizeId ?? elementId;
      const original = id ? elements.find((element) => element.id === id) : undefined;
      if (!original) {
        onSelect(null);
        return;
      }
      onSelect(original.id);
      updateInteraction({
        kind: resizeId ? "resize" : "move",
        pointerId: event.pointerId,
        id: original.id,
        start: point,
        current: point,
        original,
      });
      capturePointer(event.currentTarget, event.pointerId);
      event.preventDefault();
      return;
    }
    if (tool === "text") {
      setTextDraft({
        editingId: null,
        point: {
          x: Math.min(point.x, DESIGN_BOARD_WIDTH - 280),
          y: Math.min(point.y, DESIGN_BOARD_HEIGHT - 46),
        },
        value: "",
      });
      onSelect(null);
      event.preventDefault();
      return;
    }
    if (elements.length >= MAX_DESIGN_ELEMENTS) {
      onLimit();
      return;
    }
    if (tool === "draw") {
      updateInteraction({ kind: "draw", pointerId: event.pointerId, points: [point] });
    } else {
      updateInteraction({ kind: "rectangle", pointerId: event.pointerId, start: point, current: point });
    }
    capturePointer(event.currentTarget, event.pointerId);
    event.preventDefault();
  }

  function pointerMove(event: PointerEvent<SVGSVGElement>) {
    const current = interactionRef.current;
    if (!current || current.pointerId !== event.pointerId) return;
    const point = eventPoint(event.currentTarget, event.clientX, event.clientY);
    if (current.kind === "draw") {
      const previous = current.points.at(-1)!;
      if (Math.hypot(point.x - previous.x, point.y - previous.y) >= 1.5) {
        updateInteraction({ ...current, points: [...current.points, point] });
      }
    } else {
      updateInteraction({ ...current, current: point });
    }
    event.preventDefault();
  }

  function pointerUp(event: PointerEvent<SVGSVGElement>) {
    const current = interactionRef.current;
    if (!current || current.pointerId !== event.pointerId) return;
    const point = eventPoint(event.currentTarget, event.clientX, event.clientY);
    releasePointer(event.currentTarget, event.pointerId);
    const completed = completeInteraction(elements, current, point, color, fill, strokeWidth);
    updateInteraction(null);
    if (completed) onCommit(completed.elements, completed.selectedId);
    event.preventDefault();
  }

  function pointerCancel(event: PointerEvent<SVGSVGElement>) {
    const current = interactionRef.current;
    if (!current || current.pointerId !== event.pointerId) return;
    releasePointer(event.currentTarget, event.pointerId);
    updateInteraction(null);
  }

  function doubleClick(event: MouseEvent<SVGSVGElement>) {
    const target = event.target instanceof Element ? event.target : null;
    const id = target?.closest("[data-element-id]")?.getAttribute("data-element-id");
    const element = id ? elements.find((candidate) => candidate.id === id) : undefined;
    if (!element || element.kind !== "text") return;
    setTextDraft({ editingId: element.id, point: { x: element.x, y: element.y }, value: element.text });
    onSelect(element.id);
    event.preventDefault();
  }

  function commitText() {
    if (!textDraft) return;
    const value = textDraft.value.trim();
    if (!value) {
      setTextDraft(null);
      return;
    }
    if (!textDraft.editingId && elements.length >= MAX_DESIGN_ELEMENTS) {
      setTextDraft(null);
      onLimit();
      return;
    }
    const existing = textDraft.editingId
      ? elements.find((element) => element.id === textDraft.editingId)
      : null;
    const next: DesignElement = existing?.kind === "text"
      ? { ...existing, text: value }
      : {
        id: createDesignElementId(),
        kind: "text",
        x: textDraft.point.x,
        y: textDraft.point.y,
        text: value,
        color,
        fontSize: 24,
      };
    const nextElements = existing ? replaceDesignElement(elements, next) : [...elements, next];
    setTextDraft(null);
    onToolChange("select");
    onCommit(nextElements, next.id);
  }

  function keyDown(event: KeyboardEvent<SVGSVGElement>) {
    if (event.target instanceof HTMLInputElement) return;
    const key = event.key.toLowerCase();
    if (key === "escape") {
      if (interactionRef.current) {
        updateInteraction(null);
      } else if (textDraft) {
        setTextDraft(null);
      } else if (tool !== "select") {
        onToolChange("select");
      } else if (selectedId) {
        onSelect(null);
      } else {
        return;
      }
      event.preventDefault();
      event.stopPropagation();
      return;
    }
    if ((event.metaKey || event.ctrlKey) && key === "z") {
      if (event.shiftKey ? canRedo : canUndo) event.shiftKey ? onRedo() : onUndo();
      event.preventDefault();
      event.stopPropagation();
      return;
    }
    if ((event.metaKey || event.ctrlKey) && key === "y") {
      if (canRedo) onRedo();
      event.preventDefault();
      event.stopPropagation();
      return;
    }
    if ((key === "delete" || key === "backspace") && selectedId) {
      onCommit(elements.filter((element) => element.id !== selectedId), null);
      event.preventDefault();
      event.stopPropagation();
      return;
    }
    if (selectedId && ["arrowup", "arrowdown", "arrowleft", "arrowright"].includes(key)) {
      const selected = elements.find((element) => element.id === selectedId);
      if (!selected) return;
      const distance = event.shiftKey ? 10 : 1;
      const deltaX = key === "arrowleft" ? -distance : key === "arrowright" ? distance : 0;
      const deltaY = key === "arrowup" ? -distance : key === "arrowdown" ? distance : 0;
      onCommit(replaceDesignElement(elements, moveDesignElement(selected, deltaX, deltaY)), selected.id);
      event.preventDefault();
      event.stopPropagation();
      return;
    }
    const shortcut = TOOL_SHORTCUTS[key];
    if (!event.metaKey && !event.ctrlKey && !event.altKey && shortcut) {
      onToolChange(shortcut);
      event.preventDefault();
      event.stopPropagation();
    }
  }

  function updateInteraction(next: CanvasInteraction | null) {
    interactionRef.current = next;
    setInteraction(next);
  }

  function paste(event: ClipboardEvent<SVGSVGElement>) {
    const file = Array.from(event.clipboardData.files).find((candidate) => candidate.type.startsWith("image/"));
    if (!file) return;
    event.preventDefault();
    onImportFile(file);
  }

  function drop(event: DragEvent<SVGSVGElement>) {
    const file = Array.from(event.dataTransfer.files).find((candidate) => candidate.type.startsWith("image/"));
    if (!file) return;
    event.preventDefault();
    onImportFile(file);
  }

  return <svg
    className="a3s-design-canvas-surface"
    data-testid="design-canvas"
    data-tool={tool}
    viewBox={`0 0 ${DESIGN_BOARD_WIDTH} ${DESIGN_BOARD_HEIGHT}`}
    role="application"
    aria-label="Desired UI design canvas"
    aria-describedby={describedBy}
    tabIndex={0}
    onPointerDown={pointerDown}
    onPointerMove={pointerMove}
    onPointerUp={pointerUp}
    onPointerCancel={pointerCancel}
    onDoubleClick={doubleClick}
    onKeyDown={keyDown}
    onPaste={paste}
    onDrop={drop}
    onDragOver={(event) => { if (event.dataTransfer.types.includes("Files")) event.preventDefault(); }}
  >
    <rect className="a3s-design-canvas-background" width={DESIGN_BOARD_WIDTH} height={DESIGN_BOARD_HEIGHT} />
    <DesignCanvasElements elements={displayElements} selectedId={selectedId} />
    {textDraft && <foreignObject
      x={Math.min(textDraft.point.x, DESIGN_BOARD_WIDTH - 280)}
      y={Math.min(textDraft.point.y, DESIGN_BOARD_HEIGHT - 46)}
      width={280}
      height={46}
      className="a3s-design-text-editor"
    >
      <input
        ref={inputRef}
        aria-label="Design text"
        maxLength={240}
        value={textDraft.value}
        onChange={(event) => setTextDraft((current) => current ? { ...current, value: event.target.value } : current)}
        onBlur={commitText}
        onPointerDown={(event) => event.stopPropagation()}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            commitText();
          } else if (event.key === "Escape") {
            event.preventDefault();
            event.stopPropagation();
            setTextDraft(null);
          }
        }}
      />
    </foreignObject>}
  </svg>;
}

function interactionElements(
  elements: DesignElement[],
  interaction: CanvasInteraction | null,
  color: string,
  fill: string,
  strokeWidth: number,
): DesignElement[] {
  if (!interaction) return elements;
  if (interaction.kind === "draw") {
    return [...elements, {
      id: "design-draft",
      kind: "draw",
      points: interaction.points,
      color,
      strokeWidth,
    }];
  }
  if (interaction.kind === "rectangle") {
    const bounds = normalizeRectangle(interaction.start, interaction.current);
    return [...elements, {
      id: "design-draft",
      kind: "rectangle",
      ...bounds,
      color,
      fill,
      strokeWidth,
    }];
  }
  const replacement = interaction.kind === "move"
    ? moveDesignElement(
      interaction.original,
      interaction.current.x - interaction.start.x,
      interaction.current.y - interaction.start.y,
    )
    : resizeDesignElement(interaction.original, interaction.current);
  return replaceDesignElement(elements, replacement);
}

function completeInteraction(
  elements: DesignElement[],
  interaction: CanvasInteraction,
  point: DesignPoint,
  color: string,
  fill: string,
  strokeWidth: number,
): { elements: DesignElement[]; selectedId: string | null } | null {
  if (interaction.kind === "draw") {
    const id = createDesignElementId();
    const last = interaction.points.at(-1)!;
    const points = Math.hypot(point.x - last.x, point.y - last.y) >= 1
      ? [...interaction.points, point]
      : interaction.points;
    const safePoints = points.length > 1 ? points : [points[0]!, { x: points[0]!.x + 0.1, y: points[0]!.y + 0.1 }];
    return {
      elements: [...elements, { id, kind: "draw", points: safePoints, color, strokeWidth }],
      selectedId: id,
    };
  }
  if (interaction.kind === "rectangle") {
    const bounds = normalizeRectangle(interaction.start, point);
    if (bounds.width < 4 || bounds.height < 4) return null;
    const id = createDesignElementId();
    return {
      elements: [...elements, { id, kind: "rectangle", ...bounds, color, fill, strokeWidth }],
      selectedId: id,
    };
  }
  const replacement = interaction.kind === "move"
    ? moveDesignElement(
      interaction.original,
      point.x - interaction.start.x,
      point.y - interaction.start.y,
    )
    : resizeDesignElement(interaction.original, point);
  return {
    elements: replaceDesignElement(elements, replacement),
    selectedId: replacement.id,
  };
}

function eventPoint(svg: SVGSVGElement, clientX: number, clientY: number): DesignPoint {
  const bounds = svg.getBoundingClientRect();
  if (bounds.width <= 0 || bounds.height <= 0) return clampDesignPoint({ x: clientX, y: clientY });
  return clampDesignPoint({
    x: ((clientX - bounds.left) / bounds.width) * DESIGN_BOARD_WIDTH,
    y: ((clientY - bounds.top) / bounds.height) * DESIGN_BOARD_HEIGHT,
  });
}

function capturePointer(element: SVGSVGElement, pointerId: number) {
  try {
    element.setPointerCapture(pointerId);
  } catch {
    // Pointer capture is unavailable in some test DOMs.
  }
}

function releasePointer(element: SVGSVGElement, pointerId: number) {
  try {
    if (element.hasPointerCapture(pointerId)) element.releasePointerCapture(pointerId);
  } catch {
    // The pointer may already have been released by the browser.
  }
}
