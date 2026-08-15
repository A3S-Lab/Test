import assert from "node:assert/strict";
import test from "node:test";
import {
  validateScreenReaderAudit,
  validateScreenReaderWorkflowManifest,
} from "./screen-reader-audit-validation.mjs";

const workflows = [{ id: "dialog-navigation" }, { id: "repair-lifecycle" }];

const validWorkflowManifest = {
  protocol: "a3s.test.screen-reader-workflows/1",
  workflows: [
    {
      id: "dialog-navigation",
      title: "Dialog navigation",
      setup: "Open the review surface.",
      steps: ["Navigate every control."],
      expected: ["The dialog name and controls are announced."],
    },
  ],
};

const validAudit = {
  protocol: "a3s.test.screen-reader-audit/1",
  revision: "7dba354fa29cf329f077bfe6d5f7aa1cffdf73be",
  testkit_version: "0.4.0",
  independent: true,
  auditor: { id: "independent-auditor" },
  environment: {
    os: "macOS 15.6",
    browser: "Safari 18.6",
    screen_reader: "VoiceOver 15.6",
    input_modes: ["keyboard", "voice"],
  },
  started_at: "2026-08-15T10:00:00.000Z",
  completed_at: "2026-08-15T10:30:00.000Z",
  results: [
    {
      workflow_id: "dialog-navigation",
      outcome: "passed",
      notes: "Dialog name and navigation order were announced.",
      evidence: ["evidence/dialog-navigation.txt"],
    },
    {
      workflow_id: "repair-lifecycle",
      outcome: "failed",
      notes: "Resolved state was announced twice.",
      evidence: ["evidence/repair-lifecycle.txt"],
    },
  ],
};

test("accepts a complete independently attested audit", () => {
  assert.deepEqual(
    validateScreenReaderAudit({
      audit: validAudit,
      expectedRevision: validAudit.revision,
      testkitVersion: "0.4.0",
      workflows,
    }),
    {
      errors: [],
      summary: { blocked: 0, failed: 1, passed: 1, total: 2 },
    },
  );
});

test("accepts a bounded workflow manifest", () => {
  assert.deepEqual(
    validateScreenReaderWorkflowManifest(validWorkflowManifest),
    { errors: [], workflows: validWorkflowManifest.workflows },
  );
});

test("rejects workflow schema drift and duplicate ids", () => {
  const duplicated = {
    ...validWorkflowManifest,
    workflows: [
      { ...validWorkflowManifest.workflows[0], hidden_authority: "repair" },
      validWorkflowManifest.workflows[0],
    ],
  };
  assert.deepEqual(validateScreenReaderWorkflowManifest(duplicated).errors, [
    "Workflow manifest entry 1 contains unknown field hidden_authority.",
    "Workflow manifest id dialog-navigation is duplicated.",
  ]);
});

test("rejects incomplete, duplicated, or reordered workflow results", () => {
  const audit = {
    ...validAudit,
    results: [validAudit.results[1], validAudit.results[1]],
  };
  const result = validateScreenReaderAudit({
    audit,
    expectedRevision: validAudit.revision,
    testkitVersion: "0.4.0",
    workflows,
  });

  assert.deepEqual(result.errors, [
    "Audit results must cover every workflow exactly once in manifest order.",
    "Audit workflow repair-lifecycle is duplicated.",
    "Audit workflow dialog-navigation is missing.",
  ]);
});

test("rejects unverifiable identity, environment, timing, and evidence", () => {
  const audit = {
    ...validAudit,
    revision: "short",
    testkit_version: "0.2.0",
    independent: false,
    auditor: { id: "" },
    environment: {
      ...validAudit.environment,
      input_modes: ["keyboard", "keyboard"],
    },
    started_at: "not-a-date",
    completed_at: "2026-08-15T09:00:00.000Z",
    results: [
      {
        ...validAudit.results[0],
        evidence: [],
      },
      {
        ...validAudit.results[1],
        notes: "",
      },
    ],
  };
  const result = validateScreenReaderAudit({
    audit,
    expectedRevision: validAudit.revision,
    testkitVersion: "0.4.0",
    workflows,
  });

  assert.deepEqual(result.errors, [
    `Audit revision short does not match ${validAudit.revision}.`,
    "Audit revision must be a full lowercase Git commit SHA.",
    "Audit Test Kit version 0.2.0 does not match 0.4.0.",
    "Audit must explicitly attest independent execution.",
    "Audit auditor.id must be a non-empty bounded string.",
    "Audit environment.input_modes must contain unique bounded strings.",
    "Audit started_at must be an ISO-8601 timestamp.",
    "Workflow dialog-navigation must reference at least one evidence artifact.",
    "Workflow repair-lifecycle requires notes for a failed outcome.",
  ]);
});

test("requires every workflow to pass only for a closure audit", () => {
  const result = validateScreenReaderAudit({
    audit: validAudit,
    expectedRevision: validAudit.revision,
    requirePass: true,
    testkitVersion: "0.4.0",
    workflows,
  });

  assert.deepEqual(result.errors, [
    "Closure audit requires every workflow to pass; repair-lifecycle is failed.",
  ]);
});

test("rejects unknown fields instead of silently accepting schema drift", () => {
  const result = validateScreenReaderAudit({
    audit: { ...validAudit, hidden_authority: "repair" },
    expectedRevision: validAudit.revision,
    testkitVersion: "0.4.0",
    workflows,
  });

  assert.deepEqual(result.errors, [
    "Audit contains unknown field hidden_authority.",
  ]);
});

test("rejects nested schema drift, unsafe evidence paths, and inverted timing", () => {
  const audit = {
    ...validAudit,
    auditor: { ...validAudit.auditor, can_authorize_repair: true },
    completed_at: "2026-08-15T09:59:59.000Z",
    results: validAudit.results.map((result, index) =>
      index === 0
        ? { ...result, evidence: ["../outside.txt"], hidden: true }
        : result,
    ),
  };
  const result = validateScreenReaderAudit({
    audit,
    expectedRevision: validAudit.revision,
    testkitVersion: "0.4.0",
    workflows,
  });

  assert.deepEqual(result.errors, [
    "Audit auditor contains unknown field can_authorize_repair.",
    "Audit completed_at must not precede started_at.",
    "Workflow dialog-navigation contains unknown field hidden.",
    "Workflow dialog-navigation evidence must contain at most 20 safe relative file paths.",
  ]);
});

test("rejects impossible ISO calendar timestamps", () => {
  const result = validateScreenReaderAudit({
    audit: { ...validAudit, started_at: "2026-02-30T10:00:00.000Z" },
    expectedRevision: validAudit.revision,
    testkitVersion: "0.4.0",
    workflows,
  });

  assert.deepEqual(result.errors, [
    "Audit started_at must be an ISO-8601 timestamp.",
  ]);
});
