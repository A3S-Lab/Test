#!/usr/bin/env node

import { promises as fs } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { renderMarkdown, summarizeBenchmark } from "./lib/report.mjs";

const runnerDir = dirname(fileURLToPath(import.meta.url));
const benchmarkRoot = resolve(runnerDir, "..");
const options = parseArguments(process.argv.slice(2));
const rawPath = resolve(options.raw);
const outputRoot = resolve(
  options.outputRoot ?? resolve(benchmarkRoot, "results"),
);

const raw = JSON.parse(await fs.readFile(rawPath, "utf8"));
validateRaw(raw, rawPath);

const summary = summarizeBenchmark(raw);
const summaryJsonPath = resolve(outputRoot, "summary", `${raw.run_id}.json`);
const summaryMarkdownPath = resolve(outputRoot, "summary", `${raw.run_id}.md`);

await fs.mkdir(dirname(summaryJsonPath), { recursive: true });
await writeJsonAtomic(summaryJsonPath, summary);
await fs.writeFile(summaryMarkdownPath, renderMarkdown(summary), "utf8");

console.log(
  JSON.stringify({ rawPath, summaryJsonPath, summaryMarkdownPath }, null, 2),
);

function parseArguments(args) {
  const parsed = {};
  for (let index = 0; index < args.length; index += 1) {
    const name = args[index];
    const value = args[index + 1];
    if (!value) {
      throw new Error(`${name} requires a value.`);
    }
    index += 1;
    if (name === "--raw") parsed.raw = value;
    else if (name === "--output-root") parsed.outputRoot = value;
    else throw new Error(`Unknown argument: ${name}`);
  }
  if (!parsed.raw) {
    throw new Error("--raw is required.");
  }
  return parsed;
}

function validateRaw(raw, path) {
  if (raw?.schema !== "a3s.test.ui-benchmark-raw/1") {
    throw new Error(`Unsupported raw benchmark schema in ${path}.`);
  }
  if (raw.status !== "complete" || !raw.completed_at) {
    throw new Error(`Raw benchmark is not complete: ${path}`);
  }
  if (!/^[A-Za-z0-9._-]+$/.test(raw.run_id ?? "")) {
    throw new Error(`Raw benchmark has an unsafe run_id: ${raw.run_id}`);
  }
  if (!Array.isArray(raw.candidates) || !Array.isArray(raw.runs)) {
    throw new Error(`Raw benchmark lacks candidates or runs: ${path}`);
  }
  if (!Array.isArray(raw.probes)) {
    throw new Error(`Raw benchmark lacks probes: ${path}`);
  }
}

async function writeJsonAtomic(path, value) {
  const temporary = `${path}.tmp`;
  await fs.writeFile(temporary, `${JSON.stringify(value, null, 2)}\n`, "utf8");
  await fs.rename(temporary, path);
}
