# Changelog

## Unreleased

### Added

- Added the nested `a3s.test.ui-understanding/1` Page Context evidence
  protocol. Test Kit now derives bounded observed design tokens, typography,
  Flex/Grid/flow and stacking relationships, exact client/scroll extents,
  signed offsets, overflow/clipping state, resolved physical margin, border,
  and padding edges, box sizing, writing mode, text direction, and
  deterministic repeated-component clusters, real interaction-state
  differences, responsive conditions,
  document/scroll/view/named animation timelines and ranges, CSS/Web
  Animations, sticky, scroll-container, canvas, and media motion evidence after
  browser rendering. Every record binds an observation ID, page revision,
  viewport, scope, provenance summary, and node/state/string/byte/time budgets.
- Added strict Rust UI-understanding types and Web-driver admission. Protocol,
  revision, viewport, identifiers, confidence, geometry, box-model,
  overflow/clipping, and animation-timeline consistency, collection bounds,
  truncation metadata, JSON depth, strings, and encoded size now fail closed
  before the evidence reaches an agent. The same bounded UI evidence is
  included in explicitly submitted
  repair context without gaining action, verdict, or repair authority.
- Added an explicit `--browser-microphone synthetic` profile to deterministic
  Web runs, persistent and direct agent runs, and Web MCP hosts. It persists
  across agent-session turns, injects only Chromium's fake-device and
  fake-permission flags, never captures a real microphone, and defaults to
  `disabled` for new and legacy session metadata.

### Changed

- Reframed the bilingual homepage around fresh rendered-page observations,
  typed actions, exploration-to-ACL regression, human-authorized repair, and
  reviewed PRD/design expectations while presenting Test Kit as an optional
  context enhancement. The current quick start now routes readers by task,
  pins staged documentation to the published installer, distinguishes
  actionable browser and Page Context refs from read-only UI evidence refs,
  and keeps the live specimen's page-local storage boundary explicit.
- Tightened the desktop Test Kit dock, tool tray, finding editor, settings,
  and batch workspace while retaining 44-pixel targets on mobile and coarse
  pointers, including 320-pixel-wide viewports.
- Aligned documentation typography, tables, inline code, and light syntax
  rendering with the A3S UI documentation system. ACL examples now use a
  dedicated presentation grammar while product parsing remains owned by
  `a3s-acl`.

### Fixed

- Kept rendered UI layout graphs closed over their sampled nodes. Test Kit now
  links children through unboxed `display: contents` ancestors to the nearest
  sampled ancestor and omits scroll-container or offset-parent relationships
  whose source was not sampled; Core rejects missing parents or edge endpoints,
  requires every declared parent to have the matching containment edge, and
  rejects cyclic parent chains before the graph reaches an agent.
- Projected every private UI-understanding node identity into an
  observation-scoped ref before returning public Page Context observations or
  deterministic snapshot outputs. Unambiguous actionable nodes reuse `@cN`;
  evidence-only nodes receive non-actionable `@uN` refs, attempted `@uN`
  actions fail during ACL admission or before driver dispatch, and evidence
  that no longer fits its admitted byte budget is omitted instead of leaking
  an internal handle.
- Rejected structurally ambiguous rendered UI evidence at the Rust Web-driver
  boundary. Duplicate layout nodes or edges, missing parents or edge endpoints,
  contradictory, incomplete, or cyclic containment, repeated evidence
  references, invalid component membership, and layout counts above the
  sampled-node count now fail closed before reaching an agent.
- Restored desktop and mobile PNG evidence to the production website browser
  regression. Empty or greater-than-32-MiB Web screenshots are now rejected
  with an immediate artifact-cleanup attempt, while the pinned-runtime suite
  verifies PNG identity, viewport dimensions, media type, independent
  accessibility evidence, empty diagnostics, and cleanup.
- Replaced the homepage's indefinite Page Context loading state with an
  actionable retry message after the bounded capture deadline.
- Scrolled direct ref and CSS click targets into view before dispatch so a
  valid target below the initial viewport cannot report success without
  receiving the click.
- Kept open review-Shadow-DOM mutations and transient hover/focus evidence out
  of the page revision. Transient visual state receives its own UI observation
  ID, while semantic, layout, form, route, scroll, and viewport changes retain
  the monotonic page-revision boundary.
- Removed the forced dark documentation code theme and limited content
  typography to the real Rspress document root so sidebar labels retain their
  intended density.

## 0.17.0 - 2026-08-16

### Added

- Added English and Simplified Chinese review locales across visible controls,
  status labels, live announcements, and accessible names. The new
  `locale="auto" | "en" | "zh-CN"` option follows the page language by
  default, while typed `messages` overrides admit only known, non-empty values
  up to 2,048 characters. Automatic locale resolution observes live
  `<html lang>` changes, and the 90-entry Layout catalog presents and searches
  both English and Chinese component names.
- Added a loopback-only independent screen-reader audit fixture, canonical
  15-workflow manifest, strict revision-bound audit artifact, bounded evidence
  verifier, and separate all-passed closure gate. The shared real-browser
  fixture exposes candidate, clarification, human-review, terminal, and reset
  states without DevTools while keeping M8 open for an actual independent
  VoiceOver, NVDA, or equivalent hands-on audit.
- Added an independent `axe-core` WCAG A/AA gate to the real Chromium Test Kit
  suite. It scans the open review Shadow DOM across system, light, and dark
  themes plus preferences, marking and Layout editors, restored drafts,
  contract and design candidates, clarification replies, human review actions,
  and terminal repair states.
- Exposed every global Test Kit review shortcut through `aria-keyshortcuts`
  and a keyboard-reference section inside Review preferences. Unit and real
  Chromium coverage verify both the control metadata and accessibility-tree
  help content.

### Changed

- Refined the review Dock, target editor, findings workspace, markers, and
  preferences with clearer hierarchy and typography, mutually exclusive tool
  and findings surfaces, scroll-safe short-viewport settings, 44-pixel mobile
  targets, and mobile form sizing that avoids browser zoom.
- Rebuilt the embedded review surface as a compact floating dock with a
  secondary tool tray, target-attached finding editor, pinned batch workspace,
  and one aggregate marker per multi-selection finding. Direct `E`, `M`, `T`,
  `A`, and `D` marking shortcuts retain editable-control ownership and are
  covered by React and website ACL regressions.
- Added a staged homepage demonstration that scans the rendered page, binds
  semantic and geometric context, opens a human finding, sends its context,
  and returns a repair-ready receipt. Its five-stage state machine has an
  explicit pause and resume control, resets off-screen, stops when the live
  review flow begins, and respects reduced-motion preferences.
- Updated the Docker Buildx and registry-login Actions to their Node.js 24
  major versions, removing the deprecated Node.js 20 runtime from runner-image
  CI and the release publishing path without changing image inputs or
  registry authority.
- Release creation now waits for a fail-closed metadata and documentation
  preflight before scheduling privileged GUI certification. The gate requires
  the tag, Rust workspace version, dated changelog section, default Rspress
  version, ordered snapshot provenance, and both locale trees to agree, then
  rebuilds and verifies the complete generated site.
- Release metadata now binds the packaged Test Kit version to the active
  documentation snapshot and requires every archived snapshot to retain its
  own semantic Test Kit version.
- Raised the Rust workspace minor release to `0.17.0` and the Test Kit minor
  release to `0.4.0`, with the browser bridge deriving its reported SDK
  version directly from the package manifest.
- Shortened the repository homepage around installation, one proven Web path,
  and the shared evidence contracts. The Rspress homepage now identifies and
  preserves the selected documentation version in copyable Unix and
  PowerShell installers, while generated-site checks derive every bilingual
  route and reject broken internal references.
- Split review-overlay host lifecycle and global input policy out of the React
  overlay module so rendering, page input blocking, focus tracking, and
  shortcut dispatch retain explicit ownership below the repository file-size
  limit.

### Fixed

- Preserved free-form Layout component values across locale changes while
  translating known catalog selections, so a Chinese search such as `结账`
  finds and selects `结账表单` without leaving English UI copy behind. The Web
  driver's Shadow DOM fallback now preserves the native `searchbox` role for
  `<input type="search">` during semantic fill and visibility checks.
- Corrected localized Layout labels and made pointer multi-selection derive its
  displayed count from selected node IDs instead of parsing English copy.
- Separated staged documentation from the published install version. The
  homepage and repository README now pin the real stable release while main is
  ahead, disclose that state, and reject a version tag until both versions are
  intentionally aligned.
- Restored review Dock focus after closing with the secondary tool tray open,
  and removed transformed or filtered containing blocks that displaced compact
  Review preferences outside the viewport. The preference surface now uses
  the correct panel text color in light themes; real-browser regressions
  expand the tray and verify focus, viewport bounds, scrolling, and contrast.
- Made page-motion pause ownership-safe. Test Kit now records only running
  animations and playing media that it actually pauses, freezes motion that
  starts while review pause remains active, and resumes only that owned set;
  host animations or media that were already paused stay paused. Unit and real
  Chromium regressions cover the initial, late-starting, resumed, and
  pre-paused states. The production-website E2E now uses semantic Shadow DOM
  targets and also retains mobile Layout screenshots, accessibility output,
  and empty browser diagnostics.
- Bound screen-reader audit verification to a real Git commit and read the
  canonical workflow manifest plus Test Kit version from that revision. The
  location-independent verification v2 record now includes byte length and
  SHA-256 bindings for the audit JSON, committed manifest, every evidence file,
  and the ordered evidence set, while rejecting files replaced during hashing
  and aggregate evidence above 1 GiB.
- Raised the documentation mobile navigation and installer copy controls to a
  44-pixel minimum target without changing the compact desktop layout.
- Removed collapsed mobile documentation-menu groups from the accessibility
  tree while preserving keyboard focus restoration and disclosure semantics.
  Language, version, and resource links are now exposed only while their
  owning group is expanded.
- Raised muted text and repair-thread actor contrast to WCAG AA in system and
  light themes, and made the scrollable findings region keyboard focusable.
  The real-browser audit retains the exact failing node and rule when either
  contract regresses.
- Made the React Test Kit adapter render on Node without server-side
  `useLayoutEffect` warnings while retaining synchronous browser focus and
  boundary registration after hydration. Framework-neutral bridge inspection
  now returns `null` on Node and direct runtime enablement reports its browser
  requirement explicitly. Package and release gates now load the built ESM and
  CommonJS entries, render both React adapters on the server, type-check both
  module consumers, and require the MIT license in the tarball.
- Serialized Windows CIM process-identity queries before marker-checked
  emergency cleanup. Concurrent cold-provider lookups now retain the existing
  five-second per-query bound instead of starving one another and making
  otherwise safe process cleanup depend on a CI rerun.
- Prioritized unmodified `Escape` for active Test Kit marking and finding
  editors even when the event originates in an editable control. Idle host
  editors still retain `Escape` without closing the review panel, while
  cancelling a completed multi-selection editor now restores panel focus.
  React and real Chromium regressions cover both ownership boundaries.
- Kept keyboard multi-selection on application controls until explicit
  `Shift+Enter` completion. Starting or extending a selection no longer opens
  an empty editor or steals focus into the review Shadow DOM, and `Escape`,
  the marking Cancel control, panel toggling, and Layout Mode now discard
  incomplete candidates consistently. React and real Chromium regressions
  cover focus, selection announcements, host-action suppression, completion,
  and cancellation.
- Restored keyboard focus without scrolling to the last connected application
  control when `Hide until tab restart` removes the review Shadow DOM. React
  and real Chromium regressions cover the complete focus transfer.

## 0.16.2 - 2026-08-15

### Fixed

- Bound the checkout-free GUI certification asset upload to the explicit
  GitHub repository so the release workflow can publish its signed evidence.
- Raised the Rust workspace patch release to `0.16.2`. The unchanged Test Kit
  remains `0.3.0`.

## 0.16.1 - 2026-08-15

### Added

- Added a reusable real macOS GUI certification workflow that rebuilds the
  locked CUA source, verifies its compile-time source revision and host
  permissions, runs semantic and window-vision observations, proves exact
  fixture cleanup, and emits `a3s.test.gui-host-certification/1` evidence.
- Added a detached SHA-256 record and GitHub OIDC/Sigstore provenance for the
  certification attestation. Release tags publish the record and checksum as
  release assets.

### Fixed

- GUI cleanup now confirms that the exact owned application PID has stopped
  before reporting success, while preserving bounded retries and PID-reuse
  protection.

### Safety

- GUI certification can run only through an explicit workflow dispatch or a
  version tag, and only on the dedicated macOS arm64 self-hosted runner label.
  Pull requests and ordinary branch pushes cannot schedule that privileged
  desktop host.
- Release creation now waits for real permission, semantic, visual, and owned
  cleanup certification. The workflow uses bounded deny-by-default CUA
  policies and removes its fixture registration and daemon on every exit path.

### Changed

- Advanced the locked CUA 0.10.0 revision to the reviewed background-launch
  identity fix while retaining the existing MCP and capability contracts.
- Raised the Rust workspace patch release to `0.16.1`. The unchanged Test Kit
  remains `0.3.0`.

## 0.16.0 - 2026-08-15

### Added

- Added selection-scoped `insert_text` to ACL suites, persistent agent CLI
  sessions, the typed action JSON contract, and the standalone Web driver. It
  inserts at the browser's current caret or replaces the current selection
  without refocusing a target.
- Added real standalone-browser coverage that focuses a semantic form field,
  creates a keyboard selection, replaces it through `insert_text`, and proves
  the submitted value end to end on macOS and Windows CI.

### Fixed

- Accepted the current standalone browser's top-level and nested visibility
  response envelopes without weakening typed boolean admission.

### Safety

- `insert_text` uses the existing typed text-input policy capability and
  provenance redaction. It carries no target or locator authority and can only
  affect the editing context explicitly established by an earlier browser
  action.

### Changed

- Advanced the action protocol revision to 7 and raised the Rust workspace
  release to `0.16.0`. The unchanged Test Kit remains `0.3.0`.

## 0.15.0 - 2026-08-15

### Added

- Added strict provider protocol `a3s.test.design-audit-provider/1` for
  advisory design-quality review across hierarchy, composition, spacing,
  typography, color use, consistency, interaction clarity, content clarity,
  and responsive composition.
- Added `HttpDesignAuditProvider`, generated provider and HTTP-envelope
  schemas, digest-bound PNG transport, complete forensic Page Context input,
  typed page/node/normalized-region findings, and local response admission.
- Added `a3s-test agent audit` with ACL configuration, selected dimensions,
  latest-observation and exact-revision binding, bounded context pagination,
  post-inference revision checks, retained screenshot evidence, and optional
  projection to an embedded review surface.
- Added Test Kit protocol `a3s.test.design-audit-report/1`, a separate bounded
  Design Audit store, advisory markers and review UI, explicit retargeting,
  and human promotion into the existing single or batch Repair Ledger.

### Safety

- Design-audit output has no verdict, expected-surface, browser-action, or
  repair authority. Even high-priority advice remains non-blocking until a
  human explicitly reviews and saves or sends it.
- Local admission binds provider/model identity, observation, surface
  revision, screenshot and canonical page-context digests, dimensions,
  complete context, current node geometry, deadline, response limits, and
  provider-reported cost. Image and context bytes are rehashed again at the
  HTTP boundary, and any later page revision expires the projected advice.
- Provider ACL and credentials are admitted before browser access. HTTPS is
  required except for explicit loopback HTTP, redirects and environment
  proxies remain disabled, authorization is redacted, and inference runtime,
  capacity, privacy, and licensing remain deployment-owned.

### Changed

- Raised the Rust workspace release to `0.15.0` and `@a3s-lab/testkit` to
  `0.3.0`.
- Increased bounded Web page-context inspection from 500 to 5,000 nodes so an
  audit can assemble a complete paginated snapshot within the existing Test
  Kit protocol limit.

## 0.14.0 - 2026-08-15

### Added

- Added exclusive GUI worker profiles backed by deployment-owned `gui_host`
  ACL. Worker startup now admits the fixed CUA endpoint, policy, application,
  launch or attach target, perception profile, and explicit host-permission
  declaration before accepting jobs.
- Added read-only GUI host readiness probes. Inventory protocol
  `a3s.test.worker-capabilities/2` records the locked CUA contract, application
  target, configuration and policy digests, exact `accessibility` and
  `screen_recording` grant, attribution source, and permission digest without
  launching the configured application.
- Added GUI-aware deterministic sharding in
  `a3s.test.distributed-run/2`. GUI workers and shards have one exclusive
  desktop lane, and the coordinator requires an exact
  `host_permission_digest` pin for every inspected GUI worker.

### Safety

- Advanced remote execution to `a3s.test.remote-worker/3`. GUI submissions
  must bind the exact host-permission digest from the admitted worker
  inventory; missing, mismatched, or unexpected permission bindings fail
  before input materialization or driver startup.
- Remote requests cannot select a GUI application, executable, policy,
  endpoint, target, or shell. GUI sessions revalidate the live permission
  grant before application launch or attachment, and retain the existing
  application, PID, window, and owned-cleanup checks during execution.
- Worker authorization environment variables are explicitly removed from CUA
  proxy children. Regression coverage proves the child does not inherit the
  configured value.

### Changed

- `a3s-test worker inventory` and `a3s-test worker serve` now accept
  `--gui-host-profile`. A worker exposing GUI must use
  `--max-parallel-scenarios 1`; pools scale by deploying independent desktop
  workers.
- Distributed GUI plans now repeat required surfaces and the permission digest
  in each immutable shard and remote submission, and include both in the plan
  digest.
- Raised the Rust workspace release to `0.14.0`. The unchanged Test Kit remains
  `0.2.0`.

## 0.13.0 - 2026-08-14

### Added

- Added protocol `a3s.test.distributed-run/1` with generated strict schemas for
  deterministic plans, shard bindings, run analyses, accountable quarantine,
  flake summaries, historical changes, and shard issues.
- Added `a3s-test distributed schema`, `distributed plan <config.acl>`, and
  `distributed run <config.acl>`. ACL configuration owns contained inputs,
  worker identity and image pins, environment-supplied authorization, bounded
  deadlines, history retention, and explicit quarantine accountability.
- Added deterministic surface-aware sharding. Scarce worker surfaces are
  scheduled first, recent exact-suite median durations replace timeout
  fallbacks when available, and stable lane balancing assigns every scenario
  exactly once.
- Added bounded concurrent dispatch, independent renewable-lease supervision,
  exact remote cancellation on interrupt, strict terminal report retrieval,
  atomic local report/history persistence, count/age retention, and exclusive
  history-root locking.
- Added real multi-worker HTTP integration coverage for planning, exact
  dispatch, quarantine, a second historical run, fixed/flake detection, and
  first-interrupt exit 130 with a retained cancelled remote job.

### Safety

- Advanced the execution protocol to `a3s.test.remote-worker/2`. Every
  submission now carries a non-empty, sorted, unique scenario ID set that is
  part of the immutable request digest. The worker filters the admitted suite
  to exactly that set and rejects missing, duplicate, or surface-drifted
  selections before driver startup.
- The coordinator accepts a report only after verifying its job, dispatch,
  immutable request digest, artifact descriptor, chunk offsets and EOF,
  canonical Base64, complete SHA-256, size, media type, suite, run ID, status,
  counts, exact scenario set, and surface mapping.
- Quarantine can suppress only explicit `test.assert.*` failures and proven
  Surface Contract mismatches. Driver, cleanup, inconclusive-contract,
  transport, report, timeout, cancellation, interruption, and other
  infrastructure failures remain required.
- Distributed config, suite inputs, contract provenance, history, report
  writes, pruning, and remote upload rebinding reject symbolic links, Windows
  reparse points, containment escapes, unsafe replacement, oversized data, and
  conflicting immutable IDs. HTTP requires HTTPS except for loopback, disables
  redirects and environment proxies, and never serializes credentials.

### Changed

- Historical change comparison now uses the latest retained run, including
  across suite revisions. Flake accounting and scheduling durations remain
  restricted to the exact suite digest so changed test semantics do not enter
  reliability statistics.
- Raised the Rust workspace release to `0.13.0`. The unchanged Test Kit remains
  `0.2.0`.

## 0.12.0 - 2026-08-14

### Added

- Added the independent `a3s.test.remote-artifacts/1` protocol with generated
  strict schemas for service inspection, bounded terminal-report queries,
  paginated artifact descriptors, and chunked report or evidence reads.
- Added `a3s-test worker artifacts schema` and authenticated
  `POST /v1/artifacts` support to the loopback reference host. Readiness now
  reports both execution and artifact descriptors.
- Added deployment-owned two-tier retention. Complete inputs, reports, and
  evidence are bounded by job count, aggregate bytes, and age; compact report
  indexes have independent longer count and age windows. Age limits continue
  to run while the worker is idle.
- Added restart reconstruction, crash-recoverable `retained` to `pruning` to
  `pruned` transitions, and bounded full-index garbage collection.

### Safety

- Artifact lists and reads bind job ID, dispatch ID, immutable request digest,
  and canonical bounded cursors. Reads additionally bind the artifact SHA-256,
  exact indexed path, offset, and maximum chunk size.
- Artifact scans and reads reject symbolic links, Windows reparse points,
  containment escapes, non-regular or empty evidence, ASCII case-folded path
  collisions, file replacement, digest drift, oversized trees, and corrupted
  persisted indexes.
- Unsafe evidence cannot leave a successful terminal result. The worker
  records a durable failure, removes only the owned payload, preserves any
  external link target, and fails closed to new submissions if durable
  retention becomes unhealthy.

### Changed

- Raised the Rust workspace release to `0.12.0`. The unchanged Test Kit remains
  `0.2.0`.
- Kept artifact transport out of `a3s.test.remote-worker/1`; execution and
  retained-byte access remain independently versioned authority boundaries.

## 0.11.0 - 2026-08-14

### Added

- Added transport-neutral protocol `a3s.test.remote-worker/1` with generated
  strict request, response, and descriptor schemas. Every dispatch binds one
  exact worker instance, externally supplied image digest, complete capability
  inventory digest, absolute deadline, renewable lease, required surfaces,
  and immutable request digest.
- Added bounded, sorted, SHA-256-verified inline input bundles with portable
  paths and canonical Base64 admission before any private materialization.
- Added a persistent sequential remote worker service with bounded queueing,
  idempotent dispatch, conflict rejection, lease renewal, queued and running
  cancellation, deadline enforcement, bounded cleanup, append-only job state,
  exclusive descriptor-bound state roots, and restart interruption recovery.
- Added `a3s-test worker remote schema` and the `a3s-test worker serve`
  reference host. It serves one strict HTTP endpoint on loopback, requires an
  exact environment-supplied Authorization header, and executes only
  deployment-owned Web and TUI profiles.
- Added digest-bound terminal report summaries and private per-job Runner
  artifact roots. A real HTTP/TUI integration test proves authentication,
  dispatch, PTY execution, report persistence, evidence containment, SIGINT
  shutdown, and exact owned cleanup.

### Safety

- Remote requests cannot select commands, browser integrations, TUI backends,
  arguments, credentials, or network policy. Web origins and domains and the
  TUI executable are fixed when the host starts. Selecting a shell or an
  application with shell escapes remains an explicit deployment grant to
  authenticated jobs.
- The reference HTTP host rejects non-loopback binds, oversized or non-JSON
  bodies, and missing or incorrect authorization. TLS termination and external
  authentication policy remain deployment responsibilities, and configured
  authorization values are never printed or inherited by browser probes, Web
  commands, or TUI child processes.
- Remote command/body, browser-idle, cleanup, and retry-backoff settings have
  explicit startup bounds, so capability probing cannot be configured with an
  effectively unbounded deadline.
- Failed identity, capability, time, path, size, Base64, or digest admission
  writes no job input. Exact duplicate dispatches return their durable state;
  conflicting reuse fails closed. Non-terminal durable state is never resumed
  speculatively after restart.
- Reports and surface evidence remain private worker files. Remote responses
  expose only bounded counts and a media-type, byte-length, and SHA-256 report
  descriptor; artifact transport remains a separate milestone.
- Dropping the final remote service handle cancels its worker loop, preventing
  an embedding process from retaining the exclusive state-root lock through a
  detached task.

### Changed

- Added a configurable Runner artifact root so remote jobs do not mutate the
  process-wide working directory.
- Raised the Rust workspace release to `0.11.0`. The unchanged Test Kit remains
  `0.2.0`.

## 0.10.0 - 2026-08-14

### Added

- Added a Linux/amd64 hermetic runner image with the matching CLI, standalone
  browser 0.26.0, pinned Chrome Headless Shell, and the native Unix PTY
  backend. CI exercises Web and TUI ACL suites inside the restricted image and
  release automation publishes it as
  `ghcr.io/a3s-lab/a3s-test-runner:<version>`.
- Added `a3s-test-worker` and protocol
  `a3s.test.worker-capabilities/1` for strict, canonically ordered scheduling
  evidence covering runtime identity, concurrency, Web capabilities, TUI
  capabilities, backend features, and hard limits.
- Added `a3s-test worker inventory` and `a3s-test worker schema`. Web is
  advertised only after an explicitly selected real executable passes its
  version probe; the compiled TUI projection is available by default.
- Added strict JSON Schemas and local admission for Web and TUI capability
  projections. Standalone browser 0.26.x cannot overclaim exact-origin
  containment, and the runner image does not claim GUI execution.
- Each release now includes `a3s-test-runner-image.txt` with the immutable GHCR
  manifest reference used to bind the published runner independently of its
  mutable tag.

### Safety

- Runner inputs bind the Dockerfile frontend and Rust and Node base-image
  digests, fixed Debian snapshots, npm integrity, and the Chrome archive
  SHA-256. The final image runs as a non-root user and supports a read-only
  root filesystem.
- Image smoke tests use no external network, drop every Linux capability,
  enable `no-new-privileges`, bound PIDs, memory, CPU, and temporary storage,
  and verify screenshot, accessibility, and terminal evidence plus complete
  process, socket, and runtime cleanup.
- Worker inventories explicitly declare that they are self-reported,
  unauthenticated scheduling evidence and cannot authorize execution. A
  scheduler must independently bind the image digest and execution policy.
- Requested Web probes fail closed instead of silently omitting a surface, and
  unknown fields, duplicate surfaces, feature overclaims, invalid protocol
  revisions, and concurrency outside 1 through 64 are rejected.

### Changed

- Raised the Rust workspace release to `0.10.0`. The unchanged Test Kit remains
  `0.2.0`.

## 0.9.0 - 2026-08-14

### Added

- Added deterministic TUI suites through `a3s-test-driver-tui` and typed CLI
  host options. Unix uses owned PTY process groups with host-death watchdogs;
  Windows uses ConPTY sessions backed by kill-on-close Jobs.
- Added action protocol revision 7 with `terminal_paste`, `terminal_resize`,
  `terminal_recording`, and terminal regex waits. The semantic VT observation
  includes bounded viewport and scrollback text, cursor and mode state, exit
  status, and output truncation metadata.

- Added `a3s-test agent run <agent-run.acl>` for one bounded Web workflow with
  a deployment-supplied HTTP LLM provider, typed action capabilities, exact
  origin checks, shared Test Kit `@cN` targets, deterministic local
  verification, and `a3s.test.agent-run/1` reports.
- Added stable `a3s.test.llm-provider/1` discovery through `a3s-test provider
  schema llm`. Its authority is `proposal_only`: it may propose typed surface
  actions but cannot determine the test verdict, claim browser observation,
  or authorize repair.
- Added `a3s-test contract generate` and `a3s-test contract review` for an
  operational, two-stage path from contained PRD/design sources through a
  deployment-owned HTTP provider to reviewed ACL Surface Contracts.
- Added strict `a3s.test.contract-workflow/1` generated/reviewed artifacts and
  ACL human-review files with explicit candidate decisions, conflict
  selections, rationales, and complete audit retention.
- Added stable `a3s.test.contract-generation-provider/1` and
  `a3s.test.visual-grounding-provider/2` wire identifiers with generated JSON
  Schema 2020-12 request and response contracts.
- Added `a3s-test provider schema contract-generation` and `a3s-test provider
  schema visual-grounding` for transport-neutral adapter discovery, including
  explicit authority and safety invariants.
- Added typed HTTP adapters for deployment-owned contract-generation and
  visual-grounding services. Provider discovery now includes their versioned
  request and response envelope schemas.
- Added `a3s-test agent ground` for explicit, revision-bound visual location
  in a persistent Web session. It returns advisory semantic or image-bound
  candidates without dispatching input or authorizing repair.
- Visual-grounding HTTP version 2 now carries the admitted PNG as a bounded
  Base64 attachment with its digest and logical name, allowing remote
  deployment providers to consume evidence without client filesystem access.

### Safety

- TUI close, cancellation, timeout, Drop, EOF, crash, and repeated interrupt
  paths terminate only the exact owned process tree, including descendants
  that outlive the root. Terminal dimensions, scrollback, paste, wait patterns,
  retained output, and recording paths are bounded and contained.

- Embedded CLI runs require at least one local `expect` action. Model
  `finish` is provisional until read-only verification passes and the exact
  browser session closes successfully. One workflow deadline covers opening,
  initial navigation, observe-decide-act turns, and verification; cleanup has
  its own bounded deadline.
- Planner endpoints use HTTPS or explicit loopback HTTP, disable redirects and
  environment proxies, bound request and response bodies, and obtain optional
  authorization only from a named environment variable. Complete reports,
  including surface-open failures and Test Kit context, use the same exact
  secret and credential-key redaction policy before atomic publication.
- CLI commands run on an explicitly sized async worker stack, and command
  futures are type-erased at dispatch so large unrelated branches cannot
  exhaust the smaller Windows process-entry stack.
- Source paths are contained beneath the workflow config, file digests are
  calculated locally, provider credentials come only from explicitly named
  environment variables, and generated artifacts contain no executable ACL.
- Saved artifacts bind their full payload with SHA-256 and retain the source
  manifest, cost ceiling, and generation limits so review can rehash evidence
  and replay response, conflict, and open-decision admission. The digest is a
  mutation checksum, not an authenticity signature.
- Review regenerates and admits the contract locally, rejects unresolved or
  tampered decisions, and publishes canonical ACL plus audit with bounded,
  symlink-safe, overwrite-explicit storage.
- Provider wire fixtures reject unknown fields and retain digest, deadline,
  cost, identity, hierarchy, coordinate-space, and geometry bindings. Schema
  conformance does not grant verdict, browser-observation, or repair authority.
- HTTP providers use a fixed typed endpoint, disable redirects and environment
  proxies, require HTTPS except for explicit loopback HTTP, bound bodies,
  enforce wire and transport deadlines, and redact configured authorization
  values from diagnostics. Service-level evidence and authority admission
  remains mandatory.
- Grounding ACL, credential, provider, query, and budget admission completes
  before browser connection. Screenshots are bounded to 32 MiB, rehashed by
  both the service and HTTP adapter, and bound to the latest stored Test Kit
  revision before capture and after provider inference. Provider failure,
  cancellation, ref drift, or revision drift invalidates the observation.

### Changed

- Raised the Rust workspace release to `0.9.0`. The unchanged Test Kit remains
  `0.2.0`.

## 0.8.0 - 2026-08-13

### Added

- Added a typed optional visual-grounding provider boundary in
  `a3s-test-agent` for explicit requests and semantic fallback on canvas,
  image-only, remote-desktop, and design-reference surfaces. Requests bind a
  verified screenshot digest and dimensions to the current observation,
  deadline, trigger, query, and cost ceiling.
- Added strict response admission, provider provenance, screenshot-pixel and
  normalized coordinate support, pre-dispatch screenshot rehashing,
  visual-viewport mapping, and deterministic hit-testing against current Test
  Kit geometry. Only a unique hit upgrades to a semantic target; ambiguous and
  unmapped candidates stay image-bound.
- Added a typed source-to-contract provider boundary in `a3s-test-agent` for
  digest-bound PRD and design evidence. PRD candidates preserve exact UTF-8
  byte spans, confidence, and unresolved product decisions; design candidates
  preserve image dimensions, coordinate space, bounded regions, hierarchy, and
  confidence.
- Added deterministic PRD/design merge with explicit stable conflicts, human
  candidate approval and conflict resolution, complete provider/review audit
  retention, and checked generation into the existing ACL Surface Contract
  rather than a second DSL.
- Added optional per-element ACL citations and CLI verification that every
  citation quote exactly matches its provenance byte range after SHA-256
  verification. The same admitted contract retains stable finding IDs before
  and after repair.

### Safety

- Visual-grounding results are always advisory and observation-scoped. Stale
  provenance, truncated or mismatched page context, identity or dimension
  mismatch, invalid geometry, timeout, cancellation, oversized output, and cost
  overrun fail closed. Model weights, transports, credentials, and licenses
  remain deployment-owned and are not bundled with A3S Test.
- Source-derived candidates are never browser observations, test verdicts, or
  repair authorization. Sources are re-read after provider execution, and
  stale identity or provenance, concurrent edits, invalid or cyclic structure,
  prefilled citations, mismatched spans, inconsistent design hierarchy,
  unresolved selected decisions, timeout, cancellation, oversized output, and
  cost overrun fail closed.

### Changed

- Raised the Rust workspace release to `0.8.0`. The unchanged Test Kit remains
  `0.2.0`.

## 0.7.0 - 2026-08-13

### Added

- Added versioned Expected Surface Contracts to A3S ACL with reviewed PRD,
  design, manual-decision, and official-document provenance, digest admission,
  named variants and states, stable element identity, accessibility semantics,
  and deterministic reconciliation against bounded Test Kit observations.
- Added runner-owned `verify_contract` in action protocol revision 6. Blocking
  findings fail, advisory findings remain in passed reports, and missing or
  truncated page context fails closed as inconclusive while retaining the full
  contract report.
- Added stable finding identifiers and optional one-way projection into the
  Test Kit Quality Store. Reviewers can inspect, target, dismiss, save, and
  submit individual findings without granting repair authority merely by
  viewing or editing a candidate.
- Updated `@a3s-lab/testkit` to 0.2.0 with bounded quality-report admission,
  contract-finding markers, compact review controls, and explicit single or
  batch submission into the existing repair ledger.
- Documented source-to-contract generation and typed visual-grounding provider
  boundaries as future work. Visual candidates remain observation-scoped,
  advisory, and subordinate to browser semantics.

### Safety

- Contract and provenance files must be regular files beneath the suite
  directory, and every provenance digest is verified before a browser opens.
- Interactive agents, MCP tools, sessions, and surface drivers reject or omit
  runner-owned contract verification. Optional report projection cannot change
  the deterministic runner verdict and has a separate bounded best-effort
  budget, including when a page bridge hangs.

## 0.6.0 - 2026-08-13

### Added

- Added the development-only `@a3s-lab/testkit` package with framework-neutral
  and React entry points. Its versioned page-context bridge reports bounded
  page identity, route, readiness, component ownership, semantic locators,
  form state, application facts, and viewport/document/normalized CSS-pixel
  geometry without changing the host DOM.
- Added a Shadow DOM review overlay for element, text, rectangular, freehand,
  click/drag multi-selection, and ordered batch findings. Reviewers can edit,
  hide, restore, copy, submit, discuss, accept, reject, and reopen individual
  findings while retaining persistent markers and drafts across route or
  hot-reload changes.
- Added typed Layout Mode placement and rearrangement intent, including a
  searchable catalog of 90 component types across ten categories and a
  free-form project-specific component field.
- Added typed page-context models, atomic accessibility/context observations,
  revision-bound `@cN` refs, scoped inspection, capability discovery, CLI and
  MCP projections, generated schemas, and compatibility coverage. Pages that
  do not embed the Test Kit preserve the existing Web behavior.
- Added an append-only repair ledger and complete single/batch lifecycle for
  claim, clarification, cancellation, repair, verification, human review, and
  resolution. Workspace mutation is serialized across sessions and processes,
  and admitted ACL candidates are proved in a fresh browser before promotion.
- Added owned before/after context, screenshot, console, page-error, diff, and
  verification evidence for repair work, plus machine-readable CLI commands
  and MCP tools for repair pickup, status, replies, human actions, recovery,
  and inspection.
- Added keyboard controls, focus restoration, a polite status announcer,
  named non-modal dialog semantics, theme and marker preferences, optional
  host-pointer blocking, animation pause, page wireframe fade, panel docking,
  and bounded Markdown/JSON copy for the review overlay.
- GitHub Releases now include a checksum-protected `a3s-testkit.tgz` package in
  addition to the CLI archives, installers, and Coding Agent Skill.

### Fixed

- Persisted review drafts now rebind through stable semantic locators after
  route changes, virtualization, DOM reordering, and hot reload instead of
  relying on private node order.
- Browser command output in real-browser release gates now uses bounded
  file-backed capture, preventing persistent daemons from retaining inherited
  pipes and stalling successful commands on Windows.
- The cross-platform real-browser gates now use pinned Chrome Headless Shell
  builds, portable cleanup checks, and deterministic process fixtures on
  macOS and Windows.

### Safety

- Test Kit context is size-bounded, redacted, and explicitly untrusted. The
  browser bridge exposes no shell, filesystem, cookie, credential, arbitrary
  network, source-editing, or JavaScript-evaluation authority.
- Context and layout refs are bound to the latest observation and page
  revision; navigation, revision drift, failed actions, and state-changing
  actions expire them before input dispatch.
- Repair execution uses an exact owning session, one persistent workspace
  mutation lock, bounded evidence, typed conflict relations, and fail-closed
  verification. Human acceptance remains the default, while automatic
  resolution requires explicit session-scoped enablement.
- The review runtime and overlay both require explicit enablement, isolate
  styles and events from the host page, omit themselves from captured page
  context, and retain headless context collection when the overlay is hidden.

## 0.5.1 - 2026-08-06

### Fixed

- Browser commands now pass an explicit `--headed false` by default and
  enforce Chrome's `--headless=new` launch argument, so inherited Browser
  environment or configuration cannot unexpectedly open a window. Existing
  launch arguments such as `--no-sandbox` are preserved. `--headed` remains
  the explicit opt-in for visible debugging.
- Added a Windows regression that executes the Browser adapter through a real
  `.cmd` shim and verifies with `GetConsoleWindow` that neither the shim nor
  its child receives a console window.

## 0.5.0 - 2026-08-06

### Added

- Added the `a3s-test-driver-gui` foundation with typed CUA endpoints and
  application identities, a window-only capture profile, bounded MCP stdio
  transport, and fail-closed version/schema/capability admission.
- Added `compat/cua-stack.acl` as the reviewed source of truth for the exact
  A3S CUA revision and protocol surface, plus an architecture decision record
  for the adapter boundary.
- Added the GUI semantic driver with application launch/attach, deterministic
  window binding, accessibility observations, opaque generation-bound refs,
  semantic pointer/keyboard actions, assertions, and PNG window evidence.
- Added the surface-neutral `a3s-test-session` application layer and an MCP
  stdio projection for GUI start, observe, typed action, finish, abort, and
  action-schema discovery.
- Added the window-vision GUI profile with SHA-256-bound screenshot evidence,
  observation-scoped visual points, pixel click/drag/scroll actions, and
  explicit multimodal image attachments for embedded LLM hosts.
- Added `automation_id` and `visual_point` targets and advanced the typed
  action protocol to revision 5.
- Added a locked three-platform/two-endpoint GUI certification matrix,
  `gui-certification` inventory output, and a `gui-certify` real-host
  observation/cleanup harness. The locked CUA 0.10.0 macOS profiles are
  contract-tested; Windows and Linux remain explicitly unsupported.
- Added a typed `ProvenanceRedactor` for embedded agent runs. It removes
  registered exact secrets across the complete serializable result, redacts
  common credential-shaped JSON fields and input-bearing action payloads, and
  strips URL user information, queries, and fragments without changing the
  values sent to the trusted provider or surface driver.
- Added a loopback-only Web fixture server with dynamic ports, deterministic
  routes, a cross-origin request sentinel, owned worker cleanup, and a pinned
  standalone `agent-browser` 0.26.0 macOS/Windows CI suite that retains
  screenshot evidence and verifies removal of browser processes plus private
  runtime directories even after test failure.
- Added a typed, bounded browser domain policy for persistent agent sessions.
  Initial and navigation-approved hosts are admitted automatically, while
  `--allow-domain` permits additional network hostnames without adding them to
  A3S Test's exact-origin action and observation gates.

### Fixed

- Forced restricted standalone 0.26.x browser sessions through its explicit
  Chrome launch path, ensuring the daemon installs domain interception before
  the first navigation instead of leaking page-driven subresource requests
  through the upstream implicit auto-launch path.
- Made `domcontentloaded` waits evaluate the current document readiness state
  and kept the browser daemon idle deadline at least as long as one admitted
  command, preventing standalone 0.26.x from expiring a live session during a
  long wait.
- Made the agent-session CLI assertions parse their structured driver-log
  fields, recover the serial test lock after a failure, and use the Rust test
  binary itself for portable, registry-serialized owned-process-tree fixtures.
  Link-containment tests now compare the canonical runtime binding on
  path-aliasing hosts.
- CI test failures now retain their complete Cargo output and publish bounded
  panic/error tails as check-run annotations, so cross-platform failures remain
  diagnosable even when unauthenticated workflow logs are unavailable.
- Made the hermetic Web fixture use blocking accept with transient-handshake
  recovery and Content-Length-framed client reads, preventing intermittent
  Windows connection-aborted failures while retaining bounded response sizes.
- Made Web runtime-path tests and CLI test imports respect non-Unix hosts so
  the complete workspace test and Clippy gates also pass on Windows.
- Expanded the Rust formatting, test, and warning-free Clippy gate to a
  fail-fast-disabled macOS, Linux, and Windows matrix.
- Browser command stdout and stderr now use bounded file-backed capture, so a
  persistent daemon cannot keep an inherited pipe open and stall a successful
  command after its launcher exits. Windows browser spawns also prevent daemon
  grandchildren from retaining the calling process's capture pipes.
- Hermetic standalone-browser CI now pins Chrome Headless Shell
  147.0.7727.117 instead of downloading an unreviewed latest full-Chrome
  build, gives asynchronous browser teardown a bounded convergence window,
  and treats reaped Unix zombies as stopped during cleanup verification.
- Unix host-death watchdogs now invoke the external `kill` utility explicitly,
  avoiding shell-builtin differences when terminating negative process-group
  identifiers.
- Cancelled session opens now release their reserved name and capacity.
  Failed observations invalidate the previous observation ID, and MCP shutdown
  closes independent sessions concurrently instead of multiplying the cleanup
  deadline by the session count.
- Successful standalone browser commands now leave their session daemon alive
  for the next command. Windows emergency `taskkill` output is suppressed so
  machine-readable CLI reports remain valid JSON.
- MCP surface cleanup now runs in an owned task, so caller timeout or
  cancellation no longer cancels an already-dispatched `close`. The session
  reports retryable `cleanup_in_progress` until completion; an eventual
  retryable failure retains the exact driver in a cleanup-only state where
  `finish` or `abort` can retry without releasing its name or ownership handle.

### Safety

- CUA proxy commands on Windows are now created suspended and resume only after
  private Job assignment, closing the launch-before-containment race. In-flight
  request/notification/close cancellation immediately signals the proxy tree;
  final transport Drop performs bounded direct-child reaping, and normal
  cleanup waits for the Job or Unix process group to empty. Unix proxies also
  carry an EOF watchdog that kills the group when an uncatchable host exit
  prevents Drop from running.
- Agent sessions now publish recovery metadata before their first browser
  command. If initial navigation and exact cleanup both fail, the failed
  session and runtime remain available for `agent abort` instead of deleting
  the only PID/socket ownership evidence. Unix emergency process snapshots are
  also bounded, isolate the `ps` helper in its own process group, avoid
  back-pressured output pipes, and reap the helper tree on timeout.
- Web browser commands now enter an owned process boundary before they can
  execute. Unix retains dedicated process groups; Windows creates commands
  suspended, assigns them to kill-on-close Job Objects, and then resumes them.
  Deterministic sessions retain that boundary through close and Drop, while
  one Unix EOF watchdog per active boundary kills all recorded groups after
  abrupt host death, while empty groups are removed before PGID reuse can grant
  stale cleanup authority. Successful persistent turns explicitly disarm their
  temporary watchdog or Job; a nonzero command exit remains a failure and
  cleans the boundary before returning. Timeout, cancellation, failed startup,
  and abandoned futures terminate descendants, reap the direct child, and
  release descendant sockets without touching an independent browser tree.
  Windows PowerShell and `taskkill` fallbacks are bounded and use file-backed
  output to avoid pipe deadlock.
- Web runtime/socket directories are now canonicalized and identity-bound for
  the lifetime of each driver handle. Every command and emergency cleanup
  revalidates the binding, rejects link/reparse and same-path directory
  replacement, and refuses linked namespace components or PID sidecars. The
  persistent CLI also rejects linked runtime owner markers. Windows emergency
  cleanup now performs a bounded command-line query and requires an owned
  browser marker before invoking `taskkill`; missing or mismatched identity
  evidence terminates nothing.
- GUI transport startup now requires an absolute, existing CUA policy file,
  rejects unreviewed daemon contracts before session creation, bounds JSON-RPC
  messages, and kills a desynchronized proxy after timeout or protocol error.
- CUA proxy transport now owns the complete spawned process tree: Unix uses a
  registered process group and Windows uses a kill-on-close Job Object. Normal
  close, timeout, protocol failure, early exit, Drop, startup rollback, and the
  second-interrupt emergency path no longer leave proxy descendants behind.
- GUI cleanup re-checks the exact application identity before terminating only
  a process proven absent before launch; attached and pre-existing processes
  are never terminated. Permission checks are non-prompting and fail closed.
- GUI observations and input actions now revalidate the configured application
  identity, PID, and bound top-level window immediately before CUA dispatch.
  Runtime identity/window drift invalidates the old snapshot and fails with no
  input sent to a replacement application or window.
- GUI screenshot evidence now uses a canonical session root, component-wise
  directory preparation that rejects symbolic links and Windows reparse
  points, and post-capture plus pre-input regular-file containment checks.
- A retryable GUI app-termination or CUA session-end failure no longer closes
  the transport or marks the driver session closed before cleanup succeeds.
  Retrying cleanup rechecks the exact PID/application identity first.
- Unsupported GUI platform/endpoint combinations fail before a CUA transport
  starts. Lifecycle tests cover embedded permission attribution, PID reuse,
  dropped sessions, idempotent cleanup, and 32 repeated open/close cycles.
- The MCP server now enforces the locked `2025-06-18` initialize/version/
  initialized lifecycle, advertises only registered surfaces, and rejects
  tool calls before negotiation. GUI opening owns a cancellation guard that
  finishes ownership discovery before cleanup, even when cancellation lands
  after `launch_app` is dispatched but before its response arrives; the owned
  application and CUA session are then reaped.
- Provenance redactor configuration is bounded and fail-closed, and its
  `Debug` representation reports only the number of registered values rather
  than the values themselves.
- Browser-level domain containment now blocks page-driven cross-domain links,
  redirects, scripts, images, fetches, and related requests. Exact scheme and
  port admission remains independently enforced for explicit actions and
  observations because the admitted browser protocols expose hostname policy.
- Web evidence now uses a canonical artifact root with component-wise
  link/reparse rejection. Screenshot, download, HAR, trace, and video commands
  must produce a fresh root-contained regular file before evidence is returned;
  stale output is removed before dispatch, while active-video reconnect
  validates without deleting the in-progress file. Adapter-written JSON uses
  the same path admission and post-write validation.
- Agent sessions created before browser domain policy persistence now reject
  observation and action turns with a stable machine-readable error. They
  remain inspectable and retain `finish`/`abort` cleanup so callers can close
  the exact owned browser session before starting a contained replacement.

## 0.4.4 - 2026-08-01

### Added

- Typed `wait { visible = css(...) }` and `wait { visible = ref(...) }`
  conditions now block on stable UI structure without relying on display text.

### Fixed

- Relative upload fixture paths are now resolved against the `a3s-test`
  process working directory before dispatch to an independently launched
  browser adapter. Absolute paths remain unchanged.

## 0.4.3 - 2026-07-31

### Added

- The macOS/Linux and Windows one-click installers now default to `auto` and
  install the portable Skill only for coding agents detected from their home
  directory, environment override, or executable.
- A `universal` target installs the Skill in the cross-client
  `~/.agents/skills` convention. Explicit targets remain available for A3S
  Code, Codex, Claude Code, Cursor, Gemini CLI, GitHub Copilot CLI, OpenCode,
  Cline, Roo Code, and Windsurf.

### Fixed

- Cline installations now use its documented global `~/.cline/skills`
  directory instead of conflating Cline with the cross-agent directory.

### Safety

- Automatic installation never creates unrelated agent-specific directories.
  When no known client is detected, it falls back to the standard
  `~/.agents/skills` directory; `all` and custom directories remain explicit
  opt-ins.

## 0.4.2 - 2026-07-31

### Fixed

- Persistent agent sessions now derive a stable, 28-byte internal browser ID
  when the user-facing session name would exceed the browser daemon's Unix
  socket-path budget. Valid descriptive session names no longer fail during
  browser startup with a socket path length error.

### Safety

- Short internal browser IDs remain unchanged for compatibility. Compacted IDs
  use a SHA-256 suffix, are persisted in the ownership marker, and are
  recomputed during metadata validation so exact-session cleanup retains the
  same ownership guarantees across CLI invocations.

## 0.4.1 - 2026-07-31

### Fixed

- Context-click now moves to the resolved target and dispatches a cancelable,
  page-scoped `contextmenu` event instead of opening Chrome's native menu.
  Products without a custom context menu can therefore be observed on the
  next agent turn without a browser-command timeout.
- Agent observations validate the browser-reported URL before issuing a new
  observation identifier. Detached pages such as `about:blank` return
  `test.driver.web.session_origin_lost`, while an unapproved HTTP(S) origin
  returns `test.driver.web.navigation_origin_denied` and a malformed snapshot
  without a URL returns `test.driver.web.output_invalid`.

### Safety

- Observation-origin failures remain recorded in the workspace event log and
  leave the session available for exact-session `abort` cleanup rather than
  silently continuing in a replacement browser page.

## 0.4.0 - 2026-07-31

### Added

- Action protocol revision 2 with typed hover, focus, double-click,
  context-click, incremental type, check, uncheck, multi-value select, drag,
  modifier-aware mouse wheel, and viewport actions.
- Compact external-planner commands for every new interaction, with generated
  JSON Schema, ACL parsing, policy capability kinds, and Web capability
  reporting from the same shared action model.
- A hermetic advanced-interaction fixture, ACL example, command-mapping tests,
  failure-cleanup tests, and a real standalone agent-browser smoke workflow.

### Changed

- Context-click resolves a visible target center and performs a bounded right
  mouse down/up sequence. Drag scrolls both endpoints into view before
  dispatch.
- Mouse wheel releases held modifiers in reverse order on both success and
  failure. A target-scoped wheel emits a typed event at the resolved element;
  an untargeted wheel remains a native browser gesture.
- Advanced operations use observation refs or CSS selectors when the
  standalone browser semantic protocol cannot express the corresponding
  subaction.
- Cross-platform one-click installers and all built-in Coding Agent targets
  remain release assets and are validated against the `v0.4.0` package shape.

### Safety

- Ref provenance checks now cover every source, destination, and optional
  target in the expanded action protocol.
- Viewport dimensions, wheel deltas and modifiers, select values, and direct
  selector requirements are rejected before unsafe or ambiguous dispatch.

## 0.3.1 - 2026-07-31

### Fixed

- One-click installers now download the target checksum filename emitted by
  the release workflow (`a3s-test-<version>-<target>.sha256`).
- Offline installer fixtures now mirror the real GitHub Release asset names,
  preventing archive/checksum naming drift from passing CI.

## 0.3.0 - 2026-07-31

### Added

- Persistent external-planner sessions that let A3S Code, Codex, Claude Code,
  and compatible agents drive `start -> observe -> act -> finish` through the
  CLI without a nested LLM.
- `a3s-test agent schema` for installed-protocol discovery and generated typed
  `Action` JSON Schema.
- Compact `agent click`, `fill`, `press`, and `screenshot` commands plus the
  complete `agent act --action-json` interface.
- Workspace-local session metadata, append-only event logs, reports, and
  artifact roots.
- Coding Agent Skill guidance and a progressive Agentic CLI reference.
- Checksum-verifying macOS/Linux and Windows installers with built-in targets
  for A3S Code, Codex, Claude Code, Cursor, Gemini CLI, GitHub Copilot,
  OpenCode, Cline, Roo Code, and Windsurf, plus a custom Skill directory.
- Linux ARM64 release archives alongside Linux x64, macOS ARM64/x64, and
  Windows x64 builds.

### Changed

- Semantic JSON targets now use explicit object fields such as
  `{"type":"label","value":"Email"}`, fixing missing values in the generated
  schema.
- The README and architecture documentation now present the CLI as the
  product boundary, the coding agent as the primary planner, and the embedded
  `LlmProvider` loop as an optional SDK path.

### Safety

- Ref targets require the identifier of the latest semantic observation.
- Observation identifiers remain monotonic across state-changing actions.
- Explicit URL-bearing actions are limited to the initial HTTP(S) origin and
  `--allow-origin` values.
- Persistent browser sessions use private runtime directories, isolated
  namespaces, ownership markers, bounded idle timeouts, and exact-session
  `finish` or `abort` cleanup.

## 0.2.0 - 2026-07-31

### Added

- Browser capability discovery with typed integration, semantic version
  admission, protocol revision, and feature inventory.
- Typed ACL actions for tabs, frames, dialogs, uploads, artifact-scoped
  downloads, network route mocks, and route cleanup.
- HAR, Chrome trace, WebM video, accessibility-tree, console, and page-error
  evidence.
- Infrastructure-only retry policy with bounded retry count and backoff.
- Bounded parallel scenario execution with manifest-order reports.
- `a3s-test capabilities` for human and JSON protocol inspection.
- Repository-owned `$a3s-test` Coding Agent Skill with a progressive Web ACL
  reference.
- Multi-platform GitHub Release workflow with CLI archives, SHA-256 checksums,
  and a downloadable Skill package.

### Changed

- Step results now include an `attempts` field.
- The Web driver verifies A3S Browser `>= 0.1.1, < 0.2.0` or standalone
  agent-browser `>= 0.26.0, < 0.27.0` before opening a browser session.
- Browser evidence media types are inferred from their artifact format.

### Safety

- Only a pre-dispatch unavailable-command error is retryable. Browser command
  timeouts, output failures, non-zero action exits, and assertions are not
  retried.
- Parallel scenarios default to one and are capped at 64.
- All generated evidence and downloads remain inside the isolated scenario
  artifact root.

## 0.1.0 - 2026-07-30

- Initial typed ACL, runner, A3S Browser adapter, JSON CLI, lifecycle
  supervision, and schema-constrained agentic SDK loop.
