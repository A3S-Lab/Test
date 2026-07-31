# Agentic CLI and SDK Contract

## Current boundary

A3S Test supports two agentic planning boundaries over the same typed actions
and surface drivers:

1. The primary **external-planner CLI** lets A3S Code, Codex, Claude Code, or
   another coding agent drive a persistent session through
   `start -> observe -> act -> finish`. The coding agent is already the LLM;
   A3S Test does not call a nested model.
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
invocations through an isolated namespace and private runtime directory.

Ref targets are bound to the latest observation identifier. Explicit
URL-bearing actions are limited to the initial HTTP(S) origin and
`--allow-origin` values. Each observation independently verifies the reported
page origin before returning refs. A browser page lost to `about:blank` or
another non-Web scheme returns `test.driver.web.session_origin_lost`; an
unapproved HTTP(S) origin returns
`test.driver.web.navigation_origin_denied`. The generated
`a3s-test agent schema` output is the authoritative action contract.

Action protocol revision 2 covers the browser interactions needed to inspect
document-style applications: click, hover, focus, double-click, context-click,
fill, incremental type, check/uncheck, multi-value select, drag, key press,
modifier-aware wheel, viewport, synchronization, assertions, browser context,
files, network controls, and evidence. Compact CLI commands project the common
interactions; `agent act --action-json` projects the same model without a
second protocol.

Semantic role, text, test-ID, label, and placeholder targets are used whenever
the underlying browser command supports the requested subaction. Focus,
double-click, context-click, type, uncheck, select, drag, and target-scoped
wheel require a ref from the latest observation or explicit CSS selector with
the current standalone browser protocol. This is a protocol capability
boundary, not keyword routing or locator inference. Context-click dispatches a
page-scoped, cancelable `contextmenu` event at the visible target instead of
opening the browser-native menu, which is outside the observable page.

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
| `context.history` | Previously executed typed actions and outputs |
| `context.remaining` | Remaining turn, token, cost, and time budgets |
| `response_schema` | Generated JSON Schema for `AgentDecision` |

The provider returns one JSON value, token and micro-USD usage, and an optional
provider request ID. The loop independently deserializes the value after the
provider returns it. A provider claiming structured output does not bypass
local validation.

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

Credential values are never part of the provider contract. Observation and
action redaction is not implemented yet, so hosts must apply their normal
artifact access controls until secret-safe provenance filtering lands.
