---
name: a3s-test
description: Drive interactive agentic Web tests or author deterministic A3S Test ACL suites. Use when a coding agent needs to explore a Web application, reproduce a UI bug, make observe-decide-act testing decisions, capture bounded evidence, turn a discovered workflow into regression coverage, or diagnose a3s-test JSON results.
---

# A3S Test

Use `a3s-test` as the test engine and the current coding agent as the planner.
Do not call a second LLM to decide the next action. A3S Test owns persistent
surface sessions, typed actions, assertions, evidence, reports, and cleanup.

## Choose the right mode

- Use an **agent session** for exploration, bug reproduction, UX review, and
  any workflow where the next action depends on the latest observation.
- Use an **ACL suite** for a known regression flow that should run
  deterministically in local development and CI.

Do not force an uncertain workflow into ACL before observing the product.
After an agent session proves a stable path, promote the smallest useful path
to an ACL suite.

## Interactive agentic workflow

1. Inspect the project for fixture commands, test accounts, existing A3S Test
   sessions, ACL suites, and artifact conventions.
2. Verify the installed Web adapter and read the typed action protocol:

   ```bash
   a3s-test capabilities --json
   a3s-test agent schema
   ```

3. Start one workspace-local session with a concrete goal and observable
   success criteria:

   ```bash
   a3s-test agent start http://127.0.0.1:3000 \
     --session checkout \
     --goal "Complete checkout with the test fixture account" \
     --success "The confirmation heading is visible" \
     --json
   ```

   Add `--allow-origin` only for another origin the test must intentionally
   visit. Use `--browser-driver standalone` only when the project explicitly
   uses a compatible standalone `agent-browser`.

4. Observe before deciding:

   ```bash
   a3s-test agent observe --session checkout --interactive --json
   ```

   Read the returned `observation_id`, semantic snapshot, current URL, and
   visible state. Decide the next action from that evidence.

5. Execute exactly one typed action. Common actions have compact commands:

   ```bash
   a3s-test agent click @e3 \
     --session checkout --observation 1 --json

   a3s-test agent fill @e4 "tester@example.test" \
     --session checkout --observation 2 --json

   a3s-test agent context-click @e6 \
     --session checkout --observation 3 --json

   a3s-test agent wheel -120 --target '.document-canvas' \
     --modifier control --session checkout --json

   a3s-test agent press Enter --session checkout --json

   a3s-test agent screenshot screenshots/confirmation.png \
     --session checkout --json
   ```

   Use `agent act --action-json` for semantic targets, waits, assertions,
   tabs, frames, dialogs, network controls, or advanced evidence. Read
   [references/agentic-cli.md](references/agentic-cli.md) before constructing
   those actions.

   Focus, double-click, context-click, type, uncheck, select, drag, and
   target-scoped wheel require a ref from the latest observation or explicit
   CSS selector with the current browser protocol. Do not invent semantic
   fallbacks for unsupported subactions.

6. Observe again after every state-changing action. A ref such as `@e3` is
   accepted only with the latest observation identifier. Never reuse a ref
   after navigation or a dynamic update.
7. Keep acting until every success criterion is directly supported by an
   assertion or captured evidence. Then finish:

   ```bash
   a3s-test agent finish \
     --session checkout \
     --status passed \
     --summary "Checkout completed and confirmation was observed" \
     --json
   ```

   Use `--status failed` when the product violates a criterion. Use
   `a3s-test agent abort --session checkout --json` only when the test itself
   cannot continue safely.

## Deterministic regression workflow

1. Read [references/web-acl.md](references/web-acl.md).
2. Put project-owned suites under the project's test tree, normally
   `tests/e2e/`.
3. Prefer role, label, test ID, and placeholder targets over CSS. Use refs
   only when a preceding snapshot makes them stable inside the same scenario.
4. Validate before running:

   ```bash
   a3s-test check tests/e2e/smoke.acl --json
   ```

5. Run with machine-readable output:

   ```bash
   a3s-test run tests/e2e/smoke.acl --json
   ```

6. Distinguish a product failure, a test-specification failure, and an
   infrastructure failure before editing code.

## Evidence

Capture the smallest evidence set that proves the result:

- Use a screenshot for visible final state.
- Use accessibility output for semantic structure.
- Use console and page-error output for browser failures.
- Record HAR, trace, or video only around the relevant flow; these artifacts
  can be large.
- Keep artifact paths relative. A3S Test confines them to the session or
  scenario artifact directory.

Never place passwords, tokens, cookies, or production data in action JSON, ACL
values, network mocks, console fixtures, or committed evidence.

## Reliability and cleanup

- Drive one command at a time per agent session.
- Treat navigation origin denial and stale-ref rejection as protocol safety,
  not failures to bypass.
- Do not kill Chrome, A3S Browser, or agent-browser by process name.
- Finish or abort every session. A3S Test closes only the exact session it
  owns and retains the report and evidence.
- For deterministic runs, the first `Ctrl+C` requests bounded cleanup. A
  second `Ctrl+C` terminates only process groups owned by that run.
- Do not add arbitrary sleeps. Use typed observations, waits, and assertions.

## Diagnosis

Use `test.spec.*` errors to repair ACL admission. Use
`test.driver.web.*` errors for browser compatibility, protocol, or lifecycle
problems. Use `test.assert.*` errors to compare expected product state with
evidence. Use `test.run.*` errors for deterministic-run cancellation,
deadlines, and cleanup. Agent-session metadata and events live under
`.a3s-test/agent-sessions/<session>/`.
