import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import React from "react";
import { renderToString } from "react-dom/server";

const coreEsm = await import("@a3s-lab/testkit");
const reactEsm = await import("@a3s-lab/testkit/react");
const require = createRequire(import.meta.url);
const coreCjs = require("@a3s-lab/testkit");
const reactCjs = require("@a3s-lab/testkit/react");

assert.equal(
  await readFile(new URL("../LICENSE", import.meta.url), "utf8"),
  await readFile(new URL("../../../LICENSE", import.meta.url), "utf8"),
  "package and repository license texts differ",
);

assertFunctions(coreEsm, [
  "getPageContextBridge",
  "installTestKit",
  "registerBoundary",
], "ESM core");
assertFunctions(coreCjs, [
  "getPageContextBridge",
  "installTestKit",
  "registerBoundary",
], "CommonJS core");
assertFunctions(reactEsm, [
  "A3SReviewOverlay",
  "A3STestBoundary",
  "A3STestKit",
], "ESM React adapter");
assertFunctions(reactCjs, [
  "A3SReviewOverlay",
  "A3STestBoundary",
  "A3STestKit",
], "CommonJS React adapter");
for (const [label, core] of [["ESM", coreEsm], ["CommonJS", coreCjs]]) {
  assert.equal(core.getPageContextBridge(), null, `${label} core exposed a server bridge`);
  assert.throws(
    () => core.installTestKit({ enabled: true, page: { id: `${label}-server` } }),
    /can only be enabled in a browser/,
  );
}

const serverWarnings = [];
const originalConsoleError = console.error;
let esmHtml;
let cjsHtml;
console.error = (...values) => serverWarnings.push(values);
try {
  esmHtml = renderAdapter(reactEsm, "esm-ssr");
  cjsHtml = renderAdapter(reactCjs, "cjs-ssr");
} finally {
  console.error = originalConsoleError;
}
assert.equal(esmHtml, "<main><h1>Server rendered</h1></main>");
assert.equal(cjsHtml, esmHtml);
assert.deepEqual(
  serverWarnings,
  [],
  `React server rendering emitted warnings: ${serverWarnings.map((values) => values.join(" ")).join("\n")}`,
);

const npm = process.platform === "win32" ? "npm.cmd" : "npm";
const packed = spawnSync(npm, ["pack", "--dry-run", "--json"], {
  cwd: new URL("..", import.meta.url),
  encoding: "utf8",
});
if (packed.error) throw packed.error;
assert.equal(packed.status, 0, packed.stderr || "npm pack --dry-run failed");
const manifests = JSON.parse(packed.stdout);
assert.equal(manifests.length, 1, "npm pack returned an unexpected manifest count");
const files = new Set(manifests[0].files.map(({ path }) => path));
for (const path of [
  "LICENSE",
  "README.md",
  "package.json",
  "dist/index.cjs",
  "dist/index.d.cts",
  "dist/index.d.ts",
  "dist/index.js",
  "dist/react.cjs",
  "dist/react.d.cts",
  "dist/react.d.ts",
  "dist/react.js",
]) {
  assert(files.has(path), `packed Test Kit is missing ${path}`);
}
assert(
  [...files].some((path) => /^dist\/chunk-.*\.js$/.test(path)),
  "packed Test Kit is missing its ESM runtime chunk",
);
assert(
  [...files].some((path) => /^dist\/types-.*\.d\.ts$/.test(path)),
  "packed Test Kit is missing its ESM shared declarations",
);
assert(
  [...files].some((path) => /^dist\/types-.*\.d\.cts$/.test(path)),
  "packed Test Kit is missing its CommonJS shared declarations",
);
assert(
  [...files].every((path) => !path.startsWith("src/") && !path.startsWith("scripts/")),
  "packed Test Kit exposes development-only source or consumer fixtures",
);

console.log(`Package consumers and ${files.size} packed files verified.`);

function assertFunctions(module, names, label) {
  for (const name of names) {
    assert.equal(typeof module[name], "function", `${label} is missing ${name}`);
  }
}

function renderAdapter(adapter, pageId) {
  return renderToString(
    React.createElement(
      adapter.A3STestKit,
      { enabled: true, page: { id: pageId }, repairStorage: "memory" },
      React.createElement(
        adapter.A3STestBoundary,
        { id: "hero", name: "Hero", as: "main" },
        React.createElement("h1", null, "Server rendered"),
      ),
      React.createElement(adapter.A3SReviewOverlay, { enabled: true }),
    ),
  );
}
