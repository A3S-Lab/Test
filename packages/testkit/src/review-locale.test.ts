import { describe, expect, it } from "vitest";
import {
  createReviewI18n,
  resolveReviewLocale,
  reviewStatusLabel,
  reviewTargetSummary,
} from "./review-locale";

describe("review localization", () => {
  it("resolves Simplified and Traditional Chinese page tags to the Chinese overlay", () => {
    expect(resolveReviewLocale("auto", "zh-CN")).toBe("zh-CN");
    expect(resolveReviewLocale("auto", "zh-Hans-SG")).toBe("zh-CN");
    expect(resolveReviewLocale("auto", "zh-Hant-TW")).toBe("zh-CN");
    expect(resolveReviewLocale("auto", "de-DE")).toBe("en");
    expect(resolveReviewLocale("en", "zh-CN")).toBe("en");
  });

  it("keeps message overrides known, non-empty, and bounded", () => {
    const i18n = createReviewI18n("zh-CN", {
      reviewTitle: "页面评审",
      reviewDescription: " ",
      emptyWorkspace: "x".repeat(2_049),
    });
    expect(i18n.t("reviewTitle")).toBe("页面评审");
    expect(i18n.t("reviewDescription")).toBe("发送前仅保存在本页");
    expect(i18n.t("emptyWorkspace")).toContain("先标记一个元素");
  });

  it("localizes repair state and target summaries without changing protocol values", () => {
    const { t } = createReviewI18n("zh-CN");
    expect(t("componentPlaceholder")).toBe("区块");
    expect(reviewStatusLabel(t, "review_ready")).toBe("等待验收");
    expect(reviewTargetSummary(t, { kind: "node", nodeIds: ["one", "two"] })).toBe("2 个元素");
    expect(reviewTargetSummary(t, {
      kind: "region",
      nodeIds: [],
      region: { x: 0, y: 0, width: 80, height: 40 },
      layout: { kind: "placement", componentType: "Checkout Form", canvas: "page" },
    })).toBe("布局放置 · Checkout Form · 当前页面");
  });

  it("keeps the English component placeholder in the English catalog", () => {
    expect(createReviewI18n("en").t("componentPlaceholder")).toBe("Section");
  });
});
