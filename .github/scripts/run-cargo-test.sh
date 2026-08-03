#!/usr/bin/env bash
set -uo pipefail

if [[ $# -eq 0 ]]; then
    printf 'usage: %s <command> [argument ...]\n' "$0" >&2
    exit 2
fi

log_root="${RUNNER_TEMP:-.}"
log_path="${log_root}/a3s-test-${GITHUB_JOB:-local}.log"

set +e
"$@" 2>&1 | tee "$log_path"
status=${PIPESTATUS[0]}
set -e

if [[ $status -eq 0 ]]; then
    exit 0
fi

printf '%s\n' '::group::Rust test failure tail'
tail -n 200 "$log_path"
printf '%s\n' '::endgroup::'

# GitHub's public annotations API does not expose grouped step output. Include
# bounded panic blocks (or the final tail for non-panics) in one annotation so
# unauthenticated CI triage retains stdout, stderr, and expected/actual values.
context=$(
    awk '
        /panicked at/ { remaining = 80 }
        remaining > 0 { print; remaining-- }
    ' "$log_path" | tail -c 24000
)
if [[ -z "$context" ]]; then
    context=$(tail -n 120 "$log_path" | tail -c 24000)
fi
escaped_context=${context//'%'/'%25'}
escaped_context=${escaped_context//$'\r'/'%0D'}
escaped_context=${escaped_context//$'\n'/'%0A'}
printf '::error title=Rust test failure context::%s\n' "$escaped_context"

annotations=0
while IFS= read -r line; do
    escaped=${line//'%'/'%25'}
    escaped=${escaped//$'\r'/'%0D'}
    escaped=${escaped//$'\n'/'%0A'}
    printf '::error title=Rust test failure::%s\n' "$escaped"
    annotations=$((annotations + 1))
done < <(
    grep -E '(^| )(error:|FAILED$|failures:|panicked at|assertion .*failed)' "$log_path" \
        | tail -n 20 \
        || true
)

if [[ $annotations -eq 0 ]]; then
    while IFS= read -r line; do
        escaped=${line//'%'/'%25'}
        escaped=${escaped//$'\r'/'%0D'}
        escaped=${escaped//$'\n'/'%0A'}
        printf '::error title=Rust test command failed::%s\n' "$escaped"
    done < <(tail -n 5 "$log_path")
fi

exit "$status"
