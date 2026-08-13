<p align="center">
  <img src="./assets/readme/hero.svg" width="100%" alt="A3S Test turns coding-agent observations and typed ACL suites into policy-checked actions, evidence, and reports">
</p>

<p align="center">
  <a href="https://github.com/A3S-Lab/Test/releases/latest"><img src="https://img.shields.io/github/v/release/A3S-Lab/Test?style=flat-square&color=6ee7b7&label=release" alt="Latest release"></a>
  <a href="https://github.com/A3S-Lab/Test/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/A3S-Lab/Test/ci.yml?branch=main&style=flat-square&label=CI" alt="CI status"></a>
  <img src="https://img.shields.io/badge/Rust-1.85%2B-303846?style=flat-square" alt="Rust 1.85 or newer">
  <a href="./LICENSE"><img src="https://img.shields.io/badge/license-MIT-303846?style=flat-square" alt="MIT License"></a>
</p>

<h3 align="center">Explore unknown paths with a coding agent. Lock proven paths into typed regressions.</h3>

<p align="center">
  A3S Test is an AI-native test engine delivered as the <code>a3s-test</code> CLI.<br>
  Agent sessions and deterministic ACL suites share one action model, evidence format, and cleanup contract.
</p>

<p align="center">
  <a href="#from-a-goal-to-inspectable-evidence">See the loop</a> ·
  <a href="#install">Install</a> ·
  <a href="#turn-a-proven-path-into-a-regression">Write a regression</a> ·
  <a href="#surfaces">Check surfaces</a> ·
  <a href="#documentation">Read the contracts</a>
</p>

In the primary workflow, the coding agent remains the planner. A3S Code,
Codex, Claude Code, or another Agent Skills-compatible tool observes the
surface and chooses one action at a time; A3S Test validates that action,
keeps the surface alive, records the turn, and closes only the runtime it
owns. There is no nested model, keyword router, or second hidden test engine
in this workflow. Deployments that intentionally want a one-shot embedded
planner can instead use `a3s-test agent run` with ACL, a deployment-supplied
HTTP LLM provider, and deterministic local verification.

Web testing is available through
[A3S Browser](https://github.com/A3S-Lab/Browser) or a compatible standalone
`agent-browser`. GUI testing is contract-tested on macOS through A3S CUA. The
TUI driver remains planned.

## From a goal to inspectable evidence

Start a persistent Web session against a local product and state what success
must look like:

```bash
a3s-test agent start http://127.0.0.1:3000/checkout \
  --session checkout \
  --goal "Complete checkout with the fixture account" \
  --success "The confirmation heading is visible" \
  --json
```

Observe the semantic UI, decide from that state, then execute one typed action:

```bash
a3s-test agent observe --session checkout --interactive --json
```

The repository's CLI integration fixture exercises the same contract and
returns a fresh observation plus an actionable semantic ref:

```text
observation_id: 1
@e1 [button] Continue
```

Bind ref-based actions to that observation, observe again after every
state-changing turn, and finish only when the evidence proves the result:

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

Each workspace keeps an append-only execution record:

```text
.a3s-test/agent-sessions/checkout/
├── session.json
├── events.jsonl
├── report.json
└── artifacts/
    └── screenshots/confirmation.png
```

`agent open` aliases `agent start`, and `agent snapshot` aliases
`agent observe`. Compact commands cover common browser turns; `agent act
--action-json` exposes the complete generated action schema.

### Run one workflow with a deployment planner

The direct host keeps ACL as the only product configuration language and owns
the complete Web lifecycle:

```bash
a3s-test provider schema llm
a3s-test agent run examples/agent-web.acl --json
```

The ACL declares the initial URL, observable goal, exact action capability
set, origin and network scope, turn/token/cost/context/time budgets, provider
endpoint, and read-only verification. The provider can propose one typed
action or finish/fail; it cannot determine the verdict, claim browser
observation, or authorize repair. A model finish succeeds only after at least
one local `expect` passes and the exact browser session closes successfully.

Complete, redacted reports are atomically written even when surface opening
fails:

```text
.a3s-test/agent-runs/<run-id>/
├── report.json
└── artifacts/
```

The workflow deadline covers surface open, initial navigation, model turns,
actions, Test Kit revision checks, and deterministic verification. Cleanup
uses a separate bounded deadline so timed-out work still retains exact process
reaping responsibility. See the [agentic contract](docs/agentic.md) and
[agent-run ACL specification](docs/specification.md#agent-run-configuration).

### Embed page context and human repair review

Development frontends can embed [`@a3s-lab/testkit`](packages/testkit) to
publish bounded component/source hints, semantic locators, and element geometry
in viewport, document, and normalized coordinates. Its Shadow DOM overlay lets
reviewers mark one element or an ordered batch, add repair instructions, and
send the findings to the coding agent through the owning A3S Test session.
Layout Mode adds typed component placement and section rearrangement with
viewport CSS-pixel targets while leaving the host DOM unchanged.

Deterministic Surface Contracts compare that observed context with reviewed
expectations derived from a PRD, design reference, manual decision, or official
documentation. Blocking differences fail the suite; advisory differences stay
in the report without blocking. A compatible overlay receives the report as a
review candidate, where a human can confirm the target and authorize one or a
batch of repairs. Report projection is optional and can never change the
runner verdict.

SDK hosts can inject a typed contract-generation provider from
`a3s-test-agent`. PRD candidates retain exact UTF-8 byte spans, confidence, and
unresolved product decisions; design candidates retain the source image digest,
coordinate space, regions, hierarchy, and confidence. Conflicting sources stay
explicit until a reviewer selects candidates. Only that reviewed selection is
rendered as the existing ACL Surface Contract, with no parallel DSL and no
claim that source-derived expectations were browser-observed.

For canvas, image-only controls, remote desktops, and design references, SDK
hosts can inject a typed visual-grounding provider from `a3s-test-agent`. It
accepts an observation- and SHA-256-bound screenshot plus an explicit deadline
and cost ceiling. Screenshot bytes are rehashed before dispatch. Returned
points or boxes are strictly admitted and hit-tested
against current Test Kit geometry. A unique hit can become a current semantic
target; ambiguous or unmapped results remain advisory, image-bound candidates.
This path is an explicit request or semantic fallback, never the default Web
locator.

Persistent Web sessions can use that boundary directly without granting the
provider action authority:

```bash
a3s-test agent ground "Checkout button" \
  --session checkout \
  --observation 1 \
  --config examples/visual-grounding.acl \
  --reason no-semantic-match \
  --json
```

The command admits the ACL and credentials before connecting to the browser,
captures one PNG while the latest Test Kit revision remains stable, sends the
digest-bound bytes in the version 2 HTTP envelope, and revalidates the page
revision after provider inference. It never clicks. A unique hit may return a
current `@cN` ref; zero or multiple hits remain image-bound advisory evidence.

Both provider boundaries have stable, transport-neutral protocol identifiers
and generated JSON Schemas. Inspect the exact request, response, authority, and
safety contract before implementing a local-process or remote-service adapter:

```bash
a3s-test provider schema contract-generation
a3s-test provider schema llm
a3s-test provider schema visual-grounding
```

The protocols describe data exchange only. They do not select a backend,
bundle model weights, grant verdict authority, or authorize workspace repair.

For deployment-owned inference services, `a3s-test-agent` also provides
`HttpContractGenerationProvider` and `HttpVisualGroundingProvider`. Configure
them with a typed `HttpProviderConfig` and an explicit provider/model identity:

```rust
use std::sync::Arc;
use a3s_test_agent::{
    GroundingProviderIdentity, HttpProviderConfig, HttpProviderEndpoint,
    HttpVisualGroundingProvider, VisualGroundingService,
};

let endpoint: HttpProviderEndpoint =
    "http://127.0.0.1:8080/v1/ground".parse()?;
let transport = HttpProviderConfig::new(endpoint);
let provider = Arc::new(HttpVisualGroundingProvider::new(
    GroundingProviderIdentity {
        provider: "deployment-gateway".into(),
        model: "ui-grounding-model".into(),
    },
    transport,
)?);
let service = VisualGroundingService::new(provider, Default::default())?;
```

The visual-grounding version 2 envelope carries a logical image name plus the
admitted PNG bytes, so remote services do not need access to a client-local
filesystem path. The adapter uses one `POST application/json` version
envelope, disables redirects and environment proxies, requires HTTPS except
for explicit loopback HTTP, bounds request/response bodies, enforces both
transport and wire deadlines, and redacts configured authorization values from
remote error messages. The surrounding service still rehashes evidence and
independently admits identity, provenance, geometry, usage, and authority.

### Generate a reviewed contract from sources

The CLI provides an operational two-stage workflow for a deployment-owned
contract-generation endpoint. Generation writes a versioned JSON artifact that
contains candidates, conflicts, open decisions, provider usage, and source
digests, but no executable contract:

```acl
contract_generation "checkout" {
    max_cost_microusd = 50000

    context {
        mode = "operate"
        audience = ["customer"]
        primary_outcome = "place_order"
    }

    provider {
        name = "deployment-gateway"
        model = "interface-contract-model"
        endpoint = "https://inference.example.test/v1/contracts"
        authorization_env = "A3S_TEST_PROVIDER_AUTHORIZATION_CONTRACTS"
    }

    source "requirements" {
        kind = "prd"
        path = "./checkout.md"
        uri = "./checkout.md"
    }

    source "desktop-design" {
        kind = "design"
        path = "./checkout.png"
        uri = "./checkout.png"
        media_type = "image/png"
        width = 1440
        height = 900
    }
}
```

```bash
export A3S_TEST_PROVIDER_AUTHORIZATION_CONTRACTS='Bearer ...'
a3s-test contract generate \
  --config tests/contracts/checkout.generate.acl \
  --output tests/contracts/checkout.draft.json
```

Inspect the draft, then author explicit candidate and conflict decisions in
ACL. Candidate IDs and stable conflict IDs come from the generated artifact:

```acl
contract_review {
    reviewer = "product-owner@example.test"

    candidate "requirements:desktop:place-order" {
        action = "approve"
    }

    conflict "conflict:0123456789abcdef" {
        select = "requirements:desktop:place-order"
        rationale = "Approved product terminology"
    }
}
```

```bash
a3s-test contract review \
  --draft tests/contracts/checkout.draft.json \
  --review tests/contracts/checkout.review.acl \
  --output tests/contracts/checkout.acl \
  --audit tests/contracts/checkout.reviewed.json
```

The review command regenerates the contract locally, admits the canonical ACL,
and publishes it with the complete audit artifact. Missing decisions,
unresolved applicable conflicts or product questions, altered workflow data,
unsafe source paths, stale source bytes, or existing outputs fail closed.
Workflow artifacts carry a full-payload SHA-256 checksum for accidental or
unreviewed mutation detection, retain the generation limits and local source
manifest, and rehash every source before review. The checksum is not a digital
signature; repository review and normal access controls remain authoritative.
Credentials are accepted only through a named
`A3S_TEST_PROVIDER_AUTHORIZATION_*` environment variable and are never stored
in ACL or workflow artifacts. Source-derived expectations remain distinct from
the browser-observed accessibility and layout projection.

Install the package tarball published with every GitHub Release:

```bash
npm install https://github.com/A3S-Lab/Test/releases/latest/download/a3s-testkit.tgz
```

A3S Test captures owned before/after evidence, serializes workspace mutation
across sessions and processes, and proves an admitted ACL candidate in a fresh
browser before a repair becomes review-ready. Human acceptance is the default;
session-scoped automatic resolution must be enabled explicitly. See the
[Test Kit design and security contract](docs/testkit.md) and
[roadmap](docs/roadmap.md).

The real-browser repair matrix independently covers single and ordered batch
work, clarification, cancellation, disconnect recovery, stale refs after hot
reload, verification failure, process restart, typed Layout Mode handoff, and
fresh-browser ACL promotion.

Human-marked repairs use the owning session's append-only ledger through MCP
or the equivalent `agent repair-*` commands. One persistent mutation slot
prevents concurrent sessions or processes from editing the workspace, while
the connected coding agent remains the only planner and source editor.

## Install

The release installers select the matching CLI archive, verify its SHA-256
checksum, and install the same portable `a3s-test` Skill for the coding agent
you use. Re-run the installer to upgrade both.

### macOS and Linux

```bash
# Detect installed coding agents and install the CLI + Skill
curl -fsSL https://github.com/A3S-Lab/Test/releases/latest/download/install.sh | sh

# Or target one agent explicitly
curl -fsSL https://github.com/A3S-Lab/Test/releases/latest/download/install.sh |
  sh -s -- --agent codex
```

### Windows PowerShell

```powershell
# Detect installed coding agents and install the CLI + Skill
& ([scriptblock]::Create((irm 'https://github.com/A3S-Lab/Test/releases/latest/download/install.ps1')))

# Or target one agent explicitly
& ([scriptblock]::Create((irm 'https://github.com/A3S-Lab/Test/releases/latest/download/install.ps1'))) -Agent codex
```

<details>
<summary><strong>Agent targets and controlled installation options</strong></summary>

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

`auto` installs only for detected agents and falls back to the universal
directory when none is recognized. Use `all` for every target, or
`--skill-dir <path>` / `-SkillDir <path>` for a custom Agent Skills-compatible
directory. The installers also support `--skill-only`, `--cli-only`,
`--version`, and `--install-dir`.

Each release also publishes
[`a3s-test.skill`](https://github.com/A3S-Lab/Test/releases/latest/download/a3s-test.skill)
for manual installation. Its source lives in
[`skills/a3s-test`](skills/a3s-test).

</details>

You can instead download a prebuilt archive from
[Releases](https://github.com/A3S-Lab/Test/releases/latest), or install the
tagged Rust package:

```bash
cargo install --git https://github.com/A3S-Lab/Test \
  --tag v0.8.0 --locked a3s-test-cli
```

## Turn a proven path into a regression

Once an exploratory session reveals the stable path, express it as a closed,
typed ACL suite:

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

Validate the suite without launching a surface, then run it through the same
driver and evidence boundary used by agent sessions:

```bash
a3s-test check tests/e2e/smoke.acl --json
a3s-test run tests/e2e/smoke.acl --json
```

Admission rejects unknown blocks and attributes, duplicate identifiers,
ambiguous conditions, invalid locators, and unsafe artifact paths before a
browser launches. Runs retry only explicitly retryable infrastructure failures
that occurred before dispatch; assertions, timeouts, and ambiguous dispatched
actions are never replayed automatically.

### Verify reviewed interface expectations

Expected interface structure uses the same ACL language rather than a second
DSL. A PRD or design reference can generate a Surface Contract draft, but it
cannot claim to be a browser accessibility tree. Blocking expectations become
admissible only after authoritative human review and source digest verification.

```acl
surface_contract "checkout" {
    version = 1

    context {
        mode = "operate"
        audience = ["customer"]
        primary_outcome = "place_order"
    }

    provenance "requirements" {
        kind = "prd"
        uri = "./checkout.md"
        digest = "sha256:56ea72bad66743f4dadee9515096bb39a200bf9ca8d5669293f41912c55ec14e"
        status = "reviewed"
        confidence = 100
    }

    variant "desktop" {
        state = "ready"

        element "submit" {
            test_id = "place-order"
            role = "button"
            name = "Place order"
            severity = "blocking"

            citation "prd-submit" {
                provenance = "requirements"
                quote = "Customers can place an order."
                start = 128
                end = 157
            }
        }
    }
}
```

Reference it from a deterministic suite:

```acl
verify_contract "checkout-ready" {
    contract = "./contracts/checkout.acl"
    variant = "desktop"
    state = "ready"
}
```

The CLI loads every referenced contract, verifies its local provenance digest,
and checks each citation against the exact source byte range before opening a
browser. Reconciliation prefers test ID, component identity, role plus
accessible name, and finally role. Missing or truncated Test Kit context is
inconclusive and fails closed. See the [ACL specification](docs/specification.md)
for the complete grammar and verdict rules.

## One engine, two primary workflows

| Workflow | Planner | Best for | Interface |
| --- | --- | --- | --- |
| Agent session | The calling coding agent | Exploration, bug reproduction, UX review, unknown paths | Persistent Web CLI or GUI MCP |
| ACL suite | Closed typed manifest | Stable regressions and CI | `check` and `run` |
| Embedded agent loop | Host-injected `LlmProvider` | Products embedding A3S Test as an SDK | `a3s-test-agent` library |

All three paths use the same typed `Action`, `SurfaceDriver`, evidence, result,
and lifecycle contracts. The portable Skill is an instruction adapter around
the CLI; it is not another runner.

## Why the boundary is trustworthy

- **Fresh observations.** Semantic refs such as `@e1` carry provenance and
  require the latest `observation_id`; failed or state-changing turns
  invalidate the previous generation.
- **Typed actions.** The generated JSON Schema is authoritative. Unknown
  variants and fields fail before reaching a driver.
- **Scoped navigation and network.** Explicit URL actions stay inside the
  initial origin plus `--allow-origin` entries. Browser requests are limited
  to the admitted hostnames; `--allow-domain` expands network access without
  granting top-level navigation permission.
- **Contained evidence.** Screenshot, accessibility, console, HAR, trace,
  video, and download paths must resolve to fresh regular files inside the
  canonical session artifact root.
- **Owned cleanup.** Process groups, Windows Job Objects, private runtime
  directories, identity checks, bounded shutdown, and recovery metadata keep
  cleanup attached to the exact surface created by the test.
- **Stable automation contract.** JSON fields, business error codes, and exit
  codes stay machine-readable across interactive and deterministic runs.

Read the [Agentic CLI contract](docs/agentic.md) and
[Architecture](docs/architecture.md) for the complete policy, cancellation,
and cleanup invariants.

## Surfaces

| Surface | Status | Interface | Backing adapter |
| --- | --- | --- | --- |
| Web | Available | Persistent agent CLI, direct embedded-planner CLI, and ACL suites | A3S Browser or compatible standalone `agent-browser` |
| GUI | Contract-tested on macOS | Surface-neutral MCP agent sessions and ACL runner boundary | Locked A3S CUA `0.10.0` semantic and window-vision profiles |
| TUI | Planned | Driver contract reserved | PTY and semantic terminal model |

Inspect the reviewed GUI platform matrix without starting CUA:

```bash
a3s-test gui-certification --json
```

The macOS installed-daemon and embedded-socket profiles are contract-tested.
Windows and Linux GUI combinations currently fail closed as unsupported. A
macOS host can run `a3s-test gui-certify` to exercise real permission,
observation, evidence, and owned cleanup before enabling a worker. The `mcp`
command exposes `test_session_start`, `test_observe`, `test_act`,
`test_finish`, `test_abort`, and `test_schema` after the exact MCP `2025-06-18`
handshake.

## Web testing depth

| Concern | Available capabilities |
| --- | --- |
| Interaction | Navigate, semantic snapshots and targets, click, hover, focus, fill/type, check/select, double/context click, drag, key press, modifier wheel, viewport |
| Synchronization | Typed load, text, URL, and visibility waits; text, URL, and visibility assertions |
| Browser state | Stable tab IDs and labels, frame context, browser dialogs |
| Files | Upload fixtures and keep downloads inside the session artifact root |
| Network | Domain containment, route mocks, aborts, cleanup, and HAR capture |
| Evidence | Screenshots, accessibility trees, console logs, page errors, Chrome traces, and WebM video |
| Execution | Persistent agent turns, bounded deterministic concurrency, command deadlines, and owned cleanup |

Browser execution is headless by default. `--headed` is the explicit debugging
opt-in. Inspect the exact protocol installed on the machine with:

```bash
a3s-test capabilities --json
a3s-test agent schema
a3s-test provider schema contract-generation
a3s-test provider schema llm
a3s-test provider schema visual-grounding
```

## Architecture

<p align="center">
  <img src="./assets/readme/architecture.svg" width="100%" alt="Coding-agent sessions and deterministic ACL suites converge on the same typed A3S Test core, surface drivers, evidence ledger, and owned cleanup boundary">
</p>

The CLI and MCP server are product boundaries, not backend selectors. Browser,
desktop-perception, terminal-emulation, and LLM implementations remain typed
adapters outside the framework-independent core.

<details>
<summary><strong>Workspace map</strong></summary>

```text
crates/
├── a3s-test-cli/         # Agent sessions, deterministic runs, MCP, and CI
├── a3s-test-core/        # Typed suites, actions, observations, and surface contracts
├── a3s-test-runner/      # Deadlines, cancellation, retries, and reports
├── a3s-test-session/     # Surface-neutral long-lived session application layer
├── a3s-test-driver-gui/  # Locked MCP adapter boundary for A3S CUA
├── a3s-test-driver-web/  # A3S Browser / agent-browser adapter
└── a3s-test-agent/       # Embedded model loop, contract generation, and visual grounding

skills/
└── a3s-test/             # Portable Coding Agent Skill
```

</details>

## Results and lifecycle

Process exit codes are stable:

| Exit | Meaning |
| ---: | --- |
| `0` | Passed |
| `1` | Test or action failed |
| `2` | Invalid invocation or configuration |
| `124` | Timed out |
| `130` | Cancelled |

For deterministic runs, the first `Ctrl+C` requests cancellation and bounded
surface cleanup. A second `Ctrl+C` terminates only browser and CUA process
boundaries owned by the current process.

## Documentation

- [Architecture](docs/architecture.md) — layers, ownership, process safety,
  driver boundaries, and lifecycle state machines.
- [Agentic CLI and SDK contract](docs/agentic.md) — external-planner sessions,
  MCP tools, action provenance, policy, and embedded LLM budgets.
- [ACL specification](docs/specification.md) — the complete typed manifest
  grammar and validation rules.
- [Roadmap](docs/roadmap.md) — shipped milestones and planned TUI/distributed
  execution work.
- [Changelog](CHANGELOG.md) — release-by-release behavior and safety changes.

## Development

Run the repository gates from this workspace:

```bash
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 test --workspace --all-targets --locked
cargo +1.85.0 clippy --workspace --all-targets --locked -- -D warnings
```

## License

A3S Test is available under the [MIT License](LICENSE).
