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
`agent-browser`. The GUI adapter now provides a locked A3S CUA protocol,
semantic accessibility observations and actions, SHA-256-bound window-vision
grounding, strict window binding, MCP agent sessions, and owned-application
cleanup. The locked CUA 0.10.0 execution profiles currently support macOS;
Windows/Linux GUI execution and the TUI driver remain planned.

## Install

The installers download the matching CLI, verify its SHA-256 checksum, and
install the same portable Skill in the user-level directory for the selected
coding agent. Re-running a command replaces the previous CLI and Skill, so the
same entry point also upgrades an installation.

macOS or Linux:

```bash
# Detect installed coding agents and install the CLI + Skill
curl -fsSL https://github.com/A3S-Lab/Test/releases/latest/download/install.sh | sh

# Install for one explicit agent
curl -fsSL https://github.com/A3S-Lab/Test/releases/latest/download/install.sh |
  sh -s -- --agent codex
```

Windows PowerShell:

```powershell
# Detect installed coding agents and install the CLI + Skill
& ([scriptblock]::Create((irm 'https://github.com/A3S-Lab/Test/releases/latest/download/install.ps1')))

# Install for one explicit agent
& ([scriptblock]::Create((irm 'https://github.com/A3S-Lab/Test/releases/latest/download/install.ps1'))) -Agent codex
```

The same OS-specific script is the single installation source for every
coding agent. Pass `--agent <target>` on macOS/Linux or `-Agent <target>` on
Windows:

| Coding agent | Target | User-level Skill directory |
| --- | --- | --- |
| A3S Code | `a3s-code` | `~/.a3s/skills` |
| Codex | `codex` | `~/.codex/skills` (`CODEX_HOME` supported) |
| Claude Code | `claude-code` | `~/.claude/skills` |
| Cursor | `cursor` | `~/.cursor/skills` |
| Gemini CLI | `gemini-cli` | `~/.gemini/skills` |
| GitHub Copilot CLI | `github-copilot` | `~/.copilot/skills` |
| OpenCode | `opencode` | `~/.config/opencode/skills` |
| Cline | `cline` | `~/.cline/skills` |
| Roo Code | `roo` | `~/.roo/skills` |
| Windsurf | `windsurf` | `~/.codeium/windsurf/skills` |
| Agent Skills-compatible tools | `universal` | `~/.agents/skills` |

`auto` is the default and installs only for detected agents; if none is
recognized, it uses the universal `~/.agents/skills` convention. Use `all` for
every target, or `--skill-dir <path>` / `-SkillDir <path>` for another Agent
Skills-compatible tool. `--skill-only`, `--cli-only`, `--version`, and
`--install-dir` are available for controlled installations. The scripts
resolve the latest release through GitHub's release redirect rather than the
rate-limited API.

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
  --tag v0.5.0 --locked a3s-test-cli
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

`--allow-origin https://auth.example.test` permits explicit top-level
navigation to that exact scheme, host, and port and admits its hostname to the
browser network policy. Use `--allow-domain cdn.example.test` only for a
hostname that the page must contact; it does not add that hostname to A3S
Test's exact-origin permission for explicit `navigate` or `tab new` actions.
Session metadata created before browser domain containment remains inspectable
and can still be closed with `finish` or `abort`, but `observe` and action turns
fail with `test.session.browser_network_policy_missing`. Start a new session
instead of assuming that a running browser daemon accepted a retrofitted policy.

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

Context-click moves to the resolved element and dispatches a cancelable page
`contextmenu` event. It does not open Chrome's native menu, so the next
observation remains under A3S Test control even when the product has no custom
menu. Every observation also verifies that the browser still reports an
approved HTTP(S) origin; a detached `about:blank` session is surfaced as
`test.driver.web.session_origin_lost` instead of being reused silently.

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
  across separate CLI invocations. Human-readable session IDs remain intact in
  reports while long internal browser IDs are compacted deterministically for
  Unix socket safety.
- **Observations are semantic.** Accessibility snapshots provide compact,
  actionable refs along with the current surface state.
- **Refs have provenance.** A ref action must include the latest
  `observation_id`; stale refs are rejected before dispatch.
- **Actions are typed.** The CLI publishes the generated JSON Schema and
  rejects unknown action fields or variants.
- **Navigation remains scoped.** URL-bearing actions such as `navigate` and
  `tab new` are admitted only for the initial origin and `--allow-origin`
  values. Observations reject non-Web pages and unapproved origins reached by
  page-driven navigation.
- **Browser requests are domain-contained.** The driver prefilters page-driven
  links, redirects, scripts, images, fetches, and other requests to the initial
  and explicitly admitted hostnames. The upstream browser policy is
  hostname-based, so exact scheme and port checks remain in A3S Test's action
  and observation gates. A network-admitted hostname can still be rejected as
  a top-level page by the next observation.
- **Evidence is part of the run.** Every turn is appended to `events.jsonl`;
  screenshots, accessibility, console, HAR, trace, video, and downloads stay
  inside a canonical session artifact root. Linked/reparse descendants are
  rejected before dispatch, and browser-written files must exist as regular
  root-contained files before evidence is returned.
- **Cleanup is owned.** `finish` and `abort` close only the exact browser
  session created by this test. Runtime ownership markers prevent persisted
  metadata from redirecting cleanup to another path. The Web driver also binds
  the canonical runtime directory identity and revalidates it before every
  browser command and emergency cleanup; link/reparse or same-path directory
  replacement fails closed. Deterministic runs retain the complete command and
  browser tree until session cleanup: Unix uses owned process groups, while
  Windows creates each command suspended, assigns it to a kill-on-close Job
  Object, and only then resumes it. One EOF watchdog per active Unix boundary
  kills every recorded process group if the host disappears without running
  Drop, including an uncatchable `SIGKILL`; groups with no remaining
  descendants are removed immediately so a reused PGID cannot become cleanup
  authority. Persistent agent turns use a temporary boundary that is disarmed
  only after a successful command, so
  timeout, cancellation, and abandoned futures cannot strand reparented
  browser descendants. PID cleanup remains a fail-closed fallback: a bounded
  Windows command-line query must match an owned browser marker before
  `taskkill`, and mismatch terminates nothing. A bounded idle timeout covers
  abandoned persistent sessions. Agent start
  publishes recovery metadata before the first browser command. If both the
  initial action and its cleanup fail, the failed state and exact runtime are
  retained so `agent abort` can retry instead of deleting the only PID/socket
  ownership evidence.

Browser execution is non-interactive by default. Every browser turn explicitly
selects headless mode and enforces Chrome's headless launch argument, so a user
Browser environment or configuration cannot unexpectedly open a window.
`--headed` is the explicit debugging opt-in. On Windows, Browser command shims
and CUA proxies are also created with `CREATE_NO_WINDOW`, so they do not flash a
CMD window while tests run.

## One engine, two primary workflows

| Workflow | Planner | Best for | Interface |
| --- | --- | --- | --- |
| Agent session | A3S Code, Codex, Claude Code, or another coding agent | Exploration, bug reproduction, UX review, unknown paths | Persistent CLI (Web) or MCP stdio (GUI) |
| ACL suite | Closed typed manifest | Stable regression tests and CI | `check` and `run` |
| Embedded agent loop | Host-injected real `LlmProvider` | Products that embed A3S Test as an SDK | `a3s-test-agent` library |

The portable Skill teaches multiple coding agents to use the first two
workflows. It is an instruction adapter around the CLI, not another runner.
The embedded `AgentLoop` applies a typed provenance redactor before returning
its serializable trace: credential-shaped JSON fields and secret-bearing input
payloads are removed by default, and hosts can register exact runtime secret
values that may appear in unstructured observations or provider errors.

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
| Network | Session domain containment, route mocks, aborts, cleanup, HAR capture |
| Evidence | Screenshots, accessibility trees, console logs, page errors, Chrome traces, WebM video |
| Execution | Persistent agent turns, bounded deterministic concurrency, command deadlines, owned cleanup |

Web E2E coverage is hermetic. A test-owned loopback server chooses an available
port at runtime, serves the form and navigation fixtures, records any contact
with a second-origin sentinel, and joins its worker threads on cleanup. The
standard test gate checks the server contract without requiring Chrome; a
dedicated macOS CI job pins standalone `agent-browser` 0.26.0 and drives the
real semantic form, same-origin navigation, screenshot evidence, cross-domain
link/script/image/fetch/redirect containment, and browser runtime cleanup path.
The normal Rust formatting, test, and warning-free Clippy gate runs on macOS,
Linux, and Windows.

## GUI agentic testing

GUI testing stays behind the typed A3S CUA adapter. Application identity,
launch versus attach, the selected window, endpoint mode, policy file, and
perception profile are host configuration; an agent cannot replace them with
an arbitrary executable or capture scope during a session.

Inspect the reviewed platform matrix without starting CUA:

```bash
a3s-test gui-certification --json
```

The locked CUA 0.10.0 matrix marks installed-daemon and embedded-socket macOS
profiles as `contract_tested`. Windows and Linux combinations are
`unsupported` and fail before the transport starts. A macOS host can exercise
the real permission, observation, and cleanup path before enabling a worker:

```bash
a3s-test gui-certify \
  --gui-policy-file ./cua-policy.yaml \
  --gui-macos-bundle-id com.example.Editor \
  --gui-profile window-vision \
  --json
```

For coding-agent integration, start the surface-neutral MCP stdio projection
with the same trusted host configuration:

```bash
a3s-test mcp \
  --gui-policy-file ./cua-policy.yaml \
  --gui-macos-bundle-id com.example.Editor \
  --gui-profile window-vision
```

The MCP server exposes `test_session_start`, `test_observe`, `test_act`,
`test_finish`, `test_abort`, and `test_schema` after the exact MCP `2025-06-18`
initialize handshake. Its schema lists only surfaces registered by the host.
Semantic refs and visual image refs are valid only for the latest successful
observation; a failed observation invalidates the previous generation. Every
pixel action carries the verified screenshot as evidence. Immediately before
each observation or input dispatch, the adapter rechecks that the configured
application identity still owns the bound PID and that the bound top-level
window still belongs to it. Identity or window drift invalidates the snapshot
and fails before input. Screenshot roots are canonicalized; linked/reparse
descendants are rejected before capture, and generated or reused grounding
files are rechecked as bounded regular files inside that root. Launched
applications are killed only after their identity is rechecked again. If an
open is cancelled while the launch response is in flight, ownership discovery
finishes in the background before that cleanup runs. Attached applications are
never terminated. A caller deadline does not cancel cleanup already dispatched
to the driver; the session reports `cleanup_in_progress` until that background
operation resolves. A retryable driver failure then enters terminal
`cleanup_required`: observation and action tools are rejected, while
`test_finish` or `test_abort` can retry the same owned cleanup handle.

The CUA MCP proxy is supervised separately from the tested application. Unix
hosts place it in a registered process group coupled to an EOF watchdog, so an
uncatchable host exit still kills the proxy tree. Windows creates it suspended,
assigns it to a kill-on-close Job Object, and resumes it only after assignment,
so an eager proxy cannot launch an escaping child. Request, notification, and
close cancellation synchronously signal the owned tree; graceful close,
command timeout, protocol failure, transport drop, and the CLI emergency
interrupt path terminate the complete proxy tree, wait for descendants, and
reap the direct proxy without targeting an attached application.

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
| Cline | `unzip a3s-test.skill -d ~/.cline/skills` |
| Roo Code | `unzip a3s-test.skill -d ~/.roo/skills` |
| Windsurf | `unzip a3s-test.skill -d ~/.codeium/windsurf/skills` |
| Compatible agents | `unzip a3s-test.skill -d ~/.agents/skills` |

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
       +-- GUI: A3S CUA semantic + window-vision adapter
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
cleanup. A second `Ctrl+C` terminates only browser and CUA process boundaries
owned by the current process. Windows browser commands and CUA proxies are
protected by kill-on-close Job Objects. Browser namespaces, private runtime
directories, and per-session Jobs prevent cleanup from targeting unrelated
developer sessions.

## Workspace

```text
crates/
├── a3s-test-cli/         # CLI for agent sessions, deterministic runs, and CI
├── a3s-test-core/        # Typed suites, actions, observations, and surface contracts
├── a3s-test-runner/      # Deadlines, cancellation, retries, and reports
├── a3s-test-session/     # Surface-neutral long-lived session application layer
├── a3s-test-driver-gui/  # Locked MCP adapter boundary for A3S CUA
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
