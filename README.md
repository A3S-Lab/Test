<p align="center">
  <img src="./assets/readme/hero.svg" width="100%" alt="A3S Test is an AI-native test engine CLI that turns coding-agent decisions and typed ACL plans into browser actions and structured evidence">
</p>

<p align="center">
  <a href="https://github.com/A3S-Lab/Test/releases/latest"><img src="https://img.shields.io/github/v/release/A3S-Lab/Test?style=flat-square&color=71e6b1&label=release" alt="Latest release"></a>
  <a href="https://github.com/A3S-Lab/Test/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/A3S-Lab/Test/ci.yml?branch=main&style=flat-square&label=CI" alt="CI status"></a>
  <img src="https://img.shields.io/badge/Rust-1.85%2B-303846?style=flat-square" alt="Rust 1.85 or newer">
  <a href="./LICENSE"><img src="https://img.shields.io/badge/license-MIT-303846?style=flat-square" alt="MIT License"></a>
</p>

**A3S Test is an AI-native test engine delivered as the `a3s-test` CLI.**
A3S Code, Codex, Claude Code, developers, and CI use the same typed actions,
evidence model, reports, and cleanup contract.

For an exploratory test, the coding agent is the planner: it observes the
surface, decides one action, executes it through the CLI, and repeats. For a
known regression, the same engine runs a closed ACL suite deterministically.
There is no keyword router and no second hidden test runtime.

The Web surface is available today through
[A3S Browser](https://github.com/A3S-Lab/Browser) or a compatible standalone
`agent-browser`. GUI and TUI drivers remain planned.

## Install

The installers download the matching CLI, verify its SHA-256 checksum, and
install the same portable Skill in the user-level directory for the selected
coding agent. Re-running a command replaces the previous CLI and Skill, so the
same entry point also upgrades an installation.

macOS or Linux:

```bash
# Install for every supported coding agent
curl -fsSL https://github.com/A3S-Lab/Test/releases/latest/download/install.sh |
  sh -s -- --agent all

# Or install for one agent
curl -fsSL https://github.com/A3S-Lab/Test/releases/latest/download/install.sh |
  sh -s -- --agent codex
```

Windows PowerShell:

```powershell
# Install for every supported coding agent
& ([scriptblock]::Create((irm 'https://github.com/A3S-Lab/Test/releases/latest/download/install.ps1'))) -Agent all

# Or install for one agent
& ([scriptblock]::Create((irm 'https://github.com/A3S-Lab/Test/releases/latest/download/install.ps1'))) -Agent codex
```

Known agent targets are `a3s-code`, `codex`, `claude-code`, `cursor`,
`gemini-cli`, `github-copilot`, `opencode`, `cline`, `roo`, and `windsurf`.
Use `all` for every target, or `--skill-dir <path>` / `-SkillDir <path>` for
any other Agent Skills-compatible tool. `--skill-only`, `--cli-only`,
`--version`, and `--install-dir` are also available for controlled
installations. The scripts resolve the latest release through GitHub's release
redirect rather than the rate-limited API.

The checked-in [`install.sh`](scripts/install.sh) supports macOS and Linux
x64/ARM64. [`install.ps1`](scripts/install.ps1) supports Windows x64. Both
installers and every built-in Agent target are exercised by release fixtures;
the release workflow publishes the scripts beside the CLI archives and
`a3s-test.skill`.

You can also download an archive from
[Releases](https://github.com/A3S-Lab/Test/releases/latest), or install the
tagged Rust package manually:

```bash
cargo install --git https://github.com/A3S-Lab/Test \
  --tag v0.4.0 --locked a3s-test-cli
```

## Let a coding agent test the product

Start a persistent session with an explicit goal and observable success
criterion:

```bash
a3s-test agent start http://127.0.0.1:3000 \
  --session checkout \
  --goal "Complete checkout with the fixture account" \
  --success "The confirmation heading is visible" \
  --json
```

Observe the semantic UI, decide from the returned state, then perform one
action:

```bash
a3s-test agent observe --session checkout --interactive --json

a3s-test agent click @e3 \
  --session checkout \
  --observation 1 \
  --json
```

Observe again after every state-changing action. When the evidence proves the
result, finish and write the report:

```bash
a3s-test agent screenshot screenshots/confirmation.png \
  --session checkout --json

a3s-test agent finish \
  --session checkout \
  --status passed \
  --summary "Checkout completed and confirmation was observed" \
  --json
```

`agent open` aliases `agent start`, and `agent snapshot` aliases
`agent observe`. Compact commands cover click, hover, focus, double-click,
context-click, fill, type, check, uncheck, select, drag, key press, mouse
wheel, viewport, and screenshot turns. `agent act --action-json` exposes the
same complete typed model together with semantic targets, waits, assertions,
tabs, frames, dialogs, network controls, and evidence.

Office-grade gestures remain explicit:

```bash
a3s-test agent context-click @e8 \
  --session editor --observation 5 --json

a3s-test agent drag '#comment-1' '#comment-gutter' \
  --session editor --json

a3s-test agent wheel -120 --target '.document-canvas' \
  --modifier control --session editor --json

a3s-test agent viewport 1440 900 --scale 2 \
  --session editor --json
```

The standalone browser semantic protocol does not expose every advanced
subaction. Focus, double-click, context-click, type, uncheck, select, drag, and
target-scoped wheel therefore require an observation ref or explicit CSS
target. Basic click, hover, fill, and check continue to accept semantic
role/text/test-ID/label/placeholder targets.

Inspect the exact protocol installed on the machine:

```bash
a3s-test capabilities --json
a3s-test agent schema
```

## Why this is agentic

- **The coding agent is the planner.** A3S Code, Codex, or Claude Code reads
  each observation and chooses the next action directly. The CLI does not call
  another model for this workflow.
- **The surface stays alive between turns.** Workspace-local sessions preserve
  the browser, event history, active evidence captures, and artifact root
  across separate CLI invocations.
- **Observations are semantic.** Accessibility snapshots provide compact,
  actionable refs along with the current surface state.
- **Refs have provenance.** A ref action must include the latest
  `observation_id`; stale refs are rejected before dispatch.
- **Actions are typed.** The CLI publishes the generated JSON Schema and
  rejects unknown action fields or variants.
- **Explicit navigation is scoped.** URL-bearing actions such as `navigate`
  and `tab new` are limited to the initial origin and `--allow-origin` values.
- **Evidence is part of the run.** Every turn is appended to `events.jsonl`;
  screenshots, accessibility, console, HAR, trace, video, and downloads stay
  inside the session artifact root.
- **Cleanup is owned.** `finish` and `abort` close only the exact browser
  session created by this test. Runtime ownership markers prevent persisted
  metadata from redirecting cleanup to another path, and a bounded idle
  timeout covers abandoned sessions.

## One engine, two primary workflows

| Workflow | Planner | Best for | Interface |
| --- | --- | --- | --- |
| Agent session | A3S Code, Codex, Claude Code, or another coding agent | Exploration, bug reproduction, UX review, unknown paths | `agent start → observe → act → finish` |
| ACL suite | Closed typed manifest | Stable regression tests and CI | `check` and `run` |
| Embedded agent loop | Host-injected real `LlmProvider` | Products that embed A3S Test as an SDK | `a3s-test-agent` library |

The portable Skill teaches multiple coding agents to use the first two
workflows. It is an instruction adapter around the CLI, not another runner.

## Turn a proven path into regression coverage

Validate and run a typed ACL suite:

```bash
a3s-test check tests/e2e/smoke.acl --json
a3s-test run tests/e2e/smoke.acl --json
```

```acl
suite "product-smoke" {
    version = 1

    scenario "home-page" {
        name = "Open the home page"
        surface = "web"
        timeout_ms = 30000

        navigate "open" {
            url = "https://example.com"
        }

        wait "loaded" {
            load = "networkidle"
        }

        expect "heading" {
            text = "Example Domain"
        }

        screenshot "evidence" {
            path = "home.png"
        }
    }
}
```

The parser rejects unknown blocks, attributes, ambiguous conditions, duplicate
identifiers, unsafe artifact paths, and invalid typed locators before a
browser launches. Deterministic runs retry only a pre-dispatch infrastructure
failure; they never retry assertions, timeouts, or ambiguous dispatched
actions.

## Web testing depth

| Concern | Available capabilities |
| --- | --- |
| Interaction | Navigate, semantic snapshots, semantic and direct targets, click/hover/focus, fill/type, check/select, double/context click, drag, key press, modifier wheel, viewport |
| Synchronization | Typed load, text, and URL waits; text, URL, and visibility assertions |
| Browser state | Stable tab IDs and labels, frame context, browser dialogs |
| Files | Upload fixtures and keep downloads inside the run artifact root |
| Network | Route mocks, aborts, cleanup, HAR capture |
| Evidence | Screenshots, accessibility trees, console logs, page errors, Chrome traces, WebM video |
| Execution | Persistent agent turns, bounded deterministic concurrency, command deadlines, owned cleanup |

## Coding Agent Skill

Each release includes
[`a3s-test.skill`](https://github.com/A3S-Lab/Test/releases/latest/download/a3s-test.skill).
The one-click installers above place it in the user-level directory expected
by the selected coding agent. For a manual installation, download it once:

```bash
gh release download --repo A3S-Lab/Test --pattern a3s-test.skill
```

Install the same package for the agent you use:

| Agent | User-level install |
| --- | --- |
| A3S Code | `unzip a3s-test.skill -d ~/.a3s/skills` |
| Codex | `unzip a3s-test.skill -d "${CODEX_HOME:-$HOME/.codex}/skills"` |
| Claude Code | `unzip a3s-test.skill -d ~/.claude/skills` |
| Cursor | `unzip a3s-test.skill -d ~/.cursor/skills` |
| Gemini CLI | `unzip a3s-test.skill -d ~/.gemini/skills` |
| GitHub Copilot | `unzip a3s-test.skill -d ~/.copilot/skills` |
| OpenCode | `unzip a3s-test.skill -d ~/.config/opencode/skills` |
| Cline | `unzip a3s-test.skill -d ~/.agents/skills` |
| Roo Code | `unzip a3s-test.skill -d ~/.roo/skills` |
| Windsurf | `unzip a3s-test.skill -d ~/.codeium/windsurf/skills` |

For a project-scoped or not-yet-listed agent, pass its Skill parent directory
explicitly:

```bash
curl -fsSL https://github.com/A3S-Lab/Test/releases/latest/download/install.sh |
  sh -s -- --skill-only --skill-dir "$PWD/.agents/skills"
```

The source package lives in [`skills/a3s-test`](skills/a3s-test).

## Architecture

<p align="center">
  <img src="./assets/readme/architecture.svg" width="100%" alt="Developers and CI call the A3S Test CLI directly while coding agents use a portable Skill to run an observe-decide-act loop over the same CLI, typed runner, surface drivers, and evidence model">
</p>

The CLI is the product boundary. Interactive agent sessions and deterministic
ACL runs share typed `Action`, `SurfaceDriver`, `DriverSession`, evidence, and
lifecycle contracts. Browser, desktop-perception, terminal-emulation, and LLM
implementations remain adapters.

```text
Coding agent + Skill
       |
       | start / observe / typed action / finish
       v
  a3s-test CLI
       |
       v
SurfaceDriver -> DriverSession -> evidence
       |
       +-- Web: A3S Browser / agent-browser
       +-- GUI: A3S CUA                         planned
       +-- TUI: PTY + semantic terminal model planned
```

Read [Architecture](docs/architecture.md), the
[Agentic CLI and SDK contract](docs/agentic.md), and the
[ACL specification](docs/specification.md).

## Results and lifecycle

Agent sessions are workspace-local:

```text
.a3s-test/agent-sessions/<session>/
├── session.json
├── events.jsonl
├── report.json
└── artifacts/
```

Deterministic process exit codes remain stable:

| Exit | Meaning |
| ---: | --- |
| `0` | Passed |
| `1` | Test or action failed |
| `2` | Invalid invocation or configuration |
| `124` | Timed out |
| `130` | Cancelled |

For `run`, the first `Ctrl+C` requests cancellation and bounded surface
cleanup. A second `Ctrl+C` terminates only command process groups owned by the
current run. Browser namespaces and private runtime directories prevent
cleanup from targeting unrelated developer sessions.

## Workspace

```text
crates/
├── a3s-test-cli/         # CLI for agent sessions, deterministic runs, and CI
├── a3s-test-core/        # Typed suites, actions, observations, and surface contracts
├── a3s-test-runner/      # Deadlines, cancellation, retries, and reports
├── a3s-test-driver-web/  # A3S Browser / agent-browser adapter
└── a3s-test-agent/       # Optional schema-constrained embedded LLM loop

skills/
└── a3s-test/             # Portable Coding Agent Skill
```

## Development

```bash
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 test --workspace --all-targets --locked
cargo +1.85.0 clippy --workspace --all-targets --locked -- -D warnings
```

A3S Test is licensed under the [MIT License](LICENSE).
