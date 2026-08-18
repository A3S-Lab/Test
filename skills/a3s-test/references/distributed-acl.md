# Distributed ACL

Use distributed execution only for an already deterministic Web, GUI, or TUI
suite. A GUI worker represents one permission-bound exclusive desktop lane.
The coordinator does not turn a worker into a remote shell. Worker
executables, GUI applications, browser policy, credentials, and network
authority are fixed by the deployment.

## Discover and plan

```bash
a3s-test distributed schema
a3s-test distributed plan distributed.acl --compact
```

Review that every scenario appears exactly once and that each shard binds the
expected worker instance, image digest, inventory digest, and concurrency.
Planning contacts both authenticated worker endpoints; it is not an offline
syntax check.

For a GUI shard, also verify that `required_host_permission_digest` matches
the live inventory and that concurrency is exactly one.

## Configuration

```acl
distributed_run "ci" {
  input_root = "."
  manifest = "tests/e2e/smoke.acl"
  history_root = ".a3s-test/distributed/ci"
  history_window = 20
  job_timeout_ms = 600000
  lease_ms = 60000

  worker "runner-west" {
    endpoint = "https://runner-west.example.test"
    image_digest = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    authorization_env = "A3S_TEST_WORKER_AUTHORIZATION_WEST"
    max_parallel_scenarios = 4
  }

  quarantine "known-checkout-race" {
    reason = "Known checkout state race"
    owner = "checkout-team"
    issue = "https://issues.example.test/123"
    expires_at_ms = 4102444800000
  }
}
```

A GUI worker block additionally pins the exact permission digest discovered
from its live inventory and keeps one exclusive lane:

```acl
worker "desktop-primary" {
  endpoint = "https://desktop-primary.example.test"
  image_digest = "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
  inventory_digest = "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
  host_permission_digest = "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"
  authorization_env = "A3S_TEST_WORKER_AUTHORIZATION_DESKTOP"
  max_parallel_scenarios = 1
}
```

Keep the config at or below the intended input root. Paths cannot contain
parent traversal. Uploads, referenced Surface Contracts, and their provenance
are bundled automatically; use `additional_inputs` only for other explicit
regular files.

The endpoint must use HTTPS or loopback HTTP. Authorization environment names
must start with `A3S_TEST_WORKER_AUTHORIZATION_`; their values are the complete
Authorization header. Never put credentials in ACL, suite data, evidence, or
committed output. `inventory_digest` is an optional additional pin. Live
inspection always binds the actual inventory into the plan.

GUI execution additionally requires a deployment-owned `gui_host` ACL
profile on the worker. The remote request cannot change its CUA endpoint,
policy, application identity, launch or attach mode, window selector, or
perception profile. The worker rechecks the exact permission grant before the
application opens.

## Run and interpret

```bash
a3s-test distributed run distributed.acl --json
```

Exit codes are:

- `0`: passed, including quarantined product failures;
- `1`: required assertion or Surface Contract failure;
- `2`: coordinator, worker, transport, cleanup, or report infrastructure
  failure;
- `124`: timed out;
- `130`: cancelled.

The report is accepted only after digest-bound artifact retrieval and exact
suite, run, count, scenario, and surface verification. Treat any `shard_issues`
entry as infrastructure evidence.

Quarantine can suppress only `test.assert.*`, `test.contract.mismatch`, and
`test.contract.state_mismatch`. It cannot hide driver failures, cleanup
failures, inconclusive contracts, missing or invalid reports, timeouts,
cancellation, interruption, or transport faults. A quarantined pass remains
visible so stale entries can be removed.

The latest retained run is the change baseline, including across suite
revisions. Flake counts and duration estimates use only runs with the exact
suite digest. Reports live under the configured history root. Do not edit or
delete a history root while a run owns its exclusive lock.

The first interrupt cancels every exact submitted job and produces a retained
cancelled analysis. Do not kill worker hosts, browsers, or TUI processes by
name. The worker owns cleanup for only the job it admitted.
