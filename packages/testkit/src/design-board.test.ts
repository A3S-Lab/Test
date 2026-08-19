import { describe, expect, it } from "vitest";
import {
  commitDesignHistory,
  createDesignHistory,
  moveDesignElement,
  redoDesignHistory,
  resizeDesignElement,
  summarizeBoard,
  undoDesignHistory,
  type DesignElement,
  type DesignImageElement,
} from "./design-board-model";

describe("native design board model", () => {
  it("reports blank, screenshot, and authored sketch boards", () => {
    expect(summarizeBoard([])).toEqual({ kind: null, label: "Blank board", hasImage: false });
    expect(summarizeBoard([screenshot()])).toEqual({
      kind: "screenshot",
      label: "Screenshot",
      hasImage: true,
    });
    expect(summarizeBoard([rectangle()])).toEqual({
      kind: "sketch",
      label: "UI sketch",
      hasImage: false,
    });
    expect(summarizeBoard([screenshot(), rectangle()])).toEqual({
      kind: "sketch",
      label: "Screenshot with sketch annotations",
      hasImage: true,
    });
  });

  it("retains the kind of a restored flattened sketch", () => {
    expect(summarizeBoard([{ ...screenshot(), referenceKind: "sketch" }])).toEqual({
      kind: "sketch",
      label: "Existing sketch",
      hasImage: true,
    });
  });

  it("supports bounded undo and redo history", () => {
    const empty = createDesignHistory();
    const committed = commitDesignHistory(empty, [rectangle()]);
    expect(committed.past).toHaveLength(1);
    expect(committed.future).toHaveLength(0);
    const undone = undoDesignHistory(committed);
    expect(undone.present).toEqual([]);
    expect(undone.future).toHaveLength(1);
    expect(redoDesignHistory(undone).present).toEqual(committed.present);
  });

  it("rejects a commit above the 250-object board limit", () => {
    const history = createDesignHistory();
    const oversized = Array.from({ length: 251 }, (_, index) => ({
      ...rectangle(),
      id: `rectangle-${index}`,
    }));
    expect(commitDesignHistory(history, oversized)).toBe(history);
  });

  it("clamps object movement and resizing to the 960 by 600 surface", () => {
    const original = rectangle();
    expect(moveDesignElement(original, -200, -300)).toMatchObject({ x: 0, y: 0 });
    expect(moveDesignElement(original, 2_000, 2_000)).toMatchObject({ x: 840, y: 520 });
    expect(resizeDesignElement(original, { x: 2_000, y: 2_000 })).toMatchObject({
      width: 920,
      height: 560,
    });
  });
});

function screenshot(): DesignImageElement {
  return {
    id: "image",
    kind: "image",
    x: 40,
    y: 40,
    width: 880,
    height: 520,
    src: "data:image/png;base64,AQ==",
    mediaType: "image/png",
    referenceKind: "screenshot",
    background: true,
  };
}

function rectangle(): DesignElement {
  return {
    id: "rectangle",
    kind: "rectangle",
    x: 40,
    y: 40,
    width: 120,
    height: 80,
    color: "#111827",
    fill: "transparent",
    strokeWidth: 4,
  };
}
