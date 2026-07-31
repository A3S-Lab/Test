# Architecture

## Product boundary

A3S Test is a cross-surface test runtime for coding agents. It owns scenario
admission, orchestration, surface-driver contracts, evidence, reports, and
process lifecycle. It does not own browser implementation, desktop perception,
terminal emulation, or an LLM provider.

Those capabilities are injected through typed interfaces:

```text
                         A3S Test
  +------------------------------------------------------+
  | ACL / Agent request                                  |
  |          |                                           |
  |          v                                           |
  | Typed test IR -> Runner -> Result + evidence          |
  |                       |                              |
  |                 Driver registry                       |
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
Layer 5  Agent interface
         CLI today; MCP and SDK adapters later

Layer 4  Agentic planning
         User-supplied LLM provider -> typed action proposal -> policy gate

Layer 3  Orchestration
         deadlines, cancellation, cleanup, events, result aggregation

Layer 2  Surface contracts
         SurfaceDriver -> DriverSession -> execute / close

Layer 1  Platform adapters
         Web: A3S Browser
         GUI: A3S CUA
         TUI: PTY plus semantic terminal state

Layer 0  Host supervision
         process groups, bounded shutdown, namespaces, artifact isolation
```

The deterministic manifest path currently implements Layers 0, 2, 3, and the
CLI portion of Layer 5. Layer 4 will not be simulated with keyword rules. An
agentic step must call a configured LLM, receive a schema-constrained proposal,
validate it against capabilities and policy, execute one action, observe again,
and stop at an explicit turn or cost limit.

## Core contracts

`a3s-test-core` contains framework-independent types:

```text
TestSuite
└── TestScenario [surface, deadline]
    └── TestStep
        └── Action [typed locator / condition / assertion]

SurfaceDriver
└── open(ScenarioContext) -> DriverSession
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
5. Dropping an unclosed session runs the same owned-session cleanup, then
   schedules an emergency `close` when no PID file exists yet.
6. Browser daemons receive a bounded inactivity timeout.
7. A second SIGINT kills all currently registered command process groups.

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

It maps typed `Action` values to the native driver protocol. It does not parse
natural-language intent. A3S Browser snapshots remain the observation format
for a future LLM planner, while refs, semantic locators, and CSS locators remain
explicit target types in the deterministic suite.

The adapter keeps configuration, command execution, protocol mapping, session
behavior, and host-process supervision in separate modules. The public crate
surface remains limited to typed configuration, the driver, the injectable
command executor, and the emergency termination entry point.

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

Coding agents need predictable contracts rather than a human-only dashboard:

```text
agent writes ACL
      |
      v
a3s-test check --json
      |
      v
a3s-test run --json
      |
      +--> stable status / error code
      +--> step output
      +--> artifact paths
      +--> process exit code
```

Future MCP tools should be thin projections of the same application layer:
`test_check`, `test_run`, `test_cancel`, `test_result`, and `test_artifact`.
They must not create a second runner implementation.

## Agentic execution plan

Agentic exploration is a bounded observe-decide-act loop:

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

The LLM adapter is user-supplied as a typed object. Model names, credentials,
and providers do not belong in the core domain. A run must record the provider
identity, model, prompt template version, decision payload digest, turn count,
and cost/latency envelope without storing secrets.

## Evidence and reproducibility

Artifacts live under:

```text
.a3s-test/runs/<run-id>/<scenario-id>/
```

Relative artifact paths are admission-checked and cannot escape this root.
Planned evidence types include screenshots, accessibility snapshots, terminal
recordings, browser network logs, videos, and normalized action traces.

Reports separate assertion failure from infrastructure failure and cleanup
failure. This distinction is required for an agent to choose whether to repair
product code, repair a test, or retry infrastructure.
