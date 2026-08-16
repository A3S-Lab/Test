import { describe, expect, it } from "vitest";
import {
  COMPONENT_CATALOG,
  componentCatalogItemLabel,
  componentCatalogSize,
  filterComponentCatalog,
  localizeComponentCatalogName,
} from "./component-catalog";

describe("Web component catalog", () => {
  it("defines at least 65 unique component types across useful categories", () => {
    const names = COMPONENT_CATALOG.flatMap((group) => (
      group.components.map((component) => component.name)
    ));

    expect(COMPONENT_CATALOG.length).toBeGreaterThanOrEqual(6);
    expect(componentCatalogSize()).toBe(90);
    expect(new Set(names).size).toBe(names.length);
    expect(COMPONENT_CATALOG.every((group) => group.components.length > 0)).toBe(true);
  });

  it("filters by component name, category, and declared search terms", () => {
    expect(filterComponentCatalog("checkout").flatMap((group) => group.components.map((component) => component.name))).toContain("Checkout Form");
    expect(filterComponentCatalog("navigation").every((group) => group.name === "Navigation")).toBe(true);
    expect(filterComponentCatalog("one-time code").flatMap((group) => group.components.map((component) => component.name))).toContain("Verification Code Input");
    expect(filterComponentCatalog("no-such-component")).toEqual([]);
    expect(filterComponentCatalog(" ")).toEqual(COMPONENT_CATALOG);
  });

  it("provides complete Chinese labels and searches both catalog languages", () => {
    const components = COMPONENT_CATALOG.flatMap((group) => group.components);
    const chineseNames = components.map((component) => component.zhCNName);
    const chineseGroups = COMPONENT_CATALOG.map((group) => group.zhCNName);
    const checkout = components.find((component) => component.name === "Checkout Form")!;

    expect(chineseNames.every((name) => name.trim().length > 0)).toBe(true);
    expect(new Set(chineseNames).size).toBe(chineseNames.length);
    expect(chineseGroups.every((name) => name.trim().length > 0)).toBe(true);
    expect(new Set(chineseGroups).size).toBe(chineseGroups.length);
    expect(filterComponentCatalog("结账").flatMap((group) => group.components.map((component) => component.name))).toContain("Checkout Form");
    expect(filterComponentCatalog("导航").every((group) => group.name === "Navigation")).toBe(true);
    expect(componentCatalogItemLabel(checkout, "zh-CN")).toBe("结账表单");
    expect(componentCatalogItemLabel(checkout, "en")).toBe("Checkout Form");
    expect(localizeComponentCatalogName("Checkout Form", "zh-CN")).toBe("结账表单");
    expect(localizeComponentCatalogName("结账表单", "en")).toBe("Checkout Form");
    expect(localizeComponentCatalogName("Custom orbit panel", "zh-CN")).toBe("Custom orbit panel");
  });
});
