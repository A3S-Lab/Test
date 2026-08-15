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
- [x] A3S Browser exact-origin containment for links, redirects, scripts,
      images, fetches, workers, popups, WebSockets, and direct reads, with
      explicit network-only domain exceptions
- [x] Typed persistence of `exact_origin_v1` and `hostname_v1` containment
      modes, with fail-closed migration and retained `finish`/`abort` cleanup
- [ ] Standalone exact-origin containment after its protocol exposes scheme,
      host, and effective-port authorization; audited 0.34.0 still exposes
      hostname-only `allowedDomains`, while CI remains pinned to certified
      0.26.0
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
- [x] Direct embedded-LLM CLI host with ACL admission, a deployment-supplied
      HTTP planner, local action policy, deterministic verification, complete
      redacted reports, and exact owned Web cleanup; external coding agents
      still drive the session CLI without a nested model
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
- [x] Record real macOS semantic and window-vision host certification in
      release automation with exact source and policy identity, owned-cleanup
      proof, detached SHA-256, and GitHub OIDC/Sigstore provenance
- [ ] Windows and Linux execution, pending reviewed backends in the locked CUA
      stack

## M4: TUI

- [x] PTY lifecycle and process-group supervision
- [x] Semantic terminal viewport
- [x] Key chords, paste, resize, and alternate-screen support
- [x] Text/regex waits and terminal recording
- [x] Ctrl+C, EOF, crash, and terminal restoration tests

## M5: Distributed execution

- [x] Hermetic Linux/amd64 runner image and strict, self-reported Web/TUI
      capability inventory, with digest-pinned inputs, non-root restricted
      smoke tests, and externally bound release image identity
- [x] Authenticated remote worker protocol with exact instance, image, and
      capability-inventory binding; bounded digest-verified inline inputs;
      immutable idempotent dispatch; absolute deadlines and renewable leases;
      a persistent sequential queue; cancellation and restart recovery; and a
      loopback-only HTTP/CLI reference host for deployment-owned Web and TUI
      profiles
- [x] Separate authenticated artifact protocol with bounded report queries,
      digest-bound paginated evidence access, chunked reads, deployment-owned
      two-tier retention, idle-time garbage collection, durable pruning
      recovery, and corruption/link/reparse fail-closed checks
- [x] Deterministic surface-aware sharding, exact scenario dispatch, bounded
      concurrent coordination, accountable quarantine, flake accounting,
      cross-revision historical comparison, digest-bound report verification,
      and atomic retained analyses
- [x] Exclusive GUI worker pools with deployment-owned ACL host profiles,
      read-only CUA readiness probes, explicit permission attribution,
      configuration/policy/inventory digests, exact permission binding through
      planning and dispatch, per-session grant revalidation, and authorization
      environment scrubbing

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
- [x] Cover SSR/hydration, route changes, portals, transforms, fixed/sticky
      content, nested scroll containers, open Shadow DOM, virtualized lists,
      dialogs, and teardown in unit and real-browser tests
- [x] Add an explicit browser-zoom geometry regression covering layout and
      visual viewports, DPR, CSS-pixel element rectangles, visible ratio, and
      normalized coordinates in the real Test Kit browser suite

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
- [x] Add a non-mutating Layout Mode for typed component placement and section
      rearrangement, with page/wireframe canvases, viewport CSS-pixel regions,
      purpose metadata, pointer drawing, and keyboard source selection
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
- [x] Complete bidirectional human/agent replies through the page action queue,
      authoritative append-only ledger, and projected overlay thread
- [x] Treat DOM text and application-provided facts strictly as untrusted
      evidence; they must never become hidden agent instructions
- [x] Provide keyboard element marking, Escape focus restoration, and explicit
      fail-closed production enablement for both runtime and overlay
- [x] Expose a named non-modal review dialog, finding-specific control names,
      focused live announcements, visible focus indicators, and durable focus
      restoration, with React and real-Chromium accessibility-tree regressions
- [ ] Complete and independently audit every review workflow with a screen
      reader

## M9: Repair queue and coding-agent handoff

- [x] Define typed `RepairBatch`, per-item batch results, `RepairAttempt`
      history, `RepairFinding`, `RepairVerification`, replies, and append-only
      repair event contracts
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
- [x] Process one workspace mutation at a time by default; after each hot
      reload, observe again and re-resolve every remaining finding instead of
      reusing stale refs
- [x] Detect overlapping node/region targets and shared source hints and move
      the affected findings to `needs_input` rather than guessing order
- [x] Add a typed, non-keyword mechanism for declaring semantically
      contradictory requests across otherwise disjoint targets
- [x] Preserve typed `placement` and `rearrange` intents through the page
      bridge, Web driver, queue, append-only ledger, MCP/CLI projection, and
      ordered batch handoff without converting metadata into hidden prompts
- [x] Record exactly which changed files and checks the coding agent reports in
      each stored verification; preserving unrelated dirty-worktree changes
      remains a coding-agent safety requirement
- [x] Never automatically hand an ambiguously dispatched repair attempt to a
      second worker; lease recovery must distinguish unclaimed, pre-edit, and
      possibly-mutated work

## M10: Repair verification and regression promotion

- [x] Capture an A3S Test-owned, hash-bound before context/screenshot bundle and
      console/page-error baseline before a repair is claimable
- [x] Re-observe the target, accept an explicit browser success-criteria result,
      detect new console/page errors from the owned baseline, and attach
      focused project-check results before a repair can become `review_ready`
- [x] Require human acceptance by default; allow session-scoped automatic
      resolution only when all declared verification gates pass
- [x] Require explicit success evidence for layout repairs and verify that a
      rearranged target overlaps its requested region instead of passing only
      because the original node still exists
- [x] Let a human reject or reopen a repair while retaining every attempt,
      reply, evidence digest, and verification result
- [x] Continue independent findings after an isolated failure while preserving
      deterministic batch order and a per-item result
- [x] Generate and persist the smallest admitted ACL candidate from a stable
      locator and explicit text criterion, then prove its single same-origin Web
      scenario in a fresh browser before `review_ready`
- [x] Validate end-to-end flows for single repair, ordered batch repair,
      clarification, cancellation, agent disconnect, hot-reload ref expiry,
      verification failure, restart recovery, Layout Mode handoff, and
      promotion to ACL
- [x] Document integration for React/Vite/Next.js, security and redaction,
      coding-agent watch mode, CI behavior, and migration/compatibility rules

## M11: Embedded review workflow completion

This milestone closes the remaining reviewer-facing workflow gaps around the
existing page-context, annotation, Layout Mode, and repair protocols. The
capability audit baseline is 2026-08-13. Completion requires direct unit and
real-browser evidence for every row below; visual similarity to another tool
is not a requirement.

| Capability family | Existing foundation | Remaining completion work |
| --- | --- | --- |
| Autonomous page understanding | Bounded semantic context, component/source boundaries, locator candidates, accessibility state, nearby nodes, and viewport/document/normalized geometry | None; keep the browser accessibility tree and Test Kit context as independent evidence |
| Human capture | Element, text, multi-element, rectangular area, freehand drawing, draft edit/delete, and single/selected/all submission | Restore page-local drafts safely, add global marker controls, clear-all, and direct spatial draft editing |
| Layout authoring | Typed non-mutating placement and rearrangement intents, page/wireframe canvases, purpose, pointer regions, keyboard source selection, and explicit target geometry | Add a searchable 65+ component catalog and adjustable wireframe page fade while preserving free-form component types |
| Review controls | Pause/resume, manual theme, visible auto-send opt-in, focus restoration, named controls, and live announcements | Add documented global shortcuts with editable-target guards, persisted presentation preferences, explicit interaction blocking, movable docking, and hide-until-tab-restart |
| Integration API | Typed bridge events, structured Markdown/JSON export, repair submission callback, and custom repair endpoint | Add bounded draft add/update/delete/clear/copy callbacks and a host-provided clipboard adapter |
| Repair transport and proof | Typed MCP/CLI queue, batches, leases, replies, lifecycle replay, owned screenshots, browser errors, verification, and ACL promotion | None; continue using the owning A3S Test agent session as the sole authoritative ledger |

The completion work intentionally preserves these product boundaries:

- Layout Mode emits typed desired geometry and never rearranges or styles the
  application DOM itself.
- Framework internals are not traversed automatically. Applications declare
  stable component and source ownership with `A3STestBoundary`; undeclared
  props and state remain private.
- The overlay does not own screenshots, a model loop, a second repair
  database, or a generic remote-control webhook. A3S Test owns evidence,
  sessions, policy, MCP/CLI transport, and repair history.
- Structured repair export stays deterministic and bounded. Agents request
  `summary`, `scoped`, `diff`, or `forensic` page context explicitly instead
  of changing the wire contract through a presentation setting.

- [x] Persist validated drafts per page and route with bounded retention,
      semantic locator anchors, reload/SPA-route restoration, and fail-closed
      handling for targets that cannot be resolved uniquely
- [x] Add one global keyboard command layer for overlay toggle, Escape,
      Layout Mode, pause, marker visibility, copy, and clear, and ignore
      commands originating in inputs, textareas, selects, contenteditable
      regions, or ARIA text-entry controls
- [x] Expose the same commands through `aria-keyshortcuts` and an in-overlay
      keyboard reference, with direct unit and real-browser accessibility-tree
      evidence while keeping the independent screen-reader audit open
- [x] Add global marker visibility, clear-all, direct marker-to-editor access,
      and deletion from the active editor without making the full marker
      rectangle intercept normal page input
- [x] Persist bounded presentation preferences for theme, marker color,
      clear-on-copy, interaction blocking, panel dock, wireframe fade, and
      hide-until-tab-restart; keep auto-send and animation pause non-persistent
- [x] Add an independently defined, categorized, searchable catalog of at
      least 65 common Web component types while retaining an explicit
      free-form component field
- [x] Expose typed draft lifecycle and copy callbacks plus a host clipboard
      adapter; isolate callback failures from the overlay and never treat
      callback return values as repair instructions
- [x] Split the review overlay by concern before it exceeds the repository
      file-size limit, retaining Shadow DOM isolation and one source of truth
      for persisted workflow state
- [x] Add React tests for storage corruption and expiry, target rebinding,
      route isolation, shortcuts and editable guards, preference recovery,
      callbacks, catalog filtering, marker editing, clear-on-copy, docking,
      and hide-until-restart
- [x] Extend the real Chromium Test Kit suite to prove restored drafts,
      keyboard-only review controls, host-interaction blocking, searchable
      Layout authoring, and accessible spatial marker editing
- [x] Re-run Test Kit typecheck/tests/build, Rust formatting/tests/clippy, the
      real Chromium Test Kit suite, and the complete repair lifecycle matrix

## M12: Expected Surface Contracts and repair authorization

- [x] Extend A3S ACL with a versioned `surface_contract` document instead of
      introducing a parallel design DSL
- [x] Preserve PRD, design, manual, and official-document provenance with
      SHA-256 digests, review status, and confidence
- [x] Require reviewed 100-confidence provenance before admitting any blocking
      expectation
- [x] Add variants, named states, viewport/theme/language constraints, stable
      element identity, accessibility semantics, state, and parent structure
- [x] Add deterministic matching in test ID, component, role/name, and role
      order, with ambiguity reported instead of guessed
- [x] Keep advisory findings in passed reports and reserve failed outcomes for
      blocking findings; fail closed as inconclusive when bounded context is
      absent or truncated
- [x] Generate stable finding IDs independent of observation revision,
      temporary DOM node IDs, and actual values
- [x] Add runner-owned `verify_contract`, provenance digest verification before
      browser startup, retained failed reports, and action protocol revision 6
- [x] Keep `verify_contract` out of interactive agent and MCP action schemas
- [x] Project bounded reports into a separate Test Kit Quality Store without
      allowing projection failure to change the Runner verdict
- [x] Require human target confirmation and explicit draft save or submission
      before a projected finding enters the Repair Ledger
- [x] Support finding-level dismissal and preserve sibling findings when one
      candidate is drafted, submitted, cancelled, or dismissed

## M13: Source-to-contract generation

- [x] Generate an Expected Surface Contract draft from a PRD while preserving
      quoted source spans, uncertainty, and unresolved product decisions
- [x] Generate an Expected Surface Contract draft from a design image with
      image digest, coordinate space, candidate hierarchy, and uncertainty
- [x] Provide a review workflow that promotes selected draft expectations to
      `reviewed` without claiming that either source was browser-observed
- [x] Merge PRD and design drafts through explicit conflicts and provenance,
      never by silently choosing one source
- [x] Re-run the same admitted contract after repair and correlate findings by
      stable finding ID

## M14: Optional visual grounding providers

- [x] Define a typed provider boundary for a digest-bound screenshot,
      dimensions, query, observation ID, deadline, and cost budget
- [x] Return bounded boxes or points with provider/model identity, confidence,
      and coordinate-space metadata
- [x] Hit-test candidates against current Test Kit nodes and accessibility
      evidence before issuing any image-bound fallback target
- [x] Trigger visual grounding only for explicit requests or when deterministic
      semantic targeting cannot represent the surface
- [x] Keep provider output advisory and observation-scoped; it must not pass a
      blocking contract or become a durable ref by itself
- [x] Add provider conformance fixtures for canvas, image-only, remote-desktop,
      and design-reference cases without bundling model weights

## M15: Versioned provider interoperability

- [x] Assign stable protocol identifiers to contract-generation and
      visual-grounding provider boundaries; advance only the changed visual
      HTTP envelope to version 2
- [x] Derive JSON Schema 2020-12 request and response contracts from the same
      strict Serde types used by local admission
- [x] Expose protocol, authority, safety invariants, and schemas through
      `a3s-test provider schema` without selecting a transport or backend
- [x] Prove deadline, cost, digest, identity, coordinate-space, hierarchy, and
      point/box fields remain discoverable
- [x] Add representative wire fixtures that round-trip and reject unknown
      fields
- [x] Keep contract candidates review-gated and grounding advisory; neither
      provider may determine verdicts, claim browser observation, or authorize
      repair

## M16: Deployment-owned provider transport

- [x] Add typed HTTP adapters for contract generation and visual grounding
      without selecting or bundling a model runtime
- [x] Publish the HTTP request/response envelope schemas through existing
      provider discovery
- [x] Require HTTPS except for explicit loopback HTTP, disable redirects and
      environment proxies, and keep one fixed endpoint per typed provider
- [x] Bound serialized requests and streamed responses and enforce both the
      transport timeout and request wire deadline
- [x] Preserve provider/model identity, digest, observation, cost, and usage
      bindings through the transport while leaving final admission local
- [x] Map HTTP status and typed remote errors with retryability and redact
      configured authorization values from error output and Debug
- [x] Add real TCP conformance fixtures for both provider capabilities,
      endpoint policy, tagged envelopes, request/streamed-response limits,
      protocol mismatch, timeout, and error handling

## M17: Operational source-to-contract workflow

- [x] Add an ACL-configured CLI path from contained PRD/design files through a
      deployment-owned HTTP provider to a versioned candidate-only artifact
- [x] Calculate and verify source digests locally, bind provider/model identity,
      deadline, cost, and limits, and inject authorization only from a named
      environment variable
- [x] Add a separate ACL human-review document with explicit candidate actions,
      conflict selection, and rationale
- [x] Regenerate and admit the canonical Surface Contract locally without
      letting the provider approve expectations or claim browser observation
- [x] Persist generated and reviewed workflow stages with strict versioning,
      full-payload SHA-256, source rehash and admission replay, bounded reads,
      atomic writes, tamper detection, and complete audit data
- [x] Publish reviewed ACL and audit as a recoverable output pair while
      rejecting symlinks, path aliasing, unsafe source paths, and accidental
      overwrites
- [x] Add real HTTP CLI coverage for digest and environment-authorization
      binding plus fail-closed review and output behavior

## M18: Operational visual grounding

- [x] Add explicit `a3s-test agent ground` for the latest persistent Web
      observation without dispatching input
- [x] Admit ACL, authorization, provider identity, query, limits, and cost
      before browser connection
- [x] Capture a bounded PNG between matching Test Kit revisions and revalidate
      the revision after provider inference
- [x] Keep agent observation IDs independent from Test Kit surface revisions
      and require current `@cN` bindings to match the stored observation
- [x] Upgrade the visual-grounding HTTP protocol to version 2 with a
      digest-bound Base64 PNG attachment and logical image path
- [x] Rehash the screenshot at service and HTTP dispatch boundaries, reject
      replacement races, and keep Debug and diagnostics free of image bytes
- [x] Return a current ref only for one unambiguous semantic hit; preserve zero
      or multiple hits as image-bound advisory candidates
- [x] Invalidate the observation on provider, cancellation, binding, or
      revision failure and retain non-verdict, non-repair authority
- [x] Add driver, provider, service, schema, CLI help, and fake browser plus
      real HTTP integration coverage

## M19: Advisory design-quality audit

- [x] Define `a3s.test.design-audit-provider/1` with strict generated request,
      response, and HTTP-envelope schemas and explicit advisory authority
- [x] Bind every request and response to provider/model identity, observation,
      page revision, screenshot digest and dimensions, complete forensic page
      context digest, selected dimensions, deadline, and cost ceiling
- [x] Admit only bounded findings whose typed dimension was requested and
      whose page, current-node, or normalized-region target is locally valid
- [x] Keep design judgment separate from deterministic Surface Contract facts;
      it cannot determine a verdict, approve an expected surface, dispatch an
      action, or authorize repair
- [x] Add deployment-owned HTTP transport with digest-bound PNG attachment,
      fixed endpoint policy, bounded bodies, redacted credentials, and local
      response admission
- [x] Add `a3s-test agent audit` for the latest persistent Web observation,
      including forensic context capture and post-inference revision checks
- [x] Add `a3s.test.design-audit-report/1` projection to a separate bounded
      Test Kit store and visible advisory review candidates
- [x] Require a human to review or retarget each suggestion before it becomes
      a local draft or enters the existing single/batch Repair Ledger
- [x] Add service, wire, HTTP, driver, CLI, runtime, and React integration
      coverage without bundling or selecting a model runtime
