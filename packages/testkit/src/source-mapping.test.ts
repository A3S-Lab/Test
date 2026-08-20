import { describe, expect, it } from "vitest";
import {
  installTestKit,
  registerBoundary,
  registerSource,
  registerSourceMap,
} from "./runtime";
import { setRect } from "./test-setup";
import type { SourceSpan } from "./types";

describe("rendered node source mapping", () => {
  it("ranks declared framework ownership ahead of an enclosing boundary hint", () => {
    document.body.innerHTML =
      '<section id="checkout"><button data-testid="pay">Pay now</button></section>';
    const section = document.querySelector("section")!;
    const button = document.querySelector("button")!;
    setRect(section, { x: 20, y: 20, width: 320, height: 160 });
    setRect(button, { x: 40, y: 60, width: 100, height: 36 });
    Object.defineProperty(button, "__reactFiber$private", {
      value: { source: "src/UndeclaredSecret.tsx" },
    });

    const bridge = installTestKit({
      enabled: true,
      page: { id: "source-ranking" },
      repairStorage: "memory",
    });
    registerBoundary({
      id: "checkout",
      name: "Checkout",
      elements: () => [section],
      source: { file: "src/Checkout.tsx", line: 10, column: 3 },
    });
    const unregisterOwner = registerSource({
      id: "react:pay-button",
      framework: "react",
      elements: () => [button],
      includeDescendants: false,
      source: {
        file: "src/PayButton.tsx",
        line: 42,
        column: 5,
        privateSourceText: "must not cross the bridge",
      } as SourceSpan,
    });

    const node = bridge
      .snapshot({ detail: "forensic" })
      .nodes.find((candidate) => candidate.testId === "pay")!;
    expect(node.sourceMapping).toMatchObject({
      protocol: "a3s.test.source-mapping/1",
      truncated: false,
      candidates: [
        {
          span: { file: "src/PayButton.tsx", line: 42, column: 5 },
          confidence: 0.99,
          origin: "framework_adapter",
          relation: "exact",
          registrationId: "react:pay-button",
          framework: "react",
        },
        {
          span: { file: "src/Checkout.tsx", line: 10, column: 3 },
          origin: "boundary_hint",
          relation: "ancestor",
          registrationId: "checkout",
          componentId: "checkout",
        },
      ],
    });
    expect(JSON.stringify(node)).not.toContain("UndeclaredSecret");
    expect(JSON.stringify(node)).not.toContain("privateSourceText");
    expect(JSON.stringify(node)).not.toContain("must not cross the bridge");

    const repair = bridge.submitRepair({
      findings: [
        {
          id: "source-repair",
          instruction: "Emphasize the payment action",
          intent: "change",
          severity: "important",
          target: { kind: "node", nodeIds: [node.id] },
          createdAt: "2026-08-20T00:00:00.000Z",
        },
      ],
    })[0]!;
    expect(repair.context.nodes[0]?.sourceMapping).toEqual(node.sourceMapping);
    unregisterOwner();
    unregisterOwner();
  });

  it("traces a declared generated location through a bounded source map", () => {
    document.body.innerHTML = '<button data-testid="mapped">Mapped</button>';
    const button = document.querySelector("button")!;
    setRect(button, { x: 20, y: 20, width: 100, height: 36 });
    const bridge = installTestKit({
      enabled: true,
      page: { id: "source-map" },
      repairStorage: "memory",
    });
    const unregisterMap = registerSourceMap({
      id: "vite-app",
      generatedFile: "http://localhost:3000/assets/app.js",
      mapUrl: "http://localhost:3000/assets/app.js.map",
      map: {
        version: 3,
        file: "app.js",
        names: [],
        sources: ["../src/App.tsx"],
        sourcesContent: ["private source text must not cross the bridge"],
        mappings: "AAAA",
      },
    });
    const unregisterOwner = registerSource({
      id: "vite:mapped-button",
      framework: "react",
      elements: () => [button],
      includeDescendants: false,
      generated: {
        file: "http://localhost:3000/assets/app.js?cache=1",
        line: 1,
        column: 1,
      },
    });

    const mapped = bridge
      .snapshot()
      .nodes.find((candidate) => candidate.testId === "mapped")!;
    expect(mapped.sourceMapping?.candidates[0]).toMatchObject({
      confidence: 0.97,
      origin: "source_map",
      relation: "exact",
      registrationId: "vite:mapped-button",
      framework: "react",
      span: { line: 1, column: 1 },
      generatedSpan: {
        file: "http://localhost:3000/assets/app.js?cache=1",
        line: 1,
        column: 1,
      },
    });
    expect(mapped.sourceMapping?.candidates[0]?.span.file).toMatch(
      /\/src\/App\.tsx$/,
    );
    expect(JSON.stringify(mapped)).not.toContain("private source text");

    unregisterMap();
    unregisterMap();
    const generated = bridge
      .snapshot()
      .nodes.find((candidate) => candidate.testId === "mapped")!;
    expect(generated.sourceMapping?.candidates[0]).toMatchObject({
      confidence: 0.68,
      origin: "generated",
      span: {
        file: "http://localhost:3000/assets/app.js?cache=1",
        line: 1,
        column: 1,
      },
    });

    unregisterOwner();
    unregisterOwner();
    const removed = bridge
      .snapshot()
      .nodes.find((candidate) => candidate.testId === "mapped")!;
    expect(removed.sourceMapping).toBeUndefined();
  });

  it("bounds a ranked candidate list and reports truncation", () => {
    document.body.innerHTML = '<button data-testid="ranked">Ranked</button>';
    const button = document.querySelector("button")!;
    setRect(button, { x: 20, y: 20, width: 100, height: 36 });
    const bridge = installTestKit({
      enabled: true,
      page: { id: "source-truncation" },
      repairStorage: "memory",
    });
    for (let index = 0; index < 10; index += 1) {
      registerSource({
        id: `react:owner-${index}`,
        framework: "react",
        elements: () => [button],
        includeDescendants: false,
        source: { file: `src/Owner${index}.tsx`, line: index + 1 },
      });
    }

    const mapping = bridge
      .snapshot()
      .nodes.find((candidate) => candidate.testId === "ranked")!.sourceMapping!;
    expect(mapping.candidates).toHaveLength(8);
    expect(mapping.truncated).toBe(true);
    expect(
      mapping.candidates.every((candidate) => candidate.confidence === 0.99),
    ).toBe(true);
  });

  it("rejects ambiguous registrations and malformed or oversized maps", () => {
    document.body.innerHTML = "<button>Target</button>";
    const button = document.querySelector("button")!;
    const bridge = installTestKit({
      enabled: true,
      page: { id: "source-validation" },
      repairStorage: "memory",
    });

    registerBoundary({
      id: "b".repeat(129),
      name: "Legacy long boundary",
      elements: () => [button],
      source: { file: "src/Legacy.tsx" },
    });
    expect(
      bridge.snapshot().nodes.find((node) => node.tag === "button")
        ?.sourceMapping,
    ).toBeUndefined();

    expect(() =>
      registerSource({
        id: "invalid",
        framework: "react",
        elements: () => [button],
      }),
    ).toThrow(/source or generated/i);
    expect(() =>
      registerSourceMap({
        id: "invalid-map",
        generatedFile: "app.js",
        map: {
          version: 3,
          names: [],
          sources: ["src/App.tsx"],
          mappings: "!",
        },
      }),
    ).toThrow(/source map/i);
    expect(() =>
      registerSourceMap({
        id: "oversized-map",
        generatedFile: "app.js",
        map: {
          version: 3,
          names: [],
          sources: ["src/App.tsx"],
          mappings: "A".repeat(1_000_001),
        },
      }),
    ).toThrow(/bounded/i);
  });
});
