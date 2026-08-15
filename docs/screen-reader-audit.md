# Independent Screen-Reader Audit

## Purpose

The Test Kit automated accessibility suite checks DOM semantics, the browser
accessibility tree, keyboard behavior, focus restoration, color contrast, and
WCAG A/AA rules in real Chromium. It cannot establish how an actual screen
reader announces and navigates the complete review lifecycle.

This procedure makes that remaining hands-on audit reproducible. It provides a
fixed 15-workflow manifest, a loopback-only fixture, a strict audit artifact,
and a verifier. It does not simulate a screen reader, close roadmap milestone
M8 by itself, authorize a repair, or mutate application source.

## Independence requirement

The auditor must:

- Be a person who did not implement the Test Kit changes under audit.
- Operate an actual supported screen reader instead of relying only on an
  accessibility tree, automated scanner, or another agent.
- Check out and identify the exact Git revision being audited.
- Exercise every workflow in the committed manifest using the recorded OS,
  browser, screen reader, locale, hardware, and input modes.
- Record failures and blockers as observed. The auditor must not edit the
  product during the audit or treat the fixture controls as repair authority.

An organization may use a stable pseudonymous auditor ID when identity privacy
is required. The process owner remains responsible for establishing that the
auditor is independent; the JSON attestation cannot prove independence alone.

## Supported audit environment

Use an environment that permits hands-on assistive-technology inspection. A
typical combination is VoiceOver with Safari or Chrome on macOS, or NVDA with
Firefox or Chrome on Windows. An equivalent supported screen reader is
acceptable when its exact product and version are recorded.

Record at least:

- OS name and version.
- Browser name and version.
- Screen-reader name and version.
- Every unique input mode used, normally `keyboard` and any additional mode
  actually exercised.
- Locale and hardware when they affect the result.

Do not claim coverage for combinations that were not run.

## Start the fixture

Use a clean checkout at the revision to be audited. From the Test Kit package:

```bash
cd packages/testkit
npm ci
npm run audit:screen-reader -- --port 4173
```

The server prints the audit URL and binds only to `127.0.0.1`. If port 4173 is
already owned, choose another explicit loopback port or request an ephemeral
one:

```bash
npm run audit:screen-reader -- --port 0 --json
```

The server exposes only these routes:

```text
/health
/testkit.html
/testkit.js
/screen-reader-workflows.json
```

Open `/testkit.html` in the audit browser. The page includes accessible,
test-only controls for seeding contract and design candidates, applying repair
states, and resetting the fixture. They allow the auditor to reach every state
without DevTools. `Reset fixture` clears only A3S Test fixture storage and
reloads the fixture route.

Stop the server with `Ctrl+C` after the audit. It removes its temporary bundle
and does not close unrelated browser sessions.

## Execute the workflows

The canonical manifest is
`packages/testkit/screen-reader-audit/workflows.json`. Its protocol is
`a3s.test.screen-reader-workflows/1`. Follow the entries in manifest order and
use the named setup, steps, and expected announcements for each workflow.

For each workflow, record exactly one outcome:

- `passed`: every expected announcement, state, navigation, and focus behavior
  was directly observed.
- `failed`: the product contradicted at least one expected behavior.
- `blocked`: the environment or fixture prevented a conclusive run.

Repeat attempts may contribute multiple evidence files, but the final audit
contains one result object per workflow. Failed and blocked results require a
specific note. Passed results should also explain the decisive observation
when the evidence is not self-explanatory.

The fixture can expose candidate and repair states, but it cannot authorize a
workspace mutation. If the audit finds a defect, preserve it in the artifact
and handle repair through a separately authorized A3S Test session after the
audit.

## Evidence and privacy

Create an audit directory outside the source tree unless the repository owner
explicitly wants the evidence committed. Put `audit.json` and an `evidence/`
directory under the same root. Evidence paths in the JSON are relative to the
directory containing `audit.json`.

Useful evidence includes:

- A bounded screen-reader speech transcript with the workflow and action
  identified.
- A screenshot showing the corresponding visible state.
- A short audio or video excerpt when announcement timing is the defect.
- A concise manual action log that records focus movement and the result.

Do not capture passwords, tokens, cookies, private production data, unrelated
browser content, or a full desktop recording when a smaller excerpt proves the
result. Redact sensitive content before sharing evidence.

The verifier requires each referenced artifact to be a non-empty regular file
inside the audit directory. Symlink or path traversal escapes are rejected.
Each workflow may reference at most 20 unique files, and each file is limited
to 64 MiB. Verification reads and hashes every distinct file through a stable
file handle, rejects replacement during the read, and limits the aggregate
distinct evidence set to 1 GiB.

## Audit artifact

The audit protocol is `a3s.test.screen-reader-audit/1`. The top-level record
contains only these fields:

| Field | Requirement |
| --- | --- |
| `protocol` | Exact audit protocol string. |
| `revision` | Full lowercase 40-character Git commit SHA. |
| `testkit_version` | Exact version from `packages/testkit/package.json`. |
| `independent` | Must be `true`. |
| `auditor` | Required stable `id`; optional `name` and `organization`. |
| `environment` | Required `os`, `browser`, `screen_reader`, and non-empty `input_modes`; optional `locale` and `hardware`. |
| `started_at` | ISO-8601 timestamp with a timezone. |
| `completed_at` | ISO-8601 timestamp with a timezone and not earlier than `started_at`. |
| `notes` | Optional bounded audit-level notes. |
| `results` | Every manifest workflow exactly once and in manifest order. |

Each result contains only `workflow_id`, `outcome`, optional `notes`, and a
non-empty `evidence` list. This valid JSON fragment shows the exact object
shape; a complete artifact repeats the result shape for all 15 manifest IDs:

```json
{
  "protocol": "a3s.test.screen-reader-audit/1",
  "revision": "0123456789abcdef0123456789abcdef01234567",
  "testkit_version": "0.3.0",
  "independent": true,
  "auditor": {
    "id": "external-accessibility-reviewer",
    "organization": "Independent review team"
  },
  "environment": {
    "os": "Windows 11 24H2",
    "browser": "Firefox 142",
    "screen_reader": "NVDA 2026.2",
    "input_modes": ["keyboard"],
    "locale": "en-US",
    "hardware": "Physical Windows laptop"
  },
  "started_at": "2026-08-15T10:00:00.000Z",
  "completed_at": "2026-08-15T11:30:00.000Z",
  "notes": "The audit used default screen-reader verbosity.",
  "results": [
    {
      "workflow_id": "dialog-navigation",
      "outcome": "passed",
      "notes": "The named non-modal dialog and return path were announced.",
      "evidence": ["evidence/dialog-navigation.txt"]
    }
  ]
}
```

The fragment is intentionally incomplete and will not pass verification until
all manifest workflows and their evidence are present. Unknown fields are
rejected at every level so an unsupported authority or hidden result cannot be
smuggled into the record.

## Verify the artifact

From `packages/testkit`, verify structure, revision, version, workflow
coverage, timing, and evidence files:

```bash
revision="$(git rev-parse HEAD)"
npm run audit:check -- /absolute/path/to/audit.json --revision "$revision" \
  > /absolute/path/to/audit-verification.json
```

The revision must identify an existing commit in this repository. The
verifier reads the workflow manifest and Test Kit package version from that
commit rather than trusting mutable working-tree copies.

This command may succeed when a result is `failed` or `blocked`; success means
the audit is structurally authentic and inspectable, not that the product
passed.

After an independent auditor has rerun every failed or blocked workflow and
recorded `passed`, apply the closure gate:

```bash
npm run audit:check -- /absolute/path/to/audit.json \
  --revision "$revision" \
  --require-pass \
  > /absolute/path/to/audit-closure-verification.json
```

The verifier emits `a3s.test.screen-reader-audit-verification/2` with the exact
revision, Test Kit version, gate mode, and outcome counts. It also records the
raw audit and committed workflow-manifest byte lengths and SHA-256 digests,
plus an ordered byte length and SHA-256 digest for every referenced evidence
file. `evidence_set_sha256` hashes the compact JSON serialization of that
ordered evidence-record array.

The record contains only audit-relative paths, so identical inputs produce
identical verification JSON after the audit directory is copied or moved.
Keep the verification output with the reviewed artifact. To detect later
replacement, rerun the same gate and compare the complete record:

```bash
npm run audit:check -- /absolute/path/to/audit.json \
  --revision "$revision" \
  --require-pass \
  > /absolute/path/to/audit-closure-verification.next.json
cmp /absolute/path/to/audit-closure-verification.json \
  /absolute/path/to/audit-closure-verification.next.json
```

Even a successful `--require-pass` command does not authorize repair or close
M8 without confirmation that the recorded person independently performed the
hands-on audit at the named revision. Automated CI can validate the artifact;
it cannot manufacture the missing human observation.
