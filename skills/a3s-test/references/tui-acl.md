# TUI ACL reference

Use TUI ACL only for a known deterministic terminal workflow. A3S Test owns
the executable, pseudoterminal, process tree, semantic VT state, evidence, and
cleanup. Interactive agent sessions do not currently register a TUI host.

## Suite

```acl
suite "editor-smoke" {
    version = 1

    scenario "open-document" {
        surface = "tui"
        timeout_ms = 30000

        wait "ready" {
            text = "Ready"
        }

        terminal_resize "working-size" {
            columns = 120
            rows = 40
        }

        terminal_paste "document" {
            text = "open fixtures/report.txt"
        }

        press "submit" {
            key = "Enter"
        }

        wait "loaded" {
            regex = "Loaded [0-9]+ lines"
        }

        expect "document-visible" {
            text = "Quarterly report"
        }

        terminal_recording "evidence" {
            path = "terminal/editor.vt"
        }
    }
}
```

TUI suites share `snapshot`, `press`, `wait`, and `expect`. Terminal-specific
actions are `terminal_paste`, `terminal_resize`, and `terminal_recording`.
`press` accepts one character, named terminal keys, `Control+<letter>`, and
`Alt+<character>`. Paste honors the application's current bracketed-paste
mode. A TUI wait accepts exactly one `text` or `regex` condition. Browser load,
URL, and element waits are rejected instead of being guessed.

## CLI

```bash
a3s-test check tests/e2e/editor.acl --json

a3s-test run tests/e2e/editor.acl \
  --tui-executable ./target/debug/editor \
  --tui-arg --fixture-mode \
  --tui-columns 120 \
  --tui-rows 40 \
  --json
```

Repeat `--tui-arg` for each argument. `--tui-working-directory` must be an
absolute path. `--tui-scrollback-rows` and `--tui-max-output-bytes` set bounded
retention limits; they do not make observations or recordings unbounded.
`--command-timeout-ms` bounds terminal input and waits, while
`--cleanup-timeout-ms` bounds process-tree cleanup.

## Observation and evidence

Snapshots and successful waits return bounded viewport plus retained
scrollback text, rows and columns, cursor position and visibility,
alternate-screen, application-cursor, and bracketed-paste modes, root exit
status, and output truncation metadata. Use text or regex evidence rather than
terminal coordinates.

Raw VT recordings stay beneath the scenario artifact root. Paths must be
relative and traversal-free; links and Windows reparse points are rejected.
Record only the smallest terminal evidence needed to prove the result.

## Lifecycle

On Unix, each scenario owns one PTY process group and an EOF watchdog. On
Windows, each ConPTY session owns a kill-on-close Job. Normal completion,
failure, timeout, cancellation, Drop, a second interrupt, and host death must
reap the complete owned tree without targeting another terminal session.

Do not add sleeps to synchronize output. Wait for typed text or regex state.
The first `Ctrl+C` requests bounded cleanup; a second forces cleanup of only
the current A3S Test process's registered surface trees.
