# A3S Test

A3S Test is an agent-ready end-to-end test runtime for Web, GUI, and TUI
products. It gives coding agents and CI systems one typed scenario model,
one evidence model, and one result format while keeping platform automation
behind replaceable drivers.

The v0.2 Web runtime runs deterministic scenarios through A3S Browser or a
standalone `agent-browser` compatible executable. It covers browser protocol
admission, tabs, frames, dialogs, file transfer, network mocks, HAR, trace,
video, accessibility, console evidence, bounded concurrency, and
infrastructure-only retries. A repository-owned Coding Agent Skill projects
the same CLI and ACL contracts. GUI and TUI drivers remain planned.

## Why A3S Test

- **Agent-ready:** ACL manifests and JSON results are compact, typed, and easy
  for coding agents to generate, inspect, and repair.
- **Cross-surface:** Web, GUI, and TUI are capabilities behind one
  `SurfaceDriver` interface, not separate test products.
- **Lifecycle-safe:** cancellation, deadlines, cleanup deadlines, process
  groups, isolated browser namespaces, and browser idle shutdown are part of
  the runtime contract.
- **Deterministic core:** explicit test actions execute without natural-language
  guessing. Agentic exploration calls an injected LLM provider, validates its
  schema-constrained action, and never falls back to keyword routing.
- **Evidence-first:** step output and artifacts are structured for both human
  review and automated diagnosis.
- **Protocol-admitted:** the driver verifies a known A3S Browser or
  agent-browser version before a session can launch.
- **Resource-bounded:** scenario parallelism, retry count, command deadlines,
  cleanup deadlines, and browser idle time all have explicit limits.

## Architecture

```text
       Coding Agent / CI / Developer
              /                 \
     deterministic CLI       agentic SDK host
              |                 |
     ACL -> validated IR    goal -> LLM -> policy
              \                 /
               Runner / owning host
                        |
       SurfaceDriver -> DriverSession
        /          |          \
       /           |           \
 Web Driver    GUI Driver    TUI Driver
 A3S Browser   A3S CUA       PTY + semantic
 (working)     (planned)     terminal model
       \           |           /
        \          |          /
       evidence + structured report
```

See [Architecture](docs/architecture.md) for ownership boundaries, lifecycle
states, the LLM path, and the planned GUI/TUI adapters.

## Quick start

Check the installed browser protocol without launching Chrome:

```bash
cargo run -p a3s-test-cli -- capabilities --json
```

Validate a suite without launching a browser:

```bash
cargo run -p a3s-test-cli -- check examples/web-smoke.acl
```

Run it through A3S Browser:

```bash
cargo run -p a3s-test-cli -- run examples/web-smoke.acl
```

Run it through a standalone compatible driver:

```bash
cargo run -p a3s-test-cli -- run examples/web-smoke.acl \
  --browser-driver standalone \
  --browser-executable agent-browser
```

Coding agents should request JSON:

```bash
a3s-test check tests/smoke.acl --json
a3s-test run tests/smoke.acl --json
```

Stable process exit codes are `0` for passed, `1` for failed, `124` for timed
out, `130` for cancelled, and `2` for invalid invocation or configuration.

Prebuilt CLI archives and checksums are published on the
[Releases](https://github.com/A3S-Lab/Test/releases) page.

## ACL test suite

```acl
suite "office-smoke" {
    version = 1

    scenario "word-editor" {
        name = "Open the Word editor"
        surface = "web"
        timeout_ms = 30000

        navigate "open-playground" {
            url = "https://example.test/playground"
        }

        click "choose-word" {
            target = role("button", "Word")
        }

        wait "editor-ready" {
            load = "networkidle"
        }

        expect "title-visible" {
            text = "Word"
        }

        screenshot "final-state" {
            path = "word/final.png"
        }
    }
}
```

The parser is closed by default: unknown blocks, attributes, ambiguous
conditions, duplicate identifiers, and invalid typed locators fail before a
surface is launched. See the [ACL specification](docs/specification.md).

## Web depth

The typed Web protocol includes:

- navigation, snapshots, semantic locators, keyboard input, typed waits, and
  assertions;
- stable tab IDs and labels, frame context, and browser dialogs;
- uploads, artifact-scoped downloads, network route mocks, and HAR capture;
- screenshots, accessibility snapshots, console logs, page errors, traces, and
  video evidence.

`a3s-test run` retries only errors explicitly marked safe because the browser
command was not dispatched. It does not retry assertions, command timeouts, or
ambiguous clicks and submissions. `--max-parallel-scenarios` defaults to `1`
and is bounded to `64`.

## Coding Agent Skill

The release includes
[`a3s-test.skill`](https://github.com/A3S-Lab/Test/releases/latest/download/a3s-test.skill),
a ZIP-compatible package containing the `$a3s-test` Coding Agent Skill. It
teaches coding agents to discover the installed Web protocol, author closed ACL
manifests, run JSON-first tests, collect bounded evidence, and diagnose stable
error codes.

Install the release asset for Codex:

```bash
mkdir -p "${CODEX_HOME:-$HOME/.codex}/skills"
unzip a3s-test.skill -d "${CODEX_HOME:-$HOME/.codex}/skills"
```

The source lives in [`skills/a3s-test`](skills/a3s-test). The Skill does not
introduce a natural-language router; it drives the same typed CLI used by CI.

## LLM-driven execution

`a3s-test-agent` implements a bounded observe-decide-act loop for SDK hosts.
The host injects a real `LlmProvider` and an `ActionPolicy` as typed objects.
Every model request carries the JSON Schema for `AgentDecision`; every response
is parsed again locally and checked against action capabilities and navigation
origins before surface execution.

The result records provider and model identity, prompt version, token and cost
usage, model latency, request IDs, and SHA-256 decision digests. Turn, token,
cost, context-size, and wall-clock budgets are mandatory. The production
library has no scripted, keyword, or heuristic model substitute.

This path is currently a library API. A host still owns surface opening,
bounded cleanup, and cancellation. The shipped Skill drives deterministic ACL
execution; a direct agentic CLI and MCP projection remain on the roadmap. See
the [Agentic SDK contract](docs/agentic.md) for the host sequence,
request/response fields, budgets, policies, and result semantics.

## Interrupt behavior

The first `Ctrl+C` requests cancellation and waits for bounded surface cleanup.
A second `Ctrl+C` terminates all command process groups still owned by the
runner and exits with code `130`. Browser sessions also use a per-run namespace
and a private runtime directory for each scenario. If protocol-level `close`
stops responding, the runner validates the exact scenario session PID file and
kills only that daemon and its descendant process groups. A bounded idle
timeout remains the final fallback, so an interrupted test cannot become an
unbounded resident daemon.

This behavior is covered by end-to-end tests that start real child and
grandchild processes, send SIGINT, and verify they are reaped.

## Workspace

```text
crates/
├── a3s-test-agent/       # Schema-constrained LLM loop and action policy
├── a3s-test-core/        # Typed suite IR and surface contracts
├── a3s-test-runner/      # Deadlines, cancellation, cleanup, and reports
├── a3s-test-driver-web/  # A3S Browser / agent-browser adapter
└── a3s-test-cli/         # Deterministic human and coding-agent entry point

skills/
└── a3s-test/             # Coding Agent Skill and progressive ACL reference
```

## Development

```bash
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 test --workspace --all-targets --locked
cargo +1.85.0 clippy --workspace --all-targets --locked -- -D warnings
```

The project is licensed under the [MIT License](LICENSE).
