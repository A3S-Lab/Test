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

## Surface understanding from first principles

A3S Test separates four artifacts that answer different questions:

| Artifact | Question | Authority |
| --- | --- | --- |
| Observed Surface | What did this exact browser revision render? | Browser accessibility snapshot plus bounded Test Kit context |
| Expected Surface Contract | What should this product state expose? | Reviewed PRD, design, manual decision, or official documentation |
| Contract Report | Where does the observation differ from the expectation? | Deterministic Core reconciliation |
| Repair Authorization | Which reported problem may a coding agent change? | Explicit human submission through the repair ledger |

None of these artifacts may impersonate another. A design image or PRD can
generate an Expected Surface Contract draft, but it cannot generate a browser
accessibility tree because no browser rendered it. Test Kit context is
evidence, not an instruction. A report is a diagnosis, not permission to edit.
Opening or inspecting a finding is also not permission; only a submitted
repair enters the authoritative ledger.

The complete Web path is:

```text
PRD / design / reviewed decision
              |
              v
    Surface Contract draft --human review + digest--> admitted contract
              |                                           |
              |                                           v
              |                              deterministic Core rules
              |                                           ^
              v                                           |
browser DOM + accessibility + layout ----atomic observation
                                                          |
                                                          v
                                               Contract Report
                                                  |       |
                             optional projection--+       +--runner verdict
                                                  |
                                            human selection
                                                  |
                                                  v
                                     authoritative Repair Ledger
                                                  |
                                      agent edit + owned evidence
                                                  |
                                                  v
                                      verify the same contract again
```

This decomposition keeps the fast path deterministic. Browser facts are used
before probabilistic perception, and a model can propose expected structure or
visual candidates without acquiring verdict or mutation authority.

### Source-to-contract generation

Source interpretation is an adapter concern in `a3s-test-agent`, not a Core
concern. `ContractGenerationService` accepts a typed provider plus explicit
limits, digest-bound local PRD or design sources, a contract context, deadline,
cancellation token, and cost ceiling. It verifies every regular source file
before provider dispatch and reads it again after the call. A concurrent edit,
stale provider identity, provenance mismatch, invalid span or region, excess
cost, timeout, or cancellation fails closed.

The provider returns candidates rather than a contract. PRD candidates require
exact UTF-8 byte spans and may carry confidence and unresolved product
decisions. Design candidates require image dimensions, coordinate space,
in-bounds regions, semantic hierarchy, and matching geometric hierarchy.
Provider responses cannot contain pre-approved citations. Candidate and
variant identifiers, hierarchy, sizes, and counts are locally admitted before
merge.

```text
digest-bound PRD / design evidence
                 |
                 v
     injected generation provider
                 |
                 v
 typed candidates + uncertainty + evidence
                 |
        deterministic conflict set
                 |
                 v
     explicit human review decisions
                 |
                 v
 checked ACL Surface Contract draft
```

Merge is lossless and deterministic: disagreements become stable, explicit
conflicts instead of an implicit source preference. Review must select one
candidate for each approved variant/element, resolve every relevant conflict,
and reject candidates that depend on an unresolved product decision. The
reviewed artifact retains the complete generated draft, provider identity,
usage, request ID, rejected candidates, conflicts, resolutions, and reviewer
decisions for audit. Only selected sources become reviewed provenance, and
source spans become ACL citations. Browser observation still happens later and
independently.

The Rust trait and transport-neutral wire contract share one source of truth.
`a3s-test-agent` derives request and response schemas from the admitted wire
types and exposes protocol `a3s.test.contract-generation-provider/1`; the CLI
prints the complete bundle with `a3s-test provider schema
contract-generation`. The bundle declares candidate-only authority and the
review, digest, deadline, cost, identity, and non-mutation invariants. It does
not prescribe stdio, HTTP, RPC, or an in-process transport.

The standard HTTP projection lives in `a3s-test-agent`, not Core. It adds only
deployment plumbing: a fixed typed endpoint, optional secret-safe authorization,
version envelope, bounded bodies, deadline, status/media-type admission, and
typed remote errors. Redirects and environment proxies are disabled. HTTPS is
required except for explicit loopback HTTP used by a local inference service.
The adapter cannot choose a model, read source evidence, approve candidates, or
replace `ContractGenerationService` validation.

The CLI composes these existing boundaries without moving model or transport
concerns into Core. `a3s-test contract generate` parses one ACL workflow config,
resolves regular source files beneath that config directory, calculates their
digests locally, injects authorization from an explicitly named environment
variable, and calls the typed HTTP adapter. It atomically persists protocol
`a3s.test.contract-workflow/1` at the `generated` stage. That artifact has no
ACL contract and therefore cannot become a Runner expectation.

`a3s-test contract review` accepts that immutable candidate record plus a
separate ACL review document. It replays deterministic review admission,
requires explicit candidate decisions and applicable conflict resolutions,
round-trips and admits the canonical `surface_contract`, then publishes the ACL
and a `reviewed` audit artifact as one recoverable output pair. Loading a
saved artifact verifies its full-payload SHA-256 checksum, rehashes the retained local
source manifest under the original admission limits, recomputes conflicts and
open decisions, and replays the review again. A contract that is not the
deterministic result is rejected. The workflow artifact retains source paths
for local rehash and audit; it must be handled as workspace evidence, not as a
portable provider request or a browser observation. The checksum detects
accidental or unreviewed edits but is not a signature or an authenticity root;
repository review and workspace access controls remain authoritative.

### Understanding during rendering

The browser already computes DOM structure, accessibility semantics, CSS
layout, paint order, scrolling, and viewports. Replacing that engine would
duplicate a less accurate renderer. The embedded Test Kit instead derives a
versioned semantic projection after a stable browser frame:

- DOM and open Shadow DOM supply structure, roles, names, states, and locator
  candidates;
- `getBoundingClientRect()` and the visual viewport supply CSS-pixel geometry,
  visibility, occlusion, transforms, and scroll-container relationships;
- explicit component boundaries add stable ownership, source hints, readiness,
  and application facts without annotating every element;
- `MutationObserver`, `ResizeObserver`, route, viewport, and scroll signals
  invalidate the cached projection, while unchanged pages are never polled;
- private node IDs remain in a `WeakMap` side table. The runtime does not write
  testing metadata into application DOM attributes;
- the Web adapter captures the accessibility snapshot and Test Kit revision as
  one stable observation or rejects the race.

The overlay is optional and subordinate to the host page. Its default form is
a compact, non-modal instrument panel with a single-line header, dense
separated findings, explicit severity text, and named controls. It must not
introduce an application-style navigation shell, decorative header, or motion
that competes with the surface under review.

### Optional visual grounding

Some surfaces have no useful DOM or accessibility semantics: canvas, WebGL,
remote desktops, image-only controls, and design references. These may use an
injected visual-grounding provider after deterministic targeting fails or when
the caller explicitly requests visual grounding. The provider boundary
accepts a digest-bound screenshot, dimensions, natural-language query, current
observation ID, typed trigger, deadline, and cost budget, then returns bounded
point or box candidates with provider/model identity, confidence, coordinate
space, usage, and an optional request ID.

`a3s-test-agent::VisualGroundingService` independently validates the configured
and returned provider identity, rehashes the regular screenshot file against
its `sha256:<64 lowercase hex>` digest, and validates the
observation ID, dimensions, finite in-bounds geometry, confidence, strings,
candidate count, deadline, cancellation, and provider-reported cost. Screenshot
pixels or normalized coordinates are converted to current visual-viewport CSS
pixels before hit-testing visible, unobscured Test Kit nodes. Exactly one hit
can be upgraded to that node's current ref or preferred semantic locator.
Multiple hits are reported as ambiguous and never guessed.

Provider output is never a durable element identity. An unmapped or ambiguous
candidate remains image-bound and expires with the observation and screenshot
digest. Every result is explicitly `advisory`; it cannot independently pass a
blocking surface contract, authorize a repair, or mutate a surface. Model
transports, weights, runtimes, credentials, and licenses stay outside
`a3s-test-core` and the distribution. A deployment may implement the trait for
an externally hosted GUI-grounding model, but must perform its own license and
operational review. Research-only weights are not bundled or downloaded by A3S
Test.

The same boundary exposes protocol
`a3s.test.visual-grounding-provider/1` through `a3s-test provider schema
visual-grounding`. Its generated schemas cover observation and screenshot
binding, deadlines, costs, point/box geometry, coordinate spaces, identity,
and usage. The bundle states advisory authority and non-verdict/non-repair
invariants so a deployment adapter cannot mistake schema conformance for
execution authority.

The same transport supports a persistent local visual model server without
starting or downloading that runtime. Keeping model lifecycle deployment-owned
avoids per-request weight loading and keeps license, GPU capacity, credentials,
and health policy outside A3S Test. The adapter only exchanges a versioned
request and response; `VisualGroundingService` remains the authority boundary
for screenshot rehashing, current-observation binding, semantic hit-testing,
cost, and advisory provenance.

```text
DOM / AX / Test Kit semantic target ──success──> current semantic action
                 |
                 └─unrepresentable or explicit request
                                  |
                     verified screenshot + typed budget
                                  |
                     injected grounding provider
                                  |
                      admitted points / boxes
                                  |
              current geometry hit-test ──unique──> semantic target
                                  |
                         ambiguous / no hit
                                  |
                  observation-bound advisory candidate
```

Quality projection has its own bounded best-effort budget outside deterministic
step execution. A rejected, failed, cancelled, or hanging projection cannot
turn a completed contract verdict into a failure, cancellation, or scenario
timeout.

## Runtime layers

```text
Layer 6  Planner interface
         Coding Agent Skill and MCP stdio tools

Layer 5  Product interface
         session application layer + persistent CLI/MCP + deterministic CLI

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
both the interactive session protocol and deterministic ACL workflow. The MCP
stdio server projects the same `a3s-test-session` application layer; it does
not own a second driver or runner implementation.

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
    ├── project_quality_report(ContractReport) -> optional review projection
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
 Closing --cleanup succeeds------------------------> Reaped -> Reported
    |
    +--caller timeout/cancel--> CleanupInProgress --success--> Reaped
    |                              |
    |                              +--retryable failure--> CleanupRequired
    |                                                       |
    |                                  finish/abort----------+
    |
    +--non-retryable failure-----------------------> Failed -> Reported
    |
    +--second SIGINT-------------> kill registered command groups -> exit 130
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

1. Each Unix driver command runs in a new process group. Windows creates the
   command suspended, assigns it to a private kill-on-close Job Object, and
   resumes it only after assignment succeeds, so no descendant can win a
   launch-before-containment race. Windows Browser commands and CUA proxies
   also use `CREATE_NO_WINDOW`, including Browser `.cmd` shims. Every Browser
   command carries an explicit headed value; the default additionally enforces
   Chrome's `--headless=new` launch argument, while `--headed` is the sole
   visible-debugging opt-in.
2. Deterministic sessions retain every command boundary for the complete
   session. Timeout, cancellation, Drop, and cleanup terminate the boundary,
   wait for descendants to exit, and reap the direct command child. A single
   Unix EOF watchdog records all groups in that boundary and kills them if the
   host dies before Rust cleanup can run; normal cleanup also reaps the
   watchdog. After each command root is reaped, groups without descendants are
   removed from both registries so later PGID reuse cannot inherit ownership.
3. A persistent agent turn uses a temporary process boundary. Successful Unix
   commands stop and reap their watchdog, and successful Windows commands
   clear kill-on-close before releasing the Job handle, so the daemon survives
   the turn. Unsuccessful or cancelled commands leave containment armed and
   kill the full tree.
4. Normal scenario completion sends `close` and then terminates any survivor
   still inside the owned session boundary.
5. A stuck `close` falls back to the exact private PID file and validates the
   process before termination. Unix snapshots the command and descendants,
   then kills only their process groups; the `ps` snapshot command itself runs
   in a private group, writes bounded output outside a pipe, and is killed and
   reaped with its descendants. Windows performs a bounded command-line query
   without a back-pressured output pipe and calls `taskkill /T` only when an
   owned-browser marker matches;
   unavailable, timed-out, or mismatched identity evidence terminates nothing.
6. Dropping an unclosed deterministic or embedded session runs the same
   owned-session cleanup, then schedules an emergency `close` when no PID file
   exists yet.
7. External-planner session handles intentionally survive individual CLI
   processes; `finish`, `abort`, or the bounded daemon idle timeout closes
   them. `agent start` saves recovery metadata before dispatching the first
   browser command, and a failed start never deletes its runtime when exact
   cleanup also failed.
8. Browser daemons receive a bounded inactivity timeout. The adapter floors the
   daemon-side value at the per-command deadline because admitted 0.26.x
   runtimes start idle accounting when a command begins.
9. Restricted standalone 0.26.x sessions carry both the allowlist and an
   explicit Chrome engine selection. This selects the upstream launch path
   that installs request interception before the initial navigation; its
   implicit auto-launch path does not install the domain interceptor.
10. A second SIGINT terminates all currently registered command and session
    boundaries before exit.

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

The adapter maps typed `Action` values to pointer, form, keyboard, wheel,
viewport, tab, frame, dialog, upload, artifact-scoped download, network, HAR,
trace, video, screenshot, accessibility, console, and page-error commands. It
exposes a full A3S Browser accessibility snapshot through
`DriverSession::observe`. It does not parse natural-language intent. Refs,
semantic locators, and CSS locators remain explicit target types in both
deterministic and agentic execution.

Every Web evidence path is resolved beneath a canonical session artifact root.
The adapter prepares descendant directories one component at a time and rejects
symbolic links, Windows reparse points, and non-directory components before a
path is passed to the browser or used for adapter-written JSON. Screenshot,
download, HAR, trace, and video commands are not considered successful evidence
merely because the process exits zero: a validated prior regular file is
removed before a new capture, and the command must create a fresh regular file
that still canonicalizes inside the same root. Persistent video reconnection
uses a separate admission path so an in-progress recording is validated but
not deleted when saved state is reopened.

Each Web session also binds its canonical runtime/socket directory to a stable
filesystem identity. The binding is checked before every browser command and
before PID-based emergency cleanup, so removing the directory and replacing it
with either a link/reparse point or a different directory at the same path
dispatches no browser command. Cleanup admits only regular PID sidecars under
the bound root; linked PID files and linked namespace-directory components are
never followed. Persistent CLI sessions add a separate workspace/session owner
marker check when their saved state is loaded and again before runtime removal.

External-planner Web sessions apply two complementary navigation boundaries.
`BrowserNetworkPolicy` supplies a normalized, bounded hostname allowlist to
every browser command. It blocks cross-domain page links, redirects, scripts,
images, fetches, and related network activity before A3S Test receives another
turn. Separately, explicit URL actions and every successful observation are
checked against exact HTTP(S) origins, including scheme and effective port.
`--allow-origin` expands both layers; `--allow-domain` expands only the network
layer. The latter is not subresource-only: an upstream hostname filter also
admits document requests, while A3S Test still rejects explicit navigation and
subsequent observations unless the exact origin was separately admitted. The
browser protocols expose hostname semantics, not scheme-and-port semantics, so
the network layer alone must not be described as exact-origin enforcement.

The upstream daemon reads its hostname policy at daemon creation, not as a
per-command authorization update. Persistent state therefore stores the exact
normalized policy used at session start. Legacy state without that field is a
one-way compatibility boundary: metadata inspection and terminal cleanup are
allowed, while observation and action turns fail closed. Cleanup connects
without claiming that a newly supplied environment value retrofitted the
already-running daemon.

The shared action protocol is revisioned independently of browser executable
versions. Revision 2 adds Office-grade interactions. A basic interaction uses
the browser's semantic `find` protocol when that subaction exists. Operations
that the current standalone semantic protocol cannot express require a direct
ref or CSS selector. Context-click is implemented as visible-center pointer
movement plus a cancelable page `contextmenu` event, avoiding an unobservable
browser-native menu; drag scrolls both endpoints into view; wheel owns modifier
press/release cleanup.

Revision 6 is the current cross-surface contract. Revision 4 adds GUI
automation-ID targets, revision 5 adds observation-scoped visual points, and
revision 6 reserves `verify_contract` for deterministic runner execution. Web,
GUI, interactive-agent, and MCP action paths reject or omit that runner-owned
action instead of silently dispatching it to a surface.

The adapter keeps configuration, command execution, protocol mapping, session
behavior, and host-process supervision in separate modules. The public crate
surface remains limited to typed configuration, the driver, the injectable
command executor, and the emergency termination entry point.

Web integration tests own a pair of loopback-only HTTP servers. The primary
server binds an operating-system-assigned port and serves deterministic form,
navigation, redirect, and advanced-interaction routes. The second origin is a
request-recording sentinel for link, script, image, fetch, and redirect
containment coverage. Both servers start only after their listeners are bound,
emit `Cache-Control: no-store`, cap request headers, and stop plus join their
worker threads on drop. No fixed port or external website is part of the E2E
contract.

The normal workspace gate exercises the fixture's route and lifecycle
contract without a browser. A dedicated macOS CI job installs the exact
admitted standalone `agent-browser` 0.26.0 runtime and runs the ignored real
browser test explicitly. That path verifies semantic label targeting, form
submission, an assertion, same-origin navigation, non-empty screenshot
evidence, browser-level cross-domain containment with zero sentinel requests,
and removal of the private browser runtime directory.
The ordinary Rust quality job is a fail-fast-disabled macOS, Linux, and Windows
matrix; the real Chrome path remains isolated in its pinned macOS job.

Command executor errors carry a typed dispatch phase. Only an unavailable
executable before dispatch is retryable. A timeout or output failure may have
already applied an action and is therefore never retried. The runner bounds
retry count and backoff inside the scenario deadline. Scenario concurrency is
also bounded and report order remains the manifest order.

## GUI driver

The GUI driver adapts A3S CUA capabilities rather than copying its
implementation. The first implemented boundary lives in
`a3s-test-driver-gui`: typed endpoint and application configuration, MCP
JSON-RPC envelopes, fail-closed capability admission, and an injectable
`CuaTransport`. Platform APIs and CUA implementation types do not enter the
core crate.

`compat/cua-stack.acl` is the source of truth for the reviewed CUA Git
revision, exact driver version, MCP protocol, tools-list schema, capability
vocabulary, required tools, required per-tool capabilities, and the complete
platform/endpoint execution matrix. The adapter rejects a daemon before
session creation when any locked contract is missing or incompatible. It
consumes structured protocol fields and never parses human-readable tool
summaries. See
[`ADR 0001`](adr/0001-gui-cua-adapter.md).

The semantic GUI profile now emits:

- bounded accessibility elements with A3S observation-bound refs;
- exact window/application identity and element frames;
- labels, roles, values, optional automation IDs, and parent refs;
- a snapshot generation that expires after every state-changing action.

CUA snapshot IDs, element indices, and element tokens never enter the core
model or returned observation. The adapter resolves role, text, label,
automation-ID, and current-ref targets exactly, rejects ambiguous matches, and
fails stale refs before input dispatch.

Long-lived sessions do not trust the opening-time PID and window ID forever.
Immediately before every observation and effectful action, the adapter lists
applications and windows again, proves that the configured application
identity still owns the PID, and proves that the bound top-level window still
belongs to that process. `application_binding_lost` or `window_binding_lost`
invalidates the current semantic and visual generation and prevents the CUA
input tool from being called. Cleanup performs its own ownership check and
never follows a reused PID.

The implemented semantic milestone covers application launch or attach,
deterministic window selection, semantic click/double-click/context-click,
fill/type, key press, cardinal scroll, assertions, PNG window evidence, and
cleanup of the launched process.

The window-vision profile adds one fresh, window-scoped PNG to every
observation. Its session artifact root is canonicalized once. Requested
descendant directories are created and inspected one component at a time;
symbolic links and Windows reparse points are rejected before CUA receives a
path. The adapter then validates that the returned file still canonicalizes
inside the root, is a bounded regular file, and has the reported dimensions and
media type before hashing its bytes and issuing an A3S `@vN` reference. A pixel
target must name the latest visual reference, remain within the verified image
bounds, and pass the containment, file-type, and digest checks again immediately
before input dispatch. Pixel click, double-click, context-click, type, scroll,
and drag return the grounding image and digest as evidence. Embedded LLM
requests carry that image as an explicit attachment; the text prompt is not
asked to reconstruct pixels from a path.

Application launch or attachment is trusted host configuration, not an agent
action. A launched process is owned and reaped by the session; an attached
process is never killed. The initial capture scope is strictly window-scoped.
CUA element tokens remain private to the adapter and are projected as A3S Test
observation-bound refs.

Opening also owns a cancellation guard. The bounded ownership-acquisition task
continues after caller cancellation, so a `launch_app` response that arrives
after cancellation still establishes the exact PID before cleanup. Cancelling
permission, application, or window discovery therefore reaches the same
identity-safe cleanup as a completed `GuiSession`; a launched process cannot
fall between the open future and normal session ownership.

The locked execution matrix is deliberately fail-closed:

| Platform | Installed daemon | Embedded socket |
| --- | --- | --- |
| macOS | `contract_tested` | `contract_tested` |
| Windows | `unsupported` | `unsupported` |
| Linux | `unsupported` | `unsupported` |

`contract_tested` means the checked-in fake CUA contract, permission
attribution, semantic/visual behavior, runtime binding-drift gates, ownership
rules, and cleanup stress tests pass. It is not a claim that a particular host
has granted permissions.
`a3s-test gui-certify` performs that real-host observation and cleanup check;
release automation still needs to record a macOS host result. Windows and
Linux fail during configuration, before a transport starts, because the
locked CUA 0.10.0 revision has no reviewed application backend for them.

The CUA stdio proxy has lifecycle ownership independent of the target
application. On Unix it starts in a new process group that remains registered
for the CLI emergency interrupt path; an EOF watchdog kills that group if the
host is terminated before Drop can run. On Windows it is created suspended,
assigned to a private Job Object configured with `KILL_ON_JOB_CLOSE`, and only
then resumed. Normal protocol shutdown first closes stdin within a fixed
deadline and then terminates and waits for remaining descendants. Cancellation
guards on requests, notifications, and close signal the tree before releasing
the transport lock. Request timeout, malformed or truncated protocol data,
early proxy exit, transport drop, and emergency shutdown use the same tree
boundary and synchronously reap the direct proxy on final Drop. A failure to
establish that boundary aborts transport admission and reaps the just-started
process tree.

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
agent click/hover/type/drag/wheel/... or agent act
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
session root. Successful observations must also report an admitted HTTP(S)
origin, so a detached or page-driven replacement is surfaced before new refs
are issued. The initial and `--allow-origin` hostnames also form a browser-level
domain allowlist; `--allow-domain` adds a hostname only to that network layer,
not to the exact-origin action and observation gates. The browser runtime uses
an isolated namespace, an ownership marker, and a bounded idle timeout so
persisted metadata cannot redirect cleanup and an abandoned external planner
does not leave an unbounded process. Descriptive external session names are
preserved in state and reports; a stable SHA-256 suffix compacts only the
driver-facing session identifier when Unix socket paths require it.

Once a path is understood, the coding agent can author ACL and use
`check --json` and `run --json` for deterministic regression coverage.

The MCP stdio server is a thin projection of the same session application
layer. It exposes `test_session_start`, `test_observe`, `test_act`,
`test_finish`, `test_abort`, and `test_schema`. It enforces the exact
`initialize -> notifications/initialized -> operation` lifecycle and protocol
version from the
[MCP 2025-06-18 lifecycle](https://modelcontextprotocol.io/specification/2025-06-18/basic/lifecycle),
and its start schema advertises only drivers actually registered by the host.
Per-session turns are serialized, active session count is bounded, failed
observations invalidate earlier refs, and cancelled opens release their name
and capacity reservation. EOF closes independent surfaces concurrently, each
within the configured cleanup deadline. Host-side GUI target configuration
cannot be changed by a tool call. Deterministic `check` and `run` remain CLI
operations rather than a second MCP runner.

Terminal cleanup is also a session state, not a fire-and-forget side effect.
The manager reserves the session name while `close()` runs in an owned task. A
caller deadline or cancellation stops waiting but does not cancel that task;
turns and duplicate terminal calls return the retryable
`test.session.cleanup_in_progress`. EOF shutdown waits one cleanup deadline for
these tasks to drain. Eventual success reaps the session, while a retryable
driver failure restores the same driver session as `CleanupRequired`, blocks
`observe` and `act`, and permits another `finish` or `abort`. GUI cleanup leaves
the CUA transport and window session open when an identity-safe app termination
or `end_session` tool call reports a retryable failure, so the retry continues
from the same ownership proof. Non-retryable ownership loss still ends the CUA
session without terminating an unrelated process.

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
and action output. Provider failures preserve retryability. A single typed
provenance redactor runs at the result boundary: it removes common structured
credential fields and secret-bearing action payloads by default, sanitizes URL
credentials/query/fragment, and replaces host-registered exact secret values
throughout unstructured trace text. Provider requests and driver inputs remain
operationally complete; they are trusted transient inputs, not persistence
formats. Evidence files retain their normal artifact access controls.

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
.a3s-test/mcp-sessions/<session>/
.a3s-test/gui-certification/gui-certification/
```

Relative artifact paths are admission-checked and cannot escape this root. The
Web adapter currently records screenshots, accessibility JSON, console and
page-error JSON, downloads, HAR, traces, and WebM video. The GUI adapter
records explicit window screenshots and digest-bound grounding evidence.
Terminal recordings remain planned with the TUI driver.

Reports separate assertion failure from infrastructure failure and cleanup
failure. This distinction is required for an agent to choose whether to repair
product code, repair a test, or retry infrastructure.
