import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

const revision = "a".repeat(40);

function runVerifier(artifact, ...arguments_) {
  return spawnSync(
    process.execPath,
    [
      "scripts/check-screen-reader-audit.mjs",
      artifact,
      "--revision",
      revision,
      ...arguments_,
    ],
    { cwd: process.cwd(), encoding: "utf8" },
  );
}

test("verifies evidence files and gates closure independently", async () => {
  const workspace = await mkdtemp(path.join(tmpdir(), "a3s-test-audit-cli-"));
  try {
    const manifest = JSON.parse(
      await readFile("screen-reader-audit/workflows.json", "utf8"),
    );
    const evidenceDirectory = path.join(workspace, "evidence");
    await mkdir(evidenceDirectory);
    const results = [];
    for (const workflow of manifest.workflows) {
      const evidence = `evidence/${workflow.id}.txt`;
      await writeFile(
        path.join(workspace, evidence),
        `${workflow.id} completed with the named screen reader.\n`,
      );
      results.push({
        workflow_id: workflow.id,
        outcome: "passed",
        notes: "Expected names, states, and focus movement were announced.",
        evidence: [evidence],
      });
    }
    const audit = {
      protocol: "a3s.test.screen-reader-audit/1",
      revision,
      testkit_version: "0.3.0",
      independent: true,
      auditor: { id: "external-accessibility-reviewer" },
      environment: {
        os: "Windows 11",
        browser: "Firefox 142",
        screen_reader: "NVDA 2026.2",
        input_modes: ["keyboard"],
      },
      started_at: "2026-08-15T10:00:00.000Z",
      completed_at: "2026-08-15T11:00:00.000Z",
      results,
    };
    const artifact = path.join(workspace, "audit.json");
    await writeFile(artifact, `${JSON.stringify(audit, null, 2)}\n`);

    const accepted = runVerifier(artifact, "--require-pass");
    assert.equal(accepted.status, 0, accepted.stderr);
    assert.deepEqual(JSON.parse(accepted.stdout).summary, {
      blocked: 0,
      failed: 0,
      passed: manifest.workflows.length,
      total: manifest.workflows.length,
    });

    audit.results[0].outcome = "failed";
    audit.results[0].notes = "The review launcher name was not announced.";
    await writeFile(artifact, `${JSON.stringify(audit, null, 2)}\n`);
    assert.equal(runVerifier(artifact).status, 0);
    const rejectedClosure = runVerifier(artifact, "--require-pass");
    assert.equal(rejectedClosure.status, 1);
    assert.match(
      rejectedClosure.stderr,
      /Closure audit requires every workflow to pass/,
    );

    await rm(path.join(workspace, audit.results[0].evidence[0]));
    const missingEvidence = runVerifier(artifact);
    assert.equal(missingEvidence.status, 1);
    assert.match(missingEvidence.stderr, /evidence artifact .* is missing/);

    await writeFile(artifact, " ".repeat(8 * 1024 * 1024 + 1));
    const oversized = runVerifier(artifact);
    assert.equal(oversized.status, 1);
    assert.match(oversized.stderr, /Audit artifact exceeds 8 MiB/);
  } finally {
    await rm(workspace, { force: true, recursive: true });
  }
});
