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
      timeout, Unix process groups, suspended-before-assignment Windows Job
      Objects, per-boundary Unix host-death watchdogs, and cancellation-safe
      direct-child reaping
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
- [x] Pre-dispatch recovery metadata and failed-start runtime retention when
      exact browser cleanup must be retried
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
      cancellation, protocol failure, Drop, and emergency interrupt paths,
      using Unix process groups and suspended-before-assignment Windows
      kill-on-close Job Objects, plus Unix host-death watchdogs, bounded
      descendant wait, and direct-child reaping
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

## M6: Embedded Web Test Kit

The Test Kit is a development-only frontend SDK embedded by the application
under test. It enriches, but never replaces, the browser accessibility
snapshot. Pages without the SDK retain the existing Web behavior.

- [x] Publish a framework-neutral `@a3s-lab/testkit` runtime and a React 18+
      adapter from this repository
- [x] Expose the versioned `a3s.test.page-context/1` bridge with page-runtime
      capability probing, bounded snapshots, scoped inspection, revision
      waits, and explicit teardown
- [x] Automatically describe page identity, route, readiness, viewport,
      document geometry, scroll position, language, theme, and declared
      application facts
- [x] Discover visible and interactive DOM elements across the light DOM and
      open Shadow DOM, including accessibility identity, form state, text
      summaries, stable locator candidates, viewport/document/normalized
      geometry, visible ratio, fixed/sticky state, scroll containers, and
      occlusion
- [x] Add `A3STestBoundary` for explicit component identity, source hints,
      readiness, facts, and multi-root ownership without requiring every DOM
      element to be manually annotated
- [x] Provide bounded `summary`, `scoped`, `diff`, and `forensic` detail
      profiles with pagination and hard node/string/encoded-byte limits
- [x] Redact passwords, tokens, cookies, hidden fields, configured selectors,
      and undeclared framework props/state before data crosses the bridge
- [x] Keep the headless runtime idle when the page is unchanged; use observers
      and revision notifications instead of polling
- [x] Isolate optional overlay styles and events from host-page CSS and avoid
      changing application behavior when review mode is disabled
- [ ] Cover SSR/hydration, route changes, portals, transforms, zoom,
      fixed/sticky content, nested scroll containers, open Shadow DOM,
      virtualized lists, dialogs, and teardown in unit and real-browser tests

## M7: Page-context observation and targeting

- [x] Add typed page-context models to `a3s-test-core`; do not expose bridge
      payloads as unbounded arbitrary JSON
- [x] Make A3S Browser capability discovery report Page Context Bridge support
      independently of browser executable and action protocol revisions
- [x] Capture the browser accessibility snapshot and Test Kit context inside
      one browser-side task or fail closed when an atomic capture cannot be
      guaranteed
- [x] Return Test Kit presence, protocol revision, page revision, component
      index, bounded elements, locator candidates, and truncation cursors from
      `agent observe` and `test_observe`
- [x] Add scoped `agent inspect` and `test_inspect` operations that retrieve
      detail for one current context ref, component, or region
- [x] Bind `@cN` context refs to the latest A3S observation and page revision;
      navigation, revision changes, failed actions, and state-changing actions
      must expire them before input dispatch
- [x] Resolve context refs in the browser adapter without exposing arbitrary
      JavaScript evaluation as a public A3S Test action
- [x] Prefer role, label, test ID, and placeholder locators when a stable
      semantic locator exists; geometry remains evidence and a bounded
      fallback rather than the default targeting mechanism
- [x] Preserve current observation output and action behavior when the page
      does not embed a compatible Test Kit
- [x] Update the CLI schema, MCP tools, Coding Agent Skill, reports, and
      compatibility tests for the page-context contract

## M8: Human review overlay

- [x] Add an optional `A3SReviewOverlay` that supports element click, selected
      text, explicit multi-select, rectangular area selection, and freehand
      drawing while keeping normal application interaction blocking explicit
- [x] Let reviewers create, edit, delete, hide, and reopen local draft findings
      with instruction, optional success criteria, severity, and intent
- [x] Support single `Send and auto-fix`, `Send selected (N)`, and `Send all`
      actions; manual confirmation is the default and auto-send is a visible,
      non-persistent overlay opt-in
- [x] Enrich every submitted finding with a fresh context revision, component
      and source hints, semantic locator candidates, geometry, nearby context,
      and bounded page context
- [x] Display per-finding queue, claim, edit, verification, clarification,
      failure, review-ready, resolved, dismissed, cancelled, and reopened
      states in real time
- [ ] Complete bidirectional human/agent replies; agent clarification messages
      are projected and rendered today, but page-authored replies do not yet
      cross into the authoritative ledger
- [x] Treat DOM text and application-provided facts strictly as untrusted
      evidence; they must never become hidden agent instructions
- [ ] Provide keyboard and screen-reader-complete review workflows and ensure
      the overlay never appears in production unless explicitly enabled

## M9: Repair queue and coding-agent handoff

- [ ] Define typed `RepairBatch` and `RepairAttempt` history; typed
      `RepairFinding`, `RepairVerification`, and append-only repair event
      contracts already exist
- [x] Store repair state and evidence under the owning
      `.a3s-test/agent-sessions/<session>/` ledger rather than creating a
      second session or report system
- [x] Add single and batch submission, bounded queueing, cancellation, explicit
      claim leases, idempotent terminal transitions, reconnect replay, and
      crash recovery
- [x] Add `test_repair_watch`, `test_repair_claim`, `test_repair_progress`,
      `test_repair_reply`, `test_repair_complete`, `test_repair_fail`, and
      `test_repair_cancel` MCP tools plus equivalent machine-readable CLI
      operations
- [x] Keep the current authorized coding agent as the planner and code editor;
      A3S Test must not silently start a second model or treat a page request
      as authorization to edit, commit, push, publish, deploy, install
      dependencies, or run arbitrary commands
- [ ] Process one workspace mutation at a time by default; after each hot
      reload, observe again and re-resolve every remaining finding instead of
      reusing stale refs
- [ ] Detect overlapping targets, source files, and contradictory requests and
      move the affected findings to `needs_input` rather than guessing order
- [x] Record exactly which changed files and checks the coding agent reports in
      each stored verification; preserving unrelated dirty-worktree changes
      remains a coding-agent safety requirement
- [x] Never automatically hand an ambiguously dispatched repair attempt to a
      second worker; lease recovery must distinguish unclaimed, pre-edit, and
      possibly-mutated work

## M10: Repair verification and regression promotion

- [ ] Capture an A3S Test-owned before evidence bundle and error baseline before
      a repair is claimable; context revision is captured today and after-state
      verification already rejects non-new or non-ready revisions
- [x] Re-observe the target, accept an explicit browser success-criteria result,
      detect new console/page errors from a supplied baseline, and attach
      focused project-check results before a repair can become `review_ready`
- [ ] Require human acceptance by default; allow session-scoped automatic
      resolution only when all declared verification gates pass
- [ ] Let a human reject or reopen a repair while retaining every attempt,
      reply, evidence digest, and verification result
- [ ] Continue independent findings after an isolated failure while preserving
      deterministic batch order and a per-item result
- [ ] Prove and persist the smallest regression path; current verification can
      generate a syntax-validated ACL candidate from one stable locator and an
      explicit text criterion, but does not execute that candidate
- [ ] Validate end-to-end flows for single repair, ordered batch repair,
      clarification, cancellation, agent disconnect, hot-reload ref expiry,
      verification failure, restart recovery, and promotion to ACL
- [x] Document integration for React/Vite/Next.js, security and redaction,
      coding-agent watch mode, CI behavior, and migration/compatibility rules
