#!/usr/bin/env node

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { open, realpath, stat } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  validateScreenReaderAudit,
  validateScreenReaderWorkflowManifest,
} from "./screen-reader-audit-validation.mjs";

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const packageRoot = path.resolve(scriptDirectory, "..");
const MAX_JSON_BYTES = 8 * 1024 * 1024;
const MAX_EVIDENCE_FILE_BYTES = 64 * 1024 * 1024;
const MAX_TOTAL_EVIDENCE_BYTES = 1024 * 1024 * 1024;
const REVISION_PATTERN = /^[0-9a-f]{40}$/;

function usage() {
  return [
    "Usage: node scripts/check-screen-reader-audit.mjs <audit.json> [options]",
    "",
    "Options:",
    "  --revision <sha>  Read versioned inputs from this Git commit (defaults to HEAD)",
    "  --require-pass    Require every workflow outcome to be passed",
    "  --help            Show this help",
  ].join("\n");
}

function parseArguments(argv) {
  let artifact;
  let revision;
  let requirePass = false;
  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--help") return { help: true };
    if (value === "--require-pass") {
      requirePass = true;
      continue;
    }
    if (value === "--revision") {
      revision = argv[index + 1];
      if (!revision || revision.startsWith("--")) {
        throw new Error("--revision requires a full Git commit SHA");
      }
      index += 1;
      continue;
    }
    if (value.startsWith("--")) throw new Error(`unknown option ${value}`);
    if (artifact)
      throw new Error("exactly one audit artifact path is required");
    artifact = value;
  }
  if (!artifact) throw new Error("an audit artifact path is required");
  return { artifact, help: false, requirePass, revision };
}

function sha256(encoded) {
  return createHash("sha256").update(encoded).digest("hex");
}

function sameFileSnapshot(left, right) {
  return (
    left.dev === right.dev &&
    left.ino === right.ino &&
    left.mode === right.mode &&
    left.size === right.size &&
    left.mtimeNs === right.mtimeNs &&
    left.ctimeNs === right.ctimeNs
  );
}

function parseJson(encoded, label) {
  try {
    return JSON.parse(encoded.toString("utf8"));
  } catch (error) {
    throw new Error(`${label} is not valid JSON: ${error.message}`);
  }
}

async function readJson(filename, label) {
  let handle;
  try {
    handle = await open(filename, "r");
  } catch (error) {
    throw new Error(`${label} could not be opened: ${error.message}`);
  }
  try {
    const before = await handle.stat({ bigint: true });
    if (!before.isFile()) throw new Error(`${label} must be a regular file`);
    if (before.size > BigInt(MAX_JSON_BYTES)) {
      throw new Error(`${label} exceeds 8 MiB`);
    }
    const encoded = await handle.readFile();
    const after = await handle.stat({ bigint: true });
    if (
      encoded.length !== Number(before.size) ||
      !sameFileSnapshot(before, after)
    ) {
      throw new Error(`${label} changed while it was being read`);
    }
    return {
      bytes: encoded.length,
      sha256: sha256(encoded),
      value: parseJson(encoded, label),
    };
  } finally {
    await handle.close();
  }
}

function headRevision() {
  return execFileSync("git", ["rev-parse", "HEAD"], {
    cwd: packageRoot,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  }).trim();
}

function resolveRevision(requestedRevision) {
  const revision = requestedRevision ?? headRevision();
  if (!REVISION_PATTERN.test(revision)) {
    throw new Error("revision must be a full lowercase Git commit SHA");
  }
  let commit;
  try {
    commit = execFileSync(
      "git",
      ["rev-parse", "--verify", `${revision}^{commit}`],
      {
        cwd: packageRoot,
        encoding: "utf8",
        stdio: ["ignore", "pipe", "pipe"],
      },
    ).trim();
  } catch {
    throw new Error(`revision ${revision} does not identify a Git commit`);
  }
  if (commit !== revision) {
    throw new Error(`revision ${revision} does not identify a Git commit`);
  }
  return revision;
}

function repositoryRelativePackagePath() {
  const repositoryRoot = execFileSync("git", ["rev-parse", "--show-toplevel"], {
    cwd: packageRoot,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  }).trim();
  const relative = path.relative(repositoryRoot, packageRoot);
  if (
    relative.length === 0 ||
    path.isAbsolute(relative) ||
    relative.split(path.sep).includes("..")
  ) {
    throw new Error("Test Kit package is not contained by the Git repository");
  }
  return relative.split(path.sep).join("/");
}

function readGitJson(revision, repositoryPath, label) {
  let encoded;
  try {
    encoded = execFileSync("git", ["show", `${revision}:${repositoryPath}`], {
      cwd: packageRoot,
      encoding: null,
      maxBuffer: MAX_JSON_BYTES + 1,
      stdio: ["ignore", "pipe", "pipe"],
    });
  } catch (error) {
    throw new Error(
      `${label} could not be read from revision ${revision}: ${error.message}`,
    );
  }
  if (encoded.length > MAX_JSON_BYTES)
    throw new Error(`${label} exceeds 8 MiB`);
  return {
    bytes: encoded.length,
    sha256: sha256(encoded),
    value: parseJson(encoded, label),
  };
}

function evidenceLabel(workflowId, evidence) {
  return `Workflow ${workflowId} evidence artifact ${evidence}`;
}

async function resolveEvidencePath(root, candidate, label) {
  let resolved;
  try {
    resolved = await realpath(candidate);
  } catch {
    throw new Error(`${label} is missing.`);
  }
  if (!resolved.startsWith(`${root}${path.sep}`)) {
    throw new Error(`${label} escapes the audit directory.`);
  }
  return resolved;
}

async function inspectEvidenceFile(root, candidate, label) {
  const resolved = await resolveEvidencePath(root, candidate, label);
  let metadata;
  try {
    metadata = await stat(resolved, { bigint: true });
  } catch {
    throw new Error(`${label} is missing.`);
  }
  if (!metadata.isFile() || metadata.size === 0n) {
    throw new Error(`${label} must be a non-empty regular file.`);
  }
  if (metadata.size > BigInt(MAX_EVIDENCE_FILE_BYTES)) {
    throw new Error(`${label} exceeds 64 MiB.`);
  }
  return {
    bytes: Number(metadata.size),
    resolved,
    snapshot: metadata,
  };
}

async function digestEvidenceFile(
  root,
  candidate,
  resolved,
  expectedSnapshot,
  label,
) {
  let pathBefore;
  let handle;
  try {
    pathBefore = await stat(resolved, { bigint: true });
    handle = await open(resolved, "r");
  } catch {
    throw new Error(`${label} is missing.`);
  }
  try {
    const before = await handle.stat({ bigint: true });
    if (
      !sameFileSnapshot(expectedSnapshot, pathBefore) ||
      !sameFileSnapshot(pathBefore, before)
    ) {
      throw new Error(`${label} changed while it was being opened.`);
    }
    if (!before.isFile() || before.size === 0n) {
      throw new Error(`${label} must be a non-empty regular file.`);
    }
    if (before.size > BigInt(MAX_EVIDENCE_FILE_BYTES)) {
      throw new Error(`${label} exceeds 64 MiB.`);
    }

    const hash = createHash("sha256");
    let bytes = 0;
    const stream = handle.createReadStream({ autoClose: false });
    for await (const chunk of stream) {
      bytes += chunk.length;
      if (bytes > MAX_EVIDENCE_FILE_BYTES) {
        throw new Error(`${label} exceeds 64 MiB.`);
      }
      hash.update(chunk);
    }

    const after = await handle.stat({ bigint: true });
    if (bytes !== Number(before.size) || !sameFileSnapshot(before, after)) {
      throw new Error(`${label} changed while it was being verified.`);
    }
    const finalResolved = await resolveEvidencePath(root, candidate, label);
    let finalMetadata;
    try {
      finalMetadata = await stat(finalResolved, { bigint: true });
    } catch {
      throw new Error(`${label} changed while it was being verified.`);
    }
    if (finalResolved !== resolved || !sameFileSnapshot(after, finalMetadata)) {
      throw new Error(`${label} changed while it was being verified.`);
    }
    return {
      bytes,
      sha256: hash.digest("hex"),
      snapshot: after,
    };
  } finally {
    await handle.close();
  }
}

async function evidencePathStillMatches(
  root,
  candidate,
  resolved,
  snapshot,
  label,
) {
  const finalResolved = await resolveEvidencePath(root, candidate, label);
  let finalMetadata;
  try {
    finalMetadata = await stat(finalResolved, { bigint: true });
  } catch {
    throw new Error(`${label} changed while it was being verified.`);
  }
  if (
    finalResolved !== resolved ||
    !sameFileSnapshot(snapshot, finalMetadata)
  ) {
    throw new Error(`${label} changed while it was being verified.`);
  }
}

async function verifyEvidenceFiles(audit, artifactDirectory) {
  const errors = [];
  const references = [];
  const root = await realpath(artifactDirectory);
  const uniqueFiles = new Map();
  let totalBytes = 0;

  for (const result of audit.results) {
    for (const evidence of result.evidence) {
      const label = evidenceLabel(result.workflow_id, evidence);
      const candidate = path.resolve(root, evidence);
      try {
        const inspected = await inspectEvidenceFile(root, candidate, label);
        let file = uniqueFiles.get(inspected.resolved);
        if (!file) {
          totalBytes += inspected.bytes;
          if (totalBytes > MAX_TOTAL_EVIDENCE_BYTES) {
            errors.push(
              "Audit evidence exceeds the 1 GiB aggregate byte limit.",
            );
            return { errors, evidenceRecords: [] };
          }
          file = {
            ...inspected,
            candidates: new Map([[candidate, label]]),
          };
          uniqueFiles.set(inspected.resolved, file);
        } else {
          if (!sameFileSnapshot(file.snapshot, inspected.snapshot)) {
            throw new Error(`${label} changed while it was being inspected.`);
          }
          file.candidates.set(candidate, label);
        }
        references.push({
          workflow_id: result.workflow_id,
          path: evidence,
          resolved: inspected.resolved,
        });
      } catch (error) {
        errors.push(error instanceof Error ? error.message : String(error));
      }
    }
  }

  if (errors.length > 0) return { errors, evidenceRecords: [] };

  for (const [resolved, file] of uniqueFiles) {
    const [candidate, label] = file.candidates.entries().next().value;
    try {
      file.digest = await digestEvidenceFile(
        root,
        candidate,
        resolved,
        file.snapshot,
        label,
      );
    } catch (error) {
      errors.push(error instanceof Error ? error.message : String(error));
    }
  }

  if (errors.length > 0) return { errors, evidenceRecords: [] };

  for (const [resolved, file] of uniqueFiles) {
    for (const [candidate, label] of file.candidates) {
      try {
        await evidencePathStillMatches(
          root,
          candidate,
          resolved,
          file.digest.snapshot,
          label,
        );
      } catch (error) {
        errors.push(error instanceof Error ? error.message : String(error));
      }
    }
  }

  const evidenceRecords = references.map((reference) => {
    const digest = uniqueFiles.get(reference.resolved).digest;
    return {
      workflow_id: reference.workflow_id,
      path: reference.path,
      bytes: digest.bytes,
      sha256: digest.sha256,
    };
  });
  return { errors, evidenceRecords };
}

async function main() {
  let options;
  try {
    options = parseArguments(process.argv.slice(2));
  } catch (error) {
    console.error(`${error.message}\n\n${usage()}`);
    process.exitCode = 2;
    return;
  }
  if (options.help) {
    console.log(usage());
    return;
  }

  const expectedRevision = resolveRevision(options.revision);
  const packageRepositoryPath = repositoryRelativePackagePath();
  const artifact = path.resolve(process.cwd(), options.artifact);
  const [auditDocument, manifestDocument, packageDocument] = await Promise.all([
    readJson(artifact, "Audit artifact"),
    readGitJson(
      expectedRevision,
      `${packageRepositoryPath}/screen-reader-audit/workflows.json`,
      "Workflow manifest",
    ),
    readGitJson(
      expectedRevision,
      `${packageRepositoryPath}/package.json`,
      "Test Kit package",
    ),
  ]);
  const audit = auditDocument.value;
  const manifest = manifestDocument.value;
  const packageJson = packageDocument.value;
  const manifestValidation = validateScreenReaderWorkflowManifest(manifest);
  if (manifestValidation.errors.length > 0) {
    for (const error of manifestValidation.errors) console.error(`- ${error}`);
    process.exitCode = 1;
    return;
  }

  const validation = validateScreenReaderAudit({
    audit,
    expectedRevision,
    requirePass: options.requirePass,
    testkitVersion: packageJson.version,
    workflows: manifestValidation.workflows,
  });
  let evidenceRecords = [];
  if (validation.errors.length === 0) {
    const evidenceVerification = await verifyEvidenceFiles(
      audit,
      path.dirname(artifact),
    );
    validation.errors.push(...evidenceVerification.errors);
    evidenceRecords = evidenceVerification.evidenceRecords;
  }
  if (validation.errors.length > 0) {
    for (const error of validation.errors) console.error(`- ${error}`);
    process.exitCode = 1;
    return;
  }

  console.log(
    JSON.stringify(
      {
        protocol: "a3s.test.screen-reader-audit-verification/2",
        audit: {
          path: path.basename(artifact),
          bytes: auditDocument.bytes,
          sha256: auditDocument.sha256,
        },
        workflow_manifest: {
          protocol: manifest.protocol,
          path: "screen-reader-audit/workflows.json",
          bytes: manifestDocument.bytes,
          sha256: manifestDocument.sha256,
        },
        revision: expectedRevision,
        testkit_version: packageJson.version,
        require_pass: options.requirePass,
        summary: validation.summary,
        evidence: evidenceRecords,
        evidence_set_sha256: sha256(
          Buffer.from(JSON.stringify(evidenceRecords), "utf8"),
        ),
      },
      null,
      2,
    ),
  );
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
