# A3S Test

A3S Test is an agent-ready end-to-end test runtime for Web, GUI, and TUI
products. It gives coding agents and CI systems one typed scenario model,
one evidence model, and one result format while keeping platform automation
behind replaceable drivers.

The first working slice runs deterministic Web scenarios through A3S Browser
or a standalone `agent-browser` compatible executable. GUI and TUI drivers
share the same core contract and are planned without coupling their native
details to the runner.

## Why A3S Test

- **Agent-ready:** ACL manifests and JSON results are compact, typed, and easy
  for coding agents to generate, inspect, and repair.
- **Cross-surface:** Web, GUI, and TUI are capabilities behind one
  `SurfaceDriver` interface, not separate test products.
- **Lifecycle-safe:** cancellation, deadlines, cleanup deadlines, process
  groups, isolated browser namespaces, and browser idle shutdown are part of
  the runtime contract.
- **Deterministic core:** explicit test actions execute without natural-language
  guessing. Agentic exploration will use a real LLM provider that returns
  validated typed actions; it will not use keyword routing.
- **Evidence-first:** step output and artifacts are structured for both human
  review and automated diagnosis.

## Architecture

```text
 Coding Agent / CI / Developer
              |
        CLI + JSON output
              |
        ACL admission layer
      parse -> validate -> typed IR
              |
       cancellation-safe Runner
      deadline | events | results
              |
       SurfaceDriver contract
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
states, the LLM extension path, and the planned GUI/TUI adapters.

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
├── a3s-test-core/        # Typed suite IR and surface contracts
├── a3s-test-runner/      # Deadlines, cancellation, cleanup, and reports
├── a3s-test-driver-web/  # A3S Browser / agent-browser adapter
└── a3s-test-cli/         # Human and coding-agent entry point
```

## Development

```bash
cargo fmt --all -- --check
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
```

The project is licensed under the [MIT License](LICENSE).
