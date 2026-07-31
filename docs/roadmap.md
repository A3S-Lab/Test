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

- [x] Browser capability discovery and version admission
- [x] Tabs, frames, dialogs, uploads, downloads, network mocks, and HAR
- [x] Trace, video, accessibility snapshot, console, and page-error evidence
- [x] Retry policy constrained to pre-dispatch infrastructure failures
- [x] Parallel scenarios with explicit resource limits and stable report order
- [ ] Local fixture server for hermetic Web E2E tests

## M2: Agentic testing

- [x] Persistent external-planner CLI sessions for coding agents
- [x] Workspace-local event log, report, and evidence roots
- [x] Observation-bound refs and origin gates for explicit URL actions
- [x] Observation-time detection of detached pages and unapproved origins
- [ ] Browser-level origin enforcement for link-, script-, and
      redirect-triggered navigation
- [x] Compact pointer, form, keyboard, wheel, viewport, and evidence turns plus
      full typed actions
- [x] Typed LLM provider and planner interface
- [x] Schema-constrained observe-decide-act loop
- [x] Goal, success criteria, turn, token, cost, context, and time budgets
- [x] Safety policy and capability validation before every proposed action
- [ ] Secret-safe provenance redaction; provider/model/prompt version, request
      ID, decision digest, usage, and model latency are already recorded
- [x] Coding Agent Skill for interactive sessions and deterministic ACL
- [ ] MCP projection over the same session application layer
- [ ] Direct embedded-LLM CLI host; external coding agents already drive the
      session CLI without a nested model
- [x] No keyword-based intent router or production heuristic fallback

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
