---
name: a3s-test
description: Author, validate, run, and diagnose A3S Test ACL end-to-end suites for Web applications. Use when a coding agent needs to add or repair browser E2E coverage, reproduce a Web UI bug, collect screenshots/HAR/traces/video/console evidence, exercise tabs/frames/dialogs/uploads/downloads/network mocks, or interpret a3s-test JSON results in local development or CI.
---

# A3S Test

Use typed ACL scenarios for known workflows and machine-readable results for
diagnosis. Keep product assertions deterministic. Do not replace typed actions
with keyword routing or shell scripts that guess user intent.

## Workflow

1. Inspect the repository for existing A3S Test manifests, fixture commands,
   test accounts, and artifact conventions.
2. Verify the installed Web adapter before launching a browser:

   ```bash
   a3s-test capabilities --json
   ```

   If the project uses standalone agent-browser, add
   `--browser-driver standalone`. Stop and report an unsupported version rather
   than bypassing admission.
3. Read [references/web-acl.md](references/web-acl.md) before authoring or
   changing a manifest. Prefer role, label, test ID, and placeholder targets.
   Use `ref("@eN")` only when a preceding snapshot makes that ref stable inside
   the same session.
4. Put project-owned suites with the project's tests, normally under
   `tests/e2e/`. Use stable English identifiers and descriptions.
5. Validate before running:

   ```bash
   a3s-test check tests/e2e/smoke.acl --json
   ```

6. Run with JSON output:

   ```bash
   a3s-test run tests/e2e/smoke.acl --json
   ```

   Add `--browser-driver standalone` only when the standalone integration is
   intended. Keep `--max-parallel-scenarios 1` unless the application,
   fixtures, and test identities are isolated.
7. Inspect the stable status, error code, step attempts, and evidence paths.
   Distinguish a product failure from a test-specification failure and an
   infrastructure failure before editing code.
8. Re-run the narrow failing scenario or suite after the fix, then run the
   project's broader relevant checks.

## Evidence

Capture the smallest useful evidence set:

- Use `screenshot` for a visible final state.
- Use `accessibility` for semantic structure and agent diagnosis.
- Use `console` and `page_errors` for browser failures.
- Wrap only the relevant flow with `har`, `trace`, or `video` start/stop
  actions; these artifacts can be large.
- Keep every output path relative. A3S Test writes it below the run's isolated
  artifact directory and rejects traversal.

Never place passwords, tokens, cookies, or production data in ACL values,
console fixtures, network mock bodies, or committed evidence. Provision test
credentials outside the manifest.

## Reliability rules

- Treat the first `Ctrl+C` as graceful cancellation. Use a second `Ctrl+C` only
  to force cleanup of process groups owned by the current run.
- Do not kill Chrome, agent-browser, or A3S Browser processes by name.
- Do not reuse another developer's browser namespace or session.
- Leave infrastructure retries at the default unless the failure is explicitly
  classified retryable. A3S Test never retries product assertions or ambiguous
  dispatched actions.
- Use bounded parallelism. Never increase the limit merely to hide a slow test.

## Diagnosis

Use `test.spec.*` errors to repair ACL admission first. Use
`test.driver.web.*` errors for browser compatibility, protocol, or lifecycle
problems. Use `test.assert.*` errors to compare the expected product state with
captured evidence. Use `test.run.*` errors for cancellation, deadlines, and
cleanup.

Do not weaken an assertion until evidence shows the product behavior is
intended. Do not add arbitrary sleeps; use typed load, text, or URL waits.
