# UI benchmark results

`raw/` contains one lossless JSON record per benchmark invocation. `summary/`
contains generated JSON and Markdown projections of the same run.

See the [benchmark protocol and interpretation](../README.md) for candidate
versions, metric definitions, reproduction commands, and scope limits. Use
`runners/summarize.mjs` from the benchmark root to regenerate a projection
without rerunning browser sessions.

Results are machine-specific evidence, not universal product claims. Compare
runs only when `benchmark.lock.acl`, candidate versions, viewport, host facts,
task seeds, and repetition counts match.
