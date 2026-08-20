import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { DesignCanvas } from "./design-canvas";
import { exportDesignBoard } from "./design-board-export";
import { captureBrowserRegion, PageCaptureOverlay } from "./design-page-capture";
import {
  designSummaryLabel,
  designToolLabel,
  useDesignBoardI18n,
} from "./design-board-i18n";
import { DesignGlyph, type DesignGlyphName } from "./design-icons";
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
import type { Rect, RepairDesignReference } from "./types";

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
  const captureButtonRef = useRef<HTMLButtonElement | null>(null);
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const { locale, t } = useDesignBoardI18n();
  const [history, setHistory] = useState<DesignHistory>(() => createDesignHistory());
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [tool, setTool] = useState<DesignTool>("select");
  const [color, setColor] = useState("#1c1917");
  const [fill, setFill] = useState("transparent");
  const [strokeWidth, setStrokeWidth] = useState(4);
  const [busyAction, setBusyAction] = useState<BusyAction | null>(null);
  const [captureMode, setCaptureMode] = useState(false);
  const [error, setError] = useState("");
  const summary = useMemo(() => summarizeBoard(history.present), [history.present]);
  const titleId = `${idPrefix}-design-board-title`;
  const descriptionId = `${idPrefix}-design-board-description`;
  const statusId = `${idPrefix}-design-board-status`;
  const busy = busyAction !== null;
  const summaryLabel = designSummaryLabel(t, summary);
  const showHistory = history.past.length > 0 || history.future.length > 0 || Boolean(summary.kind);
  const showStyles = tool !== "select";

  useEffect(() => {
    dialogRef.current?.querySelector<HTMLButtonElement>("[data-testid='design-tool-select']")?.focus();
  }, []);

  useEffect(() => {
    let cancelled = false;
    setHistory(createDesignHistory());
    setSelectedId(null);
    setTool("select");
    setError("");
    if (!initialReference) return () => { cancelled = true; };
    if (initialReference.image.kind !== "inline") {
      setError(t("storedArtifactUnavailable"));
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
      if (!cancelled) setError(errorMessage(cause, t("existingReferenceOpenFailed"), locale));
    }).finally(() => {
      if (!cancelled) setBusyAction(null);
    });
    return () => { cancelled = true; };
  }, [initialReference, locale, t]);

  const commitElements = useCallback((elements: DesignElement[], nextSelectedId?: string | null) => {
    setHistory((current) => commitDesignHistory(current, elements));
    if (nextSelectedId !== undefined) setSelectedId(nextSelectedId);
    setError("");
  }, []);

  async function importFile(file: File | null | undefined) {
    if (!file) return;
    if (!(ACCEPTED_IMAGE_TYPES as readonly string[]).includes(file.type)) {
      setError(t("invalidImageType"));
      return;
    }
    if (file.size === 0 || file.size > MAX_DESIGN_REFERENCE_SOURCE_BYTES) {
      setError(t("invalidImageSize"));
      return;
    }
    setBusyAction("import");
    setError("");
    try {
      const mediaType = file.type as (typeof ACCEPTED_IMAGE_TYPES)[number];
      const image = await createImageElement(await readFile(file), mediaType, "screenshot");
      const elements = replaceBackgroundImage(history.present, image);
      if (!elements) {
        setError(t("removeObjectBeforeScreenshot", { limit: MAX_DESIGN_ELEMENTS }));
        return;
      }
      setHistory((current) => commitDesignHistory(current, elements));
      setSelectedId(image.id);
      setTool("select");
    } catch (cause) {
      setError(errorMessage(cause, t("screenshotOpenFailed"), locale));
    } finally {
      setBusyAction(null);
      if (fileInputRef.current) fileInputRef.current.value = "";
    }
  }

  function startPageCapture() {
    setError("");
    setCaptureMode(true);
  }

  function cancelPageCapture() {
    if (busyAction === "capture") return;
    setCaptureMode(false);
    restoreCaptureButtonFocus();
  }

  async function capturePage(region: Rect) {
    setBusyAction("capture");
    setError("");
    try {
      const image = await createImageElement(
        await captureBrowserRegion(region),
        "image/jpeg",
        "screenshot",
      );
      const elements = replaceBackgroundImage(history.present, image);
      if (!elements) {
        setError(t("removeObjectBeforeScreenshot", { limit: MAX_DESIGN_ELEMENTS }));
        return;
      }
      setHistory((current) => commitDesignHistory(current, elements));
      setSelectedId(image.id);
      setTool("select");
    } catch (cause) {
      setError(errorMessage(cause, t("captureFailed"), locale));
    } finally {
      setBusyAction(null);
      setCaptureMode(false);
      restoreCaptureButtonFocus();
    }
  }

  function restoreCaptureButtonFocus() {
    window.requestAnimationFrame(() => captureButtonRef.current?.focus({ preventScroll: true }));
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
        setError(t("referenceTooLarge"));
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
        setError(t("referenceInvalid"));
        return;
      }
      onAttach(reference);
    } catch (cause) {
      setError(errorMessage(cause, t("exportFailed"), locale));
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

  if (captureMode) {
    return <PageCaptureOverlay
      busy={busyAction === "capture"}
      title={t("captureRegionTitle")}
      help={t("captureRegionHelp")}
      busyLabel={t("capturingRegion")}
      tooSmallLabel={t("captureRegionTooSmall")}
      cancelLabel={t("cancelCapture")}
      onCancel={cancelPageCapture}
      onSelect={(region) => void capturePage(region)}
    />;
  }

  const tools = (["select", "draw", "rectangle", "text"] as const);

  return <div className="a3s-design-layer" data-side="right">
    <section
      ref={dialogRef}
      className="a3s-design-board"
      data-theme={theme}
      role="region"
      aria-labelledby={titleId}
      aria-describedby={descriptionId}
      onKeyDownCapture={onDialogKeyDownCapture}
    >
      <div className="a3s-design-header">
        <span className="a3s-design-header-icon" aria-hidden="true"><DesignGlyph name="board" /></span>
        <div className="a3s-design-heading">
          <strong id={titleId}>{t("referenceTitle")}</strong>
          <small id={descriptionId}>{t("referenceDescription")}</small>
        </div>
        <button type="button" className="a3s-close" aria-label={t("closeBoard")} title={t("closeBoard")} onClick={onCancel}>
          <DesignGlyph name="close" />
          <span className="a3s-sr-only">{t("closeBoard")}</span>
        </button>
      </div>
      <div className="a3s-design-body">
        <div className="a3s-design-toolbar toolbar" role="toolbar" aria-label={t("boardActions")}>
          <div className="a3s-design-tool-group" aria-label={t("tools")}>
            {tools.map((value) => {
              const label = designToolLabel(t, value);
              return <button
              key={value}
              type="button"
              data-testid={`design-tool-${value}`}
              aria-label={label}
              aria-pressed={tool === value}
              className={`a3s-design-tool${tool === value ? " selected" : ""}`}
              disabled={busy}
              title={label}
              onClick={() => setTool(value)}
              ><DesignGlyph name={value as DesignGlyphName} /><span>{label}</span></button>;
            })}
          </div>
          <span className="a3s-design-divider" aria-hidden="true" />
          <div className="a3s-design-tool-group a3s-design-media" aria-label={t("imageHelp")}>
            <button ref={captureButtonRef} type="button" disabled={busy} aria-label={t("capturePage")} title={t("capturePageHelp")} onClick={startPageCapture}>
              <DesignGlyph name="capture" /><span>{busyAction === "capture" ? t("capturingPage") : t("capturePage")}</span>
            </button>
            <button type="button" disabled={busy} title={t("uploadScreenshot")} onClick={() => fileInputRef.current?.click()}>
              <DesignGlyph name="upload" /><span>{busyAction === "import" ? t("importing") : t("uploadScreenshot")}</span>
            </button>
            <input ref={fileInputRef} aria-label={t("screenshotInput")} hidden type="file" accept="image/png,image/jpeg" onChange={(event) => void importFile(event.target.files?.[0])} />
          </div>
          {showHistory && <>
            <span className="a3s-design-divider" aria-hidden="true" />
            <div className="a3s-design-history" aria-label={t("history")}>
              <IconButton icon="undo" label={t("undo")} disabled={busy || history.past.length === 0} onClick={undo} />
              <IconButton icon="redo" label={t("redo")} disabled={busy || history.future.length === 0} onClick={redo} />
              <IconButton icon="trash" label={t("clearBoard")} disabled={busy || !summary.kind} onClick={clearBoard} />
            </div>
          </>}
          {showStyles && <div className="a3s-design-style" aria-label={t("styles")}>
            <label className="a3s-design-color" title={t("strokeColor")}>
              <span className="a3s-sr-only">{t("strokeColor")}</span>
              <input aria-label={t("strokeColor")} type="color" value={color} disabled={busy} onChange={(event) => setColor(event.target.value)} />
            </label>
            {tool === "rectangle" && <label><span>{t("shapeFill")}</span><select aria-label={t("shapeFill")} value={fill} disabled={busy} onChange={(event) => setFill(event.target.value)}>
              <option value="transparent">{t("fillNone")}</option>
              <option value="#e0f2fe">{t("fillBlue")}</option>
              <option value="#fef3c7">{t("fillAmber")}</option>
              <option value="#dcfce7">{t("fillGreen")}</option>
              <option value="#fce7f3">{t("fillPink")}</option>
            </select></label>}
            {tool !== "text" && <label><span>{t("strokeWidth")}</span><select aria-label={t("strokeWidth")} value={strokeWidth} disabled={busy} onChange={(event) => setStrokeWidth(Number(event.target.value))}>
              <option value="2">{t("widthSmall")}</option><option value="4">{t("widthMedium")}</option><option value="8">{t("widthLarge")}</option>
            </select></label>}
          </div>}
        </div>
        <div className="a3s-design-stage" data-content={summaryLabel}>
          <div className="a3s-design-canvas">
            {!summary.kind && <div className="a3s-design-empty" aria-hidden="true">
              <span><DesignGlyph name="draw" /></span>
              <strong>{t("emptyTitle")}</strong>
              <small>{t("emptyDescription")}</small>
            </div>}
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
              onLimit={() => setError(t("objectLimit", { limit: MAX_DESIGN_ELEMENTS }))}
            />
          </div>
        </div>
      </div>
      <div className="a3s-design-footer">
        <output className="a3s-sr-only" aria-label={t("objectCount")}>{history.present.length}/{MAX_DESIGN_ELEMENTS}</output>
        <p id={statusId} className={`a3s-design-status${error ? " is-error" : ""}`} role={error ? "alert" : "status"}>
          {error || t("status", { summary: summaryLabel, help: t("canvasHelp") })}
        </p>
        <div className="a3s-design-actions">
          <button type="button" className="quiet" onClick={onCancel}>{t("cancel")}</button>
          <button type="button" className="a3s-design-attach" disabled={!summary.kind || busy} onClick={() => void attachReference()}>
            <DesignGlyph name="check" /><span>{busyAction === "export" ? t("exporting") : t("attach")}</span>
          </button>
        </div>
      </div>
    </section>
  </div>;
}

function IconButton({
  icon,
  label,
  disabled,
  onClick,
}: {
  icon: DesignGlyphName;
  label: string;
  disabled: boolean;
  onClick(): void;
}) {
  return <button type="button" aria-label={label} title={label} disabled={disabled} onClick={onClick}>
    <DesignGlyph name={icon} />
    <span className="a3s-sr-only">{label}</span>
  </button>;
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

function errorMessage(
  cause: unknown,
  fallback: string,
  locale: "en" | "zh-CN",
): string {
  return locale === "en" && cause instanceof Error && cause.message
    ? cause.message
    : fallback;
}
