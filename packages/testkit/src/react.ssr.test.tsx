// @vitest-environment node

import { renderToString } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { A3SReviewOverlay, A3STestBoundary, A3STestKit } from "./react";

describe("React server rendering", () => {
  it("renders an enabled provider and overlay without layout-effect warnings", () => {
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});

    try {
      const html = renderToString(
        <A3STestKit enabled page={{ id: "ssr" }} repairStorage="memory">
          <A3STestBoundary id="hero" name="Hero" as="main">
            <h1>Server rendered</h1>
          </A3STestBoundary>
          <A3SReviewOverlay enabled />
        </A3STestKit>,
      );

      expect(html).toBe("<main><h1>Server rendered</h1></main>");
      expect(consoleError).not.toHaveBeenCalled();
    } finally {
      consoleError.mockRestore();
    }
  });
});
