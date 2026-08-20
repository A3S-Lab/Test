# Roadmap

## M0: Runtime foundation

- [x] Independent Rust workspace
- [x] Bounded ACL suite admission
- [x] Typed actions, targets, waits, and assertions
- [x] Action protocol revision 8 control-state assertions for exact value,
      enabled, checked, selected, and exact selected-value sets, with honest
      per-surface capability failures
- [x] Action protocol revision 9 target-bound rendered-text and visible-set
      cardinality assertions with exact observation ownership, zero-count
      evidence, and honest per-surface capability failures
- [x] Action protocol revision 10 bounded ordered rendered-text collection
      assertions with exact order/duplicate evidence, observable empty sets,
      and honest per-surface capability failures
- [x] Action protocol revision 11 deterministic rendered-layout relations
      between two stable targets with atomic geometry evidence and honest
      per-surface capability failures
- [x] Action protocol revision 12 deterministic visual-viewport intersection
      and pointer hit-reachability assertions with atomic browser evidence and
      honest per-surface capability failures
- [x] Action protocol revision 13 deterministic exact and component-scoped
      focus ownership with flat-tree evidence and honest per-surface
      capability failures
- [x] Action protocol revision 14 deterministic disclosure, toggle, read-only,
      required, and validity state with native/ARIA precedence and honest
      per-surface capability failures
- [x] Action protocol revision 15 bounded visual-viewport coverage thresholds
      with independently recomputed geometry and honest per-surface capability
      failures
- [x] Surface driver and session contracts
- [x] Cancellation-safe sequential runner
- [x] Bounded sampled assertion stability with static duration/sample limits,
      final boundary sampling, stable transient-failure reporting, and
      cancellation-safe cleanup
- [x] Runner-owned `hidden` target assertions with stable-locator admission,
      visible counter-evidence, driver-error preservation, and sampled
      reappearance detection without an action-protocol revision change
- [x] Runner-owned `wait hidden` synchronization with immediate completion,
      fixed bounded polling, first/last counter-evidence, static probe limits,
      deadline/cancellation cleanup, and shared agent-host verification policy
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
- [x] Explicit, persisted synthetic browser microphone profile for deterministic
      media-permission tests without host microphone access

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
- [x] Cover warning-free SSR/hydration, route changes, portals, transforms,
      fixed/sticky content, nested scroll containers, open Shadow DOM,
      virtualized lists, dialogs, and teardown in unit and real-browser tests;
      verify built ESM and CommonJS React consumers independently
- [x] Add an explicit browser-zoom geometry regression covering layout and
      visual viewports, DPR, CSS-pixel element rectangles, visible ratio, and
      normalized coordinates in the real Test Kit browser suite
- [x] Resolve role, label, test ID, and placeholder targets across light DOM
      and open Shadow DOM for click, fill, and check actions, including native
      `searchbox` semantics and pointer clicks bound to post-scroll coordinates
- [x] Build the production Rspress site inside the real-browser suite and
      verify single submission, ordered batch submission, focused and complete
      accessibility evidence, empty browser diagnostics, and exact cleanup on
      macOS and Windows CI
- [x] Restore bounded desktop and mobile PNG evidence to the production website
      suite under the pinned Web runtime, validate its signature, viewport
      dimensions, media type, byte ceiling, and exact cleanup, and retain
      independent interactive plus complete semantic evidence

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
- [x] Let element and rectangular-area findings attach a bounded desired-UI
      sketch or screenshot through a responsive design board, and materialize
      browser-inline images as hash-bound Web artifacts before agent handoff
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
- [x] Keep keyboard multi-selection on application controls until explicit
      `Shift+Enter` completion, announce each bounded addition without
      activating the host control, and discard incomplete candidates on
      Escape or visible cancellation with React and real-Chromium regressions
- [x] Expose a named non-modal review dialog, finding-specific control names,
      focused live announcements, visible focus indicators, and durable focus
      restoration, with React and real-Chromium accessibility-tree regressions
- [x] Restore focus without scrolling to the last connected application
      control when tab-scoped hiding removes the review Shadow DOM, with direct
      React and real-Chromium active-element regressions
- [x] Localize the complete review workflow in English and Simplified Chinese,
      follow the page language by default, expose bounded host copy overrides,
      and keep visible text, live announcements, status labels, and accessible
      names on one typed message catalog; observe live page-language changes
      and localize plus search all built-in Layout catalog entries without
      rewriting project-specific free-form component values
- [ ] Complete and independently audit every review workflow with a screen
      reader

  Automated prerequisite evidence now includes an independent `axe-core`
  WCAG A/AA scan in real Chromium across all three themes, review preferences,
  marking and Layout editors, restored drafts, deterministic and advisory
  candidates, clarification replies, human review actions, and terminal repair
  states. It also verifies that scrollable findings remain keyboard reachable.
  A committed 15-workflow manifest, loopback-only shared fixture, strict
  revision-bound audit artifact, digest-bound location-independent verifier,
  and closure gate now make the remaining manual audit reproducible. The
  verifier loads its manifest and Test Kit version from the audited Git commit
  and binds every evidence file by byte length and SHA-256. This does not close
  the item: an independent VoiceOver, NVDA, or equivalent hands-on lifecycle
  audit is still required.

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
| Autonomous page understanding | Bounded semantic context, component/source boundaries, locator candidates, accessibility state, geometry, observed style profiles, box-model and overflow-aware layout graphs, repeated-component clusters, state differences, and timeline-aware motion facts | None; keep the browser accessibility tree, Page Context, and UI understanding as independent evidence |
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
- The overlay does not own observation or verification screenshots, a model
  loop, a second repair database, or a generic remote-control webhook. It may
  attach one bounded reviewer-authored design reference to a finding; A3S Test
  still owns evidence materialization, sessions, policy, MCP/CLI transport,
  and repair history.
- Structured repair export stays deterministic and bounded. Agents request
  `summary`, `scoped`, `diff`, or `forensic` page context explicitly instead
  of changing the wire contract through a presentation setting.

- [x] Persist validated drafts per page and route with bounded retention,
      semantic locator anchors, reload/SPA-route restoration, and fail-closed
      handling for targets that cannot be resolved uniquely
- [x] Add one global keyboard command layer for overlay toggle, Escape,
      Layout Mode, pause, marker visibility, copy, and clear. Editable controls
      retain letter commands and the panel toggle; active marking or an open
      editor receives unmodified Escape first, while an idle editable retains
      Escape without closing the panel
- [x] Expose the same commands through `aria-keyshortcuts` and an in-overlay
      keyboard reference, with direct unit and real-browser accessibility-tree
      evidence while keeping the independent screen-reader audit open
- [x] Add global marker visibility, clear-all, direct marker-to-editor access,
      and deletion from the active editor without making the full marker
      rectangle intercept normal page input
- [x] Persist bounded presentation preferences for theme, marker color,
      clear-on-copy, interaction blocking, panel dock, wireframe fade, and
      hide-until-tab-restart; restore application focus when hiding the UI and
      keep auto-send and animation pause non-persistent
- [x] Replace the secondary tool tray and target-attached editor with one side
      panel whose New feedback, Findings, and Preferences views share the same
      surface; replace that panel while the design board is open, bound content
      to short viewports, and preserve mobile touch-target and form-text sizing
      without changing the headless bridge
- [x] Recompute node markers from live DOM rectangles after page or nested
      scrolling, preserve region scroll origins, and render no page markers
      while the review panel is closed
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
      keyboard-only review controls and multi-selection, host-interaction
      blocking, searchable Layout authoring, and accessible spatial marker
      editing, including Escape cancellation from a completed multi-select
      editor with panel-focus restoration
- [x] Add a finding-level, dependency-free SVG design board with freehand,
      rectangles, text, object transforms and history, upload/paste/drop/capture
      screenshot paths, a localized icon toolbar, a responsive right-side
      drawer, bounded raster attachment, no license or remote-asset requirement,
      and a focused real-Chromium test
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

## M20: Rendered UI understanding

- [x] Add nested protocol `a3s.test.ui-understanding/1` without replacing the
      browser accessibility tree or creating a parallel image-understanding
      runtime
- [x] Derive observed colors, typography, spacing, radii, shadows, safe custom
      properties, responsive conditions, and source counts from bounded
      computed-style samples
- [x] Publish Flex, Grid, flow, overflow, containing, scroll-container, and
      stacking-context relationships as a current-node layout graph
- [x] Record client and scroll extents, signed scroll offsets, per-axis
      overflow, and derived active clipping with strict consistency checks
- [x] Record resolved physical margin, border, and padding edges together with
      box sizing, writing mode, and text direction without deriving logical
      intent or a second layout tree
- [x] Cluster repeated structures with deterministic tag, role, semantic
      state, subtree, and style fingerprints instead of class-name guessing
- [x] Record only naturally observed interaction-state style/accessibility
      differences and keep transient state under a separate observation ID
- [x] Detect CSS and Web Animations, transitions, keyframes, document, scroll,
      view, and unresolved named timelines, animation ranges, sticky behavior,
      scroll containers, canvas, and media while respecting reduced motion
- [x] Bind every record to page revision, viewport, scope, evidence sources,
      and caller-lowerable node, state, string, byte, and time budgets
- [x] Project private UI node identities to actionable `@cN` or read-only
      `@uN` observation refs, rejecting `@uN` actions and omitting evidence
      that cannot remain valid within its admitted encoded-size budget
- [x] Add strict Rust admission, including closed and acyclic layout-graph and
      component-reference integrity, repair-context propagation, unit tests,
      and a real Chromium driver regression without granting action, verdict,
      or repair authority

## M21: Deterministic control-state depth

- [x] Add ACL `value`, `enabled`/`disabled`, `checked`/`unchecked`,
      `selected`/`unselected`, and duplicate-free exact-set `selected_values`
      expectations under action protocol revision 8
- [x] Separate strict target resolution, supported state observation, and
      expected-value comparison so missing, ambiguous, invalid, or unsupported
      targets can never satisfy a negative expectation
- [x] Read live Web DOM properties, give native checkbox/radio state priority
      over ARIA, admit boolean ARIA state for custom controls, and support
      semantic `listbox` and `option` roles across open Shadow DOM
- [x] Preserve current observation and Page Context ref binding, redact all
      new targets and values, and reuse the same typed expectations in Agent
      Host deterministic verification
- [x] Support exact GUI values only when CUA supplied the value, and fail
      closed for GUI boolean/multi-selection and every TUI control-state query
- [x] Compose every new expectation with bounded assertion stability without
      changing retry, deadline, cancellation, or cleanup ownership
- [x] Prove 400/400 deterministic Web classifications: 100 exact values, 100
      checked states, 100 selected-value sets, and 100 missing-target negative
      cases
- [x] Prove 100/100 stable state windows are accepted and 100/100 transient
      state windows are rejected as `test.assert.unstable`
- [x] Prove 15/15 positive live-state checks and 4/4 negative classifications
      in standalone Chromium, including a 100 ms value-stability window and no
      leaked private runtime directory, and keep the same regression in the
      macOS and Windows real-browser CI matrix. The initial pre-role-hardening
      run took 24.26 seconds on the development host; current semantic-role
      runs took 56.92, 55.80, 23.21, and 26.72 seconds. These timings are
      environment diagnostics, not a performance benchmark.

## M22: Deterministic rendered-output depth

- [x] Add target-bound `rendered_text` and stable-locator `visible_count`
      expectations under action protocol revision 9
- [x] Normalize rendered whitespace at the Web probe and Rust comparison
      boundaries while retaining exact expected and actual evidence
- [x] Require zero/one/many target resolution for rendered text, but treat an
      empty visible locator set as the valid numeric observation zero
- [x] Separate CSS visual visibility from semantic accessibility visibility:
      include visually rendered `aria-hidden` CSS matches, exclude semantic
      accessibility-hidden ancestry, and traverse composed open Shadow DOM
- [x] Reject observation refs and visual points for collection cardinality,
      while preserving current refs and Page Context resolution for
      single-target rendered text
- [x] Preserve invalid selectors and single-target missing/ambiguity as driver
      errors; reserve `test.assert.rendered_text` and
      `test.assert.visible_count` for observed product mismatches
- [x] Redact targets and expected copy in provenance, reuse the same typed
      actions in Agent Host deterministic verification, and fail closed on GUI
      and TUI instead of estimating unsupported state
- [x] Prove 600/600 deterministic Web classifications across text match,
      text mismatch, missing target, count match including zero, count
      mismatch, and invalid-selector datasets
- [x] Prove 200/200 stable text/count windows are accepted and 200/200
      transient text/count windows are rejected as `test.assert.unstable`
- [x] Prove seven positive observations and seven negative classifications in
      standalone Chromium, including normalized nested copy, hidden and
      transparent elements, CSS/accessibility visibility separation, open
      Shadow DOM, two 100 ms windows, exact cleanup, and no leaked private
      runtime directory; keep the same regression in macOS and Windows CI
- [x] Add stable-locator `rendered_texts` under action protocol revision 10,
      preserving locator order and duplicates while normalizing each item
      independently
- [x] Treat an empty visible match set as the observed vector `[]`, reject
      refs and visual points at ACL admission, and retain invalid selectors as
      driver errors
- [x] Enforce `MAX_RENDERED_TEXT_ITEMS = 256` at ACL admission, typed Web
      expectation dispatch, page-probe result production, and untrusted result
      decoding
- [x] Reuse CSS visual and semantic accessibility visibility planes, including
      deterministic open-Shadow-DOM traversal for semantic collections
- [x] Preserve Page Context target binding and provenance redaction, reuse the
      expectation in Agent Host verification, and fail closed on GUI and TUI
- [x] Prove 600/600 ordered-sequence Web classifications across matches,
      reordered mismatches, duplicate/content mismatches, empty matches,
      empty-versus-expected mismatches, and invalid selectors
- [x] Prove 300/300 stable scalar-text/sequence/count windows are accepted and
      300/300 transients are rejected as `test.assert.unstable`
- [x] Prove 12 positive observations and 12 negative classifications in
      standalone Chromium, including duplicate and order evidence, empty
      sequences, open Shadow DOM, three accepted and three rejected 100 ms
      windows, exact cleanup, and no leaked private runtime directory; keep
      the same regression in macOS and Windows CI

## M23: Deterministic rendered-layout depth

- [x] Add a two-target `layout` expectation under action protocol revision 11
      with 17 explicit direction, containment, overlap, alignment, and size
      relations
- [x] Admit only stable semantic or CSS targets, resolve both Page Context
      refs before dispatch, and reject browser refs and visual points whose
      geometry is observation-bound
- [x] Bound tolerance to 0 through 1,024 integer CSS pixels and bound finite
      rectangle coordinates, dimensions, right edges, and bottom edges before
      relation evaluation
- [x] Resolve both Web targets and capture both rectangles atomically in one
      page evaluation, retaining CSS visual visibility and semantic
      accessibility visibility across open Shadow DOM
- [x] Preserve missing, ambiguous, invalid, hidden-semantic, and malformed
      geometry as driver errors; reserve `test.assert.layout` for two valid
      rectangles that violate the requested relation
- [x] Resolve both GUI frames from one fresh CUA snapshot and fail closed for
      unstable refs, unsupported semantic evidence, and all TUI layout queries
- [x] Redact both targets, reuse the expectation in Agent Host verification,
      and retain first/last dual-rectangle evidence through bounded stability
      sampling
- [x] Prove 3,400/3,400 deterministic Web classifications across 17 relations
      with 100 matching and 100 violating cases per relation
- [x] Prove 100/100 sustained layout windows pass and 100/100 transient
      relations fail as `test.assert.unstable`
- [x] Prove all 17 relations, 25 positive layout assertions, and 15 negative or
      driver-error classifications in standalone Chromium, including semantic,
      CSS, open Shadow DOM, accessibility-hidden, tolerance, invalid geometry,
      exact fixture cleanup, and no private runtime leak

## M24: Deterministic viewport and pointer-reachability depth

- [x] Add separate `in_viewport` and `pointer_reachable` expectations under
      action protocol revision 12 so rendered presence does not imply either
      viewport intersection or pointer reachability
- [x] Define viewport membership as positive-area rectangle intersection with
      the current visual viewport and retain the target rectangle, viewport
      rectangle, and independently recomputed intersection ratio
- [x] Define pointer reachability as at least one target or composed-descendant
      hit in a deterministic 3 by 3 grid over the clipped target rectangle,
      without inferring enabled state, keyboard access, or business clickability
- [x] Traverse open Shadow DOM for semantic resolution and deep hit testing,
      preserve CSS visual versus semantic accessibility-hidden behavior, and
      treat transparent blockers as blockers while ignoring pointer-transparent
      overlays through native hit testing
- [x] Admit only stable semantic or CSS locators, resolve current Page Context
      refs before dispatch, and reject browser refs and visual points whose
      identity belongs to one observation
- [x] Validate all untrusted rectangles, the exact nine-sample count, sample
      order, coordinates, and booleans in Rust; preserve target resolution and
      malformed-output failures as driver errors
- [x] Fail closed on GUI and TUI where current protocols cannot provide
      equivalent visual-viewport and deep pointer-hit evidence; preserve Agent
      provenance redaction and runner stability semantics
- [x] Prove 1,000/1,000 Core geometry cases: 200 fully inside, 400 partially
      intersecting, and 400 offscreen or boundary-touching rectangles
- [x] Prove 2,000/2,000 Web protocol cases: 500 positive and 500 negative
      viewport classifications plus 500 positive and 500 negative pointer
      classifications
- [x] Prove 200/200 sustained interactability windows pass and 200/200
      transient windows fail as `test.assert.unstable`
- [x] Prove 20 positive assertions and 15 negative or driver-error
      classifications in standalone Chromium, including partial viewport,
      complete and partial occlusion, transparent blockers,
      `pointer-events: none`, child hits, open Shadow DOM, invalid geometry,
      transient windows, exact fixture cleanup, and no private runtime leak

## M25: Deterministic focus-ownership depth

- [x] Add `focused`, `unfocused`, `focus_within`, and `focus_outside`
      expectations under action protocol revision 13 so sending a focus or key
      action does not imply where keyboard focus ended
- [x] Define exact ownership against the deepest active element observable
      through nested open shadow roots, while keeping a closed root opaque at
      its host
- [x] Define component-scoped ownership through rendered flat-tree ancestry,
      following assigned slots, DOM parents, and open-shadow hosts
- [x] Require a successfully resolved target for both positive and negative
      forms so missing elements cannot prove `unfocused` or `focus_outside`
- [x] Admit only stable semantic or CSS locators, resolve current Page Context
      refs before dispatch, reject browser refs and visual points in ACL, and
      fail programmatic browser-ref ownership queries as unsupported
- [x] Resolve semantic targets across open Shadow DOM while excluding
      accessibility-hidden composed ancestry, including hidden slot wrappers;
      preserve CSS current-document query semantics
- [x] Capture target resolution and focus ownership in one Web page evaluation,
      preserve missing, ambiguous, invalid, and unsupported evidence as driver
      errors, and reserve four distinct `test.assert.*` codes for observed
      mismatches
- [x] Fail closed on GUI and TUI where current protocols cannot provide
      equivalent deepest-active-element evidence; preserve Agent provenance
      redaction and runner stability semantics
- [x] Prove 600/600 deterministic Web classifications across exact and scoped
      positive states, observed mismatches, and missing negative targets
- [x] Prove 200/200 sustained focus windows pass and 200/200 transient windows
      fail as `test.assert.unstable`
- [x] Prove 17 positive assertions and 11 negative or driver-error
      classifications in standalone Chromium, including forward and reverse
      Tab, open Shadow DOM, assigned slots, accessibility-hidden slot ancestry,
      timed focus movement, exact fixture cleanup, and no private runtime leak

## M26: Deterministic live semantic-state depth

- [x] Add `expanded`/`collapsed`, `pressed`/`unpressed`,
      `readonly`/`writable`, `required`/`optional`, and `invalid`/`valid`
      expectations under action protocol revision 14
- [x] Keep the five state dimensions orthogonal so writable does not imply
      enabled, optional does not imply valid, and callers can compose the exact
      product requirement
- [x] Give `<details>.open` and applicable native read-only, required, and
      Constraint Validation properties priority over contradictory ARIA
- [x] Accept only exact boolean ARIA tokens, map `grammar` and `spelling` to
      invalid, and fail mixed pressed state, unknown tokens, or absent evidence
      closed as unsupported
- [x] Require a successfully resolved target for both positive and negative
      forms so missing elements cannot prove collapsed, unpressed, writable,
      optional, or valid
- [x] Admit only stable semantic or CSS locators, resolve current Page Context
      refs before dispatch, reject browser refs and visual points in ACL, and
      fail programmatic browser-ref state queries as unsupported
- [x] Traverse open Shadow DOM for semantic targets, preserve CSS
      current-document semantics, and capture target resolution plus live state
      in one Web page evaluation
- [x] Preserve missing, ambiguous, invalid, unsupported, and malformed evidence
      as driver errors, with ten distinct `test.assert.*` mismatch codes
- [x] Fail closed on GUI and TUI where current protocols cannot provide
      equivalent live state; preserve Agent provenance redaction and runner
      stability semantics
- [x] Prove 1,000/1,000 deterministic Web classifications across all five
      positive and negative state dimensions
- [x] Prove 100/100 sustained semantic-state windows pass and 100/100 transient
      windows fail as `test.assert.unstable`
- [x] Prove 27 positive assertions and 17 negative or driver-error
      classifications in standalone Chromium, including native controls,
      valid and invalid ARIA, open Shadow DOM, precedence, transient state,
      exact fixture cleanup, and no private runtime leak

## M27: Deterministic visual-viewport coverage depth

- [x] Add `viewport_coverage_at_least` and `viewport_coverage_at_most` under
      action protocol revision 15 without weakening revision 12 intersection or
      pointer-hit semantics
- [x] Define coverage as target/visual-viewport intersection area divided by
      complete rendered target area, with integer percentage thresholds
- [x] Admit `at_least` from 1 through 100 and `at_most` from 0 through 99 while
      rejecting the two unconditionally true endpoint claims
- [x] Require stable semantic or CSS locators, resolve current Page Context refs
      before dispatch, and reject browser refs and visual points
- [x] Capture both rectangles atomically in Web, validate them again in Rust,
      recompute the ratio independently, and preserve resolution or malformed
      geometry as driver errors
- [x] Reserve `test.assert.viewport_coverage_at_least` and
      `test.assert.viewport_coverage_at_most` for valid observed mismatches
- [x] Fail closed on GUI and TUI without equivalent visual-viewport evidence;
      preserve Agent provenance redaction and bounded assertion stability
- [x] Prove 2,000/2,000 Core threshold classifications and 2,000/2,000 Web
      protocol classifications
- [x] Prove 100/100 sustained coverage windows pass and 100/100 transient
      windows fail as `test.assert.unstable`
- [x] Extend standalone Chromium coverage to 37 passing assertions and 25
      negative or driver-error classifications, including exact boundaries,
      one-pixel intersection, four-sided clipping, oversized and offscreen
      targets, open Shadow DOM, accessibility-hidden differences, transient
      geometry, exact fixture cleanup, and no private runtime leak

## M28: Low-friction Vibe Loop

This milestone starts from the shortest useful feedback cycle: express intent,
observe the rendered product, identify the smallest mismatch, change only the
owning source, and rerun only the evidence needed to accept or reject that
change. Features that do not remove a step, improve the observation, preserve
intent, or shorten verification do not belong in this loop.

- [x] Add `a3s-test init` with bounded package-manager, script, framework,
      port, and Test Kit discovery plus a typed workspace-local ACL profile;
      never mutate dependencies or start a process during discovery
- [x] Add `a3s-test doctor` with machine-readable project, executable,
      declared-versus-installed Test Kit, semver compatibility, and optional
      URL diagnostics with exact repair commands
- [x] Add `a3s-test dev` with existing-server reuse, bounded readiness,
      headed review-session startup, compact JSONL lifecycle events, stderr
      log isolation, exact browser abort, and ownership-safe server cleanup
- [x] Contain development servers in Unix process groups with a host-death
      watchdog and in suspended-before-assignment Windows Job Objects; cover
      late descendants, early server exit, Ctrl+C, and SIGKILL recovery
- [x] Publish `@a3s-lab/testkit` to the npm Registry with provenance,
      immutable release metadata, and one supported install command per
      package manager
- [ ] Replace package-version inference with an explicit CLI, browser adapter,
      and live Test Kit protocol handshake that reports the incompatible
      boundary and exact upgrade path
- [ ] Add a local repair bridge that lets an ordinary development browser and
      the workspace-owning coding agent exchange submitted findings without a
      manually coordinated session command
- [ ] Map a selected rendered node to ranked, confidence-bearing source spans
      across framework boundaries, source maps, and explicit
      `A3STestBoundary` hints without reading undeclared framework state
- [ ] Stream revision-scoped Page Context diffs and invalidate only evidence
      affected by the latest source or DOM change instead of recapturing the
      complete page after every edit
- [ ] Generate and run the smallest deterministic verification slice from the
      finding, changed files, stable locator, browser-error delta, and prior
      proof; expand to broader regression only when impact evidence requires it
- [ ] Preserve intent, source mapping, change, verification evidence, and ACL
      promotion as one inspectable loop record so a later agent can resume
      without reconstructing the task from chat history
