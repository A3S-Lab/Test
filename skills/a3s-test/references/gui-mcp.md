# GUI MCP reference

## Host boundary

The host starts `a3s-test mcp` with a trusted CUA policy, endpoint,
application identity, launch/attach mode, window selector, and perception
profile. These values are not agent tool arguments. Do not ask to change the
application executable, capture scope, or CUA policy during a session.
The MCP client must negotiate protocol `2025-06-18` and send
`notifications/initialized` before listing or calling tools.

Inspect the platform matrix before configuring a worker:

```bash
a3s-test gui-certification --json
```

The locked CUA 0.10.0 adapter admits macOS installed-daemon and embedded-socket
profiles. Windows and Linux GUI profiles fail closed until their CUA backends
are reviewed. A macOS worker should pass `a3s-test gui-certify` with its real
application and permissions before use.

Version tags additionally require the repository's reusable real macOS
certification workflow. Its `a3s.test.gui-host-certification/1` record binds
the exact A3S Test and CUA source revisions, binary and policy digests, host
permissions, semantic and window-vision observations, and zero surviving
fixture processes. The detached checksum and GitHub OIDC/Sigstore provenance
are the release proof; a local JSON result alone is not a release
certification.

## Tool loop

Use exactly this state machine:

```text
test_session_start -> test_observe -> test_act -> test_observe -> ...
                   -> test_finish
```

Call `test_abort` if the test cannot continue safely. Always finish or abort
the session; the MCP server also closes active sessions on EOF.

If `test_finish` or `test_abort` returns `cleanup_error.retryable: true`, the
session is retained only for cleanup. Do not call `test_observe` or `test_act`;
`test.session.cleanup_in_progress` means the dispatched close is still running,
while `test.session.cleanup_required` means it ended with a retryable failure.
Retry `test_abort` or `test_finish` with the same session identifier.

Start with an explicit goal and observable success criteria:

```json
{
  "session": "editor-save",
  "surface": "gui",
  "goal": "Save the open document",
  "success_criteria": ["The document reports a saved state"]
}
```

Observe before the first action. Use `test_schema` as the authoritative action
contract. Send one typed action at a time:

```json
{
  "session": "editor-save",
  "observation_id": 1,
  "action": {
    "type": "click",
    "target": { "type": "ref", "value": "@g1.3" }
  }
}
```

Semantic refs (`@gN.M`) and visual refs (`@vN`) are bound to the latest
observation. Include its `observation_id` and observe again after every
state-changing action. Never reuse a ref after an action or another
observation. A failed observation also invalidates the previous refs; obtain a
new successful observation before acting.

## Grounded visual actions

Prefer exact role, text, label, automation-ID, or semantic ref targets. Use a
visual point only when accessibility cannot identify the intended control and
the window-vision observation includes the relevant pixels:

```json
{
  "session": "editor-save",
  "observation_id": 2,
  "action": {
    "type": "click",
    "target": {
      "type": "visual_point",
      "snapshot": "@v2",
      "x": 420,
      "y": 96
    }
  }
}
```

Use only coordinates grounded in the attached PNG evidence. The adapter
rejects an old visual ref, out-of-bounds point, changed screenshot digest, or
reused CUA snapshot before input. It also rejects artifact-directory links,
Windows reparse points, linked files, and files resolving outside the
canonical session artifact root. Visual actions return the grounding image and
digest as evidence.

## Cleanup and diagnosis

- A launched application is terminated only when the adapter proves it was
  absent before launch and its PID still has the configured identity.
- An attached or pre-existing application is never terminated.
- The CUA proxy is a separate owned process tree. Timeout, protocol failure,
  transport drop, or emergency interruption terminates that proxy tree without
  changing the launch/attach ownership rule for the tested application.
- `test.session.stale_observation` means the action did not name the latest
  observation ID.
- `test.driver.gui.stale_reference` and `test.driver.gui.stale_image` require a
  new observation; do not retry the same target.
- `test.driver.gui.application_binding_lost` and
  `test.driver.gui.window_binding_lost` mean the host-selected PID or window
  changed. The adapter invalidates current refs and dispatches no input; finish
  or abort instead of following the replacement.
- Permission errors are non-prompting. The host must grant Accessibility and
  Screen Recording to the reported daemon or embedded-host identity.
- Ambiguous semantic or window matches require a more exact host selector or
  target; do not pick one by order.
