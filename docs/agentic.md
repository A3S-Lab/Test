# Agentic CLI and SDK Contract

## Current boundary

A3S Test supports two agentic planning boundaries over the same typed actions
and surface drivers:

1. The primary **external-planner interface** lets A3S Code, Codex, Claude
   Code, or another coding agent drive `start -> observe -> act -> finish`
   through the persistent Web CLI or the surface-neutral GUI MCP server. The
   coding agent is already the LLM; A3S Test does not call a nested model.
2. The optional **embedded planner SDK** in `a3s-test-agent` is for a host that
   intentionally injects its own `LlmProvider` and owns the surrounding
   surface lifecycle.
3. The **direct embedded-planner CLI** is that host for one bounded Web run.
   `a3s-test agent run <config.acl>` opens the surface, calls a
   deployment-supplied HTTP provider, admits each proposal locally, verifies
   success deterministically, publishes one report, and closes the exact
   surface it opened.

Both boundaries reject unknown action shapes, use typed policies, preserve
structured evidence, and must close the exact owned surface. They do not use
keyword routing.

## External-planner CLI

```text
coding agent + $a3s-test Skill
              |
              v
agent start [goal + observable success criteria]
              |
              v
agent observe -> observation_id + semantic surface state
              |
              v
coding agent returns one typed action
              |
              v
schema + stale-ref + origin validation
              |
              v
DriverSession::execute -> event log + evidence
              |
              +--------------------------> observe again
              |
              v
agent finish -> report + DriverSession::close
```

The session metadata, append-only event log, report, and evidence live under
`.a3s-test/agent-sessions/<session>/`. Browser state is preserved across CLI
invocations through an isolated namespace and private runtime directory. The
user-facing session name remains unchanged in commands and reports. Only the
driver-facing identifier is deterministically compacted when necessary to fit
the daemon's Unix socket-path budget.

The private runtime path is not trusted solely because it was persisted. On
each connection the Web driver binds its canonical directory identity, then
rechecks that identity before every browser command and cleanup attempt. Link,
reparse-point, and same-path directory replacement therefore fail closed before
dispatch. The CLI separately checks the workspace/session ownership marker and
rejects a linked runtime or linked marker when loading saved state.

Ref targets are bound to the latest observation identifier. Explicit
URL-bearing actions are limited to the initial HTTP(S) origin and
`--allow-origin` values. Each observation independently verifies the reported
page origin before returning refs. A browser page lost to `about:blank` or
another non-Web scheme returns `test.driver.web.session_origin_lost`; an
unapproved HTTP(S) origin returns
`test.driver.web.navigation_origin_denied`. The generated
`a3s-test agent schema` output is the authoritative action contract.

The browser also receives an exact-origin policy derived from the initial URL
and every `--allow-origin`. A3S Browser 0.4.x rejects links, redirects,
scripts, images, fetches, workers, popups, WebSockets, and direct reads whose
scheme, host, and effective port are not admitted. Add `--allow-domain` only
when a page needs hostname-wide network access for a CDN or API without adding
that hostname to A3S Test's exact-origin navigation permission. A page-driven
document request admitted only by a domain exception can occur, but the next
observation still rejects it unless `--allow-origin` admits the exact origin.
Standalone 0.26.x projects origins to hostnames because its protocol cannot
distinguish schemes or ports; explicit-action and observation checks retain the
exact navigation boundary.

The policy and its typed deployment mode are persisted when `agent start`
creates the browser daemon. `exact_origin_v1` identifies A3S Browser;
`hostname_v1` identifies standalone. A session file from an earlier release
has no proof of either deployment. A3S Test keeps that state readable but
rejects `observe` and action turns with
`test.session.browser_network_policy_missing`. `finish` and `abort` remain
available so the exact owned browser session and runtime can be cleaned up;
start a new session before continuing the test. A stored mode that conflicts
with the selected driver returns `test.session.browser_containment_mismatch`;
non-canonical or drifted policy lists return
`test.session.browser_network_policy_mismatch`.

Action protocol revision 7 is the current cross-surface schema. Revision 2
introduced the browser interactions needed to inspect document-style
applications: click, hover, focus, double-click, context-click,
fill, incremental type, check/uncheck, multi-value select, drag, key press,
modifier-aware wheel, viewport, synchronization, assertions, browser context,
files, network controls, and evidence. Compact CLI commands project the common
interactions; `agent act --action-json` projects the same model without a
second protocol. Revision 4 adds GUI automation-ID targets, revision 5 adds
image-bound visual points, and revision 6 adds runner-owned deterministic
surface-contract verification. Interactive action schemas omit
`verify_contract`; a planner cannot use it to bypass suite admission,
provenance digest verification, or the Runner's verdict semantics. Revision 7
adds terminal paste, resize, VT recording, and regex waits for deterministic
TUI suites. Persistent external-planner sessions do not yet register a TUI
host, so those terminal actions remain unavailable there.

Semantic role, text, test-ID, label, and placeholder targets are used whenever
the underlying browser command supports the requested subaction. Focus,
double-click, context-click, type, uncheck, select, drag, and target-scoped
wheel require a ref from the latest observation or explicit CSS selector with
the current standalone browser protocol. This is a protocol capability
boundary, not keyword routing or locator inference. Context-click dispatches a
page-scoped, cancelable `contextmenu` event at the visible target instead of
opening the browser-native menu, which is outside the observable page.

## External-planner MCP

`a3s-test mcp` exposes the same typed session application layer over MCP stdio
protocol `2025-06-18`. Clients must complete
`initialize -> notifications/initialized` with that exact version before
listing or calling tools. It currently hosts GUI sessions and publishes these
tools:

| Tool | Application operation |
| --- | --- |
| `test_session_start` | Open the host-configured surface |
| `test_observe` | Return a new observation and observation ID |
| `test_act` | Execute exactly one typed action |
| `test_finish` | Record a result and close the exact owned surface |
| `test_abort` | Abort and close the exact owned surface |
| `test_schema` | Return action protocol revision 7 and its interactive JSON Schema |

The server serializes turns within each session, bounds active sessions and
request size, advertises only registered surfaces, and closes independent
sessions concurrently on EOF. Cancelling an opening request releases its
session reservation. A failed observation invalidates all refs from the prior
observation. Application identity, launch/attach mode, window selector,
capture scope, CUA endpoint, and policy file are fixed when the host starts
the server and are absent from tool arguments. Both semantic refs and visual
image refs require the latest `observation_id`.

If `test_finish` or `test_abort` reaches its caller deadline, driver cleanup
continues in an owned background task. Calls made before it resolves return the
retryable `test.session.cleanup_in_progress`. If the driver then reports a
retryable failure, the same session enters `cleanup_required`. Do not observe
or act again; retry `test_abort` or `test_finish` so the retained ownership
handle can complete cleanup. Eventual success reaps the session, while a
non-retryable cleanup error is terminal and does not make the identifier
reusable.

The semantic GUI profile returns accessibility elements with A3S-owned opaque
refs. The window-vision profile additionally returns a fresh `@vN` reference,
image dimensions, SHA-256 digest, and PNG evidence. A visual point names that
reference and pixel coordinates. The GUI adapter rejects stale references,
out-of-bounds coordinates, a changed image digest, ambiguous semantic targets,
and CUA snapshot reuse before input dispatch. It also revalidates the bound
application identity and top-level window before every observation and input;
binding drift invalidates all current refs and dispatches no input. GUI
screenshot paths are canonical-root contained, reject linked/reparse
descendants, and are revalidated before a grounding image can authorize input.

## Embedded host sequence

```text
host selects typed SurfaceDriver and real LlmProvider
                         |
                         v
             open ScenarioContext
                         |
                         v
               AgentLoop::run
                         |
          +--------------+--------------+
          |                             |
          v                             v
 DriverSession::observe       LlmProvider::complete
          |                   JSON Schema response
          +--------------+--------------+
                         |
                         v
              local schema parsing
                         |
                         v
          ActionPolicy capability/origin gate
                         |
                         v
             DriverSession::execute
                         |
                repeat or finish
                         |
                         v
       host performs bounded DriverSession::close
```

The host must close the session on success, model failure, policy denial,
budget exhaustion, timeout, and cancellation. The deterministic `Runner`
and external-planner session application demonstrate the required
bounded-cleanup behavior.

## Direct embedded-planner CLI

Use this path when the deployment intentionally wants A3S Test to own a
complete one-shot Web workflow around its model service:

```bash
a3s-test provider schema llm
a3s-test agent run examples/agent-web.acl --json
```

The config is A3S ACL, the same product configuration language used by suites
and workflows:

```acl
agent_run "checkout" {
  url = "http://127.0.0.1:3000/checkout"
  goal = "Complete checkout with the fixture account"
  success_criteria = ["The order confirmation is visible"]
  allow_actions = ["click", "fill", "wait"]
  max_turns = 8
  max_total_tokens = 20000
  max_cost_microusd = 50000
  timeout_ms = 120000

  provider {
    name = "deployment"
    model = "planner"
    endpoint = "https://models.example.test/v1/plan"
    authorization_env = "A3S_TEST_PROVIDER_AUTHORIZATION_DEPLOYMENT"
  }

  verification {
    expect "confirmation" { text = "Order confirmed" }
    screenshot "final" { path = "confirmation.png" }
  }
}
```

`allow_origins` adds exact HTTP(S) origins for browser requests, explicit URL
actions, and every successful observation when the A3S Browser integration is
used. `allow_domains` adds only hostname-wide browser network access, for
example an API or CDN; it never grants exact-origin navigation. Standalone
0.26.x receives the origin hostnames because its network protocol cannot
express scheme or effective port.

The workflow deadline begins before surface opening and covers opening,
initial navigation, every observation, provider call, proposed action,
page-context revision check, and deterministic verification. Cleanup has a
separate `--cleanup-timeout-ms` because process reaping must still be attempted
after a workflow timeout. Per-command browser and HTTP limits remain bounded by
`--command-timeout-ms`.

The model's `finish` decision is provisional. Verification accepts only
`snapshot`, `wait`, `expect`, `screenshot`, `accessibility`, `console`, and
`page_errors`, and it requires at least one `expect`. No verification action
may mutate the page. The final status is successful only when the model
finishes, every local verification step passes, and exact surface cleanup
succeeds.

Every admitted run writes protocol `a3s.test.agent-run/1` to
`.a3s-test/agent-runs/<run-id>/report.json` by default, including failures
during surface opening. The report contains provider identity and usage,
decision digests, observations, action outputs, verification results, and a
separate cleanup error. It is bounded, redacted, and atomically published;
`--report` selects another path and `--force` is required to replace a regular
file. A report path cannot be placed inside the run's browser artifact
directory, which is prepared under the Web driver's stricter fresh-artifact
rules.

## Provider contract

`LlmProvider` is an object-safe `Send + Sync` interface:

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    fn identity(&self) -> LlmIdentity;

    async fn complete(
        &self,
        request: StructuredLlmRequest,
    ) -> Result<StructuredLlmResponse, LlmError>;
}
```

An implementation must call a real configured LLM. A scripted provider is used
only as a test double inside the test suite. Production code has no keyword,
regular-expression, or heuristic fallback.

Each request contains:

| Field | Meaning |
| --- | --- |
| `prompt_version` | Stable version of the system instruction |
| `system_instruction` | Planner rules, separate from the user goal |
| `context.goal` | Instruction and explicit success criteria |
| `context.surface` | Typed `web`, `gui`, or `tui` surface |
| `context.observation` | Latest semantic surface observation |
| `image_attachments` | Verified GUI grounding images referenced by the observation |
| `context.history` | Previously executed typed actions and outputs |
| `context.remaining` | Remaining turn, token, cost, and time budgets |
| `response_schema` | Generated JSON Schema for `AgentDecision` |

The provider returns one JSON value, token and micro-USD usage, and an optional
provider request ID. The loop independently deserializes the value after the
provider returns it. A provider claiming structured output does not bypass
local validation. Prompt contract `a3s-test-agent/v2` tells multimodal hosts to
ground pixel actions only in the attached observation image.

For this protocol, `context.remaining.time_ms` is the request deadline budget,
`context.remaining.cost_microusd` is the cumulative cost ceiling available to
the response, and the configured provider/model identity is bound by the host
rather than repeated in each response. The loop updates cumulative usage and
rejects a response that takes the run above its token or cost budget before
executing a proposed action.

The HTTP projection uses protocol `a3s.test.llm-provider/1` and `POST
application/json`. The request envelope is:

```json
{
  "protocol": "a3s.test.llm-provider/1",
  "request": { "prompt_version": "a3s-test-agent/v2" }
}
```

The service returns either `status = "success"` with one typed `response`, or
`status = "failure"` with a bounded `error`. The generated schema is
authoritative. The adapter requires HTTPS except for explicit loopback HTTP,
disables redirects and environment proxies, bounds both bodies, enforces its
configured timeout, and redacts the configured authorization value from
diagnostics. Provider output has `proposal_only` authority: it may propose a
typed action but cannot determine the test verdict, claim that an observation
occurred, or authorize workspace repair.

The authoritative discovery command is:

```bash
a3s-test provider schema llm
```

It publishes the transport-neutral request and response schemas, the standard
HTTP envelopes, and the deadline, cumulative cost, configured-identity, local
admission, observation scope, and non-authority invariants.

## Optional visual-grounding provider

Visual grounding is a separate typed facility from `LlmProvider` and the
observe-decide-act loop. It is intended only for an explicit request or a typed
semantic-fallback reason: `canvas`, `image_only`, `remote_desktop`,
`design_reference`, or `no_semantic_match`. Natural-language keyword routing
cannot activate it.

```rust
#[async_trait]
pub trait VisualGroundingProvider: Send + Sync {
    fn identity(&self) -> GroundingProviderIdentity;

    async fn locate(
        &self,
        request: GroundingProviderRequest,
    ) -> Result<GroundingProviderResponse, GroundingError>;
}
```

The caller supplies a screenshot path owned by its surface adapter, a
`sha256:` digest, positive dimensions, query, current observation ID, typed
trigger, and micro-USD ceiling. The service requires a regular non-link file,
rehashes its bytes before provider dispatch, adds issue/deadline times, and
applies its own timeout and cancellation token. The provider returns point or
box candidates in screenshot pixels or normalized coordinates, confidence,
identity, complete image/observation binding, bounded usage, and an optional
request ID.

Admission fails closed for stale provenance, identity or dimension mismatch,
page-context observation, revision, completeness, or node-bound mismatch,
missing or changed screenshot bytes, non-finite or out-of-bounds geometry,
invalid confidence, oversized fields or candidate sets, timeout, cancellation,
and reported cost above the request ceiling. After admission, the service maps candidates into visual-viewport CSS
pixels and hit-tests the current Test Kit snapshot. A unique hit returns a
current semantic target. An ambiguous or unmapped candidate stays image-bound
with the matching node IDs, observation ID, and screenshot digest. All
outcomes carry `authority = advisory`; callers must not use them to satisfy
blocking contracts or bypass repair authorization.

A deployment-specific adapter may call a local process or remote inference
service. A3S Test neither bundles model weights nor selects one by model-name
string. The deploying host is responsible for credentials, capacity, privacy,
and license compliance.

External adapters can discover the transport-neutral version 2 wire contract
without linking the Rust trait:

```bash
a3s-test provider schema visual-grounding
```

The output identifies `a3s.test.visual-grounding-provider/2`, declares
`authority = advisory`, records the non-authority invariants, and includes
generated JSON Schema 2020-12 documents for `GroundingProviderRequest` and
`GroundingProviderResponse`. Unknown fields are rejected by the wire types. A
breaking field or semantic change requires a new protocol identifier.

`HttpVisualGroundingProvider` is the standard execution adapter for a
deployment-owned inference service. Its typed `HttpProviderConfig` fixes one
endpoint and optional authorization value; public SDK options do not select a
backend with a model-name string. The adapter posts the version envelope shown
in the discovered `http.request_envelope_schema` and admits the corresponding
response envelope before `VisualGroundingService` performs semantic admission.
Version 2 embeds the admitted PNG as a digest-bound Base64 attachment and uses
`observation.png` as its logical path; a remote provider never dereferences the
client's local evidence path.

The persistent external-planner CLI exposes this path explicitly:

```bash
a3s-test agent ground "Primary checkout action" \
  --session checkout \
  --observation 7 \
  --config examples/visual-grounding.acl \
  --reason no-semantic-match \
  --json
```

Configuration, authorization, provider identity, query size, and grounding
limits are admitted before browser connection. The Web driver captures one
bounded PNG between two matching Test Kit revisions. The CLI rebuilds the
current `@cN` bindings and requires them to exactly match the latest
observation, invokes the provider, then verifies the revision again. Any
provider failure, cancellation, binding change, or revision drift invalidates
the observation. Success records advisory evidence only and never dispatches
input.

## Advisory design audit

Use `agent audit` when the goal is to review hierarchy, composition, spacing,
typography, color use, consistency, clarity, or responsive composition rather
than locate an element or verify a deterministic requirement:

```bash
a3s-test provider schema design-audit
a3s-test agent audit \
  --session checkout \
  --observation 7 \
  --config examples/design-audit.acl \
  --dimension visual-hierarchy,spacing-rhythm \
  --json
```

The command requires the latest observation and an embedded, complete Test Kit
snapshot. It captures a PNG, requests forensic page context up to the Test Kit
bound, and sends both through `a3s.test.design-audit-provider/1`. The provider
receives semantic nodes and state, current geometry, component/source hints,
locators, facts, and bounded computed styles in addition to pixels. It does not
receive workspace mutation or browser action authority.

The provider may return page-, node-, or normalized-region-bound findings in
the explicitly requested dimensions. A3S Test independently verifies the
identity, observation, surface revision, screenshot and context digests,
dimension scope, current node geometry, string and finding limits, deadline,
and cost. It then rechecks the page revision. Any failure invalidates the
observation before another action can use stale evidence.

The result is `a3s.test.design-audit-report/1` with `authority = advisory`.
There is deliberately no verdict. The provider cannot create a Surface
Contract expectation or enqueue a repair. If the page embeds a compatible
Test Kit, the Web driver projects admitted findings into its separate Design
Audit store. The reviewer must dismiss, edit, or retarget each suggestion and
explicitly save or send it before the existing Repair Ledger can process it.
Single and batch submission then use the same repair verification and human
acceptance gates as manually marked findings.

The inference endpoint and model runtime are deployment-owned. A3S Test does
not download weights, infer a backend from the model name, or grant additional
authority based on provider identity.

## Contract-generation provider

Expected-interface generation is separate from both `LlmProvider` planning and
visual grounding. An SDK host injects one typed provider:

```rust
#[async_trait]
pub trait ContractGenerationProvider: Send + Sync {
    fn identity(&self) -> ContractGenerationProviderIdentity;

    async fn generate(
        &self,
        request: ContractGenerationProviderRequest,
    ) -> Result<ContractGenerationProviderResponse, ContractGenerationError>;
}
```

The request contains a contract name, explicit product context, digest-bound
PRD or design sources, issue/deadline times, and a cost ceiling. The service
reads every regular non-link source asynchronously and verifies its digest both
before and after the provider call. Returned PRD source spans must match exact
UTF-8 bytes. Returned design regions must match the declared image dimensions,
coordinate space, and semantic parent hierarchy.

The response proposes bounded candidates with confidence and unresolved
decisions; it cannot return approved ACL citations. Local validation rejects
duplicate or cyclic structure, stale identity or provenance, source races,
invalid evidence, oversized responses, excess cost, timeout, and cancellation.
Deterministic merge exposes every source disagreement as a conflict. A separate
human review approves candidates and resolves conflicts with rationale before
the service constructs a checked ACL Surface Contract draft.

The reviewed result retains the full provider response and review record for
audit. It does not claim browser observation, cannot pass a test by itself, and
does not authorize repair. Model transports, credentials, runtimes, weights,
and license decisions remain deployment-owned.

The equivalent external discovery command is:

```bash
a3s-test provider schema contract-generation
```

It returns protocol `a3s.test.contract-generation-provider/1`,
`authority = candidate_only`, explicit review and non-authority invariants,
and generated request/response schemas. The schema is a discoverable wire
contract, not an execution transport. An adapter may use stdio, HTTP, RPC, or
an in-process call as long as the host independently enforces A3S Test
admission and lifecycle policy.

`HttpContractGenerationProvider` implements the same HTTP envelope and
endpoint policy for source-to-contract generation. Both adapters disable
redirects and environment proxies, accept HTTPS or explicit loopback HTTP,
bound serialized requests and streamed responses, honor the earlier of the
configured transport timeout and wire deadline, require JSON media type and
HTTP 200, and map bounded typed remote errors without exposing configured
authorization values. HTTP conformance never bypasses the service-level
source, identity, cost, evidence, review, or authority checks.

The CLI exposes this path without embedding an inference runtime:

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

The generation config is ACL and names the contract context, contained PRD or
design files, HTTP endpoint, provider/model identity, limits, cost ceiling,
and optionally an `A3S_TEST_PROVIDER_AUTHORIZATION_*` environment variable.
Authorization values are read only from the environment. The generated JSON
artifact is candidate-only. The review ACL contains the reviewer identity,
one `candidate` block per explicit approval or rejection, and one `conflict`
block per resolution with the selected candidate and rationale. Review locally
reconstructs and admits the Surface Contract; it never asks the provider to
approve its own proposal.

## Decisions

The model can return exactly one of:

```text
Act    { action: typed Action }
Finish { summary: string }
Fail   { reason: string }
```

Unknown variants and fields are rejected. `Act` is checked by `ActionPolicy`
before it reaches a driver. `Finish` ends successfully, and `Fail` records an
explicit model failure rather than inventing a surface action.

## Budgets

`AgentOptions` requires bounded execution:

| Option | Enforcement point |
| --- | --- |
| `max_turns` | Stops after the final model turn |
| `max_total_tokens` | Accounts input plus output tokens before action execution |
| `max_cost_microusd` | Accounts provider-reported cost before action execution |
| `max_context_bytes` | Bounds serialized context before the provider call |
| `timeout` | Covers observations, model calls, and actions in the loop |

Provider usage that crosses a token or cost limit is still reported because the
cost has already been incurred, but its proposed action is not executed.

## Policy

`CapabilityPolicy` uses typed `ActionKind` values. Navigation has a separate
scope:

- `NavigationScope::Denied` rejects all model-proposed navigation.
- `NavigationScope::Origins` compares parsed URL origins, not string prefixes.
- `NavigationScope::Any` is an explicit unrestricted opt-in.

A host can implement `ActionPolicy` when it needs additional product rules.
Policy code receives the goal, surface, current observation, and executed
history. It must validate typed fields and must not route natural-language
intent by keyword.

## Result and provenance

`AgentRunResult` distinguishes:

| Status | Meaning |
| --- | --- |
| `succeeded` | Model returned a valid `Finish` decision |
| `failed` | Provider, schema, driver, or explicit model failure |
| `policy_denied` | Proposed action failed the policy gate |
| `budget_exceeded` | Turn, token, cost, or context bound was reached |
| `timed_out` | Overall agent deadline expired |
| `cancelled` | Host cancellation won the current operation |

The trace records provider/model identity, prompt version, request IDs, token
and cost usage, model latency, SHA-256 decision digests, observations, typed
decisions, and action outputs. Provider failures preserve their retryability so
an owning coding agent can distinguish retryable infrastructure from a product
failure.

Before `AgentLoop::run` returns, the complete `AgentRunResult` passes through
the `AgentOptions.provenance_redactor` object. Its secure defaults:

- recursively replace values under common credential-shaped JSON keys;
- replace `fill`, `type`, `select`, dialog-prompt, and network-body payloads;
- remove URL user information, query strings, and fragments from recorded
  URL-bearing actions; and
- keep decision digests, usage, latency, and retryability intact.

Unstructured summaries, observations, paths, request IDs, provider errors, and
provider/model names can also echo operational secrets. Register every such
runtime value explicitly:

```rust
let provenance_redactor =
    ProvenanceRedactor::from_exact_secrets([session_secret])?;
let options = AgentOptions {
    provenance_redactor,
    ..AgentOptions::default()
};
```

Exact values are matched as case-sensitive substrings across all trace text.
Configuration is size-bounded, and `Debug` output exposes only the registered
value count. Provider transport credentials remain outside the provider
contract. Operational context and actions are intentionally passed to the
trusted provider and surface driver before the result is redacted, so hosts
must not persist raw `StructuredLlmRequest` values.

The filter protects JSON provenance metadata; it does not rewrite screenshots,
HAR, trace, video, or other evidence files. Continue to apply normal artifact
access controls when those files may contain sensitive product data.
