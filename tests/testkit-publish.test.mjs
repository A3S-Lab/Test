import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const workflow = await readFile(
  new URL("../.github/workflows/publish-testkit.yml", import.meta.url),
  "utf8",
);
const manifest = JSON.parse(
  await readFile(
    new URL("../packages/testkit/package.json", import.meta.url),
    "utf8",
  ),
);

test("Test Kit package is admitted for public npm publication", () => {
  assert.equal(manifest.name, "@a3s-lab/testkit");
  assert.deepEqual(manifest.publishConfig, {
    access: "public",
    registry: "https://registry.npmjs.org/",
  });
  assert.deepEqual(manifest.repository, {
    type: "git",
    url: "git+https://github.com/A3S-Lab/Test.git",
    directory: "packages/testkit",
  });
});

test("npm publication is manual, source-bound, provenance-enabled, and gated", () => {
  assert.match(workflow, /workflow_dispatch:/);
  assert.match(workflow, /source_ref:/);
  assert.match(workflow, /environment: npm/);
  assert.match(workflow, /id-token: write/);
  assert.match(workflow, /ref: \$\{\{ inputs\.source_ref \}\}/);
  assert.match(workflow, /check-release-metadata\.mjs --tag \"\$SOURCE_REF\"/);
  assert.match(
    workflow,
    /test \"\$\(git rev-parse HEAD\)\" = \"\$SOURCE_REF\"/,
  );
  assert.match(workflow, /registry-url: https:\/\/registry\.npmjs\.org/);
  assert.match(workflow, /npm run typecheck/);
  assert.match(workflow, /npm test/);
  assert.match(workflow, /npm run check:package/);
  assert.match(workflow, /npm publish --access public --provenance/);
  assert.doesNotMatch(workflow, /pull_request:/);
  assert.doesNotMatch(workflow, /push:/);
});
