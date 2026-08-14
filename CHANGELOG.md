# Changelog

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
