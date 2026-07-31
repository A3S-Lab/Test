# Roadmap

## M0: Runtime foundation

- [x] Independent Rust workspace
- [x] Bounded ACL suite admission
- [x] Typed actions, targets, waits, and assertions
- [x] Surface driver and session contracts
- [x] Cancellation-safe sequential runner
- [x] Structured JSON report and stable exit codes
- [x] A3S Browser and standalone agent-browser command layouts
- [x] Private runtime, unique sessions, graceful close, PID-validated emergency
      cleanup, idle timeout, and process groups
- [x] Single and repeated SIGINT integration tests
- [x] macOS CI for formatting, tests, and warning-free Clippy

## M1: Web depth

- [ ] Browser capability discovery and version admission
- [ ] Tabs, frames, dialogs, uploads, downloads, network mocks, and HAR
- [ ] Trace, video, accessibility snapshot, and console evidence
- [ ] Retry policy constrained to infrastructure failures
- [ ] Parallel scenarios with explicit resource limits
- [ ] Local fixture server for hermetic Web E2E tests

## M2: LLM agentic testing

- [ ] Typed LLM planner interface
- [ ] Schema-constrained observe-decide-act loop
- [ ] Goal, success criteria, turn, token, cost, and time budgets
- [ ] Safety policy and capability validation before every proposed action
- [ ] Reproducible decision trace with secret-safe provenance
- [ ] Coding-agent Skill and MCP projection
- [ ] No keyword-based intent router

## M3: GUI through A3S CUA

- [ ] Accessibility-tree observation adapter
- [ ] Window and application lifecycle
- [ ] Semantic pointer and keyboard actions
- [ ] Visual fallback with screenshot-region evidence
- [ ] macOS, Windows, and Linux execution profiles

## M4: TUI

- [ ] PTY lifecycle and process-group supervision
- [ ] Semantic terminal viewport
- [ ] Key chords, paste, resize, and alternate-screen support
- [ ] Text/regex waits and terminal recording
- [ ] Ctrl+C, EOF, crash, and terminal restoration tests

## M5: Distributed execution

- [ ] Hermetic runner image and capability inventory
- [ ] Remote worker protocol
- [ ] Artifact retention and report indexing
- [ ] Sharding, quarantine, flake accounting, and historical comparison
- [ ] GUI worker pools with explicit host permissions
