# Changelog

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
