#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { readFile, realpath, stat } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  validateScreenReaderAudit,
  validateScreenReaderWorkflowManifest,
} from "./screen-reader-audit-validation.mjs";

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const packageRoot = path.resolve(scriptDirectory, "..");
const MAX_JSON_BYTES = 8 * 1024 * 1024;

function usage() {
  return [
    "Usage: node scripts/check-screen-reader-audit.mjs <audit.json> [options]",
    "",
    "Options:",
    "  --revision <sha>  Require the audited Git revision (defaults to HEAD)",
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

async function readJson(filename, label) {
  let metadata;
  try {
    metadata = await stat(filename);
  } catch (error) {
    throw new Error(`${label} could not be inspected: ${error.message}`);
  }
  if (!metadata.isFile()) throw new Error(`${label} must be a regular file`);
  if (metadata.size > MAX_JSON_BYTES) {
    throw new Error(`${label} exceeds 8 MiB`);
  }
  let encoded;
  try {
    encoded = await readFile(filename, "utf8");
  } catch (error) {
    throw new Error(`${label} could not be read: ${error.message}`);
  }
  try {
    return JSON.parse(encoded);
  } catch (error) {
    throw new Error(`${label} is not valid JSON: ${error.message}`);
  }
}

function headRevision() {
  return execFileSync("git", ["rev-parse", "HEAD"], {
    cwd: packageRoot,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  }).trim();
}

async function verifyEvidenceFiles(audit, artifactDirectory) {
  const errors = [];
  const root = await realpath(artifactDirectory);
  for (const result of audit.results) {
    for (const evidence of result.evidence) {
      const candidate = path.resolve(root, evidence);
      let resolved;
      let metadata;
      try {
        resolved = await realpath(candidate);
        metadata = await stat(resolved);
      } catch {
        errors.push(
          `Workflow ${result.workflow_id} evidence artifact ${evidence} is missing.`,
        );
        continue;
      }
      if (!resolved.startsWith(`${root}${path.sep}`)) {
        errors.push(
          `Workflow ${result.workflow_id} evidence artifact ${evidence} escapes the audit directory.`,
        );
      } else if (!metadata.isFile() || metadata.size === 0) {
        errors.push(
          `Workflow ${result.workflow_id} evidence artifact ${evidence} must be a non-empty regular file.`,
        );
      } else if (metadata.size > 64 * 1024 * 1024) {
        errors.push(
          `Workflow ${result.workflow_id} evidence artifact ${evidence} exceeds 64 MiB.`,
        );
      }
    }
  }
  return errors;
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

  const artifact = path.resolve(process.cwd(), options.artifact);
  const [audit, manifest, packageJson] = await Promise.all([
    readJson(artifact, "Audit artifact"),
    readJson(
      path.join(packageRoot, "screen-reader-audit", "workflows.json"),
      "Workflow manifest",
    ),
    readJson(path.join(packageRoot, "package.json"), "Test Kit package"),
  ]);
  const manifestValidation = validateScreenReaderWorkflowManifest(manifest);
  if (manifestValidation.errors.length > 0) {
    for (const error of manifestValidation.errors) console.error(`- ${error}`);
    process.exitCode = 1;
    return;
  }

  const expectedRevision = options.revision ?? headRevision();
  const validation = validateScreenReaderAudit({
    audit,
    expectedRevision,
    requirePass: options.requirePass,
    testkitVersion: packageJson.version,
    workflows: manifestValidation.workflows,
  });
  if (validation.errors.length === 0) {
    validation.errors.push(
      ...(await verifyEvidenceFiles(audit, path.dirname(artifact))),
    );
  }
  if (validation.errors.length > 0) {
    for (const error of validation.errors) console.error(`- ${error}`);
    process.exitCode = 1;
    return;
  }

  console.log(
    JSON.stringify(
      {
        protocol: "a3s.test.screen-reader-audit-verification/1",
        artifact,
        revision: expectedRevision,
        testkit_version: packageJson.version,
        require_pass: options.requirePass,
        summary: validation.summary,
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
