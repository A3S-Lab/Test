import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { DesignCanvas } from "./design-canvas";
import { exportDesignBoard } from "./design-board-export";
import {
  commitDesignHistory,
  createDesignElementId,
  createDesignHistory,
  MAX_DESIGN_ELEMENTS,
  redoDesignHistory,
  summarizeBoard,
  undoDesignHistory,
  type DesignElement,
  type DesignHistory,
  type DesignImageElement,
  type DesignTool,
} from "./design-board-model";
import {
  DESIGN_BOARD_HEIGHT,
  DESIGN_BOARD_WIDTH,
  MAX_DESIGN_REFERENCE_SOURCE_BYTES,
  validDesignReference,
} from "./design-reference";
import type { OverlayTheme } from "./review-model";
import type { RepairDesignReference } from "./types";

export type DesignBoardProps = {
  idPrefix: string;
  initialReference: RepairDesignReference | null;
  theme: OverlayTheme;
  onAttach(reference: RepairDesignReference): void;
  onCancel(): void;
};

type BusyAction = "capture" | "import" | "export";

const ACCEPTED_IMAGE_TYPES = ["image/png", "image/jpeg"] as const;
const IMAGE_PADDING = 40;

export function DesignBoard({
  idPrefix,
  initialReference,
  theme,
  onAttach,
  onCancel,
}: DesignBoardProps) {
  const dialogRef = useRef<HTMLElement | null>(null);
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const [history, setHistory] = useState<DesignHistory>(() => createDesignHistory());
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [tool, setTool] = useState<DesignTool>("select");
  const [color, setColor] = useState("#1c1917");
  const [fill, setFill] = useState("transparent");
  const [strokeWidth, setStrokeWidth] = useState(4);
  const [busyAction, setBusyAction] = useState<BusyAction | null>(null);
  const [error, setError] = useState("");
  const summary = useMemo(() => summarizeBoard(history.present), [history.present]);
  const titleId = `${idPrefix}-design-board-title`;
  const descriptionId = `${idPrefix}-design-board-description`;
  const statusId = `${idPrefix}-design-board-status`;
  const screenCaptureAvailable = typeof navigator !== "undefined"
    && typeof navigator.mediaDevices?.getDisplayMedia === "function";
  const busy = busyAction !== null;

  useEffect(() => {
    dialogRef.current?.querySelector<SVGSVGElement>("[data-testid='design-canvas']")?.focus();
  }, []);

  useEffect(() => {
    let cancelled = false;
    setHistory(createDesignHistory());
    setSelectedId(null);
    setTool("select");
    setError("");
    if (!initialReference) return () => { cancelled = true; };
    if (initialReference.image.kind !== "inline") {
      setError("This stored artifact cannot be edited in the browser. Import its PNG or JPEG copy instead.");
      return () => { cancelled = true; };
    }
    setBusyAction("import");
    void createImageElement(
      initialReference.image.dataUrl,
      initialReference.image.mediaType,
      initialReference.kind,
    ).then((image) => {
      if (!cancelled) setHistory(createDesignHistory([image]));
    }).catch((cause: unknown) => {
      if (!cancelled) setError(errorMessage(cause, "The existing design reference could not be opened."));
    }).finally(() => {
      if (!cancelled) setBusyAction(null);
    });
    return () => { cancelled = true; };
  }, [initialReference]);

  const commitElements = useCallback((elements: DesignElement[], nextSelectedId?: string | null) => {
    setHistory((current) => commitDesignHistory(current, elements));
    if (nextSelectedId !== undefined) setSelectedId(nextSelectedId);
    setError("");
  }, []);

  async function importFile(file: File | null | undefined) {
    if (!file) return;
    if (!(ACCEPTED_IMAGE_TYPES as readonly string[]).includes(file.type)) {
      setError("Choose a PNG or JPEG screenshot.");
      return;
    }
    if (file.size === 0 || file.size > MAX_DESIGN_REFERENCE_SOURCE_BYTES) {
      setError("The screenshot must be between 1 byte and 8 MiB.");
      return;
    }
    setBusyAction("import");
    setError("");
    try {
      const mediaType = file.type as (typeof ACCEPTED_IMAGE_TYPES)[number];
      const image = await createImageElement(await readFile(file), mediaType, "screenshot");
      const elements = replaceBackgroundImage(history.present, image);
      if (!elements) {
        setError(`Remove an object before adding a screenshot; the board limit is ${MAX_DESIGN_ELEMENTS}.`);
        return;
      }
      setHistory((current) => commitDesignHistory(current, elements));
      setSelectedId(image.id);
      setTool("select");
    } catch (cause) {
      setError(errorMessage(cause, "The screenshot could not be opened."));
    } finally {
      setBusyAction(null);
      if (fileInputRef.current) fileInputRef.current.value = "";
    }
  }

  async function captureScreen() {
    if (!screenCaptureAvailable) {
      setError("Screen capture is unavailable in this browser. Upload or paste a screenshot instead.");
      return;
    }
    setBusyAction("capture");
    setError("");
    let stream: MediaStream | null = null;
    try {
      stream = await navigator.mediaDevices.getDisplayMedia({ video: true, audio: false });
      const video = document.createElement("video");
      video.muted = true;
      video.srcObject = stream;
      await waitForVideo(video);
      await video.play();
      const frame = document.createElement("canvas");
      frame.width = Math.max(1, video.videoWidth);
      frame.height = Math.max(1, video.videoHeight);
      const context = frame.getContext("2d");
      if (!context) throw new Error("The browser could not create a screenshot canvas.");
      context.drawImage(video, 0, 0, frame.width, frame.height);
      const image = await createImageElement(
        frame.toDataURL("image/jpeg", 0.9),
        "image/jpeg",
        "screenshot",
      );
      const elements = replaceBackgroundImage(history.present, image);
      if (!elements) {
        setError(`Remove an object before adding a screenshot; the board limit is ${MAX_DESIGN_ELEMENTS}.`);
        return;
      }
      setHistory((current) => commitDesignHistory(current, elements));
      setSelectedId(image.id);
      setTool("select");
    } catch (cause) {
      if (cause instanceof DOMException && cause.name === "NotAllowedError") {
        setError("Screen capture was cancelled. Upload or paste a screenshot instead.");
      } else {
        setError(errorMessage(cause, "Screen capture failed."));
      }
    } finally {
      stream?.getTracks().forEach((track) => track.stop());
      setBusyAction(null);
    }
  }

  function undo() {
    setHistory((current) => undoDesignHistory(current));
    setSelectedId(null);
    setError("");
  }

  function redo() {
    setHistory((current) => redoDesignHistory(current));
    setSelectedId(null);
    setError("");
  }

  function clearBoard() {
    setHistory(createDesignHistory());
    setSelectedId(null);
    setTool("select");
    setError("");
  }

  async function attachReference() {
    const current = summarizeBoard(history.present);
    if (!current.kind) return;
    setBusyAction("export");
    setError("");
    try {
      const exported = await exportDesignBoard(history.present, current.hasImage);
      if (!exported) {
        setError("The design reference is still too large. Remove detail or use a smaller screenshot.");
        return;
      }
      const reference: RepairDesignReference = {
        kind: current.kind,
        width: exported.width,
        height: exported.height,
        image: {
          kind: "inline",
          mediaType: exported.mediaType,
          dataUrl: exported.dataUrl,
        },
      };
      if (!validDesignReference(reference)) {
        setError("The browser produced an invalid design reference. Try a smaller screenshot.");
        return;
      }
      onAttach(reference);
    } catch (cause) {
      setError(errorMessage(cause, "The design reference could not be exported."));
    } finally {
      setBusyAction(null);
    }
  }

  function onDialogKeyDownCapture(event: React.KeyboardEvent<HTMLElement>) {
    if (event.key !== "Escape" || event.defaultPrevented) return;
    const target = event.target instanceof Element ? event.target : null;
    const canvasShouldHandleEscape = target?.closest(".a3s-design-canvas-surface")
      && (tool !== "select" || selectedId !== null);
    if (canvasShouldHandleEscape) return;
    event.preventDefault();
    event.stopPropagation();
    onCancel();
  }

  function onDialogKeyDown(event: React.KeyboardEvent<HTMLElement>) {
    if (event.key !== "Tab") return;
    const focusable = Array.from(dialogRef.current?.querySelectorAll<HTMLElement>(
      "button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [href], [tabindex]:not([tabindex='-1']), [contenteditable='true']",
    ) ?? []).filter((element) => {
      if (element.hidden || element.closest("[hidden], [aria-hidden='true']")) return false;
      const style = getComputedStyle(element);
      return style.display !== "none" && style.visibility !== "hidden";
    });
    if (focusable.length === 0) return;
    const first = focusable[0]!;
    const last = focusable.at(-1)!;
    const root = dialogRef.current?.getRootNode();
    const active = root instanceof ShadowRoot ? root.activeElement : document.activeElement;
    if (event.shiftKey && active === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && active === last) {
      event.preventDefault();
      first.focus();
    }
  }

  return <div className="a3s-design-scrim">
    <section
      ref={dialogRef}
      className="a3s-design-board"
      data-theme={theme}
      role="dialog"
      aria-modal="true"
      aria-labelledby={titleId}
      aria-describedby={descriptionId}
      onKeyDownCapture={onDialogKeyDownCapture}
      onKeyDown={onDialogKeyDown}
    >
      <div className="a3s-design-header">
        <div>
          <strong id={titleId}>Design reference</strong>
          <small id={descriptionId}>Sketch the desired UI or add a screenshot.</small>
        </div>
        <button type="button" className="a3s-close" aria-label="Close design board" onClick={onCancel}>
          <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M3.5 3.5 12.5 12.5M12.5 3.5 3.5 12.5" /></svg>
        </button>
      </div>
      <div className="a3s-design-body">
        <div className="a3s-design-import" aria-label="Design board actions">
          <button type="button" disabled={busy || !screenCaptureAvailable} onClick={() => void captureScreen()}>{busyAction === "capture" ? "Capturing…" : "Capture screen"}</button>
          <button type="button" disabled={busy} onClick={() => fileInputRef.current?.click()}>{busyAction === "import" ? "Importing…" : "Upload screenshot"}</button>
          <input ref={fileInputRef} hidden type="file" accept="image/png,image/jpeg" onChange={(event) => void importFile(event.target.files?.[0])} />
          <span className="a3s-design-history" aria-label="Canvas history">
            <button type="button" disabled={busy || history.past.length === 0} onClick={undo}>Undo</button>
            <button type="button" disabled={busy || history.future.length === 0} onClick={redo}>Redo</button>
            <button type="button" className="quiet" disabled={busy || !summary.kind} onClick={clearBoard}>Clear board</button>
          </span>
          <small>PNG or JPEG, up to 8 MiB. Paste or drop one onto the canvas.</small>
        </div>
        <div className="a3s-design-stage" data-content={summary.label}>
          <div className="a3s-design-toolbar" role="toolbar" aria-label="Design tools">
            {(["select", "draw", "rectangle", "text"] as const).map((value) => <button
              key={value}
              type="button"
              data-testid={`design-tool-${value}`}
              aria-label={toolLabel(value)}
              aria-pressed={tool === value}
              className={tool === value ? "selected" : ""}
              disabled={busy}
              onClick={() => setTool(value)}
            >{toolLabel(value)}</button>)}
            <label>Stroke <input aria-label="Stroke color" type="color" value={color} disabled={busy} onChange={(event) => setColor(event.target.value)} /></label>
            <label>Fill <select aria-label="Shape fill" value={fill} disabled={busy} onChange={(event) => setFill(event.target.value)}>
              <option value="transparent">None</option>
              <option value="#e0f2fe">Blue</option>
              <option value="#fef3c7">Amber</option>
              <option value="#dcfce7">Green</option>
              <option value="#fce7f3">Pink</option>
            </select></label>
            <label>Width <select aria-label="Stroke width" value={strokeWidth} disabled={busy} onChange={(event) => setStrokeWidth(Number(event.target.value))}>
              <option value="2">S</option><option value="4">M</option><option value="8">L</option>
            </select></label>
            <output aria-label="Design object count">{history.present.length}/{MAX_DESIGN_ELEMENTS}</output>
          </div>
          <div className="a3s-design-canvas">
            <DesignCanvas
              elements={history.present}
              selectedId={selectedId}
              tool={tool}
              color={color}
              fill={fill}
              strokeWidth={strokeWidth}
              disabled={busy}
              canUndo={history.past.length > 0}
              canRedo={history.future.length > 0}
              describedBy={statusId}
              onSelect={setSelectedId}
              onToolChange={setTool}
              onCommit={commitElements}
              onUndo={undo}
              onRedo={redo}
              onImportFile={(file) => void importFile(file)}
              onLimit={() => setError(`The design board is limited to ${MAX_DESIGN_ELEMENTS} objects.`)}
            />
          </div>
        </div>
        <p id={statusId} className={`a3s-design-status${error ? " is-error" : ""}`} role={error ? "alert" : "status"}>
          {error || `${summary.label}. Draw, add rectangles or text, then use Select to move and resize objects.`}
        </p>
      </div>
      <div className="a3s-design-actions">
        <button type="button" className="quiet" onClick={onCancel}>Cancel</button>
        <button type="button" disabled={!summary.kind || busy} onClick={() => void attachReference()}>{busyAction === "export" ? "Exporting…" : "Attach to finding"}</button>
      </div>
    </section>
  </div>;
}

async function createImageElement(
  dataUrl: string,
  mediaType: "image/png" | "image/jpeg",
  referenceKind: RepairDesignReference["kind"],
): Promise<DesignImageElement> {
  const image = await decodeImage(dataUrl);
  const sourceWidth = Math.max(1, image.naturalWidth || image.width);
  const sourceHeight = Math.max(1, image.naturalHeight || image.height);
  const scale = Math.min(
    (DESIGN_BOARD_WIDTH - IMAGE_PADDING * 2) / sourceWidth,
    (DESIGN_BOARD_HEIGHT - IMAGE_PADDING * 2) / sourceHeight,
  );
  const width = Math.max(1, sourceWidth * scale);
  const height = Math.max(1, sourceHeight * scale);
  return {
    id: createDesignElementId(),
    kind: "image",
    x: (DESIGN_BOARD_WIDTH - width) / 2,
    y: (DESIGN_BOARD_HEIGHT - height) / 2,
    width,
    height,
    src: dataUrl,
    mediaType,
    referenceKind,
    background: true,
  };
}

function toolLabel(tool: DesignTool): string {
  return ({ select: "Select", draw: "Draw", rectangle: "Rectangle", text: "Text" } as const)[tool];
}

function replaceBackgroundImage(
  elements: DesignElement[],
  image: DesignImageElement,
): DesignElement[] | null {
  const retained = elements.filter((element) => element.kind !== "image" || !element.background);
  return retained.length < MAX_DESIGN_ELEMENTS ? [image, ...retained] : null;
}

function decodeImage(dataUrl: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error("The screenshot could not be decoded."));
    image.src = dataUrl;
  });
}

function readFile(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => typeof reader.result === "string"
      ? resolve(reader.result)
      : reject(new Error("The screenshot could not be read."));
    reader.onerror = () => reject(new Error("The screenshot could not be read."));
    reader.readAsDataURL(file);
  });
}

function waitForVideo(video: HTMLVideoElement): Promise<void> {
  if (video.readyState >= HTMLMediaElement.HAVE_METADATA && video.videoWidth > 0) return Promise.resolve();
  return new Promise((resolve, reject) => {
    const timer = window.setTimeout(() => reject(new Error("Screen capture did not become ready.")), 8_000);
    video.onloadedmetadata = () => {
      window.clearTimeout(timer);
      resolve();
    };
    video.onerror = () => {
      window.clearTimeout(timer);
      reject(new Error("Screen capture could not be read."));
    };
  });
}

function errorMessage(cause: unknown, fallback: string): string {
  return cause instanceof Error && cause.message ? cause.message : fallback;
}
