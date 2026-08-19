import { describe, expect, it } from "vitest";
import { installTestKit } from "./runtime";
import {
  clearReviewDrafts,
  loadReviewDrafts,
  reviewDraftStorageKey,
  reviewScope,
  saveReviewDrafts,
  type ReviewDraftItem,
} from "./review-storage";
import type { RepairDraft, TestKitRuntime } from "./types";

const NOW = Date.parse("2026-08-13T09:00:00.000Z");

describe("review draft storage", () => {
  it("rebinds a page-local node draft by a unique semantic locator after reload", () => {
    document.body.innerHTML = '<main><button data-testid="save-target">Save target</button></main>';
    const first = runtime("persisted-review");
    const firstNode = targetNode(first);
    const item = reviewItem(draft("persisted", firstNode.id));
    item.draft.designReference = {
      kind: "sketch",
      width: 960,
      height: 600,
      image: { kind: "inline", mediaType: "image/png", dataUrl: "data:image/png;base64,AAAA" },
    };

    saveReviewDrafts(first, [item], window.localStorage, NOW);
    first.dispose();
    document.querySelector("main")?.insertAdjacentHTML("afterbegin", "<section>New sibling</section>");
    const second = runtime("persisted-review");
    const secondNode = targetNode(second);

    expect(secondNode.id).not.toBe(firstNode.id);
    expect(loadReviewDrafts(second, window.localStorage, NOW)).toEqual([
      {
        ...item,
        draft: {
          ...item.draft,
          target: { ...item.draft.target, nodeIds: [secondNode.id] },
        },
      },
    ]);
  });

  it("isolates drafts by page identity and SPA route", () => {
    document.body.innerHTML = '<button data-testid="save-target">Save target</button>';
    window.history.replaceState(null, "", "/account/profile?mode=review");
    const bridge = runtime("route-review");
    const item = reviewItem(draft("profile", targetNode(bridge).id));
    saveReviewDrafts(bridge, [item], window.localStorage, NOW);
    const profileScope = reviewScope(bridge);

    window.history.pushState(null, "", "/account/security");
    expect(loadReviewDrafts(bridge, window.localStorage, NOW)).toEqual([]);
    expect(reviewScope(bridge)).not.toEqual(profileScope);

    window.history.pushState(null, "", "/account/profile?mode=review");
    expect(loadReviewDrafts(bridge, window.localStorage, NOW)).toHaveLength(1);
    clearReviewDrafts(profileScope, window.localStorage);
    expect(window.localStorage.getItem(reviewDraftStorageKey(profileScope))).toBeNull();
  });

  it("fails closed when a required target no longer resolves uniquely", () => {
    document.body.innerHTML = '<button data-testid="save-target">Save target</button>';
    const first = runtime("ambiguous-review");
    saveReviewDrafts(
      first,
      [reviewItem(draft("ambiguous", targetNode(first).id))],
      window.localStorage,
      NOW,
    );
    first.dispose();

    document.body.innerHTML = [
      '<button data-testid="save-target">Save target</button>',
      '<button data-testid="save-target">Save target</button>',
    ].join("");
    const second = runtime("ambiguous-review");
    expect(loadReviewDrafts(second, window.localStorage, NOW)).toEqual([]);
  });

  it("rejects corrupt, expired, oversized, and structurally invalid records", () => {
    document.body.innerHTML = '<button data-testid="save-target">Save target</button>';
    const bridge = runtime("invalid-review");
    const scope = reviewScope(bridge);
    const key = reviewDraftStorageKey(scope);

    for (const encoded of [
      "{broken",
      JSON.stringify({ version: 1, pageId: scope.pageId, route: scope.route, savedAt: NOW - 8 * 24 * 60 * 60 * 1_000, items: [] }),
      JSON.stringify({ version: 1, pageId: scope.pageId, route: scope.route, savedAt: NOW, items: [{ draft: { instruction: "missing target" } }] }),
      "x".repeat(600_000),
    ]) {
      window.localStorage.setItem(key, encoded);
      expect(loadReviewDrafts(bridge, window.localStorage, NOW)).toEqual([]);
      expect(window.localStorage.getItem(key)).toBeNull();
    }
  });
});

function runtime(pageId: string): TestKitRuntime {
  return installTestKit({ enabled: true, page: { id: pageId }, repairStorage: "memory" });
}

function targetNode(bridge: TestKitRuntime) {
  const node = bridge
    .snapshot({ detail: "forensic", limits: { nodes: 5_000 } })
    .nodes.find((candidate) => candidate.testId === "save-target");
  if (!node) throw new Error("missing target node");
  return node;
}

function draft(id: string, nodeId: string): RepairDraft {
  return {
    id: `finding-${id}`,
    instruction: `Fix ${id}`,
    intent: "fix",
    severity: "important",
    target: { kind: "node", nodeIds: [nodeId] },
    createdAt: "2026-08-13T08:00:00.000Z",
  };
}

function reviewItem(value: RepairDraft): ReviewDraftItem {
  return { draft: value, selected: true, hidden: false };
}
