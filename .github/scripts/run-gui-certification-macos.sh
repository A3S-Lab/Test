#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 4 ]]; then
    printf 'usage: %s <a3s-test-bin> <cua-source-root> <records-root> <a3s-test-revision>\n' "$0" >&2
    exit 2
fi

a3s_test_bin="$1"
cua_source_root="$2"
records_root="$3"
expected_a3s_test_revision="$4"

for command_name in git jq shasum sw_vers uname; do
    command -v "$command_name" >/dev/null || {
        printf 'required command is unavailable: %s\n' "$command_name" >&2
        exit 2
    }
done

[[ "$(uname -s)" == "Darwin" ]] || {
    printf 'GUI certification requires macOS\n' >&2
    exit 2
}
[[ "$(uname -m)" == "arm64" ]] || {
    printf 'GUI certification requires the reviewed macOS arm64 host profile\n' >&2
    exit 2
}
[[ "$expected_a3s_test_revision" =~ ^[0-9a-f]{40}$ ]] || {
    printf 'A3S Test revision must be a full lowercase commit SHA\n' >&2
    exit 2
}
[[ -x "$a3s_test_bin" ]] || {
    printf 'A3S Test executable is unavailable: %s\n' "$a3s_test_bin" >&2
    exit 2
}
git -C "$cua_source_root" rev-parse --is-inside-work-tree >/dev/null 2>&1 || {
    printf 'CUA source checkout is unavailable: %s\n' "$cua_source_root" >&2
    exit 2
}

repository_root="$(git rev-parse --show-toplevel)"
actual_a3s_test_revision="$(git -C "$repository_root" rev-parse HEAD)"
[[ "$actual_a3s_test_revision" == "$expected_a3s_test_revision" ]] || {
    printf 'A3S Test checkout %s does not match requested revision %s\n' \
        "$actual_a3s_test_revision" "$expected_a3s_test_revision" >&2
    exit 1
}

mkdir -p "$records_root"
records_root="$(cd "$records_root" && pwd -P)"
lock_record="$records_root/cua-lock.json"
"$a3s_test_bin" gui-certification --json > "$lock_record"

expected_cua_repository="https://github.com/A3S-Lab/cua"
expected_cua_revision="$(jq -er '.cua_revision' "$lock_record")"
expected_cua_version="$(jq -er '.cua_driver_version' "$lock_record")"
expected_mcp_protocol="$(jq -er '.mcp_protocol' "$lock_record")"
actual_cua_repository="$(jq -er '.cua_repository' "$lock_record")"

[[ "$actual_cua_repository" == "$expected_cua_repository" ]] || {
    printf 'locked CUA repository is not the reviewed repository: %s\n' \
        "$actual_cua_repository" >&2
    exit 1
}
[[ "$expected_cua_revision" =~ ^[0-9a-f]{40}$ ]] || {
    printf 'locked CUA revision is not a full lowercase commit SHA\n' >&2
    exit 1
}

actual_cua_revision="$(git -C "$cua_source_root" rev-parse HEAD)"
[[ "$actual_cua_revision" == "$expected_cua_revision" ]] || {
    printf 'CUA checkout %s does not match locked revision %s\n' \
        "$actual_cua_revision" "$expected_cua_revision" >&2
    exit 1
}

cua_bin="$cua_source_root/libs/cua-driver/rust/target/release/cua-driver"
fixture_source_app="$cua_source_root/libs/cua-driver/rust/test-apps/harness-appkit/CuaTestHarness.AppKit.app"
applications_root="${A3S_GUI_CERTIFICATION_APPLICATIONS_DIR:-${HOME:?}/Applications}"
run_token="${GITHUB_RUN_ID:-local}-${GITHUB_RUN_ATTEMPT:-1}-$$"
fixture_app="$applications_root/CuaTestHarness.AppKit.a3s-cert-$run_token.app"
fixture_executable="$fixture_app/Contents/MacOS/CuaTestHarness.AppKit"
fixture_bundle_id="com.trycua.harness.appkit"
fixture_window_title="CuaTestHarness AppKit"
policy_file="$repository_root/.github/gui-certification/cua-policy.yaml"
session_policy_file="$repository_root/.github/gui-certification/session-policy.yaml"

[[ -x "$cua_bin" ]] || {
    printf 'required executable is unavailable: %s\n' "$cua_bin" >&2
    exit 2
}
[[ -x "$fixture_source_app/Contents/MacOS/CuaTestHarness.AppKit" ]] || {
    printf 'fixture source executable is unavailable: %s\n' "$fixture_source_app" >&2
    exit 2
}
[[ -d "$applications_root" && ! -L "$applications_root" ]] || {
    printf 'certification Applications directory is unavailable or linked: %s\n' \
        "$applications_root" >&2
    exit 2
}
[[ ! -e "$fixture_app" ]] || {
    printf 'unique certification fixture path already exists: %s\n' "$fixture_app" >&2
    exit 2
}
for regular_file in "$policy_file" "$session_policy_file"; do
    [[ -f "$regular_file" && ! -L "$regular_file" ]] || {
        printf 'required policy is not a regular unlinked file: %s\n' "$regular_file" >&2
        exit 2
    }
done

runtime_root="$(mktemp -d "${RUNNER_TEMP:-/tmp}/a3s-gui-certification.XXXXXX")"
socket_path="$runtime_root/cua.sock"
daemon_pid=''
fixture_registered=0
fixture_scope_established=0
fixture_staged=0

lsregister=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister

fixture_inventory() {
    "$cua_bin" call list_apps '{}' --socket "$socket_path" \
        | jq --arg bundle_id "$fixture_bundle_id" '{
            bundle_id: $bundle_id,
            running_instances: [
                .apps[]
                | select(.bundle_id == $bundle_id and .running == true and .pid > 0)
                | {pid, bundle_id, name}
            ]
        }'
}

cleanup() {
    local original_status=$?
    trap - EXIT INT TERM
    set +e

    if [[ -S "$socket_path" && "$fixture_scope_established" == 1 ]]; then
        while IFS= read -r fixture_pid; do
            [[ "$fixture_pid" =~ ^[1-9][0-9]*$ ]] || continue
            "$cua_bin" call kill_app "{\"pid\":$fixture_pid}" --socket "$socket_path" \
                >> "$records_root/cleanup.log" 2>&1
        done < <(fixture_inventory 2>/dev/null | jq -r '.running_instances[].pid')
    fi

    if [[ -S "$socket_path" ]]; then
        "$cua_bin" stop --socket "$socket_path" >> "$records_root/cleanup.log" 2>&1
    fi

    if [[ -n "$daemon_pid" ]]; then
        for _ in {1..100}; do
            kill -0 "$daemon_pid" 2>/dev/null || break
            sleep 0.1
        done
        if kill -0 "$daemon_pid" 2>/dev/null; then
            kill "$daemon_pid" 2>/dev/null
        fi
        wait "$daemon_pid" 2>/dev/null
    fi

    if [[ "$fixture_registered" == 1 ]]; then
        "$lsregister" -u "$fixture_app" >> "$records_root/cleanup.log" 2>&1
    fi
    if [[ "$fixture_staged" == 1 && -d "$fixture_app" && ! -e "$fixture_source_app" ]]; then
        mv "$fixture_app" "$fixture_source_app" >> "$records_root/cleanup.log" 2>&1
    fi

    if [[ -S "$socket_path" ]]; then
        printf 'certification socket survived cleanup: %s\n' "$socket_path" \
            >> "$records_root/cleanup.log"
        original_status=1
    fi
    rmdir "$runtime_root" 2>/dev/null
    exit "$original_status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

mv "$fixture_source_app" "$fixture_app"
fixture_staged=1
"$lsregister" -f "$fixture_app"
fixture_registered=1

CUA_DRIVER_EMBEDDED=1 "$cua_bin" serve \
    --socket "$socket_path" \
    --embedded \
    --no-overlay \
    --no-permissions-gate \
    --permission-mode bounded \
    --session-policy "$session_policy_file" \
    --approve-session-policy \
    > "$records_root/cua-daemon.log" 2>&1 &
daemon_pid=$!

daemon_ready=0
for _ in {1..100}; do
    if "$cua_bin" status --socket "$socket_path" >/dev/null 2>&1; then
        daemon_ready=1
        break
    fi
    kill -0 "$daemon_pid" 2>/dev/null || break
    sleep 0.1
done
[[ "$daemon_ready" == 1 ]] || {
    printf 'CUA daemon did not become ready\n' >&2
    exit 1
}

config_record="$records_root/cua-config.json"
permissions_record="$records_root/cua-permissions.json"
"$cua_bin" call get_config '{}' --socket "$socket_path" > "$config_record"
"$cua_bin" call check_permissions '{}' --socket "$socket_path" > "$permissions_record"

jq -e \
    --arg revision "$expected_cua_revision" \
    --arg version "$expected_cua_version" \
    '.source_sha == $revision and .version == $version and .platform == "macos"' \
    "$config_record" >/dev/null
jq -e \
    '.accessibility == true and .screen_recording == true and .source.attribution == "host"' \
    "$permissions_record" >/dev/null

fixture_inventory > "$records_root/preflight-fixture-inventory.json"
jq -e '.running_instances | length == 0' \
    "$records_root/preflight-fixture-inventory.json" >/dev/null
fixture_scope_established=1

run_profile() {
    local profile="$1"
    local profile_slug="$2"
    local profile_root="$records_root/$profile_slug-artifacts"
    local result_record="$records_root/$profile_slug.json"
    local cleanup_record="$records_root/$profile_slug-fixture-inventory.json"

    "$a3s_test_bin" gui-certify \
        --gui-policy-file "$policy_file" \
        --cua-proxy-executable "$cua_bin" \
        --cua-embedded-socket "$socket_path" \
        --gui-macos-bundle-id "$fixture_bundle_id" \
        --gui-window-title "$fixture_window_title" \
        --gui-profile "$profile" \
        --command-timeout-ms 30000 \
        --cleanup-timeout-ms 10000 \
        --artifacts-root "$profile_root" \
        --json > "$result_record"

    jq -e '
        .status == "passed"
        and .failure == null
        and .observation.semantic_element_count > 0
        and .cleanup.cleanup_error == null
    ' "$result_record" >/dev/null
    if [[ "$profile" == "window-vision" ]]; then
        jq -e '.observation.visual_evidence_count > 0' "$result_record" >/dev/null
    fi

    fixture_inventory > "$cleanup_record"
    jq -e '.running_instances | length == 0' "$cleanup_record" >/dev/null
}

run_profile semantic semantic
run_profile window-vision window-vision

sha256_file() {
    local file="$1"
    printf 'sha256:%s' "$(shasum -a 256 "$file" | awk '{print $1}')"
}

attestation_record="$records_root/attestation.json"
jq -n \
    --arg protocol 'a3s.test.gui-host-certification/1' \
    --arg created_at "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" \
    --arg a3s_test_revision "$actual_a3s_test_revision" \
    --arg a3s_test_version "$("$a3s_test_bin" --version | awk '{print $2}')" \
    --arg a3s_test_sha256 "$(sha256_file "$a3s_test_bin")" \
    --arg cua_repository "$expected_cua_repository" \
    --arg cua_revision "$expected_cua_revision" \
    --arg cua_version "$expected_cua_version" \
    --arg cua_mcp_protocol "$expected_mcp_protocol" \
    --arg cua_sha256 "$(sha256_file "$cua_bin")" \
    --arg fixture_sha256 "$(sha256_file "$fixture_executable")" \
    --arg policy_sha256 "$(sha256_file "$policy_file")" \
    --arg session_policy_sha256 "$(sha256_file "$session_policy_file")" \
    --arg os_version "$(sw_vers -productVersion)" \
    --arg os_build "$(sw_vers -buildVersion)" \
    --arg architecture "$(uname -m)" \
    --arg github_repository "${GITHUB_REPOSITORY:-}" \
    --arg github_run_id "${GITHUB_RUN_ID:-}" \
    --arg github_run_attempt "${GITHUB_RUN_ATTEMPT:-}" \
    --arg github_ref "${GITHUB_REF:-}" \
    --slurpfile config "$config_record" \
    --slurpfile permissions "$permissions_record" \
    --slurpfile semantic "$records_root/semantic.json" \
    --slurpfile window_vision "$records_root/window-vision.json" \
    --slurpfile preflight_inventory "$records_root/preflight-fixture-inventory.json" \
    --slurpfile semantic_inventory "$records_root/semantic-fixture-inventory.json" \
    --slurpfile window_vision_inventory "$records_root/window-vision-fixture-inventory.json" \
    '{
        protocol: $protocol,
        status: "passed",
        created_at: $created_at,
        a3s_test: {
            repository: "https://github.com/A3S-Lab/Test",
            revision: $a3s_test_revision,
            version: $a3s_test_version,
            executable_sha256: $a3s_test_sha256
        },
        cua: {
            repository: $cua_repository,
            revision: $cua_revision,
            version: $cua_version,
            mcp_protocol: $cua_mcp_protocol,
            executable_sha256: $cua_sha256,
            reported_config: $config[0]
        },
        policies: {
            proxy_sha256: $policy_sha256,
            daemon_session_sha256: $session_policy_sha256
        },
        fixture: {
            bundle_id: "com.trycua.harness.appkit",
            executable_sha256: $fixture_sha256
        },
        host: {
            operating_system: "macos",
            version: $os_version,
            build: $os_build,
            architecture: $architecture,
            permissions: $permissions[0]
        },
        automation: {
            repository: $github_repository,
            run_id: $github_run_id,
            run_attempt: $github_run_attempt,
            ref: $github_ref
        },
        profiles: {
            semantic: {
                execution_profile: $semantic[0].execution_profile,
                observation: $semantic[0].observation,
                cleanup: $semantic[0].cleanup
            },
            window_vision: {
                execution_profile: $window_vision[0].execution_profile,
                observation: $window_vision[0].observation,
                cleanup: $window_vision[0].cleanup
            }
        },
        fixture_cleanup: {
            before: $preflight_inventory[0],
            after_semantic: $semantic_inventory[0],
            after_window_vision: $window_vision_inventory[0]
        }
    }' > "$attestation_record"

jq -e \
    --arg a3s_revision "$actual_a3s_test_revision" \
    --arg cua_revision "$expected_cua_revision" '
        .protocol == "a3s.test.gui-host-certification/1"
        and .status == "passed"
        and .a3s_test.revision == $a3s_revision
        and .cua.revision == $cua_revision
        and .cua.reported_config.source_sha == $cua_revision
        and .host.permissions.accessibility == true
        and .host.permissions.screen_recording == true
        and .profiles.semantic.observation.semantic_element_count > 0
        and .profiles.semantic.cleanup.cleanup_error == null
        and .profiles.window_vision.observation.semantic_element_count > 0
        and .profiles.window_vision.observation.visual_evidence_count > 0
        and .profiles.window_vision.cleanup.cleanup_error == null
        and ([
            .fixture_cleanup.before.running_instances,
            .fixture_cleanup.after_semantic.running_instances,
            .fixture_cleanup.after_window_vision.running_instances
        ] | all(length == 0))
    ' "$attestation_record" >/dev/null

(
    cd "$records_root"
    shasum -a 256 attestation.json > attestation.json.sha256
)
printf 'GUI certification attestation: %s\n' "$attestation_record"
