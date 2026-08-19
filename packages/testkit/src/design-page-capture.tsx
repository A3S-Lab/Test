import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent as ReactKeyboardEvent,
  type PointerEvent as ReactPointerEvent,
} from "react";
import { DesignGlyph } from "./design-icons";
import { normalizedArea, rectStyle } from "./review-utils";
import type { Rect } from "./types";

const MIN_CAPTURE_SIZE = 12;
const SIZE_LABEL_WIDTH = 88;
const SIZE_LABEL_HEIGHT = 26;
const SIZE_LABEL_GAP = 8;

type CaptureDrag = {
  pointerId: number;
  startX: number;
  startY: number;
  currentX: number;
  currentY: number;
};

export type PageCaptureOverlayProps = {
  busy: boolean;
  title: string;
  help: string;
  busyLabel: string;
  tooSmallLabel: string;
  cancelLabel: string;
  onCancel(): void;
  onSelect(region: Rect): void;
};

export function PageCaptureOverlay(props: PageCaptureOverlayProps) {
  const rootRef = useRef<HTMLDivElement | null>(null);
  const [drag, setDrag] = useState<CaptureDrag | null>(null);
  const [tooSmall, setTooSmall] = useState(false);
  const selection = useMemo(
    () =>
      drag
        ? normalizedArea(drag.startX, drag.startY, drag.currentX, drag.currentY)
        : null,
    [drag],
  );

  useEffect(() => {
    rootRef.current?.focus({ preventScroll: true });
    const preventScroll = (event: Event) => event.preventDefault();
    window.addEventListener("wheel", preventScroll, {
      capture: true,
      passive: false,
    });
    window.addEventListener("touchmove", preventScroll, {
      capture: true,
      passive: false,
    });
    return () => {
      window.removeEventListener("wheel", preventScroll, true);
      window.removeEventListener("touchmove", preventScroll, true);
    };
  }, []);

  function onPointerDown(event: ReactPointerEvent<HTMLDivElement>) {
    if (props.busy || event.button !== 0 || captureControl(event.target))
      return;
    event.preventDefault();
    const point = capturePoint(event.clientX, event.clientY);
    setTooSmall(false);
    setDrag({
      pointerId: event.pointerId,
      startX: point.x,
      startY: point.y,
      currentX: point.x,
      currentY: point.y,
    });
    if (typeof event.currentTarget.setPointerCapture === "function") {
      event.currentTarget.setPointerCapture(event.pointerId);
    }
  }

  function onPointerMove(event: ReactPointerEvent<HTMLDivElement>) {
    if (!drag || drag.pointerId !== event.pointerId || props.busy) return;
    event.preventDefault();
    const point = capturePoint(event.clientX, event.clientY);
    setDrag((current) =>
      current && current.pointerId === event.pointerId
        ? { ...current, currentX: point.x, currentY: point.y }
        : current,
    );
  }

  function onPointerUp(event: ReactPointerEvent<HTMLDivElement>) {
    if (!drag || drag.pointerId !== event.pointerId || props.busy) return;
    event.preventDefault();
    const point = capturePoint(event.clientX, event.clientY);
    const region = normalizedArea(drag.startX, drag.startY, point.x, point.y);
    if (typeof event.currentTarget.releasePointerCapture === "function") {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    if (region.width < MIN_CAPTURE_SIZE || region.height < MIN_CAPTURE_SIZE) {
      setDrag(null);
      setTooSmall(true);
      return;
    }
    setDrag((current) =>
      current ? { ...current, currentX: point.x, currentY: point.y } : current,
    );
    props.onSelect(region);
  }

  function onKeyDown(event: ReactKeyboardEvent<HTMLDivElement>) {
    if (event.key === "Escape" && !props.busy) {
      event.preventDefault();
      event.stopPropagation();
      props.onCancel();
      return;
    }
    if (
      [
        " ",
        "ArrowDown",
        "ArrowLeft",
        "ArrowRight",
        "ArrowUp",
        "End",
        "Home",
        "PageDown",
        "PageUp",
      ].includes(event.key)
    ) {
      event.preventDefault();
    }
  }

  const status = props.busy
    ? props.busyLabel
    : tooSmall
      ? props.tooSmallLabel
      : props.help;

  return (
    <div
      ref={rootRef}
      className={`a3s-page-capture${selection ? " has-selection" : ""}`}
      role="dialog"
      aria-modal="true"
      aria-label={props.title}
      aria-busy={props.busy}
      tabIndex={-1}
      onKeyDown={onKeyDown}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={() => {
        if (!props.busy) setDrag(null);
      }}
    >
      <div
        className="a3s-page-capture-bar"
        data-capture-control=""
        onPointerDown={(event) => event.stopPropagation()}
      >
        <span className="a3s-page-capture-icon" aria-hidden="true">
          <DesignGlyph name="capture" />
        </span>
        <span className="a3s-page-capture-copy">
          <strong>{props.title}</strong>
          <small aria-live="polite">{status}</small>
        </span>
        <button
          type="button"
          disabled={props.busy}
          aria-label={props.cancelLabel}
          title={props.cancelLabel}
          onClick={props.onCancel}
        >
          <DesignGlyph name="close" />
          <span className="a3s-sr-only">{props.cancelLabel}</span>
        </button>
      </div>
      {selection && (
        <>
          <div
            className="a3s-page-capture-selection"
            style={rectStyle(selection)}
            aria-hidden="true"
          />
          <output
            className="a3s-page-capture-size"
            style={captureSizeStyle(selection)}
          >
            {Math.round(selection.width)} × {Math.round(selection.height)}
          </output>
        </>
      )}
    </div>
  );
}

export async function captureBrowserRegion(region: Rect): Promise<string> {
  const viewport = captureViewportSize();
  const viewportImage = await captureBrowserViewport(viewport);
  return cropCapturedViewport(viewportImage, region, viewport);
}

type CaptureViewport = { width: number; height: number };

async function captureBrowserViewport(
  viewport: CaptureViewport,
): Promise<string> {
  const { domToJpeg } = await import("modern-screenshot");
  return domToJpeg(document.documentElement, {
    width: viewport.width,
    height: viewport.height,
    quality: 0.9,
    scale: 1,
    backgroundColor: pageBackgroundColor(),
    maximumCanvasSize: 4_096,
    timeout: 10_000,
    features: { restoreScrollPosition: true },
    filter: (node) =>
      !(
        node instanceof Element && node.hasAttribute("data-a3s-testkit-overlay")
      ),
  });
}

async function cropCapturedViewport(
  dataUrl: string,
  region: Rect,
  viewport: CaptureViewport,
): Promise<string> {
  const image = await decodeCapturedImage(dataUrl);
  const scaleX =
    Math.max(1, image.naturalWidth || image.width) / viewport.width;
  const scaleY =
    Math.max(1, image.naturalHeight || image.height) / viewport.height;
  const left = clamp(region.x, 0, viewport.width);
  const top = clamp(region.y, 0, viewport.height);
  const width = clamp(region.width, 1, viewport.width - left);
  const height = clamp(region.height, 1, viewport.height - top);
  const sourceX = Math.round(left * scaleX);
  const sourceY = Math.round(top * scaleY);
  const sourceWidth = Math.max(1, Math.round(width * scaleX));
  const sourceHeight = Math.max(1, Math.round(height * scaleY));
  const canvas = document.createElement("canvas");
  canvas.width = sourceWidth;
  canvas.height = sourceHeight;
  const context = canvas.getContext("2d");
  if (!context)
    throw new Error("The selected screenshot area could not be cropped.");
  context.drawImage(
    image,
    sourceX,
    sourceY,
    sourceWidth,
    sourceHeight,
    0,
    0,
    sourceWidth,
    sourceHeight,
  );
  return canvas.toDataURL("image/jpeg", 0.9);
}

function decodeCapturedImage(dataUrl: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = () =>
      reject(new Error("The captured browser page could not be decoded."));
    image.src = dataUrl;
  });
}

function captureViewportSize(): CaptureViewport {
  const viewport = window.visualViewport;
  return {
    width: Math.max(1, Math.round(viewport?.width ?? window.innerWidth)),
    height: Math.max(1, Math.round(viewport?.height ?? window.innerHeight)),
  };
}

function capturePoint(x: number, y: number): { x: number; y: number } {
  const viewport = captureViewportSize();
  return {
    x: clamp(x, 0, viewport.width),
    y: clamp(y, 0, viewport.height),
  };
}

function captureSizeStyle(region: Rect): CSSProperties {
  const viewport = captureViewportSize();
  const below = region.y + region.height + SIZE_LABEL_GAP;
  return {
    left: clamp(
      region.x + region.width - SIZE_LABEL_WIDTH,
      SIZE_LABEL_GAP,
      viewport.width - SIZE_LABEL_WIDTH - SIZE_LABEL_GAP,
    ),
    top:
      below + SIZE_LABEL_HEIGHT <= viewport.height - SIZE_LABEL_GAP
        ? below
        : Math.max(
            SIZE_LABEL_GAP,
            region.y - SIZE_LABEL_HEIGHT - SIZE_LABEL_GAP,
          ),
  };
}

function captureControl(target: EventTarget): boolean {
  return (
    target instanceof Element &&
    Boolean(target.closest("[data-capture-control]"))
  );
}

function pageBackgroundColor(): string {
  for (const element of [document.documentElement, document.body]) {
    const color = window.getComputedStyle(element).backgroundColor;
    if (color && color !== "transparent" && color !== "rgba(0, 0, 0, 0)")
      return color;
  }
  return "#ffffff";
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(Math.max(value, minimum), Math.max(minimum, maximum));
}
