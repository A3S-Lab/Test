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
  [--allow-origin <origin>] \
  [--headed] \
  --json

a3s-test agent observe \
  --session <id> \
  [--interactive] \
  --json

a3s-test agent finish \
  --session <id> \
  --status passed|failed \
  --summary <summary> \
  --json

a3s-test agent abort --session <id> --json
a3s-test agent show --session <id> --json
a3s-test agent list --json
```

`open` is an alias for `start`. `snapshot` is an alias for `observe`.

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

A target matching `@e` followed by digits is a ref. Every other compact target
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

## Safety invariants

- Session identifiers contain only ASCII letters, digits, `-`, or `_`.
- Explicit HTTP and HTTPS URL actions are limited to the initial origin plus
  `--allow-origin` values.
- Ref actions require the latest observation identifier.
- Artifact paths are relative and cannot escape the session root.
- A persistent browser runtime uses an isolated namespace and bounded idle
  timeout.
- `finish` and `abort` close only the current owned browser session; a runtime
  ownership marker prevents cleanup from following edited session metadata.
