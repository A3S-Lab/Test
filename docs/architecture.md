# Architecture

## Product boundary

A3S Test is a cross-surface test engine delivered as the `a3s-test` CLI. It
owns persistent agent sessions, deterministic scenario admission,
orchestration, surface-driver contracts, evidence, reports, and process
lifecycle. It does not own browser implementation, desktop perception,
terminal emulation, or an LLM provider.

Coding agents are external planners in the primary agentic workflow. A3S Code,
Codex, Claude Code, or another agent repeatedly calls `agent observe` and one
typed action command through the portable Skill. The CLI preserves the surface
between calls and records the run. The optional `a3s-test-agent` library
supports hosts that intentionally inject a separate LLM provider.

Those capabilities are injected through typed interfaces:

```text
                         A3S Test
  +------------------------------------------------------+
  | ACL suite              Coding agent / SDK goal        |
  |    |                      |              |             |
  |    v                      v              v             |
  | Typed IR -> Runner   Session CLI    LLM -> policy      |
  |                 \        |          /                  |
  |                  Driver session                       |
  +-----------------------+------------------------------+
                          |
            +-------------+-------------+
            |             |             |
            v             v             v
      A3S Browser      A3S CUA      TUI runtime
       Web/CDP         GUI/a11y      PTY/terminal
```

The runner never branches on backend names. Each backend is a typed
`SurfaceDriver` object registered for one `Surface`.

## Runtime layers

```text
Layer 6  Planner interface
         Coding Agent Skill today; MCP later

Layer 5  Product interface
         persistent agent-session CLI + deterministic JSON CLI

Layer 4  Agentic planning
         external coding agent, or user-supplied LLM provider in SDK hosts

Layer 3  Orchestration
         deadlines, cancellation, cleanup, events, result aggregation

Layer 2  Surface contracts
         SurfaceDriver -> DriverSession -> observe / execute / close

Layer 1  Platform adapters
         Web: A3S Browser
         GUI: A3S CUA
         TUI: PTY plus semantic terminal state

Layer 0  Host supervision
         process groups, bounded shutdown, namespaces, artifact isolation
```

The deterministic manifest path implements Layers 0, 2, 3, and the closed-run
portion of Layer 5. The persistent agent-session path projects the same typed
driver through `start`, `observe`, `act`, and `finish`; the external coding
agent owns Layer 4 decisions. `a3s-test-agent` is the alternate embedded Layer
4 contract: it calls an injected LLM, receives a schema-constrained proposal,
validates it against capabilities and policy, executes one action, observes
again, and stops at explicit turn, token, cost, context, cancellation, or time
limits.

The shipped Skill teaches A3S Code, Codex, Claude Code, and compatible agents
both the interactive session protocol and deterministic ACL workflow. Future
MCP tools must project the same application layer.

## Core contracts

`a3s-test-core` contains framework-independent types:

```text
TestSuite
└── TestScenario [surface, deadline]
    └── TestStep
        └── Action [typed locator / condition / assertion]

SurfaceDriver
└── open(ScenarioContext) -> DriverSession
    ├── observe() -> SurfaceObservation
    ├── execute(TestStep) -> StepOutput
    └── close()
```

Every public driver object is `Send + Sync`. A session is `Send` and owned by
one scenario execution. The runner never shares mutable session state across
scenarios.

## Lifecycle and interrupts

```text
 Created
    |
    v
 Opening --failure/timeout--> Reported
    |
    v
 Running --failure----------> Closing
    |  \
    |   +--deadline---------> Closing
    |   +--first SIGINT-----> Cancelling -> Closing
    v
 Closing --bounded cleanup--> Reaped -> Reported
    |
    +--second SIGINT--------> kill registered command groups -> exit 130
```

The Web adapter adds independent containment:

```text
one test run
└── browser namespace derived from the run id (or an explicit override)
    └── one scenario
        ├── private socket/PID runtime directory
        └── browser session derived from the scenario id
            └── owned daemon and Chrome process tree
```

Protection is layered:

1. Each driver command runs in a process group owned by A3S Test.
2. Dropping a cancelled command kills that complete process group.
3. Normal scenario completion sends `close` to its browser session.
4. A stuck `close` falls back to the exact private PID file, validates the
   executable, snapshots descendants, and kills only those process groups.
5. Dropping an unclosed deterministic or embedded session runs the same
   owned-session cleanup, then schedules an emergency `close` when no PID file
   exists yet.
6. External-planner session handles intentionally survive individual CLI
   processes; `finish`, `abort`, or the bounded daemon idle timeout closes
   them.
7. Browser daemons receive a bounded inactivity timeout.
8. A second SIGINT kills all currently registered command process groups.

The per-run namespace prevents cleanup from touching a developer's unrelated
browser sessions.

## Web driver

The Web driver supports two typed command layouts:

```text
BrowserCommand::A3s
  a3s use browser ...

BrowserCommand::Standalone
  agent-browser ...
```

Before opening a session, the driver runs a version probe and admits only a
verified protocol window. The discovered `BrowserCapabilities` records the
typed integration, semantic version, protocol revision, and feature set.
Concurrent scenarios share a single asynchronous capability result.

The adapter maps typed `Action` values to tabs, frames, dialogs, uploads,
artifact-scoped downloads, network routes, HAR, trace, video, screenshots,
accessibility snapshots, console logs, and page errors. It exposes a full A3S
Browser accessibility snapshot through `DriverSession::observe`. It does not
parse natural-language intent. Refs, semantic locators, and CSS locators remain
explicit target types in both deterministic and agentic execution.

The adapter keeps configuration, command execution, protocol mapping, session
behavior, and host-process supervision in separate modules. The public crate
surface remains limited to typed configuration, the driver, the injectable
command executor, and the emergency termination entry point.

Command executor errors carry a typed dispatch phase. Only an unavailable
executable before dispatch is retryable. A timeout or output failure may have
already applied an action and is therefore never retried. The runner bounds
retry count and backoff inside the scenario deadline. Scenario concurrency is
also bounded and report order remains the manifest order.

## GUI driver plan

The GUI driver will adapt A3S CUA capabilities rather than copy its
implementation. Its observation should combine:

- accessibility tree and stable node identifiers;
- window/application identity and bounds;
- screenshot regions for visual-only controls;
- pointer and keyboard state;
- permission and focus diagnostics.

The first GUI milestone should cover application launch, window selection,
semantic click/type, screenshot evidence, and cleanup of the launched process
tree. Pixel-coordinate actions remain a fallback with explicit evidence.

## TUI driver plan

The TUI driver will own a PTY session and expose semantic terminal state:

- viewport text and cursor position;
- alternate-screen and raw-mode state;
- process exit status;
- key chords and pasted text;
- bounded waits over exact text or regular expressions;
- terminal recording as evidence.

It must launch the tested program in a dedicated process group and restore the
terminal even when the runner is cancelled.

## Coding-agent interface

Coding agents need a persistent, inspectable control loop rather than a
human-only dashboard or one opaque prompt. The `$a3s-test` Skill supplies the
workflow and progressive references:

```text
agent start [goal + success criteria]
      |
      v
agent observe -> observation_id + semantic snapshot
      |
      v
coding agent decides one typed action
      |
      v
agent click/fill/press/... or agent act
      |
      +--> event log + scoped evidence
      |
      +---------------> observe again
      |
      v
agent finish -> report + owned cleanup
```

Agent sessions live under `.a3s-test/agent-sessions/<session>/`. A ref target
must carry the latest observation identifier, explicit URL-bearing actions are
limited to admitted HTTP(S) origins, and evidence paths cannot leave the
session root. The browser runtime uses an isolated namespace, an ownership
marker, and a bounded idle timeout so persisted metadata cannot redirect
cleanup and an abandoned external planner does not leave an unbounded process.

Once a path is understood, the coding agent can author ACL and use
`check --json` and `run --json` for deterministic regression coverage.

Future MCP tools should be thin projections of the same application layer:
`test_session_start`, `test_observe`, `test_act`, `test_finish`, `test_check`,
`test_run`, and `test_artifact`. They must not create a second runner.

## Embedded agentic execution

An SDK host can alternatively inject a real LLM provider and run the bounded
observe-decide-act library loop:

```text
surface observation
       |
       v
LLM provider + goal + history + policy
       |
       v
schema-constrained Action proposal
       |
       v
capability and safety validation
       |
       v
execute -> evidence -> next observation
```

The LLM adapter is user-supplied as a typed object. Model credentials and
provider-specific transports do not belong in the core domain. The working
loop sends a versioned system instruction, typed context, remaining budgets,
and the generated `AgentDecision` JSON Schema. It independently parses the
returned JSON and applies a typed capability and origin policy before calling
the surface.

Each trace records provider and model identity, prompt version, request ID,
decision payload digest, turn, token and cost usage, model latency, observation,
and action output. Provider failures preserve retryability. Secret-safe
provenance redaction remains planned.

The external-planner CLI does not call this provider. A3S Code, Codex, or
Claude Code is already the planner and drives typed CLI turns directly.

`AgentLoop` operates on an already-open session and deliberately does not own
`close()`. The runner or SDK host that opens the session must retain bounded
cleanup responsibility.

## Evidence and reproducibility

Artifacts live under:

```text
.a3s-test/runs/<run-id>/<scenario-id>/
.a3s-test/agent-sessions/<session>/artifacts/
```

Relative artifact paths are admission-checked and cannot escape this root. The
Web adapter currently records screenshots, accessibility JSON, console and
page-error JSON, downloads, HAR, traces, and WebM video. Terminal recordings
and GUI visual regions remain planned with their surface drivers.

Reports separate assertion failure from infrastructure failure and cleanup
failure. This distinction is required for an agent to choose whether to repair
product code, repair a test, or retry infrastructure.
