# A3S Test

A3S Test is an agent-ready end-to-end test runtime for Web, GUI, and TUI
products. It gives coding agents and CI systems one typed scenario model,
one evidence model, and one result format while keeping platform automation
behind replaceable drivers.

The first working slice runs deterministic Web scenarios through A3S Browser
or a standalone `agent-browser` compatible executable. It also provides a
library-level, schema-constrained LLM loop over the same Web observations and
typed actions. GUI and TUI drivers share the core contract and remain planned.

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
bounded cleanup, and cancellation, while CLI, MCP, and Skill projections remain
on the roadmap. See the [Agentic SDK contract](docs/agentic.md) for the host
sequence, request/response fields, budgets, policies, and result semantics.

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
```

## Development

```bash
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 test --workspace --all-targets --locked
cargo +1.85.0 clippy --workspace --all-targets --locked -- -D warnings
```

The project is licensed under the [MIT License](LICENSE).
