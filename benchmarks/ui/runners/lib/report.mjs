export function summarizeBenchmark(raw) {
  const candidates = {};
  for (const candidate of raw.candidates) {
    const runs = raw.runs.filter((run) => run.candidate === candidate.id);
    candidates[candidate.id] = summarizeCandidate(runs, raw.probes);
  }

  const pairedCommonSuccess = summarizePairedCommonSuccess(
    raw,
    "a3s-test",
    "agent-browser",
  );

  return {
    schema: "a3s.test.ui-benchmark-summary/1",
    run_id: raw.run_id,
    generated_at: raw.completed_at ?? new Date().toISOString(),
    lock_digest: raw.lock_digest,
    protocol: raw.protocol,
    host: raw.host,
    candidates,
    paired_common_success: pairedCommonSuccess,
    limitations: [
      "This compares execution layers with a deterministic solver, not model reasoning quality.",
      `The locked MiniWoB subset covers ${raw.protocol.task_count} interaction mechanisms, not all 125 registered tasks.`,
      "Timing includes a fresh CLI process for each turn and is specific to this host.",
      "WebArena and VisualWebArena remain gated behind their multi-service environment setup.",
    ],
  };
}

export function renderMarkdown(summary) {
  const candidateRows = Object.entries(summary.candidates)
    .map(([id, candidate]) =>
      [
        id,
        percent(candidate.success_rate),
        format(candidate.execution_ms.p50),
        format(candidate.execution_ms.p95),
        format(candidate.mean_action_count),
        formatBytes(candidate.mean_observation_bytes),
        percent(candidate.flake_rate),
        percent(candidate.cleanup_success_rate),
        formatBytes(candidate.mean_builtin_artifact_bytes),
      ].join(" | "),
    )
    .join("\n");

  const dimensionRows = dimensionTable(summary);
  const failureRows = failureTable(summary);
  const probeRows = Object.entries(summary.candidates)
    .map(([id, candidate]) => {
      const stale = candidate.probes.stale_ref;
      return `${id} | ${stale.runs} | ${percent(stale.rejection_rate)} | ${percent(stale.page_mutation_rate)}`;
    })
    .join("\n");
  const dragRows = Object.entries(summary.candidates)
    .map(([id, candidate]) => {
      const drag = candidate.probes.drag_box;
      return `${id} | ${drag.runs} | ${percent(drag.success_rate)} | ${percent(drag.timeout_rate)} | ${percent(drag.cleanup_success_rate)} | ${percent(drag.cleanup_recovery_rate)}`;
    })
    .join("\n");
  const wheelRows = Object.entries(summary.candidates)
    .map(([id, candidate]) => {
      const wheel = candidate.probes.wheel_scroll;
      return `${id} | ${wheel.runs} | ${percent(wheel.success_rate)} | ${percent(wheel.timeout_rate)} | ${percent(wheel.cleanup_success_rate)} | ${percent(wheel.cleanup_recovery_rate)}`;
    })
    .join("\n");
  const pairedLatency = renderPairedLatency(summary.paired_common_success);

  return `# UI execution-layer benchmark: ${summary.run_id}

Lock digest: \`${summary.lock_digest}\`

## Aggregate results

Candidate | Success | p50 execution (ms) | p95 execution (ms) | Mean actions | Mean observation bytes | Flake rate | Cleanup | Built-in artifacts
--- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---:
${candidateRows}

Execution time is the sum of candidate setup, observation, and task-action
commands. MiniWoB scoring and owned cleanup are recorded separately.

${pairedLatency}

## Success by dimension

Dimension | Candidate | Passed / Runs | Success
--- | --- | ---: | ---:
${dimensionRows}

## Failure attribution

Candidate | Product | Benchmark/spec | Infrastructure
--- | ---: | ---: | ---:
${failureRows}

## Stale-reference safety probe

Candidate | Runs | Rejected old ref | Page mutated
--- | ---: | ---: | ---:
${probeRows}

This probe deliberately observes twice, then attempts an action with the first
observation's ref. Rejection is a safety property and is not included in task
success rate.

## Drag capability probe

Candidate | Runs | Success | Timeout | Final cleanup | Fallback cleanup
--- | ---: | ---: | ---: | ---: | ---:
${dragRows}

The drag probe uses the official MiniWoB \`drag-box\` task once per candidate.
It is isolated from the main success rate because a driver timeout would
otherwise dominate three repeated timing samples.

## Wheel capability probe

Candidate | Runs | Success | Timeout | Final cleanup | Fallback cleanup
--- | ---: | ---: | ---: | ---: | ---:
${wheelRows}

The wheel probe uses the official MiniWoB \`scroll-text-2\` task once per
candidate and is isolated for the same timeout-control reason.

## Scope limits

${summary.limitations.map((item) => `- ${item}`).join("\n")}
`;
}

export function summarizePairedCommonSuccess(raw, candidateId, baselineId) {
  const candidatePresent = raw.candidates.some(
    (candidate) => candidate.id === candidateId,
  );
  const baselinePresent = raw.candidates.some(
    (candidate) => candidate.id === baselineId,
  );
  if (!candidatePresent || !baselinePresent) {
    return null;
  }

  const groupedRuns = Map.groupBy(raw.runs, (run) =>
    JSON.stringify([run.task, run.seed, run.repetition]),
  );
  const includedPairs = [];
  const excludedPairs = [];

  for (const runs of groupedRuns.values()) {
    const candidateRuns = runs.filter((run) => run.candidate === candidateId);
    const baselineRuns = runs.filter((run) => run.candidate === baselineId);
    if (candidateRuns.length > 1 || baselineRuns.length > 1) {
      const identity = runs[0];
      throw new Error(
        `Duplicate paired run for ${identity.task} repetition ${identity.repetition}.`,
      );
    }
    const [candidate] = candidateRuns;
    const [baseline] = baselineRuns;
    const identity = candidate ?? baseline;
    if (!identity) {
      continue;
    }

    if (!candidate || !baseline || !candidate.passed || !baseline.passed) {
      excludedPairs.push({
        task: identity.task,
        seed: identity.seed,
        repetition: identity.repetition,
        candidate_passed: candidate?.passed ?? null,
        baseline_passed: baseline?.passed ?? null,
      });
      continue;
    }

    const candidateExecution = requireExecutionMs(candidate);
    const baselineExecution = requireExecutionMs(baseline);
    if (baselineExecution <= 0) {
      throw new Error(
        `Paired baseline execution must be positive for ${identity.task} repetition ${identity.repetition}.`,
      );
    }
    const overheadMs = candidateExecution - baselineExecution;
    includedPairs.push({
      candidate_execution_ms: candidateExecution,
      baseline_execution_ms: baselineExecution,
      overhead_ms: overheadMs,
      overhead_ratio: overheadMs / baselineExecution,
    });
  }

  excludedPairs.sort(
    (left, right) =>
      left.repetition - right.repetition ||
      compareText(left.task, right.task) ||
      left.seed - right.seed,
  );

  const overheadValues = includedPairs.map((pair) => pair.overhead_ms);
  return {
    candidate: candidateId,
    baseline: baselineId,
    pair_key: ["task", "seed", "repetition"],
    total_pair_count: groupedRuns.size,
    common_success_pair_count: includedPairs.length,
    excluded_pair_count: excludedPairs.length,
    excluded_pairs: excludedPairs,
    candidate_execution_ms: distribution(
      includedPairs.map((pair) => pair.candidate_execution_ms),
    ),
    baseline_execution_ms: distribution(
      includedPairs.map((pair) => pair.baseline_execution_ms),
    ),
    overhead_ms: distribution(overheadValues),
    overhead_ratio: distribution(
      includedPairs.map((pair) => pair.overhead_ratio),
    ),
    candidate_faster_pairs: overheadValues.filter((value) => value < 0).length,
    baseline_faster_pairs: overheadValues.filter((value) => value > 0).length,
    tied_pairs: overheadValues.filter((value) => value === 0).length,
  };
}

function summarizeCandidate(runs, probes) {
  const passed = runs.filter((run) => run.passed).length;
  const executionTimes = runs.map((run) => run.metrics.execution_ms);
  const groups = Map.groupBy(runs, (run) => `${run.task}:${run.seed}`);
  const flakyGroups = [...groups.values()].filter(
    (group) => new Set(group.map((run) => run.passed)).size > 1,
  ).length;
  const cleanupSuccesses = runs.filter(
    (run) => run.cleanup_error === null,
  ).length;
  const staleProbes = probes.filter(
    (probe) =>
      probe.candidate === runs[0]?.candidate && probe.id === "stale-ref",
  );
  const dragProbes = probes.filter(
    (probe) =>
      probe.candidate === runs[0]?.candidate && probe.id === "drag-box",
  );
  const wheelProbes = probes.filter(
    (probe) =>
      probe.candidate === runs[0]?.candidate && probe.id === "wheel-scroll",
  );

  return {
    runs: runs.length,
    passed,
    success_rate: ratio(passed, runs.length),
    first_attempt_success_rate: ratio(passed, runs.length),
    execution_ms: {
      p50: percentile(executionTimes, 0.5),
      p95: percentile(executionTimes, 0.95),
      mean: mean(executionTimes),
    },
    mean_action_count: mean(runs.map((run) => run.metrics.action_count)),
    mean_observation_count: mean(
      runs.map((run) => run.metrics.observation_count),
    ),
    mean_observation_bytes: mean(
      runs.map((run) => run.metrics.observation_response_bytes),
    ),
    mean_action_request_bytes: mean(
      runs.map((run) => run.metrics.action_request_bytes),
    ),
    mean_action_response_bytes: mean(
      runs.map((run) => run.metrics.action_response_bytes),
    ),
    flake_rate: ratio(flakyGroups, groups.size),
    flaky_task_instances: flakyGroups,
    cleanup_success_rate: ratio(cleanupSuccesses, runs.length),
    mean_cleanup_ms: mean(runs.map((run) => run.metrics.cleanup_ms)),
    mean_scoring_ms: mean(runs.map((run) => run.metrics.scoring_ms)),
    mean_builtin_artifact_bytes: mean(runs.map((run) => run.artifacts.bytes)),
    evidence_file_rate: ratio(
      runs.filter((run) => run.artifacts.files.length > 0).length,
      runs.length,
    ),
    failure_attribution: countFailures(runs),
    dimensions: summarizeDimensions(runs),
    probes: {
      stale_ref: {
        runs: staleProbes.length,
        rejection_rate: ratio(
          staleProbes.filter((probe) => probe.rejected).length,
          staleProbes.length,
        ),
        page_mutation_rate: ratio(
          staleProbes.filter((probe) => probe.page_mutated).length,
          staleProbes.length,
        ),
      },
      drag_box: {
        runs: dragProbes.length,
        success_rate: ratio(
          dragProbes.filter((probe) => probe.passed).length,
          dragProbes.length,
        ),
        timeout_rate: ratio(
          dragProbes.filter((probe) => probe.timed_out).length,
          dragProbes.length,
        ),
        cleanup_success_rate: ratio(
          dragProbes.filter((probe) => probe.cleanup_error === null).length,
          dragProbes.length,
        ),
        cleanup_recovery_rate: ratio(
          dragProbes.filter((probe) => probe.cleanup_recovered).length,
          dragProbes.length,
        ),
      },
      wheel_scroll: {
        runs: wheelProbes.length,
        success_rate: ratio(
          wheelProbes.filter((probe) => probe.passed).length,
          wheelProbes.length,
        ),
        timeout_rate: ratio(
          wheelProbes.filter((probe) => probe.timed_out).length,
          wheelProbes.length,
        ),
        cleanup_success_rate: ratio(
          wheelProbes.filter((probe) => probe.cleanup_error === null).length,
          wheelProbes.length,
        ),
        cleanup_recovery_rate: ratio(
          wheelProbes.filter((probe) => probe.cleanup_recovered).length,
          wheelProbes.length,
        ),
      },
    },
  };
}

function renderPairedLatency(paired) {
  if (!paired) {
    return "";
  }

  const excludedTasks = [
    ...new Set(paired.excluded_pairs.map((pair) => pair.task)),
  ];
  const exclusionNote =
    excludedTasks.length === 0
      ? "No task instances were excluded."
      : `Excluded task instances: ${excludedTasks.map((task) => `\`${task}\``).join(", ")}.`;

  return `## Paired common-success latency

Runs are paired by task, seed, and repetition. Only pairs where both
candidates passed are included, so an early product failure cannot appear as
a latency advantage. Included pairs: ${paired.common_success_pair_count} / ${paired.total_pair_count}.
${exclusionNote}

Measure | Median | Mean | p95
--- | ---: | ---: | ---:
${paired.candidate} execution | ${formatMs(paired.candidate_execution_ms.median)} | ${formatMs(paired.candidate_execution_ms.mean)} | ${formatMs(paired.candidate_execution_ms.p95)}
${paired.baseline} execution | ${formatMs(paired.baseline_execution_ms.median)} | ${formatMs(paired.baseline_execution_ms.mean)} | ${formatMs(paired.baseline_execution_ms.p95)}
${paired.candidate} overhead vs ${paired.baseline} | ${formatSignedMs(paired.overhead_ms.median)} (${formatSignedPercent(paired.overhead_ratio.median)}) | ${formatSignedMs(paired.overhead_ms.mean)} (${formatSignedPercent(paired.overhead_ratio.mean)}) | ${formatSignedMs(paired.overhead_ms.p95)} (${formatSignedPercent(paired.overhead_ratio.p95)})

Lower execution time: ${paired.candidate} ${paired.candidate_faster_pairs},
${paired.baseline} ${paired.baseline_faster_pairs}, ties ${paired.tied_pairs}.`;
}

function summarizeDimensions(runs) {
  const dimensions = {};
  for (const [dimension, dimensionRuns] of Map.groupBy(
    runs,
    (run) => run.dimension,
  )) {
    const passed = dimensionRuns.filter((run) => run.passed).length;
    dimensions[dimension] = {
      runs: dimensionRuns.length,
      passed,
      success_rate: ratio(passed, dimensionRuns.length),
    };
  }
  return dimensions;
}

function countFailures(runs) {
  const counts = { product: 0, benchmark_spec: 0, infrastructure: 0 };
  for (const run of runs) {
    if (!run.passed && run.failure_attribution) {
      counts[run.failure_attribution] += 1;
    }
  }
  return counts;
}

function dimensionTable(summary) {
  const rows = [];
  for (const [candidateId, candidate] of Object.entries(summary.candidates)) {
    for (const [dimension, result] of Object.entries(candidate.dimensions)) {
      rows.push(
        `${dimension} | ${candidateId} | ${result.passed} / ${result.runs} | ${percent(result.success_rate)}`,
      );
    }
  }
  return rows.sort().join("\n");
}

function failureTable(summary) {
  return Object.entries(summary.candidates)
    .map(([id, candidate]) => {
      const failures = candidate.failure_attribution;
      return `${id} | ${failures.product} | ${failures.benchmark_spec} | ${failures.infrastructure}`;
    })
    .join("\n");
}

function percentile(values, quantile) {
  if (values.length === 0) {
    return null;
  }
  const sorted = [...values].sort((left, right) => left - right);
  const index = Math.min(
    sorted.length - 1,
    Math.max(0, Math.ceil(quantile * sorted.length) - 1),
  );
  return round(sorted[index]);
}

function distribution(values) {
  return {
    median: percentile(values, 0.5),
    mean: mean(values),
    p95: percentile(values, 0.95),
  };
}

function requireExecutionMs(run) {
  const value = run.metrics?.execution_ms;
  if (!Number.isFinite(value)) {
    throw new Error(
      `Run ${run.candidate}/${run.task}/r${run.repetition} has no finite execution_ms.`,
    );
  }
  return value;
}

function compareText(left, right) {
  if (left < right) return -1;
  if (left > right) return 1;
  return 0;
}

function mean(values) {
  if (values.length === 0) {
    return null;
  }
  return round(values.reduce((sum, value) => sum + value, 0) / values.length);
}

function ratio(numerator, denominator) {
  return denominator === 0 ? 0 : round(numerator / denominator);
}

function percent(value) {
  return `${format(value * 100)}%`;
}

function format(value) {
  return value === null ? "n/a" : String(round(value));
}

function formatBytes(value) {
  if (value === null) {
    return "n/a";
  }
  if (value < 1024) {
    return `${format(value)} B`;
  }
  return `${format(value / 1024)} KiB`;
}

function formatMs(value) {
  return value === null ? "n/a" : `${format(value)} ms`;
}

function formatSignedMs(value) {
  return value === null ? "n/a" : `${signed(value)} ms`;
}

function formatSignedPercent(value) {
  return value === null ? "n/a" : `${signed(value * 100)}%`;
}

function signed(value) {
  const formatted = format(value);
  return value > 0 ? `+${formatted}` : formatted;
}

function round(value) {
  return Math.round(value * 1_000) / 1_000;
}
