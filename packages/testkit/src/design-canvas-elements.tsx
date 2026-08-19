import type { ReactNode } from "react";
import {
  elementBounds,
  type DesignElement,
} from "./design-board-model";

export function DesignCanvasElements({
  elements,
  selectedId,
}: {
  elements: DesignElement[];
  selectedId: string | null;
}) {
  const selected = selectedId
    ? elements.find((element) => element.id === selectedId) ?? null
    : null;
  return <>
    {elements.map((element) => <DesignCanvasElement key={element.id} element={element} />)}
    {selected && <SelectionOutline element={selected} />}
  </>;
}

function DesignCanvasElement({ element }: { element: DesignElement }) {
  let content: ReactNode;
  if (element.kind === "draw") {
    content = <path
      d={drawPath(element.points)}
      fill="none"
      stroke={element.color}
      strokeWidth={element.strokeWidth}
      strokeLinecap="round"
      strokeLinejoin="round"
    />;
  } else if (element.kind === "rectangle") {
    content = <rect
      x={element.x}
      y={element.y}
      width={element.width}
      height={element.height}
      rx={6}
      fill={element.fill}
      stroke={element.color}
      strokeWidth={element.strokeWidth}
    />;
  } else if (element.kind === "text") {
    content = <text
      x={element.x}
      y={element.y}
      fill={element.color}
      fontFamily="Inter, ui-sans-serif, system-ui, sans-serif"
      fontSize={element.fontSize}
      dominantBaseline="hanging"
    >
      {element.text.split("\n").map((line, index) => <tspan
        key={`${element.id}-${index}`}
        x={element.x}
        dy={index === 0 ? 0 : element.fontSize * 1.3}
      >{line || " "}</tspan>)}
    </text>;
  } else {
    content = <image
      href={element.src}
      x={element.x}
      y={element.y}
      width={element.width}
      height={element.height}
      preserveAspectRatio="none"
    />;
  }
  return <g
    className={`a3s-design-element is-${element.kind}`}
    data-element-id={element.id}
    role="img"
    aria-label={elementLabel(element)}
  >
    <title>{elementLabel(element)}</title>
    {content}
  </g>;
}

function SelectionOutline({ element }: { element: DesignElement }) {
  const bounds = elementBounds(element);
  return <g className="a3s-design-selection" aria-hidden="true">
    <rect
      x={bounds.x - 4}
      y={bounds.y - 4}
      width={bounds.width + 8}
      height={bounds.height + 8}
      rx={4}
    />
    <circle
      cx={bounds.x + bounds.width + 4}
      cy={bounds.y + bounds.height + 4}
      r={7}
      data-resize-id={element.id}
    />
  </g>;
}

function elementLabel(element: DesignElement): string {
  if (element.kind === "draw") return "Freehand stroke";
  if (element.kind === "rectangle") return "Rectangle";
  if (element.kind === "text") return `Text: ${element.text}`;
  return element.referenceKind === "sketch" ? "Existing design sketch" : "Screenshot";
}

function drawPath(points: Array<{ x: number; y: number }>): string {
  const first = points[0];
  if (!first) return "";
  if (points.length === 1) return `M ${first.x} ${first.y} L ${first.x + 0.1} ${first.y + 0.1}`;
  if (points.length === 2) return `M ${first.x} ${first.y} L ${points[1]!.x} ${points[1]!.y}`;
  let path = `M ${first.x} ${first.y}`;
  for (let index = 1; index < points.length - 1; index += 1) {
    const point = points[index]!;
    const next = points[index + 1]!;
    path += ` Q ${point.x} ${point.y} ${(point.x + next.x) / 2} ${(point.y + next.y) / 2}`;
  }
  const last = points.at(-1)!;
  path += ` L ${last.x} ${last.y}`;
  return path;
}
