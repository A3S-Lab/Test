# ADR 0001: Keep GUI Agentic Testing Behind an A3S CUA Adapter

- Status: Accepted
- Date: 2026-08-03

## Context

A3S Test already owns typed scenarios, the runner, persistent coding-agent
sessions, the bounded embedded agent loop, evidence, and cleanup. A3S CUA owns
platform accessibility, capture, input delivery, application discovery, and
window control. Combining those responsibilities in either repository would
create a second runner or duplicate platform automation.

The macOS CUA daemon also needs a stable application identity for TCC grants.
Launching a raw daemon from an unrelated terminal or gateway is not a supported
production topology.

## Decision

A3S Test integrates CUA as an external executor and observer through a typed
MCP JSON-RPC transport in `a3s-test-driver-gui`.

- `a3s-test-core` remains independent of CUA and platform APIs.
- A3S Test remains the only owner of planning, action policy, budgets, session
  persistence, evidence, reports, and bounded cleanup.
- The adapter consumes JSON-RPC envelopes, `isError`, and
  `structuredContent`. It never interprets human-readable tool summaries.
- Runtime admission is fail-closed against `compat/cua-stack.acl`, including
  the exact driver version, MCP version, tools schema, capability vocabulary,
  required tools, required per-tool capabilities, and every platform/endpoint
  execution profile.
- Application launch or attachment is host configuration, not an agent action.
- The initial capture scope is strictly window-scoped. Desktop scope requires a
  future, separately approved execution profile.
- CUA element tokens stay inside the adapter. Agents receive A3S Test opaque,
  observation-bound references.
- Pixel targets are admitted only against the latest digest-bound window
  screenshot and return that image as explicit evidence.

## Consequences

The adapter can evolve independently of the platform implementations and can
be tested with a fake transport. A CUA update that changes a load-bearing
contract is rejected until the compatibility lock and contract tests are
reviewed together.

macOS deployments use the installed CuaDriver application or an embedded
socket owned by the application that holds the grants. The locked CUA 0.10.0
revision has no reviewed Windows or Linux application backend, so those four
platform/endpoint combinations are explicit unsupported profiles and fail
before transport startup.

A profile cannot claim host certification merely because a platform binary
starts or its fake contract passes. The `gui-certify` harness must pass
permission attribution, semantic or visual observation, and exact owned
cleanup on the real worker before that host is enabled.
