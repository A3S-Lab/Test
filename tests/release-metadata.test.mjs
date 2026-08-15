import assert from "node:assert/strict";
import test from "node:test";
import { validateReleaseMetadata } from "../scripts/release-metadata.mjs";

const validInput = {
  changelog: "# Changelog\n\n## 0.16.2 - 2026-08-15\n",
  defaultVersion: "v0.16.2",
  publishedVersion: "v0.16.2",
  repositoryReadme:
    "sh -s -- --version v0.16.2\n-Version v0.16.2\n",
  releaseTag: "v0.16.2",
  snapshots: {
    current: {
      version: "v0.16.2",
      sourceTag: "v0.16.2",
      sourceCommit: "e207e6a2578209d7d648d9365f08c7c591dffdd7",
      testkitVersion: "0.3.0",
    },
    archives: [
      {
        version: "v0.15.0",
        sourceTag: "v0.15.0",
        sourceCommit: "1c973610efca7692bfed58ae31573cef6ec98f7f",
        testkitVersion: "0.3.0",
      },
    ],
  },
  testKitManifest: JSON.stringify({
    name: "@a3s-lab/testkit",
    version: "0.3.0",
  }),
  versions: ["v0.16.2", "v0.15.0"],
  workspaceManifest: `[workspace.package]
version = "0.16.2"
edition = "2021"
`,
};

test("accepts aligned release metadata", () => {
  assert.deepEqual(validateReleaseMetadata(validInput), {
    errors: [],
    expectedTag: "v0.16.2",
    workspaceVersion: "0.16.2",
  });
});

test("rejects a release tag that differs from the workspace version", () => {
  const result = validateReleaseMetadata({
    ...validInput,
    releaseTag: "v0.17.0",
  });

  assert.deepEqual(result.errors, [
    "Release tag v0.17.0 does not match workspace version v0.16.2.",
  ]);
});

test("allows staged docs on main but blocks a tag before the version is published", () => {
  const staged = {
    ...validInput,
    publishedVersion: "v0.15.0",
    repositoryReadme:
      "sh -s -- --version v0.15.0\n-Version v0.15.0\n",
    releaseTag: undefined,
  };

  assert.deepEqual(validateReleaseMetadata(staged).errors, []);
  assert.deepEqual(
    validateReleaseMetadata({ ...staged, releaseTag: "v0.16.2" }).errors,
    [
      "Published documentation version v0.15.0 does not match release tag v0.16.2.",
    ],
  );
});

test("requires the published version to remain in the documentation set", () => {
  const result = validateReleaseMetadata({
    ...validInput,
    publishedVersion: "v0.14.0",
  });

  assert.deepEqual(result.errors, [
    "Published documentation version v0.14.0 is not present in versions.mjs.",
  ]);
});

test("binds repository installation examples to the published version", () => {
  const result = validateReleaseMetadata({
    ...validInput,
    repositoryReadme:
      "sh -s -- --version v0.15.0\n-Version v0.15.0\n",
  });

  assert.deepEqual(result.errors, [
    "README.md does not pin the Unix installer to published version v0.16.2.",
    "README.md does not pin the PowerShell installer to published version v0.16.2.",
  ]);
});

test("binds release metadata to the packaged Test Kit version", () => {
  const result = validateReleaseMetadata({
    ...validInput,
    snapshots: {
      ...validInput.snapshots,
      current: {
        ...validInput.snapshots.current,
        testkitVersion: "0.2.0",
      },
    },
  });

  assert.deepEqual(result.errors, [
    "Current snapshot Test Kit version 0.2.0 does not match package version 0.3.0.",
  ]);
});

test("requires every documentation snapshot to bind a Test Kit version", () => {
  const result = validateReleaseMetadata({
    ...validInput,
    snapshots: {
      ...validInput.snapshots,
      archives: [
        {
          version: "v0.15.0",
          sourceTag: "v0.15.0",
          sourceCommit: "1c973610efca7692bfed58ae31573cef6ec98f7f",
        },
      ],
    },
  });

  assert.deepEqual(result.errors, [
    "Archive v0.15.0 Test Kit version <missing> is not semantic.",
  ]);
});

test("reports documentation and provenance drift together", () => {
  const result = validateReleaseMetadata({
    ...validInput,
    changelog: "# Changelog\n\n## Unreleased\n",
    defaultVersion: "v0.15.0",
    snapshots: {
      current: {
        version: "v0.15.0",
        sourceTag: "v0.15.0",
        sourceCommit: "short",
        testkitVersion: "0.3.0",
      },
      archives: [],
    },
  });

  assert.deepEqual(result.errors, [
    "Default documentation v0.15.0 does not match workspace version v0.16.2.",
    "The first documentation version must be the default version v0.15.0.",
    "Current snapshot v0.15.0 does not match workspace version v0.16.2.",
    "Current snapshot source tag v0.15.0 does not match v0.16.2.",
    "Current snapshot source commit must be a full lowercase Git commit SHA.",
    "Archive versions do not match the non-default documentation versions.",
    "CHANGELOG.md has no dated 0.16.2 release section.",
  ]);
});

test("rejects duplicate and malformed version records", () => {
  const result = validateReleaseMetadata({
    ...validInput,
    versions: ["v0.16.2", "v0.16.2", "16.0"],
    snapshots: {
      ...validInput.snapshots,
      archives: [
        {
          version: "v0.16.2",
          sourceTag: "v0.16.1",
          sourceCommit: "1c973610efca7692bfed58ae31573cef6ec98f7f",
          testkitVersion: "0.3.0",
        },
        {
          version: "16.0",
          sourceTag: "16.0",
          sourceCommit: "not-a-commit",
          testkitVersion: "0.3.0",
        },
      ],
    },
  });

  assert.deepEqual(result.errors, [
    "Documentation versions must be unique.",
    "Documentation version 16.0 is not a supported vMAJOR.MINOR.PATCH version.",
    "Archive v0.16.2 source tag v0.16.1 does not match its version.",
    "Archive 16.0 source commit must be a full lowercase Git commit SHA.",
  ]);
});
