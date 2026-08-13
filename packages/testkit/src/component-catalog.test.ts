import { describe, expect, it } from "vitest";
import {
  COMPONENT_CATALOG,
  componentCatalogSize,
  filterComponentCatalog,
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
});
