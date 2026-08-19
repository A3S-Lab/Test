import type {
  DesignDrawElement,
  DesignElement,
  DesignRectangleElement,
  DesignTextElement,
} from "./design-board-model";
import {
  DESIGN_BOARD_HEIGHT,
  DESIGN_BOARD_WIDTH,
  MAX_DESIGN_REFERENCE_DATA_URL_BYTES,
} from "./design-reference";

export type ExportedDesignBoard = {
  width: number;
  height: number;
  mediaType: "image/png" | "image/jpeg";
  dataUrl: string;
};

export async function exportDesignBoard(
  elements: DesignElement[],
  hasScreenshot: boolean,
): Promise<ExportedDesignBoard | null> {
  const scales = [1, 0.8, 2 / 3];
  const formats: ReadonlyArray<{ mediaType: "image/png" | "image/jpeg"; quality?: number }> = hasScreenshot
    ? [
      { mediaType: "image/jpeg", quality: 0.86 },
      { mediaType: "image/jpeg", quality: 0.72 },
    ]
    : [
      { mediaType: "image/png" },
      { mediaType: "image/jpeg", quality: 0.86 },
    ];
  let lastError: unknown = null;
  let producedImage = false;
  for (const scale of scales) {
    for (const format of formats) {
      try {
        const exported = await renderDesignBoard(elements, scale, format.mediaType, format.quality);
        producedImage = true;
        if (exported.dataUrl.length <= MAX_DESIGN_REFERENCE_DATA_URL_BYTES) return exported;
      } catch (cause) {
        lastError = cause;
      }
    }
  }
  if (!producedImage && lastError) throw lastError;
  return null;
}

async function renderDesignBoard(
  elements: DesignElement[],
  scale: number,
  mediaType: "image/png" | "image/jpeg",
  quality?: number,
): Promise<ExportedDesignBoard> {
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.round(DESIGN_BOARD_WIDTH * scale));
  canvas.height = Math.max(1, Math.round(DESIGN_BOARD_HEIGHT * scale));
  const context = canvas.getContext("2d");
  if (!context) throw new Error("The browser could not create the design export canvas.");
  context.save();
  context.scale(scale, scale);
  context.fillStyle = "#ffffff";
  context.fillRect(0, 0, DESIGN_BOARD_WIDTH, DESIGN_BOARD_HEIGHT);
  for (const element of elements) await renderElement(context, element);
  context.restore();
  const blob = await canvasBlob(canvas, mediaType, quality);
  const dataUrl = await readBlob(blob);
  return {
    width: canvas.width,
    height: canvas.height,
    mediaType,
    dataUrl,
  };
}

async function renderElement(context: CanvasRenderingContext2D, element: DesignElement) {
  if (element.kind === "image") {
    const image = await decodeImage(element.src);
    context.drawImage(image, element.x, element.y, element.width, element.height);
    return;
  }
  if (element.kind === "draw") {
    renderDraw(context, element);
    return;
  }
  if (element.kind === "rectangle") {
    renderRectangle(context, element);
    return;
  }
  renderText(context, element);
}

function renderDraw(context: CanvasRenderingContext2D, element: DesignDrawElement) {
  const first = element.points[0];
  if (!first) return;
  context.save();
  context.beginPath();
  context.moveTo(first.x, first.y);
  for (const point of element.points.slice(1)) context.lineTo(point.x, point.y);
  context.strokeStyle = element.color;
  context.lineWidth = element.strokeWidth;
  context.lineCap = "round";
  context.lineJoin = "round";
  context.stroke();
  context.restore();
}

function renderRectangle(context: CanvasRenderingContext2D, element: DesignRectangleElement) {
  context.save();
  if (element.fill !== "transparent") {
    context.fillStyle = element.fill;
    context.fillRect(element.x, element.y, element.width, element.height);
  }
  context.strokeStyle = element.color;
  context.lineWidth = element.strokeWidth;
  context.strokeRect(element.x, element.y, element.width, element.height);
  context.restore();
}

function renderText(context: CanvasRenderingContext2D, element: DesignTextElement) {
  context.save();
  context.fillStyle = element.color;
  context.font = `${element.fontSize}px Inter, ui-sans-serif, system-ui, sans-serif`;
  context.textBaseline = "top";
  element.text.split("\n").forEach((line, index) => {
    context.fillText(line, element.x, element.y + index * element.fontSize * 1.3);
  });
  context.restore();
}

function canvasBlob(
  canvas: HTMLCanvasElement,
  mediaType: "image/png" | "image/jpeg",
  quality?: number,
): Promise<Blob> {
  return new Promise((resolve, reject) => {
    canvas.toBlob(
      (blob) => blob ? resolve(blob) : reject(new Error("The browser could not encode the design reference.")),
      mediaType,
      quality,
    );
  });
}

function decodeImage(source: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error("A screenshot on the board could not be decoded."));
    image.src = source;
  });
}

function readBlob(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => typeof reader.result === "string"
      ? resolve(reader.result)
      : reject(new Error("The exported design reference could not be read."));
    reader.onerror = () => reject(new Error("The exported design reference could not be read."));
    reader.readAsDataURL(blob);
  });
}
