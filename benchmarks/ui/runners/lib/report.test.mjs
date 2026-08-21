import assert from "node:assert/strict";
import test from "node:test";
import {
  renderMarkdown,
  summarizeBenchmark,
  summarizePairedCommonSuccess,
} from "./report.mjs";

test("summarizes latency only across paired common successes", () => {
  const raw = fixtureRaw();
  const paired = summarizePairedCommonSuccess(raw, "a3s-test", "agent-browser");

  assert.deepEqual(paired, {
    candidate: "a3s-test",
    baseline: "agent-browser",
    pair_key: ["task", "seed", "repetition"],
    total_pair_count: 3,
    common_success_pair_count: 2,
    excluded_pair_count: 1,
    excluded_pairs: [
      {
        task: "task-three",
        seed: 3,
        repetition: 1,
        candidate_passed: true,
        baseline_passed: false,
      },
    ],
    candidate_execution_ms: { median: 110, mean: 125, p95: 140 },
    baseline_execution_ms: { median: 100, mean: 100, p95: 100 },
    overhead_ms: { median: 10, mean: 25, p95: 40 },
    overhead_ratio: { median: 0.1, mean: 0.25, p95: 0.4 },
    candidate_faster_pairs: 0,
    baseline_faster_pairs: 2,
    tied_pairs: 0,
  });
});

test("renders the paired methodology and uses raw completion time", () => {
  const summary = summarizeBenchmark(fixtureRaw());
  const markdown = renderMarkdown(summary);

  assert.equal(summary.generated_at, "2026-08-21T00:00:01.000Z");
  assert.match(markdown, /## Paired common-success latency/);
  assert.match(markdown, /Included pairs: 2 \/ 3\./);
  assert.match(markdown, /Excluded task instances: `task-three`\./);
  assert.match(markdown, /\+10 ms \(\+10%\)/);
  assert.match(markdown, /\+25 ms \(\+25%\)/);
  assert.match(markdown, /\+40 ms \(\+40%\)/);
});

test("rejects duplicate runs for a paired key", () => {
  const raw = fixtureRaw();
  raw.runs.push({ ...raw.runs[0] });

  assert.throws(
    () => summarizePairedCommonSuccess(raw, "a3s-test", "agent-browser"),
    /Duplicate paired run for task-one repetition 1/,
  );
});

function fixtureRaw() {
  return {
    schema: "a3s.test.ui-benchmark-raw/1",
    status: "complete",
    run_id: "fixture",
    completed_at: "2026-08-21T00:00:01.000Z",
    lock_digest: "sha256:fixture",
    protocol: { task_count: 3 },
    host: { platform: "fixture" },
    candidates: [{ id: "a3s-test" }, { id: "agent-browser" }],
    runs: [
      run("a3s-test", "task-one", 1, 1, true, 110),
      run("agent-browser", "task-one", 1, 1, true, 100),
      run("agent-browser", "task-two", 2, 1, true, 100),
      run("a3s-test", "task-two", 2, 1, true, 140),
      run("a3s-test", "task-three", 3, 1, true, 150),
      run("agent-browser", "task-three", 3, 1, false, 50),
    ],
    probes: [],
  };
}

function run(candidate, task, seed, repetition, passed, executionMs) {
  return {
    candidate,
    task,
    dimension: task,
    seed,
    repetition,
    passed,
    cleanup_error: null,
    failure_attribution: passed ? null : "product",
    metrics: {
      execution_ms: executionMs,
      action_count: 1,
      observation_count: 1,
      observation_response_bytes: 100,
      action_request_bytes: 10,
      action_response_bytes: 20,
      cleanup_ms: 5,
      scoring_ms: 5,
    },
    artifacts: { bytes: 0, files: [] },
  };
}
