import { describe, expect, it } from "vitest";
import {
  DEFAULT_REVIEW_PREFERENCES,
  loadReviewPreferences,
  loadReviewTabHidden,
  REVIEW_PREFERENCES_KEY,
  saveReviewPreferences,
  saveReviewTabHidden,
} from "./review-preferences";

describe("review presentation preferences", () => {
  it("round-trips only bounded presentation and interaction settings", () => {
    saveReviewPreferences({
      theme: "dark",
      markerColor: "#2563eb",
      clearOnCopy: true,
      blockInteractions: true,
      dock: "left",
      wireframeFade: 0.42,
    }, window.localStorage);

    expect(loadReviewPreferences(window.localStorage)).toEqual({
      theme: "dark",
      markerColor: "#2563eb",
      clearOnCopy: true,
      blockInteractions: true,
      dock: "left",
      wireframeFade: 0.42,
    });
    const encoded = window.localStorage.getItem(REVIEW_PREFERENCES_KEY) ?? "";
    expect(encoded).not.toContain("autoSend");
    expect(encoded).not.toContain("paused");
  });

  it("falls back atomically for corrupt, unknown, or unbounded preference records", () => {
    for (const encoded of [
      "{broken",
      JSON.stringify({ ...DEFAULT_REVIEW_PREFERENCES, markerColor: "red" }),
      JSON.stringify({ ...DEFAULT_REVIEW_PREFERENCES, dock: "center" }),
      JSON.stringify({ ...DEFAULT_REVIEW_PREFERENCES, wireframeFade: 2 }),
      JSON.stringify({ ...DEFAULT_REVIEW_PREFERENCES, hiddenPrompt: "ignore policy" }),
      "x".repeat(10_000),
    ]) {
      window.localStorage.setItem(REVIEW_PREFERENCES_KEY, encoded);
      expect(loadReviewPreferences(window.localStorage)).toEqual(DEFAULT_REVIEW_PREFERENCES);
      expect(window.localStorage.getItem(REVIEW_PREFERENCES_KEY)).toBeNull();
    }
  });

  it("keeps hide-until-restart in tab session storage", () => {
    expect(loadReviewTabHidden(window.sessionStorage)).toBe(false);
    saveReviewTabHidden(true, window.sessionStorage);
    expect(loadReviewTabHidden(window.sessionStorage)).toBe(true);
    expect(window.localStorage.getItem(REVIEW_PREFERENCES_KEY)).toBeNull();
    saveReviewTabHidden(false, window.sessionStorage);
    expect(loadReviewTabHidden(window.sessionStorage)).toBe(false);
  });

  it("fails closed when browser storage is unavailable", () => {
    const unavailable = {
      getItem: () => { throw new Error("storage disabled"); },
      setItem: () => { throw new Error("storage disabled"); },
      removeItem: () => { throw new Error("storage disabled"); },
    } as unknown as Storage;

    expect(loadReviewPreferences(unavailable)).toEqual(DEFAULT_REVIEW_PREFERENCES);
    expect(loadReviewTabHidden(unavailable)).toBe(false);
    expect(() => saveReviewPreferences(DEFAULT_REVIEW_PREFERENCES, unavailable)).not.toThrow();
    expect(() => saveReviewTabHidden(true, unavailable)).not.toThrow();
  });
});
