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

The browser also receives a hostname allowlist derived from the initial URL
and every `--allow-origin`. This network layer rejects cross-domain links,
redirects, scripts, images, fetches, and similar requests before the next
observation. Add `--allow-domain` only when a page needs a CDN or API hostname
without adding that hostname to A3S Test's exact-origin permission. The
underlying filter applies to document requests as well as subresources, so a
page-driven request to such a hostname can occur; the next observation still
rejects it unless `--allow-origin` admits the exact origin. The filter also
cannot distinguish two schemes or ports on the same hostname, so exact-origin
enforcement remains the responsibility of the explicit-action and observation
checks above.

The domain policy is persisted when `agent start` creates the browser daemon.
A session file from an earlier release has no proof that its already-running
daemon was started with this policy. A3S Test therefore keeps that state
readable but rejects `observe` and action turns with
`test.session.browser_network_policy_missing`. `finish` and `abort` remain
available so the exact owned browser session and runtime can be cleaned up;
start a new session before continuing the test.

Action protocol revision 5 is the current cross-surface schema. Revision 2
introduced the browser interactions needed to inspect document-style
applications: click, hover, focus, double-click, context-click,
fill, incremental type, check/uncheck, multi-value select, drag, key press,
modifier-aware wheel, viewport, synchronization, assertions, browser context,
files, network controls, and evidence. Compact CLI commands project the common
interactions; `agent act --action-json` projects the same model without a
second protocol. Revision 4 adds GUI automation-ID targets, and revision 5
adds image-bound visual points.

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
| `test_schema` | Return action protocol revision 5 and its JSON Schema |

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
