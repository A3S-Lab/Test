import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { execFileSync, spawnSync } from "node:child_process";
import {
  copyFile,
  cp,
  mkdir,
  mkdtemp,
  rm,
  truncate,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

const revision = execFileSync("git", ["rev-parse", "HEAD"], {
  cwd: process.cwd(),
  encoding: "utf8",
}).trim();

function gitBlob(repositoryPath) {
  return execFileSync("git", ["show", `${revision}:${repositoryPath}`], {
    cwd: process.cwd(),
    encoding: null,
  });
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function runVerifier(artifact, ...arguments_) {
  return runVerifierAtRevision(artifact, revision, ...arguments_);
}

function runVerifierAtRevision(artifact, expectedRevision, ...arguments_) {
  return spawnSync(
    process.execPath,
    [
      "scripts/check-screen-reader-audit.mjs",
      artifact,
      "--revision",
      expectedRevision,
      ...arguments_,
    ],
    { cwd: process.cwd(), encoding: "utf8" },
  );
}

test("verifies evidence files and gates closure independently", async () => {
  const workspace = await mkdtemp(path.join(tmpdir(), "a3s-test-audit-cli-"));
  try {
    const manifestEncoded = gitBlob(
      "packages/testkit/screen-reader-audit/workflows.json",
    );
    const manifest = JSON.parse(manifestEncoded.toString("utf8"));
    const packageJson = JSON.parse(
      gitBlob("packages/testkit/package.json").toString("utf8"),
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
      testkit_version: packageJson.version,
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
    const auditEncoded = `${JSON.stringify(audit, null, 2)}\n`;
    await writeFile(artifact, auditEncoded);

    const accepted = runVerifier(artifact, "--require-pass");
    assert.equal(accepted.status, 0, accepted.stderr);
    const verification = JSON.parse(accepted.stdout);
    assert.equal(
      verification.protocol,
      "a3s.test.screen-reader-audit-verification/2",
    );
    assert.deepEqual(verification.audit, {
      path: "audit.json",
      bytes: Buffer.byteLength(auditEncoded),
      sha256: sha256(auditEncoded),
    });
    assert.deepEqual(verification.workflow_manifest, {
      protocol: "a3s.test.screen-reader-workflows/1",
      path: "screen-reader-audit/workflows.json",
      bytes: manifestEncoded.length,
      sha256: sha256(manifestEncoded),
    });
    assert.deepEqual(verification.summary, {
      blocked: 0,
      failed: 0,
      passed: manifest.workflows.length,
      total: manifest.workflows.length,
    });
    assert.equal(verification.evidence.length, manifest.workflows.length);
    assert.deepEqual(verification.evidence[0], {
      workflow_id: manifest.workflows[0].id,
      path: results[0].evidence[0],
      bytes: Buffer.byteLength(
        `${manifest.workflows[0].id} completed with the named screen reader.\n`,
      ),
      sha256: sha256(
        `${manifest.workflows[0].id} completed with the named screen reader.\n`,
      ),
    });
    assert.equal(
      verification.evidence_set_sha256,
      sha256(JSON.stringify(verification.evidence)),
    );

    const mirror = path.join(workspace, "mirror");
    await mkdir(mirror);
    await cp(evidenceDirectory, path.join(mirror, "evidence"), {
      recursive: true,
    });
    await copyFile(artifact, path.join(mirror, "audit.json"));
    const mirrored = runVerifier(
      path.join(mirror, "audit.json"),
      "--require-pass",
    );
    assert.equal(mirrored.status, 0, mirrored.stderr);
    assert.deepEqual(JSON.parse(mirrored.stdout), verification);

    await writeFile(
      path.join(workspace, results[0].evidence[0]),
      "Replacement evidence with different content.\n",
    );
    const replacedEvidence = runVerifier(artifact, "--require-pass");
    assert.equal(replacedEvidence.status, 0, replacedEvidence.stderr);
    const replacementVerification = JSON.parse(replacedEvidence.stdout);
    assert.equal(
      replacementVerification.audit.sha256,
      verification.audit.sha256,
    );
    assert.notEqual(
      replacementVerification.evidence[0].sha256,
      verification.evidence[0].sha256,
    );
    assert.notEqual(
      replacementVerification.evidence_set_sha256,
      verification.evidence_set_sha256,
    );

    const unknownRevision = runVerifierAtRevision(artifact, "f".repeat(40));
    assert.equal(unknownRevision.status, 1);
    assert.match(
      unknownRevision.stderr,
      /revision .* does not identify a Git commit/,
    );

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

    const originalEvidence = audit.results[0].evidence[0];
    const aggregateEvidence = [];
    for (let index = 0; index < 17; index += 1) {
      const relativePath = `evidence/aggregate-${index}.bin`;
      const filename = path.join(workspace, relativePath);
      await writeFile(filename, "x");
      await truncate(filename, 64 * 1024 * 1024);
      aggregateEvidence.push(relativePath);
    }
    audit.results[0].evidence = aggregateEvidence;
    await writeFile(artifact, `${JSON.stringify(audit, null, 2)}\n`);
    const oversizedAggregate = runVerifier(artifact);
    assert.equal(oversizedAggregate.status, 1);
    assert.match(oversizedAggregate.stderr, /exceeds the 1 GiB aggregate/);

    audit.results[0].evidence = [originalEvidence];
    await writeFile(artifact, `${JSON.stringify(audit, null, 2)}\n`);
    const oversizedEvidencePath = path.join(workspace, originalEvidence);
    await writeFile(oversizedEvidencePath, "x");
    await truncate(oversizedEvidencePath, 64 * 1024 * 1024 + 1);
    const oversizedEvidence = runVerifier(artifact);
    assert.equal(oversizedEvidence.status, 1);
    assert.match(
      oversizedEvidence.stderr,
      /evidence artifact .* exceeds 64 MiB/,
    );

    await writeFile(artifact, " ".repeat(8 * 1024 * 1024 + 1));
    const oversized = runVerifier(artifact);
    assert.equal(oversized.status, 1);
    assert.match(oversized.stderr, /Audit artifact exceeds 8 MiB/);
  } finally {
    await rm(workspace, { force: true, recursive: true });
  }
});
