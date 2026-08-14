#!/bin/sh
set -eu

image="${1:-a3s-test-runner:local}"
root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
cache_root="${A3S_TEST_RUNNER_CACHE_DIR:-${TMPDIR:-/tmp}/a3s-test-runner-cache}"
chrome_version=147.0.7727.117
chrome_sha256=b8aeb7dfc8ebb25b66fcc375c5505ea3715556f019bc4a87ffd19d7bfaa4ff51
chrome_size=117924094
chrome_archive="${cache_root}/chrome-headless-shell.zip"
chrome_partial="${cache_root}/chrome-headless-shell-${chrome_version}.partial"
chrome_chunks="${cache_root}/chrome-headless-shell-${chrome_version}.chunks"
download_run=$$
owned_download_pids=""

stop_owned_downloads() {
    for owned_pid in ${owned_download_pids}; do
        kill "${owned_pid}" 2>/dev/null || true
    done
    for owned_pid in ${owned_download_pids}; do
        wait "${owned_pid}" 2>/dev/null || true
    done
}

trap 'stop_owned_downloads; exit 130' HUP INT TERM

verify_chrome() {
    test -f "$1" || return 1
    if command -v sha256sum >/dev/null 2>&1; then
        printf '%s  %s\n' "${chrome_sha256}" "$1" | sha256sum -c - >/dev/null 2>&1
    else
        test "$(shasum -a 256 "$1" | awk '{ print $1 }')" = "${chrome_sha256}"
    fi
}

file_size() {
    wc -c < "$1" | tr -d ' '
}

download_chrome_chunks() {
    chunk_size=1048576
    chunk_index=0
    chunk_start=0
    chunk_pids=""
    chunk_jobs=0
    mkdir -p "${chrome_chunks}"
    find "${chrome_chunks}" -type f -name 'chunk-*.download.*' -delete

    while test "${chunk_start}" -lt "${chrome_size}"; do
        chunk_end=$((chunk_start + chunk_size - 1))
        if test "${chunk_end}" -ge "${chrome_size}"; then
            chunk_end=$((chrome_size - 1))
        fi
        chunk_expected=$((chunk_end - chunk_start + 1))
        chunk_file=$(printf '%s/chunk-%06d' "${chrome_chunks}" "${chunk_index}")
        if ! test -f "${chunk_file}" || ! test "$(file_size "${chunk_file}")" -eq "${chunk_expected}"; then
            (
                chunk_download="${chunk_file}.download.${download_run}"
                curl --fail --location --retry 10 --retry-all-errors \
                    --connect-timeout 30 --max-time 300 --proto '=https' --tlsv1.2 \
                    --range "${chunk_start}-${chunk_end}" \
                    --silent --show-error --output "${chunk_download}" \
                    "https://storage.googleapis.com/chrome-for-testing-public/${chrome_version}/linux64/chrome-headless-shell-linux64.zip"
                test "$(file_size "${chunk_download}")" -eq "${chunk_expected}"
                mv "${chunk_download}" "${chunk_file}"
            ) &
            chunk_pid=$!
            chunk_pids="${chunk_pids} ${chunk_pid}"
            owned_download_pids="${owned_download_pids} ${chunk_pid}"
            chunk_jobs=$((chunk_jobs + 1))
        fi
        if test "${chunk_jobs}" -ge 16; then
            chunk_failed=0
            for chunk_pid in ${chunk_pids}; do
                wait "${chunk_pid}" || chunk_failed=1
            done
            test "${chunk_failed}" -eq 0
            chunk_pids=""
            owned_download_pids=""
            chunk_jobs=0
        fi
        chunk_index=$((chunk_index + 1))
        chunk_start=$((chunk_end + 1))
    done
    chunk_failed=0
    for chunk_pid in ${chunk_pids}; do
        wait "${chunk_pid}" || chunk_failed=1
    done
    test "${chunk_failed}" -eq 0
    owned_download_pids=""
    cat "${chrome_chunks}"/chunk-[0-9][0-9][0-9][0-9][0-9][0-9] > "${chrome_partial}"
}

mkdir -p "${cache_root}"
if ! verify_chrome "${chrome_archive}"; then
    download_chrome_chunks
    verify_chrome "${chrome_partial}"
    mv "${chrome_partial}" "${chrome_archive}"
fi

docker buildx build \
    --build-arg CHROME_ARCHIVE_SOURCE=chrome-archive \
    --build-arg "IMAGE_VERSION=${A3S_TEST_RUNNER_IMAGE_VERSION:-dev}" \
    --build-arg "SOURCE_REVISION=${A3S_TEST_RUNNER_SOURCE_REVISION:-unknown}" \
    --build-context "chrome-archive=${cache_root}" \
    --file "${root}/images/runner/Dockerfile" \
    --platform linux/amd64 \
    --load \
    --tag "${image}" \
    "${root}"

test "$(docker image inspect --format '{{.Architecture}}' "${image}")" = "amd64"
test "$(docker image inspect --format '{{.Config.User}}' "${image}")" = "node:node"

docker run --rm \
    --platform linux/amd64 \
    --network none \
    --read-only \
    --cap-drop ALL \
    --security-opt no-new-privileges \
    --pids-limit 256 \
    --memory 2g \
    --cpus 2 \
    --tmpfs /tmp:rw,nosuid,nodev,noexec,size=512m,mode=1777 \
    --tmpfs /workspace:rw,nosuid,nodev,noexec,size=256m,uid=1000,gid=1000,mode=700 \
    "${image}" \
    node /opt/a3s-test-runner/smoke/smoke.mjs
