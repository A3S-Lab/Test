#!/usr/bin/env node

import { createHash, randomUUID } from "node:crypto";
import { promises as fs } from "node:fs";
import { cpus, platform, release, totalmem } from "node:os";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { A3sTestAdapter, AgentBrowserAdapter } from "./lib/adapters.mjs";
import { CommandFailure, runCommand } from "./lib/command.mjs";
import { renderMarkdown, summarizeBenchmark } from "./lib/report.mjs";
import { startMiniwobServer } from "./lib/server.mjs";
import {
  BenchmarkSpecFailure,
  CandidateBehaviorFailure,
  findRef,
  refTarget,
  TASKS,
} from "./lib/tasks.mjs";

const MINI_WOB_COMMIT = "7fd85d71a4b60325c6585396ec4f48377d049838";
const BROWSERGYM_COMMIT = "9e779f087de9a65668b6974d11f9ce9816026e96";
const VIEWPORT = { width: 332, height: 214 };
const DEFAULT_REPETITIONS = 3;
const runnerDir = dirname(fileURLToPath(import.meta.url));
const benchmarkRoot = resolve(runnerDir, "..");
const repositoryRoot = resolve(benchmarkRoot, "../..");

const options = parseArguments(process.argv.slice(2));
const miniwobRoot = resolve(
  options.miniwobRoot ?? `${benchmarkRoot}/.cache/miniwob-plusplus`,
);
const outputRoot = resolve(options.outputRoot ?? `${benchmarkRoot}/results`);
const a3sTestExecutable = resolve(
  options.a3sTestExecutable ?? `${repositoryRoot}/target/debug/a3s-test`,
);
const agentBrowserExecutable =
  options.agentBrowserExecutable ?? "agent-browser";
const repetitions = options.repetitions ?? DEFAULT_REPETITIONS;
const selectedTasks = selectTasks(options.tasks);
const selectedCandidates = selectCandidates(options.candidates);
const runId = timestampId();
const rawPath = resolve(outputRoot, "raw", `${runId}.json`);
const summaryJsonPath = resolve(outputRoot, "summary", `${runId}.json`);
const summaryMarkdownPath = resolve(outputRoot, "summary", `${runId}.md`);

await verifyMiniwobCommit(miniwobRoot);
const preflight = await collectPreflight({
  a3sTestExecutable,
  agentBrowserExecutable,
});
await fs.mkdir(dirname(rawPath), { recursive: true });
await fs.mkdir(dirname(summaryJsonPath), { recursive: true });

const lockSource = await fs.readFile(
  resolve(benchmarkRoot, "benchmark.lock.acl"),
  "utf8",
);
const raw = {
  schema: "a3s.test.ui-benchmark-raw/1",
  status: "running",
  run_id: runId,
  started_at: new Date().toISOString(),
  completed_at: null,
  lock_digest: `sha256:${createHash("sha256").update(lockSource).digest("hex")}`,
  sources: {
    browsergym: {
      commit: BROWSERGYM_COMMIT,
      version: "0.14.3",
      license: "Apache-2.0",
    },
    miniwob_plusplus: {
      commit: MINI_WOB_COMMIT,
      license: "MIT",
    },
  },
  protocol: {
    repetitions,
    viewport: VIEWPORT,
    task_count: selectedTasks.length,
    tasks: selectedTasks.map(({ id, dimension, seed }) => ({
      id,
      dimension,
      seed,
    })),
    solver: "deterministic-common-plan",
    scoring: "MiniWoB WOB_RAW_REWARD_GLOBAL > 0",
    execution_timing_phases: ["setup", "observation", "action"],
    excluded_timing_phases: ["score", "cleanup"],
    candidate_order: "alternating-by-repetition",
  },
  host: hostFacts(),
  preflight,
  candidates: selectedCandidates.map((candidate) => candidate.metadata),
  runs: [],
  probes: [],
};

await writeJsonAtomic(rawPath, raw);
const server = await startMiniwobServer(miniwobRoot);

try {
  for (let repetition = 1; repetition <= repetitions; repetition += 1) {
    const candidateOrder =
      repetition % 2 === 1
        ? selectedCandidates
        : [...selectedCandidates].reverse();
    for (const task of selectedTasks) {
      for (const candidate of candidateOrder) {
        const result = await runTask({
          candidate,
          task,
          repetition,
          baseUrl: server.baseUrl,
        });
        raw.runs.push(result);
        await writeJsonAtomic(rawPath, raw);
        console.log(
          `${raw.runs.length}/${repetitions * selectedTasks.length * selectedCandidates.length} ` +
            `${candidate.metadata.id} ${task.id} r${repetition}: ` +
            `${result.passed ? "passed" : "failed"} (${result.metrics.execution_ms} ms)`,
        );
      }
    }
  }

  if (!options.noProbes) {
    for (let repetition = 1; repetition <= repetitions; repetition += 1) {
      for (const candidate of selectedCandidates) {
        const probe = await runStaleRefProbe({
          candidate,
          repetition,
          baseUrl: server.baseUrl,
        });
        raw.probes.push(probe);
        await writeJsonAtomic(rawPath, raw);
        console.log(
          `probe ${candidate.metadata.id} stale-ref r${repetition}: ` +
            `${probe.rejected ? "rejected" : "allowed"}`,
        );
      }
    }
    for (const candidate of selectedCandidates) {
      const probe = await runDragProbe({ candidate, baseUrl: server.baseUrl });
      raw.probes.push(probe);
      await writeJsonAtomic(rawPath, raw);
      console.log(
        `probe ${candidate.metadata.id} drag-box: ` +
          `${probe.passed ? "passed" : probe.timed_out ? "timed out" : "failed"}`,
      );
    }
    for (const candidate of selectedCandidates) {
      const probe = await runWheelProbe({ candidate, baseUrl: server.baseUrl });
      raw.probes.push(probe);
      await writeJsonAtomic(rawPath, raw);
      console.log(
        `probe ${candidate.metadata.id} wheel-scroll: ` +
          `${probe.passed ? "passed" : probe.timed_out ? "timed out" : "failed"}`,
      );
    }
  }
} finally {
  await server.close();
}

raw.status = "complete";
raw.completed_at = new Date().toISOString();
await writeJsonAtomic(rawPath, raw);

const summary = summarizeBenchmark(raw);
await writeJsonAtomic(summaryJsonPath, summary);
await fs.writeFile(summaryMarkdownPath, renderMarkdown(summary), "utf8");

console.log(JSON.stringify({ rawPath, summaryJsonPath, summaryMarkdownPath }));

async function runTask({ candidate, task, repetition, baseUrl }) {
  const session = sessionName(candidate.metadata.id, task.id, repetition);
  const adapter = candidate.create(session);
  const url = taskUrl(baseUrl, task.id, task.seed);
  let passed = false;
  let score = null;
  let error = null;
  let failureAttribution = null;
  let cleanupError = null;

  try {
    await adapter.open(url, VIEWPORT);
    const observation = await adapter.observe();
    await task.solve(adapter, observation);
    score = await adapter.score();
    passed = score.done && score.passed;
    if (!passed) {
      failureAttribution = "product";
      error = {
        name: "MiniwobRewardFailure",
        message: score.done
          ? "MiniWoB raw reward was not positive."
          : "MiniWoB did not terminate.",
      };
    }
  } catch (caught) {
    error = serializeError(caught);
    failureAttribution = classifyFailure(caught);
  }

  try {
    await adapter.finish(
      passed,
      passed
        ? "MiniWoB returned a positive raw reward."
        : "Benchmark run failed.",
    );
  } catch (caught) {
    cleanupError = serializeError(caught);
    passed = false;
    failureAttribution ??= "product";
    try {
      await adapter.abort();
    } catch (abortError) {
      cleanupError.abort = serializeError(abortError);
    }
  }

  let artifacts = { bytes: 0, files: [] };
  try {
    artifacts = await adapter.artifactInventory();
  } catch (caught) {
    cleanupError ??= serializeError(caught);
    passed = false;
    failureAttribution ??= "infrastructure";
  }

  return {
    candidate: candidate.metadata.id,
    task: task.id,
    dimension: task.dimension,
    seed: task.seed,
    repetition,
    session,
    url,
    passed,
    score,
    failure_attribution: passed ? null : failureAttribution,
    error,
    cleanup_error: cleanupError,
    metrics: measure(adapter.operations),
    artifacts,
    operations: adapter.operations,
  };
}

async function runStaleRefProbe({ candidate, repetition, baseUrl }) {
  const task = TASKS.find((entry) => entry.id === "click-button");
  const session = sessionName(candidate.metadata.id, "stale-ref", repetition);
  const adapter = candidate.create(session);
  let rejected = false;
  let pageMutated = false;
  let rejection = null;
  let cleanupError = null;

  try {
    await adapter.open(taskUrl(baseUrl, task.id, task.seed), VIEWPORT);
    const first = await adapter.observe();
    const label = first.snapshot.match(
      /Click on the \\"([^\"]+)\\" button\./,
    )?.[1];
    if (!label) {
      throw new BenchmarkSpecFailure("Could not parse stale-ref target label.");
    }
    const ref = findRef(first, "button", label);
    await adapter.observe();
    try {
      await adapter.click(refTarget(ref), first);
    } catch (caught) {
      if (!(caught instanceof CommandFailure)) {
        throw caught;
      }
      rejected = true;
      rejection = serializeError(caught);
    }

    if (!rejected && adapter.id === "agent-browser") {
      const score = await adapter.score();
      pageMutated = score.done;
    }
  } catch (caught) {
    rejection ??= serializeError(caught);
  }

  try {
    await adapter.finish(
      rejected,
      rejected
        ? "The stale observation ref was rejected."
        : "The stale observation ref was not rejected.",
    );
  } catch (caught) {
    cleanupError = serializeError(caught);
    try {
      await adapter.abort();
    } catch (abortError) {
      cleanupError.abort = serializeError(abortError);
    }
  }

  return {
    id: "stale-ref",
    candidate: candidate.metadata.id,
    repetition,
    rejected,
    page_mutated: pageMutated,
    rejection,
    cleanup_error: cleanupError,
    metrics: measure(adapter.operations),
    operations: adapter.operations,
  };
}

async function runDragProbe({ candidate, baseUrl }) {
  const session = sessionName(candidate.metadata.id, "drag-box", 1);
  const adapter = candidate.create(session, {
    processTimeoutMs: 40_000,
    driverCommandTimeoutMs: 12_000,
  });
  let passed = false;
  let timedOut = false;
  let error = null;
  let cleanupError = null;
  let cleanupAttemptError = null;
  let cleanupRecovered = false;

  try {
    await adapter.open(taskUrl(baseUrl, "drag-box", 42), VIEWPORT);
    await adapter.observe();
    await adapter.drag(
      { kind: "css", selector: "#draggableSmall" },
      { kind: "css", selector: "#draggableLarge" },
    );
    await adapter.click({ kind: "css", selector: "#subbtn" });
    const score = await adapter.score();
    passed = score.done && score.passed;
  } catch (caught) {
    error = serializeError(caught);
    timedOut = isTimeoutFailure(caught);
  }

  try {
    await adapter.finish(
      passed,
      passed ? "The drag task passed." : "The drag task did not pass.",
    );
  } catch (caught) {
    cleanupAttemptError = serializeError(caught);
    try {
      await adapter.abort();
      cleanupRecovered = true;
    } catch (abortError) {
      cleanupError = serializeError(abortError);
    }
  }

  return {
    id: "drag-box",
    candidate: candidate.metadata.id,
    repetition: 1,
    passed,
    timed_out: timedOut,
    error,
    cleanup_attempt_error: cleanupAttemptError,
    cleanup_recovered: cleanupRecovered,
    cleanup_error: cleanupError,
    metrics: measure(adapter.operations),
    operations: adapter.operations,
  };
}

async function runWheelProbe({ candidate, baseUrl }) {
  const session = sessionName(candidate.metadata.id, "wheel-scroll", 1);
  const adapter = candidate.create(session, {
    processTimeoutMs: 40_000,
    driverCommandTimeoutMs: 12_000,
  });
  let passed = false;
  let timedOut = false;
  let error = null;
  let cleanupError = null;
  let cleanupAttemptError = null;
  let cleanupRecovered = false;

  try {
    await adapter.open(taskUrl(baseUrl, "scroll-text-2", 303), VIEWPORT);
    const observation = await adapter.observe();
    const direction = observation.snapshot.match(
      /Scroll the textarea to the (bottom|top) of the text hit submit\./,
    )?.[1];
    if (!direction) {
      throw new BenchmarkSpecFailure("Could not parse wheel-scroll direction.");
    }
    await adapter.hover({ kind: "css", selector: "#text-area" });
    await adapter.wheel(direction === "bottom" ? 500 : -500);
    await adapter.click({ kind: "css", selector: "#subbtn" });
    const score = await adapter.score();
    passed = score.done && score.passed;
  } catch (caught) {
    error = serializeError(caught);
    timedOut = isTimeoutFailure(caught);
  }

  try {
    await adapter.finish(
      passed,
      passed ? "The wheel task passed." : "The wheel task did not pass.",
    );
  } catch (caught) {
    cleanupAttemptError = serializeError(caught);
    try {
      await adapter.abort();
      cleanupRecovered = true;
    } catch (abortError) {
      cleanupError = serializeError(abortError);
    }
  }

  return {
    id: "wheel-scroll",
    candidate: candidate.metadata.id,
    repetition: 1,
    passed,
    timed_out: timedOut,
    error,
    cleanup_attempt_error: cleanupAttemptError,
    cleanup_recovered: cleanupRecovered,
    cleanup_error: cleanupError,
    metrics: measure(adapter.operations),
    operations: adapter.operations,
  };
}

function measure(operations) {
  const byPhase = (phase) => operations.filter((item) => item.phase === phase);
  const execution = operations.filter((item) =>
    ["setup", "observation", "action"].includes(item.phase),
  );
  const actions = byPhase("action");
  const observations = byPhase("observation");
  return {
    execution_ms: round(sum(execution, "duration_ms")),
    setup_ms: round(sum(byPhase("setup"), "duration_ms")),
    observation_ms: round(sum(observations, "duration_ms")),
    action_ms: round(sum(actions, "duration_ms")),
    scoring_ms: round(sum(byPhase("score"), "duration_ms")),
    cleanup_ms: round(sum(byPhase("cleanup"), "duration_ms")),
    action_count: actions.length,
    observation_count: observations.length,
    observation_response_bytes: sum(observations, "stdout_bytes"),
    action_request_bytes: sum(actions, "request_bytes"),
    action_response_bytes: sum(actions, "stdout_bytes"),
    total_stdout_bytes: sum(operations, "stdout_bytes"),
    total_stderr_bytes: sum(operations, "stderr_bytes"),
  };
}

function classifyFailure(error) {
  if (error instanceof BenchmarkSpecFailure) {
    return "benchmark_spec";
  }
  if (error instanceof CandidateBehaviorFailure) {
    return "product";
  }
  if (error instanceof CommandFailure) {
    return error.operation?.phase === "setup" ? "infrastructure" : "product";
  }
  return "benchmark_spec";
}

function isTimeoutFailure(error) {
  if (!(error instanceof CommandFailure)) {
    return false;
  }
  const operation = error.operation;
  return (
    operation?.timed_out === true ||
    operation?.exit_code === 124 ||
    /timed? out|timeout|exceeded \d+ ms/i.test(
      `${operation?.stdout ?? ""}\n${operation?.stderr ?? ""}`,
    )
  );
}

function serializeError(error) {
  const body = {
    name: error?.name ?? "Error",
    message: error?.message ?? String(error),
  };
  if (error?.operation) {
    body.operation = {
      phase: error.operation.phase,
      exit_code: error.operation.exit_code,
      timed_out: error.operation.timed_out,
      stdout: error.operation.stdout,
      stderr: error.operation.stderr,
    };
  }
  return body;
}

function taskUrl(baseUrl, task, seed) {
  const url = new URL(`/miniwob/${task}.html`, baseUrl);
  url.searchParams.set("seed", String(seed));
  url.searchParams.set("episode_max_time", "120000");
  return url.href;
}

function sessionName(candidate, task, repetition) {
  const prefix = candidate === "a3s-test" ? "a3s" : "ab";
  const suffix = randomUUID().slice(0, 8);
  return `ui-${prefix}-${task.slice(0, 18)}-${repetition}-${suffix}`;
}

async function collectPreflight({ a3sTestExecutable, agentBrowserExecutable }) {
  const a3sVersion = await simpleCommand(a3sTestExecutable, ["--version"]);
  const agentBrowserVersion = await simpleCommand(agentBrowserExecutable, [
    "--version",
  ]);
  const capabilitiesOperation = await runCommand({
    executable: a3sTestExecutable,
    args: ["capabilities", "--browser-driver", "standalone", "--json"],
    cwd: repositoryRoot,
    timeoutMs: 30_000,
    phase: "preflight",
  });
  if (capabilitiesOperation.exit_code !== 0) {
    throw new CommandFailure(
      "A3S Test capability preflight failed.",
      capabilitiesOperation,
    );
  }
  const capabilities = JSON.parse(capabilitiesOperation.stdout);
  if (capabilities.protocol_revision !== 15) {
    throw new Error(
      `Expected A3S Test protocol revision 15, found ${capabilities.protocol_revision}.`,
    );
  }
  if (capabilities.version !== "0.26.0") {
    throw new Error(
      `Expected standalone driver 0.26.0, found ${capabilities.version}.`,
    );
  }
  return {
    a3s_test_version: a3sVersion,
    agent_browser_version: agentBrowserVersion,
    standalone_capabilities: capabilities,
    repository_commit: await simpleCommand("git", ["rev-parse", "HEAD"]),
    repository_status: await simpleCommand("git", ["status", "--short"]),
  };
}

async function verifyMiniwobCommit(root) {
  const commit = await simpleCommand("git", ["-C", root, "rev-parse", "HEAD"]);
  if (commit !== MINI_WOB_COMMIT) {
    throw new Error(`Expected MiniWoB ${MINI_WOB_COMMIT}, found ${commit}.`);
  }
}

async function simpleCommand(executable, args) {
  const operation = await runCommand({
    executable,
    args,
    cwd: repositoryRoot,
    timeoutMs: 30_000,
    phase: "preflight",
  });
  if (operation.exit_code !== 0) {
    throw new CommandFailure(`${executable} ${args[0]} failed.`, operation);
  }
  return operation.stdout.trim();
}

function selectTasks(requested) {
  if (requested.length === 0) {
    return TASKS;
  }
  const selected = TASKS.filter((task) => requested.includes(task.id));
  const missing = requested.filter(
    (id) => !selected.some((task) => task.id === id),
  );
  if (missing.length > 0) {
    throw new Error(`Unknown task(s): ${missing.join(", ")}`);
  }
  return selected;
}

function selectCandidates(requested) {
  const definitions = [
    {
      metadata: {
        id: "a3s-test",
        version: "1.0.0",
        browser_driver: "standalone",
        protocol_revision: 15,
      },
      create: (session, timeouts = {}) =>
        new A3sTestAdapter({
          cwd: repositoryRoot,
          session,
          commandTimeoutMs: timeouts.processTimeoutMs ?? 35_000,
          driverCommandTimeoutMs: timeouts.driverCommandTimeoutMs ?? 30_000,
          a3sTestExecutable,
        }),
    },
    {
      metadata: { id: "agent-browser", version: "0.26.0" },
      create: (session, timeouts = {}) =>
        new AgentBrowserAdapter({
          cwd: repositoryRoot,
          session,
          commandTimeoutMs: timeouts.processTimeoutMs ?? 35_000,
          agentBrowserExecutable,
        }),
    },
  ];
  if (requested.length === 0) {
    return definitions;
  }
  const selected = definitions.filter((candidate) =>
    requested.includes(candidate.metadata.id),
  );
  const missing = requested.filter(
    (id) => !selected.some((candidate) => candidate.metadata.id === id),
  );
  if (missing.length > 0) {
    throw new Error(`Unknown candidate(s): ${missing.join(", ")}`);
  }
  return selected;
}

function parseArguments(args) {
  const parsed = {
    tasks: [],
    candidates: [],
    noProbes: args.includes("--no-probes"),
  };
  for (let index = 0; index < args.length; index += 1) {
    const name = args[index];
    if (name === "--no-probes") {
      continue;
    }
    const value = args[index + 1];
    if (!value) {
      throw new Error(`${name} requires a value.`);
    }
    index += 1;
    if (name === "--miniwob-root") parsed.miniwobRoot = value;
    else if (name === "--output-root") parsed.outputRoot = value;
    else if (name === "--a3s-test") parsed.a3sTestExecutable = value;
    else if (name === "--agent-browser") parsed.agentBrowserExecutable = value;
    else if (name === "--task") parsed.tasks.push(value);
    else if (name === "--candidate") parsed.candidates.push(value);
    else if (name === "--repetitions")
      parsed.repetitions = positiveInteger(value);
    else throw new Error(`Unknown argument: ${name}`);
  }
  return parsed;
}

function positiveInteger(value) {
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed <= 0) {
    throw new Error(`Expected a positive integer, received ${value}.`);
  }
  return parsed;
}

function hostFacts() {
  const processors = cpus();
  return {
    platform: platform(),
    release: release(),
    architecture: process.arch,
    cpu_model: processors[0]?.model ?? "unknown",
    cpu_count: processors.length,
    memory_bytes: totalmem(),
    node: process.version,
  };
}

async function writeJsonAtomic(path, value) {
  const temporary = `${path}.tmp`;
  await fs.writeFile(temporary, `${JSON.stringify(value, null, 2)}\n`, "utf8");
  await fs.rename(temporary, path);
}

function timestampId() {
  return new Date()
    .toISOString()
    .replace(/[-:]/g, "")
    .replace(/\.\d{3}Z$/, "Z");
}

function sum(items, key) {
  return items.reduce((total, item) => total + item[key], 0);
}

function round(value) {
  return Math.round(value * 1_000) / 1_000;
}
