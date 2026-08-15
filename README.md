<p align="center">
  <img src="./assets/readme/hero.svg" width="100%" alt="A3S Test connects fresh interface context to typed actions and inspectable evidence">
</p>

<p align="center">
  <a href="https://github.com/A3S-Lab/Test/releases/latest"><img src="https://img.shields.io/github/v/release/A3S-Lab/Test?style=flat-square&color=55d6a5&label=release" alt="Latest release"></a>
  <a href="https://github.com/A3S-Lab/Test/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/A3S-Lab/Test/ci.yml?branch=main&style=flat-square&label=CI" alt="CI status"></a>
  <a href="https://a3s-lab.github.io/Test/"><img src="https://img.shields.io/badge/docs-中文%20%7C%20English-303a35?style=flat-square" alt="Chinese and English documentation"></a>
  <img src="https://img.shields.io/badge/Rust-1.85%2B-303a35?style=flat-square" alt="Rust 1.85 or newer">
  <a href="./LICENSE"><img src="https://img.shields.io/badge/license-MIT-303a35?style=flat-square" alt="MIT License"></a>
</p>

<h3 align="center">Explore unknown interface paths. Preserve proven paths as typed regressions.</h3>

<p align="center">
  A3S Test is an evidence-first test engine for coding agents and deterministic ACL suites.<br>
  Web, GUI, and TUI runs share one typed action model, result format, and cleanup contract.
</p>

<p align="center">
  <a href="https://a3s-lab.github.io/Test/"><strong>Documentation</strong></a> ·
  <a href="#install">Install</a> ·
  <a href="#start-with-one-real-path">Quick start</a> ·
  <a href="#embed-rendered-page-context">Test Kit</a> ·
  <a href="#architecture">Architecture</a>
</p>

The calling coding agent remains the planner during exploration. It observes a
surface, chooses one action, and asks A3S Test to validate and execute that
action. A3S Test owns the surface lifecycle, records provenance and evidence,
and closes only the runtime it created. Once a path is stable, the same action
and evidence contracts run from a closed ACL suite in local development or CI.

## Start with one real path

Start a persistent Web session against a local product and state an observable
success condition:

```bash
a3s-test agent start http://127.0.0.1:3000/checkout \
  --session checkout \
  --goal "Complete checkout with the fixture account" \
  --success "The confirmation heading is visible" \
  --json

a3s-test agent observe --session checkout --interactive --json
```

The observation returns a new generation and actionable semantic refs:

```text
observation_id: 1
@e1 [button] Continue
```

Bind every action to the observation that produced its ref, then observe again
after state changes:

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

Each session keeps an append-only record under the workspace:

```text
.a3s-test/agent-sessions/checkout/
├── session.json
├── events.jsonl
├── report.json
└── artifacts/
    └── screenshots/confirmation.png
```

[Read the full quick start](https://a3s-lab.github.io/Test/guide/)

## Install

The release installers download the matching CLI archive, verify its SHA-256,
and install the same portable `a3s-test` Skill for detected coding agents.
Run the command again to upgrade both.

### macOS and Linux

```bash
curl -fsSL https://github.com/A3S-Lab/Test/releases/latest/download/install.sh | sh
```

### Windows PowerShell

```powershell
& ([scriptblock]::Create((irm 'https://github.com/A3S-Lab/Test/releases/latest/download/install.ps1')))
```

Target one agent or pin a release when reproducibility matters:

```bash
curl -fsSL https://github.com/A3S-Lab/Test/releases/latest/download/install.sh |
  sh -s -- --agent codex --version v0.16.2
```

```powershell
& ([scriptblock]::Create((irm 'https://github.com/A3S-Lab/Test/releases/latest/download/install.ps1'))) -Agent codex -Version v0.16.2
```

Supported targets include A3S Code, Codex, Claude Code, Cursor, Gemini CLI,
GitHub Copilot CLI, OpenCode, Cline, Roo Code, Windsurf, and the universal
Agent Skills directory. The installers also support CLI-only, Skill-only, and
custom installation directories.

You can instead use a prebuilt archive from
[Releases](https://github.com/A3S-Lab/Test/releases/latest) or build the tagged
Rust package:

```bash
cargo install --git https://github.com/A3S-Lab/Test \
  --tag v0.16.2 --locked a3s-test-cli
```

[See every installation option](https://a3s-lab.github.io/Test/guide/installation.html)

## One core, two primary workflows

| Workflow | Planner | Best for | Interface |
| --- | --- | --- | --- |
| Agent session | Calling coding agent | Exploration, bug reproduction, unknown paths, UX review | Persistent Web CLI or GUI MCP |
| ACL suite | Closed typed manifest | Stable regression, CI, cross-surface testing | `check` and `run` |
| Embedded agent loop | Host-injected `LlmProvider` | Products embedding A3S Test | `a3s-test-agent` library |

All three paths use the same typed `Action`, `SurfaceDriver`, evidence, result,
and lifecycle contracts. The portable Skill is an instruction adapter around
the CLI, not another test runner.

Turn a proven path into a regression:

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

Validate before opening a surface, then run through the same driver boundary:

```bash
a3s-test check tests/e2e/smoke.acl --json
a3s-test run tests/e2e/smoke.acl --json
```

Unknown blocks and attributes, ambiguous conditions, invalid locators, and
unsafe artifact paths fail during admission. Assertions, timeouts, and
ambiguously dispatched actions are never replayed automatically.

[Compare both workflows](https://a3s-lab.github.io/Test/guide/workflows.html)

## Embed rendered page context

Development frontends can embed `@a3s-lab/testkit` so A3S Test can understand
the rendered page without guessing from pixels alone:

```bash
npm install https://github.com/A3S-Lab/Test/releases/latest/download/a3s-testkit.tgz
```

```tsx
import {
  A3SReviewOverlay,
  A3STestBoundary,
  A3STestKit,
} from "@a3s-lab/testkit/react";

export function App() {
  return (
    <A3STestKit
      enabled={import.meta.env.DEV}
      page={{ id: "checkout" }}
      repairEndpoint="/__a3s-test/repairs"
      redact={["[data-payment-field]"]}
    >
      <A3STestBoundary
        id="checkout-form"
        name="Checkout form"
        source={{ file: "src/Checkout.tsx" }}
      >
        <Checkout />
      </A3STestBoundary>
      <A3SReviewOverlay enabled={import.meta.env.DEV} />
    </A3STestKit>
  );
}
```

After browser rendering, Test Kit publishes bounded, revisioned context:

- Accessible semantics, DOM and open Shadow DOM structure, and form state.
- Component identity, bounded source hints, and preferred semantic locators.
- Element geometry in viewport, document, and normalized coordinate spaces.
- Layout viewport, device pixel ratio, and optional visual-viewport state.
- Bounded computed styles, product facts, and explicit redaction.

Mutation, resize, scroll, viewport, and navigation signals advance the surface
revision. An unchanged page is not polled, and stale refs fail closed.

The Shadow DOM overlay lets a reviewer mark one element or an ordered batch,
add repair instructions, save drafts, and explicitly send findings to the
owning A3S Test session. Layout Mode emits typed placement or rearrangement
intent without changing host DOM. The coding agent remains the only source
editor, and A3S Test verifies admitted changes in a fresh browser before human
acceptance.

[Integrate Test Kit](https://a3s-lab.github.io/Test/guide/testkit.html)

## Generate contracts without inventing observations

PRDs, designs, and browser page context describe different facts:

| Source | Authoritative for | Never treated as |
| --- | --- | --- |
| PRD | Product intent, copy, outcomes, business constraints | Browser-observed state |
| Design | Regions, hierarchy, geometry, image digest | Accessibility semantics |
| Page context | Current rendered semantics, state, components, locators, geometry | Product intent |

A deployment-owned provider can generate cited candidates and explicit
conflicts from PRDs or design images. A person reviews those candidates before
the CLI renders a Surface Contract in ACL. The deterministic runner then
reconciles that reviewed expectation with current browser and Test Kit facts.

```bash
a3s-test contract generate \
  --config tests/contracts/checkout.generate.acl \
  --output tests/contracts/checkout.draft.json

a3s-test contract review \
  --draft tests/contracts/checkout.draft.json \
  --review tests/contracts/checkout.review.acl \
  --output tests/contracts/checkout.acl \
  --audit tests/contracts/checkout.reviewed.json
```

Optional visual grounding returns digest-bound point or box candidates and
never clicks. Optional design audit remains advisory and cannot set a verdict
or authorize repair. Provider protocols describe transport only; A3S Test
does not bundle model weights or select a backend with a raw string.

[Read the source-to-contract workflow](https://a3s-lab.github.io/Test/guide/contracts.html)

## What the runtime guarantees

- **Fresh observations.** Semantic refs carry provenance and require the
  latest `observation_id`; state-changing turns invalidate prior generations.
- **Typed actions.** Generated JSON Schema is authoritative. Unknown variants
  and fields fail before reaching a driver.
- **Scoped navigation and network.** URL actions stay inside the initial
  origin plus explicit policy exceptions.
- **Contained evidence.** Screenshots, accessibility trees, console, HAR,
  traces, video, and downloads stay inside the canonical artifact root.
- **Owned cleanup.** Process groups, Windows Jobs, private runtime directories,
  bounded shutdown, and identity checks bind cleanup to the exact test surface.
- **Stable automation results.** JSON fields, error codes, and process exit
  codes stay machine-readable across interactive and deterministic runs.
- **Separate authority layers.** Deterministic facts, model advice, human
  authorization, and workspace mutation cannot impersonate one another.

## Surfaces

| Surface | Status | Backing adapter |
| --- | --- | --- |
| Web | Available for persistent Agent sessions and ACL suites | [A3S Browser](https://github.com/A3S-Lab/Browser) or compatible standalone `agent-browser` |
| GUI | Contract-tested and release-certified on macOS | Locked A3S CUA `0.10.0` semantic and window-vision profiles |
| TUI | Available for deterministic ACL suites | Owned PTY / ConPTY process tree and bounded VT semantics |

Inspect capabilities without opening a surface:

```bash
a3s-test capabilities --json
a3s-test agent schema
a3s-test provider schema design-audit
a3s-test provider schema visual-grounding
a3s-test worker inventory
```

The macOS release certification rebuilds the locked CUA revision on a dedicated
arm64 host, verifies Accessibility and Screen Recording grants, exercises both
perception profiles, proves exact fixture cleanup, and publishes signed
`a3s.test.gui-host-certification/1` evidence. Windows and Linux GUI combinations
currently fail closed as unsupported.

## Architecture

<p align="center">
  <img src="./assets/readme/architecture.svg" width="100%" alt="Coding-agent sessions and ACL suites converge on one typed core, surface adapters, evidence ledger, and owned cleanup boundary">
</p>

The CLI and MCP server are product boundaries, not backend selectors. Browser,
desktop perception, terminal emulation, and LLM implementations remain typed
adapters outside the framework-independent core.

<details>
<summary><strong>Workspace map</strong></summary>

```text
crates/
├── a3s-test-cli/         # Sessions, local and distributed runs, MCP, CI
├── a3s-test-core/        # Typed suites, actions, observations, contracts
├── a3s-test-runner/      # Deadlines, cancellation, retries, reports
├── a3s-test-session/     # Surface-neutral long-lived session layer
├── a3s-test-worker/      # Inventory and persistent remote worker service
├── a3s-test-driver-gui/  # Locked MCP adapter for A3S CUA
├── a3s-test-driver-tui/  # Owned PTY / ConPTY and bounded VT semantics
├── a3s-test-driver-web/  # A3S Browser / agent-browser adapter
└── a3s-test-agent/       # Providers, grounding, contracts, design audit

packages/
└── testkit/              # Rendered page context and human review SDK

skills/
└── a3s-test/             # Portable coding-agent Skill
```

</details>

[Study the architecture](https://a3s-lab.github.io/Test/concepts/architecture.html)

## Advanced execution paths

| Capability | Boundary | Source documentation |
| --- | --- | --- |
| Direct embedded planner | Deployment HTTP `LlmProvider`; local verification decides the verdict | [Agentic contract](docs/agentic.md) |
| Hermetic runner | Immutable Linux/amd64 image reference with strict capability inventory | [Architecture](docs/architecture.md#hermetic-runner-and-capability-inventory) |
| Remote workers | Authenticated execution and separate digest-bound artifact protocols | [ACL specification](docs/specification.md#remote-worker-protocol) |
| Distributed suites | Immutable plans, capability pinning, leases, quarantine, report verification | [ACL specification](docs/specification.md#distributed-run-configuration) |
| Surface Contracts | Reviewed expectations with source digest and citation verification | [ACL specification](docs/specification.md#expected-surface-contracts) |

## Documentation

The Rspress site is versioned and available in Chinese by default:

- [中文文档](https://a3s-lab.github.io/Test/)
- [English documentation](https://a3s-lab.github.io/Test/en/)
- [v0.15.0 historical snapshot](https://a3s-lab.github.io/Test/v0.15.0/)

Repository-level specifications remain the source for exhaustive protocol
details:

- [Architecture](docs/architecture.md)
- [Agentic CLI and SDK contract](docs/agentic.md)
- [ACL specification](docs/specification.md)
- [Embedded Test Kit contract](docs/testkit.md)
- [Roadmap](docs/roadmap.md)
- [Changelog](CHANGELOG.md)

## Development

Run Rust gates from the repository root:

```bash
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 test --workspace --all-targets --locked
cargo +1.85.0 clippy --workspace --all-targets --locked -- -D warnings
```

Run documentation gates from `website/`:

```bash
npm ci
npm run format:check
npm run check
npm run build
npm run check:site
```

## License

A3S Test is available under the [MIT License](LICENSE).
