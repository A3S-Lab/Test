<p align="center">
  <img src="./assets/readme/hero.svg" width="100%" alt="A3S Test turns fresh interface context into typed actions and inspectable evidence">
</p>

<p align="center">
  <a href="https://github.com/A3S-Lab/Test/releases/latest"><img src="https://img.shields.io/github/v/release/A3S-Lab/Test?style=flat-square&color=1264ff&label=release" alt="Latest release"></a>
  <a href="https://github.com/A3S-Lab/Test/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/A3S-Lab/Test/ci.yml?branch=main&style=flat-square&label=CI" alt="CI status"></a>
  <a href="https://a3s-lab.github.io/Test/"><img src="https://img.shields.io/badge/docs-%E4%B8%AD%E6%96%87%20%7C%20English-1264ff?style=flat-square" alt="Chinese and English documentation"></a>
  <img src="https://img.shields.io/badge/Rust-1.85%2B-56657b?style=flat-square" alt="Rust 1.85 or newer">
  <a href="./LICENSE"><img src="https://img.shields.io/badge/license-MIT-56657b?style=flat-square" alt="MIT License"></a>
</p>

<h3 align="center">Ground vibe coding in the interface that actually rendered.</h3>

<p align="center">
  A3S Test gives coding agents fresh browser facts, admits one typed action at a time,<br>
  maps visible nodes back to source, and preserves proven paths as deterministic ACL regressions.
</p>

<p align="center">
  <a href="https://a3s-lab.github.io/Test/"><strong>中文文档</strong></a> ·
  <a href="https://a3s-lab.github.io/Test/en/"><strong>English</strong></a> ·
  <a href="#install">Install</a> ·
  <a href="#run-the-shortest-trustworthy-loop">Quick start</a> ·
  <a href="#add-rendered-page-context">Test Kit</a> ·
  <a href="#how-it-works">Architecture</a>
</p>

## Why A3S Test exists

Fast coding is useful only when the agent can answer four questions reliably:

1. What did the current interface actually render?
2. Which action is valid in that exact state?
3. Which source code owns the visible result?
4. What evidence proves the change worked, and what regression will detect drift?

Ordinary browser automation answers only part of this. Screenshots lose DOM
semantics and source ownership. Raw DOM dumps lose layout and revision context.
Natural-language actions are ambiguous. A passing exploratory session is not a
repeatable regression.

A3S Test closes those gaps with one evidence boundary:

| Fundamental risk                                           | A3S Test mechanism                                                                  |
| ---------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| The model acts on an old or imagined page                  | Fresh browser observations and monotonic page revisions                             |
| A target moves, disappears, or changes identity            | Observation-bound `@eN` and `@cN` refs that expire with the page                    |
| Natural language reaches the browser unchecked             | Typed actions admitted through schema, capability, policy, and origin checks        |
| The visible defect is disconnected from its implementation | Component boundaries and ranked rendered-node source spans                          |
| A suggestion silently becomes a source edit                | Separate browser-fact, model-advice, human-authorization, and mutation authorities  |
| A successful exploration cannot be repeated                | The same action and evidence contracts can be preserved as deterministic ACL suites |

## The shortest trustworthy loop

```text
intent
  -> fresh rendered facts
  -> smallest observable difference
  -> owning source span
  -> explicit repair authority
  -> fresh affected evidence
  -> deterministic ACL regression
```

The loop is intentionally narrow:

1. Declare an observable goal instead of a vague instruction.
2. Read the current browser revision after rendering.
3. Choose one typed action or one explicit difference.
4. Use Test Kit source mapping when the visible node must lead back to code.
5. Require a person to submit the repair scope before workspace mutation.
6. Verify the result from a newer page revision with owned evidence.
7. Preserve the smallest proven path as ACL for local and CI runs.

## Install

### CLI and Agent Skill

The release installer downloads the matching CLI archive, verifies its
SHA-256, and installs the same portable A3S Test Skill for detected coding
agents.

macOS or Linux:

```bash
curl -fsSL https://github.com/A3S-Lab/Test/releases/latest/download/install.sh | sh
```

Windows PowerShell:

```powershell
& ([scriptblock]::Create((irm 'https://github.com/A3S-Lab/Test/releases/latest/download/install.ps1')))
```

Pin the published release for reproducible environments:

```bash
curl -fsSL https://github.com/A3S-Lab/Test/releases/latest/download/install.sh |
  sh -s -- --version v1.0.0
```

```powershell
& ([scriptblock]::Create((irm 'https://github.com/A3S-Lab/Test/releases/latest/download/install.ps1'))) -Version v1.0.0
```

### Web Test Kit

Install Test Kit only when a page needs component ownership, source mapping,
rendered geometry, or the in-page review surface:

```bash
npm install --save-dev @a3s-lab/testkit@0.5.0
npm ls @a3s-lab/testkit
```

`@a3s-lab/testkit` 0.5.0 is published on the official npm Registry with
GitHub OIDC provenance. Its version advances independently from the CLI
release.

See the [installation guide](https://a3s-lab.github.io/Test/guide/installation.html)
for package-manager commands, agent targets, custom destinations, and build
from source.

## Run the shortest trustworthy loop

Start a persistent Web session with a goal the page can prove:

```bash
a3s-test agent start http://127.0.0.1:3000/checkout \
  --session checkout \
  --goal "Complete checkout with the fixture account" \
  --success "The confirmation heading is visible" \
  --json

a3s-test agent observe --session checkout --interactive --json
```

The observation returns a fresh generation and semantic refs:

```text
observation_id: 1
@e1 [button] Continue
```

Bind one action to that observation, then capture evidence and finish:

```bash
a3s-test agent click @e1 \
  --session checkout \
  --observation 1 \
  --json

a3s-test agent screenshot screenshots/confirmation.png \
  --session checkout \
  --json

a3s-test agent finish \
  --session checkout \
  --status passed \
  --summary "Checkout completed and confirmation was observed" \
  --json
```

The session remains inspectable after the browser closes:

```text
.a3s-test/agent-sessions/checkout/
├── session.json
├── events.jsonl
├── report.json
└── artifacts/
    └── screenshots/confirmation.png
```

`events.jsonl` explains every admitted step. `report.json` stores the terminal
result. `artifacts/` contains only evidence owned by this session.

[Continue through the quick start](https://a3s-lab.github.io/Test/guide/)

## Add rendered page context

The React integration needs two components. Keep both explicitly disabled in
production:

```tsx
import { A3SReviewOverlay, A3STestKit } from "@a3s-lab/testkit/react";

const testKitEnabled = import.meta.env.DEV;

<A3STestKit enabled={testKitEnabled} page={{ id: "app" }}>
  <App />
  <A3SReviewOverlay enabled={testKitEnabled} locale="auto" />
</A3STestKit>;
```

This adds two independent layers:

- The headless Context Runtime publishes bounded, revisioned facts after the
  browser has computed DOM, accessibility, layout, scrolling, and viewports.
- The optional Review Overlay lets a person point, multi-select, draw a region,
  sketch an intended UI, or attach a browser-page crop in one right-side panel.

Test Kit does not need a drawing SDK, screen-sharing permission, framework
private state, workspace credential, or source-editing capability. The page
bridge is non-enumerable and Symbol-addressed; private node IDs stay in a
`WeakMap`; observer and navigation signals advance the page revision.

When the CLI from `main` is used for the staged local review loop, it validates
the live `a3s.test.testkit-handshake/1` before reporting ready:

```bash
a3s-test init
a3s-test doctor
a3s-test dev --json
```

These project-loop commands are recorded under [Unreleased](CHANGELOG.md) and
are not part of the published v1.0.0 binary.

### Map a rendered node to source

Add a coarse component owner only where it helps:

```tsx
import { A3STestBoundary } from "@a3s-lab/testkit/react";

<A3STestBoundary
  id="checkout-form"
  name="Checkout form"
  source={{ file: "src/Checkout.tsx" }}
>
  <Checkout />
</A3STestBoundary>;
```

Framework adapters can call `registerSource` for an exact DOM owner and
`registerSourceMap` for an explicitly supplied Source Map v3. The resulting
`a3s.test.source-mapping/1` record keeps ranked spans, confidence, origin, and
`exact` or `ancestor` relation. A source span is navigation evidence, never
permission to read or edit a file.

[Read the Test Kit guide](https://a3s-lab.github.io/Test/guide/testkit.html)

## Preserve the proven path as ACL

Agent sessions discover unknown paths. ACL repeats paths whose actions, waits,
and success conditions are already explicit:

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
            stable_for_ms = 300
            sample_interval_ms = 50
        }

        screenshot "evidence" {
            path = "home.png"
        }
    }
}
```

```bash
a3s-test check tests/e2e/smoke.acl --json
a3s-test run tests/e2e/smoke.acl --json
```

The runner retries no ordinary action. Only an explicitly sampled read-only
assertion repeats inside its bounded stability window. A later false sample
fails instead of hiding flicker or an optimistic rollback.

## How it works

```text
PRD / design / human intent
             |
             v
      reviewed expectation
             |
browser render + accessibility + Test Kit Page Context
             |
             v
   revision-bound observation
             |
      typed Action admission
             |
 Web / GUI / TUI surface driver
             |
             v
 events + assertions + owned artifacts
             |
     human-authorized repair
             |
             v
 fresh verification -> ACL regression
```

| Layer                   | Technical responsibility                                                                                                         |
| ----------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| TypeScript Test Kit     | Derive bounded post-render context, component and source ownership, multi-space geometry, and optional human review              |
| Rust Core               | Define `Action`, observation, session, policy, result, evidence, and lifecycle contracts without depending on a concrete surface |
| Surface adapters        | Own Web browser, macOS GUI, or PTY execution and translate only supported typed actions                                          |
| Session and evidence    | Bind refs to observations, retain append-only events, validate artifact paths, and report distinguishable failures               |
| ACL compiler and runner | Admit a closed manifest before execution and repeat only explicit actions, waits, assertions, and evidence steps                 |

The browser remains the source of rendered facts. Models can propose
provenance-bound candidates, but they cannot set a verdict or authorize a
workspace mutation. Every launched program belongs to an owned process tree;
timeouts and cancellation reap that tree without closing unrelated developer
sessions.

[Read the architecture](https://a3s-lab.github.io/Test/concepts/architecture.html)

## Surface support

| Surface | Current boundary                                                                                |
| ------- | ----------------------------------------------------------------------------------------------- |
| Web     | Persistent agent sessions and ACL through A3S Browser or a compatible standalone browser        |
| GUI     | macOS CUA integration verified on a real arm64 host; other desktop backends remain under review |
| TUI     | ACL suites through owned PTY / ConPTY process trees with bounded terminal semantics             |

All surfaces share Core action, policy, evidence, result, and cleanup contracts.
Each adapter still owns perception and execution, and unsupported behavior
fails closed instead of being approximated.

## Documentation

- [Start with one Web test](https://a3s-lab.github.io/Test/guide/)
- [Install only the parts you need](https://a3s-lab.github.io/Test/guide/installation.html)
- [Add Web Test Kit](https://a3s-lab.github.io/Test/guide/testkit.html)
- [Understand Page Context](https://a3s-lab.github.io/Test/concepts/page-context.html)
- [Compare exploration and ACL](https://a3s-lab.github.io/Test/guide/workflows.html)
- [Review actions and evidence](https://a3s-lab.github.io/Test/guide/actions-and-evidence.html)
- [Inspect every capability](https://a3s-lab.github.io/Test/reference/capabilities.html)

Repository-level references remain in [`docs/`](docs/), including the full
[architecture](docs/architecture.md), [Test Kit protocol](docs/testkit.md),
[agentic workflow](docs/agentic.md), and [specification](docs/specification.md).

## Development

Run checks from this repository, not from the parent monorepo:

```bash
cargo fmt --all -- --check
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Validate Test Kit and the documentation website with their package-local
scripts:

```bash
npm --prefix packages/testkit test
npm --prefix website run check
npm --prefix website run build
npm --prefix website run check:site
```

## License

A3S Test and `@a3s-lab/testkit` are licensed under the [MIT License](LICENSE).
