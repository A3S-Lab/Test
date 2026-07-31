# Changelog

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
