# AGENTS.md

## Repository

This repository owns A3S Test: typed cross-surface test specifications,
cancellation-safe orchestration, surface-driver contracts, evidence, reports,
and the `a3s-test` CLI.

## Boundaries

- Keep the core independent of browser, GUI, TUI, CLI, and LLM providers.
- Inject surface drivers and future LLM providers as typed objects.
- Do not select backends with raw strings in public library APIs.
- Do not implement agentic behavior with keyword matching. Use a real,
  schema-constrained LLM adapter and validate its typed proposals.
- Keep A3S Browser, A3S CUA, and PTY implementation details in their adapters.
- Preserve stable JSON result fields, error codes, and process exit codes.

## Process safety

- Every launched test program belongs to an owned process group or equivalent
  Windows Job/process tree.
- Cancellation and timeout must kill and reap the complete owned tree.
- Always bound graceful cleanup and provide an emergency path.
- Browser runs use an isolated namespace and non-zero idle timeout.
- Never close or kill unrelated developer browser sessions.
- Tests that launch processes must verify no children or sockets remain.

## Engineering

- Use Tokio for I/O and do not block inside async contexts.
- Public traits and shared types should be `Send + Sync` where applicable.
- Return contextual typed errors; avoid production panics.
- Keep code and documentation in English.
- Use ACL for test manifests and product configuration.

## Validation

Run from this repository:

```bash
cargo fmt --all -- --check
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
```
