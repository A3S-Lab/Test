// @vitest-environment node

import { describe, expect, it } from "vitest";
import {
  getPageContextBridge,
  installTestKit,
  registerBoundary,
} from "./runtime";

describe("framework-neutral server usage", () => {
  it("stays inspectable while refusing to enable a browser runtime", () => {
    expect(getPageContextBridge()).toBeNull();
    expect(() => registerBoundary({
      id: "server",
      name: "Server boundary",
      elements: () => [],
    })).toThrowError("A3S Test Kit must be installed before registering a boundary");
    expect(() => installTestKit({
      enabled: true,
      page: { id: "server" },
    })).toThrowError("A3S Test Kit can only be enabled in a browser");

    const disabled = installTestKit({
      enabled: false,
      page: { id: "server" },
    });
    expect(disabled.listRepairs()).toEqual([]);
    expect(disabled.listQualityReports()).toEqual([]);
    disabled.dispose();
  });
});
