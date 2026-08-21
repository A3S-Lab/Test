# UI execution-layer benchmark

This benchmark compares how A3S Test and a direct `agent-browser` client
execute the same deterministic Web interactions. It measures execution-layer
behavior, safety, evidence, cleanup, and host-local latency. It does **not**
compare model reasoning quality.

The current primary benchmark is a locked MiniWoB++ subset. WebArena and
VisualWebArena are retained as follow-on environments rather than being
started with partially controlled infrastructure.

## Benchmark selection

| Benchmark              | Locked research snapshot                                                                                                                                                                                                                   | What it covers                                                                 | Environment and control                                                                                    | Decision                                                        |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------- |
| BrowserGym + MiniWoB++ | [BrowserGym `9e779f0`](https://github.com/ServiceNow/BrowserGym/tree/9e779f087de9a65668b6974d11f9ce9816026e96), [MiniWoB++ `7fd85d7`](https://github.com/Farama-Foundation/miniwob-plusplus/tree/7fd85d71a4b60325c6585396ec4f48377d049838) | 125 registered, synthetic browser tasks covering atomic interaction mechanisms | Local official HTML, deterministic seed, small viewport, direct reward signal, no account or service state | **Primary execution-layer benchmark**                           |
| WebArena               | [`dce0468`](https://github.com/web-arena-x/webarena/tree/dce04686a56253aefba7b18a4fa0937cf1dc987b)                                                                                                                                         | 812 realistic, long-horizon tasks across self-hosted Web applications          | Multiple services, authenticated state, data reset, and evaluator setup must be kept identical             | **Phase 2 after an environment lock is automated**              |
| VisualWebArena         | [`89f5af2`](https://github.com/web-arena-x/visualwebarena/tree/89f5af29305c3d1e9f97ce4421462060a70c9a03)                                                                                                                                   | 910 visual tasks: 234 classifieds, 210 Reddit-like, and 466 shopping tasks     | Requires the WebArena-style services plus a controlled visual observation and vision-capable solver        | **Visual supplement after Phase 2, not a semantic replacement** |

The task counts above are source counts at the linked commits. BrowserGym is
Apache-2.0, MiniWoB++ is MIT, WebArena is Apache-2.0, and VisualWebArena is
MIT. Only sources used by the current executable protocol are included in
[`benchmark.lock.acl`](benchmark.lock.acl); deferred research snapshots do not
change the formal MiniWoB result's lock digest.

### Why MiniWoB++ is first

The first comparison needs to separate candidate behavior from model and
environment variance. MiniWoB++ provides deterministic seeds, bounded tasks,
official reward logic, and enough interaction diversity to expose execution
differences without bringing up an application fleet. It is therefore the
best fit for the first execution-layer baseline.

WebArena is more representative of real work, but an uncontrolled deployment
would mix product behavior with service health, login state, fixture drift,
and evaluator configuration. VisualWebArena adds an important visual axis,
but it cannot replace a semantic benchmark and would also make the solver a
model comparison. Both remain valuable after their environment and solver
contracts can be locked independently.

## Locked comparison protocol

The protocol source of truth is
[`benchmark.lock.acl`](benchmark.lock.acl). The formal run used:

- A3S Test `1.0.0` with protocol revision 15 and
  `--browser-driver standalone`.
- `agent-browser` `0.26.0` directly.
- The same underlying `agent-browser` `0.26.0` browser integration for both
  candidates.
- A `332 x 214` viewport, matching BrowserGym MiniWoB defaults.
- One shared deterministic solver implemented over two thin adapters.
- Nine fixed task/seed pairs, each repeated three times.
- Candidate order reversed on even repetitions to reduce order bias.
- Official MiniWoB termination and `WOB_RAW_REWARD_GLOBAL > 0` scoring.
- Execution latency defined as setup + observation + task actions. Scoring and
  owned cleanup are measured separately.

The server serves the official pinned MiniWoB++ files. Its injected bootstrap
sets the seed and episode timeout, mirrors `core.endEpisode` reward state into
an `aria-hidden` result element, and then calls the original MiniWoB function.
It does not replace a task or its reward rule with a custom page.

### Main task matrix

| Task                 |   Seed | Dimension           | Mechanism exercised                            |
| -------------------- | -----: | ------------------- | ---------------------------------------------- |
| `click-button`       |   1337 | semantic click      | Exact accessible button target                 |
| `enter-text`         |   7331 | form fill           | Text entry and submit                          |
| `choose-list`        | 424242 | single select       | Native selection and submit                    |
| `click-checkboxes`   |     17 | multi-target state  | Repeated fresh observations and checkbox state |
| `click-scroll-list`  |     23 | multi-select scroll | Multiple values in a scrollable select         |
| `grid-coordinate`    |     42 | geometry targeting  | SVG coordinate target                          |
| `enter-text-dynamic` |    101 | dynamic layout form | Form interaction after dynamic layout          |
| `click-menu-2`       |   1337 | dynamic disclosure  | Nested jQuery menu disclosure                  |
| `click-dialog-2`     |    202 | dialog targeting    | Dialog action target                           |

Three safety/capability probes are recorded separately from the main success
and latency population:

- `stale-ref`, repeated three times, observes twice and then attempts the
  first observation's ref.
- `drag-box`, once per candidate, exercises drag and bounded cleanup.
- `wheel-scroll`, once per candidate, exercises mouse wheel and bounded
  cleanup.

Drag and wheel are isolated because the common standalone driver can block
until its command deadline. Including them three times in the main latency
population would measure timeout policy more than useful execution latency.

## Reproduce the benchmark

Run these commands from the A3S Test repository root.

Prerequisites:

- Rust 1.85 or newer.
- Node.js with `Map.groupBy` support; Node.js 22 or newer is recommended.
- `agent-browser 0.26.0` and its installed Chrome for Testing runtime.

Build and verify the candidates:

```bash
cargo build -p a3s-test-cli --bin a3s-test --locked
agent-browser --version
./target/debug/a3s-test --version
```

Fetch the exact MiniWoB++ source without replacing an existing mismatched
checkout:

```bash
node benchmarks/ui/runners/setup-miniwob.mjs
```

Run the complete protocol:

```bash
node benchmarks/ui/runners/run.mjs
```

Useful bounded variants are available for runner development:

```bash
node benchmarks/ui/runners/run.mjs \
  --task click-button \
  --candidate a3s-test \
  --repetitions 1 \
  --no-probes \
  --output-root /tmp/a3s-ui-smoke
```

`--miniwob-root`, `--a3s-test`, and `--agent-browser` can point to explicit
fixture or executable paths. A full run writes the raw record after every
sample so an interrupted run still retains completed evidence.

## Rebuild a summary without rerunning browsers

The lossless raw JSON is authoritative. Rebuild its JSON and Markdown
projections with:

```bash
node benchmarks/ui/runners/summarize.mjs \
  --raw benchmarks/ui/results/raw/20260821T171716Z.json
```

To audit the checked-in projections without overwriting them:

```bash
summary_tmp=$(mktemp -d)
node benchmarks/ui/runners/summarize.mjs \
  --raw benchmarks/ui/results/raw/20260821T171716Z.json \
  --output-root "$summary_tmp"
diff -u \
  benchmarks/ui/results/summary/20260821T171716Z.json \
  "$summary_tmp/summary/20260821T171716Z.json"
diff -u \
  benchmarks/ui/results/summary/20260821T171716Z.md \
  "$summary_tmp/summary/20260821T171716Z.md"
```

The generated timestamp comes from `raw.completed_at`, so this audit is
deterministic.

## Metric definitions

- **Success rate:** runs with official MiniWoB termination and positive raw
  reward divided by all main runs. There is no retry inside a run.
- **Execution latency:** sum of candidate setup, observation, and task-action
  command durations. Score and cleanup durations are excluded.
- **Paired common-success latency:** matches task, seed, and repetition, then
  includes only pairs where both candidates passed. Per-pair A3S Test overhead
  is `(a3s-test - agent-browser) / agent-browser`.
- **p50 / p95:** nearest-rank percentiles over the stated population. **Mean**
  is the arithmetic mean.
- **Flake rate:** task/seed groups whose three repetitions contain different
  pass outcomes, divided by all task/seed groups.
- **Observation and action bytes:** exact command stdout or request payload
  bytes recorded by the runner.
- **Built-in artifacts:** files persisted by the candidate itself, not the
  benchmark's raw JSON.
- **Cleanup success:** the candidate's exact owned session was closed without
  a final cleanup error.
- **Failure attribution:** `product` for candidate behavior after successful
  setup, `benchmark_spec` for solver or fixture admission errors, and
  `infrastructure` for setup failures.

## Formal result

The formal development run is
[`20260821T171716Z`](results/raw/20260821T171716Z.json). Its lock digest is
`sha256:19d4374a4ebdcb23b0f1db4c7c57f5f4c4d320b008fe984bd12c36dc9b4fdcaf`.
The generated projections are available as
[`JSON`](results/summary/20260821T171716Z.json) and
[`Markdown`](results/summary/20260821T171716Z.md).

### Main results

| Candidate     |  Passed | Success | p50 execution | p95 execution | Flake rate | Cleanup | Mean built-in artifacts |
| ------------- | ------: | ------: | ------------: | ------------: | ---------: | ------: | ----------------------: |
| A3S Test      | 27 / 27 |    100% |  7,488.491 ms | 19,313.249 ms |         0% |    100% |               6.119 KiB |
| agent-browser | 24 / 27 |   88.9% |  6,754.729 ms | 17,269.540 ms |         0% |    100% |                     0 B |

The only main-task outcome difference was `click-menu-2`:

- A3S Test passed 3 / 3.
- Direct agent-browser passed 0 / 3.
- After opening the menu and clicking `Playback`, the direct client did not
  expose the required `Prev` submenu item. The deterministic solver therefore
  classified all three as candidate product behavior failures, not benchmark
  or infrastructure failures.

### Paired common-success latency

The three failed `click-menu-2` pairs are excluded from latency overhead
because A3S Test completed a longer successful path while the direct client
stopped early.

| Measure                 |               Median |                 Mean |                    p95 |
| ----------------------- | -------------------: | -------------------: | ---------------------: |
| A3S Test execution      |         7,479.560 ms |         8,860.784 ms |          18,896.995 ms |
| agent-browser execution |         6,743.979 ms |         8,022.880 ms |          17,269.540 ms |
| A3S Test overhead       | +715.492 ms (+10.6%) | +837.904 ms (+10.6%) | +1,634.273 ms (+11.7%) |

All 24 common-success pairs had lower execution time through the direct
client. This is evidence of wrapper/process/output overhead on this host, not
a universal throughput claim. The same run also shows that A3S Test completed
an additional interaction mechanism and persisted session evidence; the
benchmark does not isolate the cost of each A3S Test responsibility.

### Safety, capability, and evidence probes

| Probe                    | A3S Test                                                         | agent-browser                                              | Interpretation                                                                                |
| ------------------------ | ---------------------------------------------------------------- | ---------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| Stale ref, 3 repetitions | Rejected 3 / 3; page changed 0 / 3                               | Rejected 0 / 3; page changed 3 / 3                         | A3S Test binds refs to an observation revision; the direct client accepted the old ref        |
| Drag box, 1 run          | Timed out; final cleanup succeeded                               | Timed out; first close timed out, fallback close succeeded | Shared driver drag path needs repair; neither candidate passed                                |
| Wheel scroll, 1 run      | Timed out; final cleanup succeeded                               | Timed out; final cleanup succeeded                         | Shared driver wheel path needs repair; neither candidate passed                               |
| Built-in evidence        | `session.json`, `events.jsonl`, `report.json` for every main run | No candidate-persisted files                               | The direct client was measured as a raw execution layer, not given an external report wrapper |

The drag and wheel probes have one sample per candidate. Their percentages in
the generated summary describe this run only and are not reliability rates.

## Findings and next work

### A3S Test

1. Preserve revision-bound refs, typed actions, owned cleanup, and the current
   evidence report. They produced the clearest safety and audit differences.
2. Reduce per-turn CLI wrapping and serialization overhead. The paired result
   is consistently about 10.6% on this host.
3. Offer a more compact observation/output mode. Mean observation stdout was
   2.488 KiB for A3S Test versus 1.243 KiB for the direct client.

### Shared standalone driver and direct client

1. Fix drag and mouse-wheel commands that block until the driver deadline.
2. Repair nested jQuery menu interaction so clicking `Playback` exposes the
   `Prev` submenu target consistently.
3. Add observation-bound ref identity to direct agent-browser usage if stale
   refs are expected to fail closed.
4. Consider an opt-in structured evidence record for direct use. This should
   remain separate from the raw execution comparison.

### Benchmark expansion

1. Rerun more MiniWoB tasks and seeds after drag, wheel, and menu fixes. Keep
   common-success paired analysis for latency.
2. Add WebArena only after service images, data snapshots, accounts, reset
   logic, and evaluator revisions have one environment lock and health gate.
3. Add VisualWebArena as a separate visual-reasoning track with identical
   screenshot budgets and a locked vision-capable solver. Do not merge its
   score with the semantic execution-layer result.

## Limitations

- This is one development-machine run on an Apple M2 Pro with three
  repetitions per main task. It does not establish general performance.
- The preflight record includes the repository commit and dirty worktree
  status. Treat this artifact as reproducible development evidence, not a
  release or marketing benchmark.
- Both candidates ultimately use agent-browser `0.26.0`; this compares the
  A3S Test control/evidence boundary with direct client use, not independent
  browser engines.
- The deterministic solver removes model variance, so the result says nothing
  about planning quality, token cost, or visual reasoning.
- Nine main tasks cover nine interaction mechanisms, not the full set of 125
  registered MiniWoB tasks.
- Fixed seeds improve reproducibility but do not measure task-distribution
  generalization.
- WebArena and VisualWebArena have not been executed in this result.
