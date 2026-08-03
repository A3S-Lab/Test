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
      cleanup including fail-closed Windows command-line marker admission, idle
      timeout, and process groups
- [x] Canonical Web runtime identity binding with per-command and cleanup
      revalidation, plus link-safe namespace and PID-sidecar admission
- [x] Single and repeated SIGINT integration tests
- [x] macOS, Linux, and Windows CI for formatting, tests, and warning-free
      Clippy

## M1: Web depth

- [x] Browser capability discovery and version admission
- [x] Tabs, frames, dialogs, uploads, downloads, network mocks, and HAR
- [x] Trace, video, accessibility snapshot, console, and page-error evidence
- [x] Canonical Web evidence-root containment, link/reparse rejection, and
      post-command regular-file validation for all browser-written artifacts
- [x] Retry policy constrained to pre-dispatch infrastructure failures
- [x] Parallel scenarios with explicit resource limits and stable report order
- [x] Local fixture server with dynamic ports, request sentinels, owned
      lifecycle, and a real standalone-agent-browser CI path

## M2: Agentic testing

- [x] Persistent external-planner CLI sessions for coding agents
- [x] Workspace-local event log, report, and evidence roots
- [x] Observation-bound refs and origin gates for explicit URL actions
- [x] Observation-time detection of detached pages and unapproved origins
- [x] Browser-level cross-domain containment for links, redirects, scripts,
      images, fetches, and other requests through a typed domain policy
- [x] Fail-closed migration for sessions created before browser domain policy
      persistence, with `finish` and `abort` cleanup retained
- [ ] Browser-level exact scheme-and-port containment; admitted upstream
      protocols expose hostname allowlists, while A3S Test keeps exact-origin
      validation on explicit actions and observations
- [x] Compact pointer, form, keyboard, wheel, viewport, and evidence turns plus
      full typed actions
- [x] Typed LLM provider and planner interface
- [x] Schema-constrained observe-decide-act loop
- [x] Goal, success criteria, turn, token, cost, context, and time budgets
- [x] Safety policy and capability validation before every proposed action
- [x] Secret-safe provenance redaction; provider/model/prompt version, request
      ID, decision digest, usage, and model latency are already recorded
- [x] Coding Agent Skill for interactive sessions and deterministic ACL
- [x] Surface-neutral session application layer and MCP stdio projection for
      start, observe, act, finish, abort, and schema discovery
- [x] Exact MCP lifecycle/version negotiation, registered-surface inventory,
      cancellation-safe session admission, and concurrent EOF cleanup
- [x] Cancellation-safe terminal cleanup tasks with `cleanup_in_progress`
      admission and a retryable cleanup-only state that preserves the exact
      driver session for `finish` or `abort`
- [ ] Direct embedded-LLM CLI host; external coding agents already drive the
      session CLI without a nested model
- [x] No keyword-based intent router or production heuristic fallback

## M3: GUI through A3S CUA

- [x] Exact CUA compatibility lock and adapter-boundary ADR
- [x] Typed MCP stdio transport, protocol/schema/capability admission, and
      fake-transport contract tests
- [x] Accessibility-tree observation adapter with observation-bound opaque refs
- [x] Window and application lifecycle with launch/attach ownership checks
- [x] Cancellation-safe opening cleanup, including cancellation after an
      application launch is dispatched but before ownership is returned
- [x] Semantic pointer and keyboard actions with stale-ref rejection
- [x] Window-vision fallback with SHA-256-bound screenshot evidence and
      observation-scoped pixel targets
- [x] Locked three-platform/two-endpoint certification matrix with
      contract-tested macOS profiles and fail-closed unsupported entries
- [x] Real-host GUI certification command plus lifecycle, PID-reuse, Drop, and
      repeated open/close contract tests
- [x] Retryable identity-safe app termination and CUA session cleanup without
      discarding the ownership handle after a transient tool failure
- [x] Per-observation and per-action application/window binding revalidation,
      with fail-closed zero-input behavior and stale-generation invalidation
- [x] Canonical GUI evidence-root containment with link/reparse rejection and
      post-capture plus pre-input grounding-file validation
- [x] Complete CUA proxy-tree supervision across graceful close, timeout,
      protocol failure, Drop, and emergency interrupt paths, using Unix process
      groups and Windows kill-on-close Job Objects
- [ ] Record a real macOS host certification in release automation
- [ ] Windows and Linux execution, pending reviewed backends in the locked CUA
      stack

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
