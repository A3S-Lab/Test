#!/usr/bin/env node

import { promises as fs } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { requireSuccess, runCommand } from "./lib/command.mjs";

const MINI_WOB_REPOSITORY =
  "https://github.com/Farama-Foundation/miniwob-plusplus.git";
const MINI_WOB_COMMIT = "7fd85d71a4b60325c6585396ec4f48377d049838";
const runnerDir = dirname(fileURLToPath(import.meta.url));
const benchmarkRoot = resolve(runnerDir, "..");
const target = resolve(
  argument("--target") ?? `${benchmarkRoot}/.cache/miniwob-plusplus`,
);

await fs.mkdir(dirname(target), { recursive: true });
const existingCommit = await readCommit(target);
if (existingCommit) {
  if (existingCommit !== MINI_WOB_COMMIT) {
    throw new Error(
      `Refusing to replace ${target}: expected ${MINI_WOB_COMMIT}, found ${existingCommit}.`,
    );
  }
  console.log(target);
  process.exit(0);
}

try {
  await fs.access(target);
  throw new Error(`Refusing to initialize non-repository path: ${target}`);
} catch (error) {
  if (error.code !== "ENOENT") {
    throw error;
  }
}

await fs.mkdir(target);
await git(["init"], target);
await git(["remote", "add", "origin", MINI_WOB_REPOSITORY], target);
await git(
  ["fetch", "--depth", "1", "origin", MINI_WOB_COMMIT],
  target,
  120_000,
);
await git(["checkout", "--detach", "FETCH_HEAD"], target);

const commit = await readCommit(target);
if (commit !== MINI_WOB_COMMIT) {
  throw new Error(`MiniWoB checkout verification failed: ${commit}`);
}
console.log(target);

async function git(args, cwd, timeoutMs = 30_000) {
  const operation = await runCommand({
    executable: "git",
    args,
    cwd,
    timeoutMs,
    phase: "setup",
  });
  requireSuccess(operation, `git ${args[0]}`);
}

async function readCommit(cwd) {
  try {
    const operation = await runCommand({
      executable: "git",
      args: ["rev-parse", "HEAD"],
      cwd,
      timeoutMs: 10_000,
      phase: "setup",
    });
    if (operation.exit_code !== 0) {
      return null;
    }
    return operation.stdout.trim();
  } catch {
    return null;
  }
}

function argument(name) {
  const index = process.argv.indexOf(name);
  if (index === -1) {
    return null;
  }
  const value = process.argv[index + 1];
  if (!value) {
    throw new Error(`${name} requires a value.`);
  }
  return value;
}
