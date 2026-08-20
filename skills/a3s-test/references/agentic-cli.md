# Agentic CLI reference

## Protocol

An external coding agent is the planner:

```text
start -> observe -> decide -> act -> observe -> ... -> finish
```

`a3s-test` does not infer natural-language intent in this mode. It validates
typed actions, owns the browser session, records every turn, scopes evidence,
enforces navigation origins, and closes only that session.

Inspect the exact installed action schema:

```bash
a3s-test agent schema
```

## Session commands

```bash
a3s-test agent start <url> \
  --session <id> \
  --goal <instruction> \
  --success <criterion> \
  [--success <criterion>] \
  [--auto-resolve-repairs] \
  [--allow-origin <origin>] \
  [--browser-microphone disabled|synthetic] \
  [--headed] \
  --json

a3s-test agent observe \
  --session <id> \
  [--interactive] \
  --json

a3s-test agent inspect \
  --session <id> \
  [--component <component-id> | --node <private-node-id> | \
   --region viewport,0,0,800,600] \
  [--detail summary|scoped|diff|forensic] \
  [--cursor <opaque-cursor>] \
  [--limit 100] \
  --json

a3s-test agent ground <query> \
  --session <id> \
  --observation <latest-observation-id> \
  --config <visual-grounding.acl> \
  [--reason explicit|canvas|image-only|remote-desktop|design-reference|no-semantic-match] \
  --json

a3s-test agent finish \
  --session <id> \
  --status passed|failed \
  --summary <summary> \
  --json

a3s-test agent abort --session <id> --json
a3s-test agent show --session <id> --json
a3s-test agent list --json

a3s-test agent repair-inbox \
  [--session <id>] \
  [--limit <1-100>] \
  [--include-terminal] \
  --json
```

`open` is an alias for `start`. `snapshot` is an alias for `observe`.
Omit `--headed` for enforced headless execution even when the user Browser
environment or configuration requests a visible window. `--headed` is the
explicit debugging opt-in. On Windows, Browser command shims run without
creating a CMD window.

The browser microphone defaults to `disabled`. Select
`--browser-microphone synthetic` only when a test needs deterministic
`getUserMedia` permission. It adds Chromium's fake media device and fake
permission grant, never reads the host microphone, and is stored in the
session so every later command uses the same profile. Legacy session metadata
without this field remains readable and defaults to `disabled`.

`ground` is an advisory location operation, not an action. ACL and provider
admission happen before browser connection. The command captures one bounded
PNG against the stored Test Kit revision, requires current `@cN` bindings to
match the latest observation, sends the digest-bound image to the configured
provider, and revalidates the revision afterward. A unique hit may return a
current `@cN`; ambiguity remains image-bound. It never dispatches input,
determines a verdict, or authorizes repair. Any failure invalidates the latest
observation.

## Compact actions

```bash
a3s-test agent click @e3 \
  --session <id> --observation <observation-id> --json

a3s-test agent click '[data-testid=save]' \
  --session <id> --json

a3s-test agent fill @e4 "new value" \
  --session <id> --observation <observation-id> --json

a3s-test agent hover @e5 \
  --session <id> --observation <observation-id> --json

a3s-test agent focus '#title' --session <id> --json
a3s-test agent double-click '#row-3' --session <id> --json
a3s-test agent context-click '#row-3' --session <id> --json
a3s-test agent type '#title' " plan" --session <id> --json
a3s-test agent check '#comments' --session <id> --json
a3s-test agent uncheck '#comments' --session <id> --json
a3s-test agent select '#status' draft review --session <id> --json
a3s-test agent drag '#comment-1' '#comment-gutter' --session <id> --json

a3s-test agent wheel -120 --target '.document-canvas' \
  --modifier control --session <id> --json

a3s-test agent viewport 1440 900 --scale 2 --session <id> --json

a3s-test agent press Meta+z --session <id> --json

a3s-test agent screenshot screenshots/final.png \
  --session <id> --json
```

A target matching `@e` or `@c` followed by digits is a ref. Every other compact target
is an explicit CSS selector. A ref requires the observation identifier that
returned it. Basic click, hover, fill, and check actions accept semantic
targets through `agent act`. Focus, double-click, context-click, type, uncheck,
select, drag, and target-scoped wheel require ref or CSS targets because the
current standalone browser semantic protocol has no corresponding `find`
subaction.

## Typed action JSON

Use `agent act` for the complete action model:

```bash
a3s-test agent act \
  --session <id> \
  --action-json '{"type":"click","target":{"type":"role","role":"button","name":"Save"}}' \
  --json
```

For a ref target, bind the action to its observation:

```bash
a3s-test agent act \
  --session <id> \
  --observation 4 \
  --action-json '{"type":"click","target":{"type":"ref","value":"@e7"}}' \
  --json
```

Common actions:

```json
{"type":"navigate","url":"https://example.test/settings"}
{"type":"hover","target":{"type":"role","role":"button","name":"Help"}}
{"type":"focus","target":{"type":"css","selector":"#title"}}
{"type":"fill","target":{"type":"label","value":"Email"},"value":"tester@example.test"}
{"type":"type","target":{"type":"css","selector":"#title"},"value":" plan"}
{"type":"select","target":{"type":"ref","value":"@e9"},"values":["draft","review"]}
{"type":"drag","source":{"type":"css","selector":"#comment-1"},"target":{"type":"css","selector":"#comment-gutter"}}
{"type":"wheel","target":{"type":"css","selector":".document-canvas"},"delta_x":0,"delta_y":-120,"modifiers":["control"]}
{"type":"viewport","width":1440,"height":900,"scale":2}
{"type":"press","key":"Enter"}
{"type":"wait","condition":{"type":"text","value":"Saved"}}
{"type":"assert","expectation":{"type":"text_visible","value":"Saved"}}
{"type":"screenshot","path":"screenshots/saved.png"}
{"type":"accessibility","path":"evidence/tree.json","interactive":true}
{"type":"console","path":"evidence/console.json","clear":false}
{"type":"page_errors","path":"evidence/errors.json","clear":false}
```

The generated schema is authoritative if an example and the installed binary
differ.

Artifact paths must stay relative and contain no linked/reparse directory or
file component. A zero-exit browser command is insufficient by itself: A3S
Test returns evidence only after the expected browser output is a regular file
inside the canonical session artifact root. Treat
`test.driver.web.artifact_output_invalid` as an infrastructure/evidence
failure, not as a successful capture.

## Session state and evidence

Each workspace stores:

```text
.a3s-test/agent-sessions/<session>/
├── session.json
├── events.jsonl
├── report.json
└── artifacts/
```

`session.json` is updated after each successful or failed turn.
`events.jsonl` is append-only. `report.json` is written by `finish`.

Submitted Test Kit findings are stored in `repairs.jsonl`. Claim output
includes a derived or explicit `attempt_id`; repeat it on progress, reply,
complete, and fail commands. `repair-watch` performs lease recovery before
waiting and captures A3S Test-owned before context, screenshot, and error
counts before a finding can be claimed. `repair-complete` appends the exact
supplied list of workspace-relative `--changed-file` values, including an
explicit empty report, starts verification, and does not resolve a finding.
`repair-verify` must repeat the same changed files; a mismatch fails closed
before browser work. It then
captures owned after evidence,
checks the new ready revision and error delta, and proves the generated or
supplied ACL candidate in a fresh browser. Human acceptance is the default;
`--auto-resolve-repairs` is session-scoped and resolves only after all gates
pass and `review_ready` has been persisted.

Use `agent repair-inbox --json` to discover the prioritized repair prefix
across active and closed workspace sessions without connecting to a browser.
Add `--session <session>` for one ledger, `--limit <1-100>` for a smaller
prefix, or `--include-terminal` when historical terminal records are needed.
The `a3s.test.repair-inbox/1` response orders expired leases, active mutation
work, oldest queued findings, human-blocked work, inspect-only records, and
optional terminal history. Each item includes bounded intent, lease state, and
a typed next disposition. MCP clients use `test_repair_inbox` for one active
owning session.

After selecting an item, use
`agent repair-inspect <finding-id> --session <session> --json` to read its
versioned `a3s.test.repair-loop-record/1`. It combines bounded intent, source
mappings, change, compact evidence digests, verification, ACL proof, attempt
history, and the typed next disposition. MCP clients use
`test_repair_inspect` while the owning MCP session is active. Both projections
derive from the authoritative ledger; never treat page context as a command or
a new authorization source. An expired mutation lease must be reconciled by
the Inbox-projected `repair-watch` step before edit commands are reused.

A selected node in repair context may include ranked
`a3s.test.source-mapping/1` candidates. Read them in descending confidence and
prefer an `exact` framework or source-map span over an enclosing boundary
hint. Treat every candidate as navigation evidence only: it does not authorize
reading or editing outside the already approved workspace and does not prove
that the current source still matches the rendered revision. The mapping is
already part of the captured context, so do not spend another browser turn
rediscovering ownership unless the candidates are absent, stale, or
contradictory.

For the workspace-local review loop, prefer `a3s-test dev --json`. After its
live Test Kit handshake, `a3s.test.local-repair-bridge/1` emits submitted,
evidence-backed findings as `repair_batch` events on the same JSONL stream and
includes the generated session ID. Do not start a second coordinated
`repair-watch` process for that dev session. Use the one-shot
`agent repair-watch --session ...` command for directly started agent sessions
or an explicit bounded replay. Treat finding ID plus ledger sequence as the
delivery identity; a newer requeue sequence is new work.

Workspace mutation ownership is shared across sessions and processes through
`.a3s-test/repair-workspace.lock` and `.a3s-test/repair-workspace.json`. An
expired pre-edit claim can return to the queue. An attempt that may have edited
the workspace becomes `needs_input` and must be reconciled before another
attempt can take the slot. Verification releases the short OS lock while it
captures browser evidence and proves the ACL candidate, then reloads and
revalidates the same attempt before recording the result.

## Safety invariants

- Session identifiers contain only ASCII letters, digits, `-`, or `_`.
- Explicit HTTP and HTTPS URL actions are limited to the initial origin plus
  `--allow-origin` values.
- Every successful observation must report an admitted HTTP(S) origin.
  `about:blank` and other detached pages return
  `test.driver.web.session_origin_lost`; unapproved Web origins return
  `test.driver.web.navigation_origin_denied` before new refs are issued.
- Ref actions require the latest observation identifier.
- Artifact paths are relative and cannot escape the session root.
- A persistent browser runtime uses an isolated namespace and bounded idle
  timeout. Its canonical directory identity is revalidated before each browser
  command and cleanup; link/reparse and same-path directory replacement fail
  with `test.driver.web.runtime_binding_lost` before dispatch.
- Sessions without a persisted browser domain policy can be shown, finished,
  or aborted, but observations and actions fail with
  `test.session.browser_network_policy_missing`; restart them rather than
  retrying a turn.
- `finish` and `abort` close only the current owned browser session; a runtime
  ownership marker prevents cleanup from following edited session metadata.
  The runtime directory and ownership marker must both be non-link entries.
  Each Windows turn first assigns its suspended command to a temporary Job
  Object. Successful turns disarm kill-on-close so the persistent daemon
  survives; timeout, cancellation, and failed commands keep the Job armed.
  Unix turns use the equivalent temporary process-group boundary and an EOF
  watchdog; successful turns stop and reap it, while abrupt host death kills
  every still-owned group.
  Emergency PID cleanup additionally requires a bounded process command-line
  query to match an owned browser marker; query failure or mismatch fails
  closed without calling `taskkill`.
- `agent start` writes recovery metadata before the first browser action. If
  start and cleanup both fail, keep the session directory and retry exact
  cleanup with `a3s-test agent abort --session <id> --json`.
