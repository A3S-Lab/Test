# Changelog

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
