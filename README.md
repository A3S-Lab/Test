<p align="center">
  <img src="./assets/readme/hero.svg" width="100%" alt="A3S Test is an AI-native test engine CLI that turns typed ACL plans into browser actions and structured evidence">
</p>

<p align="center">
  <a href="https://github.com/A3S-Lab/Test/releases/latest"><img src="https://img.shields.io/github/v/release/A3S-Lab/Test?style=flat-square&color=71e6b1&label=release" alt="Latest release"></a>
  <a href="https://github.com/A3S-Lab/Test/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/A3S-Lab/Test/ci.yml?branch=main&style=flat-square&label=CI" alt="CI status"></a>
  <img src="https://img.shields.io/badge/Rust-1.85%2B-303846?style=flat-square" alt="Rust 1.85 or newer">
  <a href="./LICENSE"><img src="https://img.shields.io/badge/license-MIT-303846?style=flat-square" alt="MIT License"></a>
</p>

**A3S Test is an AI-native test engine delivered as a CLI.** It gives
developers, CI, and coding agents one typed test protocol, one evidence model,
and one machine-readable result format.

The Web driver is available today through
[A3S Browser](https://github.com/A3S-Lab/Browser) or a compatible standalone
`agent-browser` executable. GUI and TUI use the same driver boundary but remain
planned.

## The shortest useful run

Install a prebuilt archive from
[Releases](https://github.com/A3S-Lab/Test/releases/latest), or build the CLI
from the tagged source:

```bash
cargo install --git https://github.com/A3S-Lab/Test \
  --tag v0.2.0 --locked a3s-test-cli
```

Then discover the local browser protocol, validate the suite without opening a
browser, and run it:

```bash
a3s-test capabilities --json
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
identifiers, unsafe artifact paths, and invalid typed locators before a browser
session launches.

## Why AI-native

A3S Test is built for software agents without turning test execution into a
guessing game.

- **CLI is the stable boundary.** Humans, CI, and coding agents invoke the same
  commands and receive the same exit codes and JSON fields.
- **Plans are typed.** ACL manifests compile into a closed test model instead
  of loosely interpreted natural language.
- **Observations are semantic.** Accessibility snapshots, stable targets, tabs,
  frames, dialogs, console output, and page errors give an agent inspectable
  state.
- **Evidence is part of the result.** Screenshots, downloads, HAR, traces,
  video, and structured browser diagnostics stay attached to the scenario that
  produced them.
- **LLM execution is bounded.** SDK hosts can inject a real `LlmProvider` into
  a schema-constrained observe-decide-act loop with capability, origin, turn,
  token, cost, context, and time limits.
- **Cleanup is owned.** Deadlines, process groups, isolated browser namespaces,
  bounded shutdown, and a second-interrupt emergency path are runtime
  contracts, not test-suite conventions.

The deterministic CLI never falls back to keyword routing. The agentic library
validates every model proposal against its JSON Schema and action policy before
it reaches a surface.

## What ships today

| Area | Status | Contract |
| --- | --- | --- |
| CLI and ACL admission | Available in `v0.2` | `check`, `capabilities`, `run`, stable JSON and exit codes |
| Web E2E driver | Available in `v0.2` | A3S Browser or compatible standalone `agent-browser` |
| Coding Agent Skill | Available in each release | One portable `SKILL.md` package for A3S Code, Codex, Claude Code, and compatible agents |
| Agentic SDK loop | Library API | Injected LLM provider, typed decisions, policy gates, budgets, trace metadata |
| GUI driver | Planned | A3S CUA adapter |
| TUI driver | Planned | PTY and semantic terminal model |
| MCP projection | Planned | Thin tools over the same application layer |

## Web testing depth

| Concern | Available capabilities |
| --- | --- |
| Interaction | Navigate, snapshot, semantic/CSS/ref locators, click, fill, key press, typed waits, assertions |
| Browser state | Stable tab IDs and labels, frame context, browser dialogs |
| Files | Upload fixtures and keep downloads inside the scenario artifact root |
| Network | Route mocks, aborts, cleanup, HAR capture |
| Evidence | Screenshots, accessibility trees, console logs, page errors, Chrome traces, WebM video |
| Execution | Bounded concurrency, infrastructure-only retries, command deadlines, cleanup deadlines |

`a3s-test run` retries only an explicitly safe failure that occurred before a
browser command was dispatched. It never retries assertions, timeouts, or
ambiguous clicks and submissions. Parallel scenarios default to one and are
capped at 64.

## One CLI, many coding agents

The release asset
[`a3s-test.skill`](https://github.com/A3S-Lab/Test/releases/latest/download/a3s-test.skill)
contains the portable `$a3s-test` Skill. It teaches an agent how to discover
the installed protocol, write closed ACL manifests, run JSON-first tests,
collect bounded evidence, and diagnose stable error codes.

Download it once:

```bash
gh release download --repo A3S-Lab/Test --pattern a3s-test.skill
```

Install the same package into the agent you use:

| Agent | User-level install |
| --- | --- |
| A3S Code | `unzip a3s-test.skill -d ~/.a3s/skills` |
| Codex | `unzip a3s-test.skill -d "${CODEX_HOME:-$HOME/.codex}/skills"` |
| Claude Code | `unzip a3s-test.skill -d ~/.claude/skills` |

For a project-scoped installation, extract it under `.a3s/skills`,
`.codex/skills`, or `.claude/skills`. The source package lives in
[`skills/a3s-test`](skills/a3s-test).

The Skill is an instruction adapter around the CLI. It does not contain a
second test runner, hide shell scripts, or introduce a natural-language router.

## Architecture

<p align="center">
  <img src="./assets/readme/architecture.svg" width="100%" alt="Developers and CI call the A3S Test CLI directly while coding agents use a portable Skill to invoke the same CLI; typed ACL runs through the runner and surface drivers to produce structured evidence">
</p>

The core stays independent of browsers, desktop perception, terminal
emulation, CLI presentation, and LLM providers. Public library APIs receive
typed driver and provider objects; they do not select backends with raw
strings.

```text
TestSuite
└── TestScenario [surface, deadline]
    └── TestStep [typed action]

SurfaceDriver
└── open(ScenarioContext) -> DriverSession
    ├── observe() -> SurfaceObservation
    ├── execute(TestStep) -> StepOutput
    └── close()
```

Read [Architecture](docs/architecture.md) for lifecycle ownership and planned
GUI/TUI adapters, [ACL specification](docs/specification.md) for the manifest
contract, and [Agentic SDK contract](docs/agentic.md) for the LLM loop.

## Results and interrupts

Process exit codes are stable:

| Exit | Meaning |
| ---: | --- |
| `0` | Passed |
| `1` | Failed |
| `2` | Invalid invocation or configuration |
| `124` | Timed out |
| `130` | Cancelled |

The first `Ctrl+C` requests cancellation and waits for bounded surface cleanup.
A second `Ctrl+C` terminates only command process groups owned by the current
run and exits with `130`. Browser runs use per-run namespaces and private
runtime directories, so cleanup does not target unrelated developer sessions.

## Workspace

```text
crates/
├── a3s-test-cli/         # CLI entry point for humans, CI, and agents
├── a3s-test-core/        # Typed suites and surface contracts
├── a3s-test-runner/      # Deadlines, cancellation, retries, and reports
├── a3s-test-driver-web/  # A3S Browser / agent-browser adapter
└── a3s-test-agent/       # Schema-constrained LLM loop and action policy

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
