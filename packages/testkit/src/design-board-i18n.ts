import { useMemo } from "react";
import type { DesignBoardSummary, DesignTool } from "./design-board-model";
import { useReviewI18n } from "./review-locale";

const EN_DESIGN_BOARD_MESSAGES = {
  referenceTitle: "Design reference",
  referenceDescription: "Sketch the desired UI or add a screenshot.",
  referencePromptTitle: "Show the intended UI",
  referencePromptDescription: "Draw a sketch or attach a screenshot for this target.",
  referenceSketchAttached: "Sketch attached",
  referenceScreenshotAttached: "Screenshot attached",
  referenceStored: "kept with this finding",
  referenceSketchAlt: "Sketch of the desired UI",
  referenceScreenshotAlt: "Screenshot of the desired UI",
  openBoard: "Open design board",
  editReference: "Edit",
  removeReference: "Remove",
  sketch: "Sketch",
  screenshot: "Screenshot",
  attachedAnnouncement: "{kind} attached to finding",
  closeBoard: "Close design board",
  boardActions: "Design board actions",
  capturePage: "Capture current page",
  capturePageHelp: "Capture only the visible browser page without screen-sharing permission",
  capturingPage: "Capturing page…",
  uploadScreenshot: "Upload screenshot",
  importing: "Importing…",
  screenshotInput: "Choose a PNG or JPEG screenshot",
  history: "Canvas history",
  undo: "Undo",
  redo: "Redo",
  clearBoard: "Clear board",
  imageHelp: "Capture the visible page without screen-sharing permission, or add a PNG or JPEG up to 8 MiB.",
  tools: "Design tools",
  styles: "Drawing style",
  toolSelect: "Select",
  toolDraw: "Draw",
  toolRectangle: "Rectangle",
  toolText: "Text",
  strokeColor: "Stroke color",
  shapeFill: "Shape fill",
  fillNone: "None",
  fillBlue: "Blue",
  fillAmber: "Amber",
  fillGreen: "Green",
  fillPink: "Pink",
  strokeWidth: "Stroke width",
  widthSmall: "Small",
  widthMedium: "Medium",
  widthLarge: "Large",
  objectCount: "Design object count",
  canvas: "Desired UI design canvas",
  textInput: "Design text",
  emptyTitle: "Draw the interface you want",
  emptyDescription: "Draw with a tool above, capture the current page, or add a screenshot.",
  blankBoard: "Blank board",
  annotatedScreenshot: "Screenshot with sketch annotations",
  uiSketch: "UI sketch",
  existingSketch: "Existing sketch",
  canvasHelp: "Draw, add rectangles or text, then use Select to move and resize objects.",
  status: "{summary}. {help}",
  cancel: "Cancel",
  attach: "Attach to finding",
  exporting: "Exporting…",
  storedArtifactUnavailable: "This stored artifact cannot be edited here. Import its PNG or JPEG copy instead.",
  existingReferenceOpenFailed: "The existing design reference could not be opened.",
  invalidImageType: "Choose a PNG or JPEG screenshot.",
  invalidImageSize: "The screenshot must be between 1 byte and 8 MiB.",
  removeObjectBeforeScreenshot: "Remove an object before adding a screenshot; the board limit is {limit}.",
  screenshotOpenFailed: "The screenshot could not be opened.",
  captureFailed: "The current page could not be captured. Upload, paste, or drop a screenshot instead.",
  referenceTooLarge: "The design reference is too large. Remove detail or use a smaller screenshot.",
  referenceInvalid: "The browser produced an invalid design reference. Try a smaller screenshot.",
  exportFailed: "The design reference could not be exported.",
  objectLimit: "The design board is limited to {limit} objects.",
  freehandStroke: "Freehand stroke",
  rectangleElement: "Rectangle",
  textElement: "Text: {text}",
  existingDesignSketch: "Existing design sketch",
  screenshotElement: "Screenshot",
} as const;

export type DesignBoardMessageKey = keyof typeof EN_DESIGN_BOARD_MESSAGES;
export type DesignBoardMessageValues = Record<string, string | number>;
export type DesignBoardTranslator = (
  key: DesignBoardMessageKey,
  values?: DesignBoardMessageValues,
) => string;

const ZH_CN_DESIGN_BOARD_MESSAGES: Record<DesignBoardMessageKey, string> = {
  referenceTitle: "设计参考",
  referenceDescription: "画出期望的界面，或添加一张截图。",
  referencePromptTitle: "补充设计参考",
  referencePromptDescription: "为当前目标手绘草图，或添加一张参考截图。",
  referenceSketchAttached: "已添加草图",
  referenceScreenshotAttached: "已添加截图",
  referenceStored: "随当前问题保存",
  referenceSketchAlt: "期望界面的草图",
  referenceScreenshotAlt: "期望界面的截图",
  openBoard: "打开画板",
  editReference: "编辑",
  removeReference: "移除",
  sketch: "草图",
  screenshot: "截图",
  attachedAnnouncement: "已将{kind}添加到问题",
  closeBoard: "关闭画板",
  boardActions: "画板操作",
  capturePage: "截取当前页面",
  capturePageHelp: "只截取当前可见的浏览器页面，不需要屏幕录制权限",
  capturingPage: "正在截取页面…",
  uploadScreenshot: "上传截图",
  importing: "正在导入…",
  screenshotInput: "选择 PNG 或 JPEG 截图",
  history: "画板历史",
  undo: "撤销",
  redo: "重做",
  clearBoard: "清空画板",
  imageHelp: "可直接截取当前可见页面，无需屏幕录制权限；也支持不超过 8 MiB 的 PNG 或 JPEG。",
  tools: "绘图工具",
  styles: "绘图样式",
  toolSelect: "选择",
  toolDraw: "画笔",
  toolRectangle: "矩形",
  toolText: "文字",
  strokeColor: "线条颜色",
  shapeFill: "形状填充",
  fillNone: "无",
  fillBlue: "蓝色",
  fillAmber: "琥珀色",
  fillGreen: "绿色",
  fillPink: "粉色",
  strokeWidth: "线条粗细",
  widthSmall: "细",
  widthMedium: "中",
  widthLarge: "粗",
  objectCount: "画板对象数量",
  canvas: "设计画布",
  textInput: "画板文字",
  emptyTitle: "画出你想要的界面",
  emptyDescription: "可以直接绘制、截取当前页面，或添加一张截图。",
  blankBoard: "空白画板",
  annotatedScreenshot: "带草图标注的截图",
  uiSketch: "界面草图",
  existingSketch: "已有草图",
  canvasHelp: "可绘制线条、矩形和文字；切换到“选择”后可以移动或缩放对象。",
  status: "{summary}。{help}",
  cancel: "取消",
  attach: "添加到问题",
  exporting: "正在生成…",
  storedArtifactUnavailable: "这个已保存的文件无法直接编辑，请导入它的 PNG 或 JPEG 副本。",
  existingReferenceOpenFailed: "无法打开已有的设计参考。",
  invalidImageType: "请选择 PNG 或 JPEG 截图。",
  invalidImageSize: "截图大小必须在 1 字节到 8 MiB 之间。",
  removeObjectBeforeScreenshot: "请先移除一个对象再添加截图，画板最多支持 {limit} 个对象。",
  screenshotOpenFailed: "无法打开这张截图。",
  captureFailed: "无法截取当前页面，请改用上传、粘贴或拖入截图。",
  referenceTooLarge: "设计参考仍然过大，请减少细节或使用更小的截图。",
  referenceInvalid: "浏览器生成的设计参考无效，请尝试更小的截图。",
  exportFailed: "无法生成设计参考。",
  objectLimit: "画板最多支持 {limit} 个对象。",
  freehandStroke: "手绘线条",
  rectangleElement: "矩形",
  textElement: "文字：{text}",
  existingDesignSketch: "已有设计草图",
  screenshotElement: "截图",
};

export function useDesignBoardI18n(): {
  locale: "en" | "zh-CN";
  t: DesignBoardTranslator;
} {
  const { locale } = useReviewI18n();
  const catalog = locale === "zh-CN"
    ? ZH_CN_DESIGN_BOARD_MESSAGES
    : EN_DESIGN_BOARD_MESSAGES;
  return useMemo(() => ({
    locale,
    t: ((key, values = {}) => catalog[key].replace(
      /\{([a-zA-Z0-9_]+)\}/g,
      (placeholder, name: string) => Object.hasOwn(values, name)
        ? String(values[name])
        : placeholder,
    )) as DesignBoardTranslator,
  }), [catalog, locale]);
}

export function designToolLabel(t: DesignBoardTranslator, tool: DesignTool): string {
  return t(({
    select: "toolSelect",
    draw: "toolDraw",
    rectangle: "toolRectangle",
    text: "toolText",
  } as const)[tool]);
}

export function designSummaryLabel(
  t: DesignBoardTranslator,
  summary: DesignBoardSummary,
): string {
  if (!summary.kind) return t("blankBoard");
  return t(({
    "Screenshot with sketch annotations": "annotatedScreenshot",
    "UI sketch": "uiSketch",
    "Existing sketch": "existingSketch",
    Screenshot: "screenshot",
  } as const)[summary.label] ?? "uiSketch");
}
