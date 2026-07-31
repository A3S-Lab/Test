#!/bin/sh

set -eu

REPOSITORY="${A3S_TEST_REPOSITORY:-A3S-Lab/Test}"
RELEASES_URL="${A3S_TEST_RELEASES_URL:-https://github.com/${REPOSITORY}/releases}"
AGENT="all"
VERSION=""
INSTALL_DIR="${A3S_TEST_INSTALL_DIR:-${HOME}/.local/bin}"
SKILL_DIR=""
INSTALL_CLI=1
INSTALL_SKILL=1

usage() {
    cat <<'EOF'
Install the A3S Test CLI and Coding Agent Skill.

Usage:
  install.sh [options]

Options:
  --agent <name>       Install the Skill for one known coding agent or all.
                       Supported: a3s-code, codex, claude-code, cursor,
                       gemini-cli, github-copilot, opencode, cline, roo,
                       windsurf, all.
  --version <vX.Y.Z>
  --install-dir <path>
  --skill-dir <path>   Install into a custom Skill parent directory.
  --skill-only
  --cli-only
  -h, --help

Environment:
  A3S_TEST_RELEASES_URL  Override the GitHub releases base URL.
  A3S_TEST_INSTALL_DIR   Override the CLI install directory.
  A3S_HOME               Override the A3S Code home directory.
  CODEX_HOME             Override the Codex home directory.
  CLAUDE_CONFIG_DIR      Override the Claude Code config directory.
  CURSOR_HOME            Override the Cursor home directory.
  GEMINI_HOME            Override the Gemini CLI home directory.
  COPILOT_HOME           Override the GitHub Copilot home directory.
  OPENCODE_CONFIG_DIR    Override the OpenCode config directory.
  AGENTS_HOME            Override the shared .agents directory used by Cline.
  ROO_HOME               Override the Roo Code home directory.
  WINDSURF_HOME          Override the Windsurf home directory.
EOF
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --agent)
            [ "$#" -ge 2 ] || {
                echo "install.sh: --agent requires a value" >&2
                exit 2
            }
            AGENT="$2"
            shift 2
            ;;
        --version)
            [ "$#" -ge 2 ] || {
                echo "install.sh: --version requires a value" >&2
                exit 2
            }
            VERSION="$2"
            shift 2
            ;;
        --install-dir)
            [ "$#" -ge 2 ] || {
                echo "install.sh: --install-dir requires a value" >&2
                exit 2
            }
            INSTALL_DIR="$2"
            shift 2
            ;;
        --skill-dir)
            [ "$#" -ge 2 ] || {
                echo "install.sh: --skill-dir requires a value" >&2
                exit 2
            }
            SKILL_DIR="$2"
            shift 2
            ;;
        --skill-only)
            INSTALL_CLI=0
            shift
            ;;
        --cli-only)
            INSTALL_SKILL=0
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "install.sh: unknown option '$1'" >&2
            usage >&2
            exit 2
            ;;
    esac
done

case "$AGENT" in
    all|a3s-code|codex|claude-code|cursor|gemini-cli|github-copilot|opencode|cline|roo|windsurf) ;;
    *)
        echo "install.sh: unsupported agent '$AGENT'" >&2
        exit 2
        ;;
esac

if [ "$INSTALL_CLI" -eq 0 ] && [ "$INSTALL_SKILL" -eq 0 ]; then
    echo "install.sh: --skill-only and --cli-only cannot be combined" >&2
    exit 2
fi

download() {
    source_url="$1"
    destination="$2"
    case "$source_url" in
        file://*)
            cp "${source_url#file://}" "$destination"
            ;;
        *)
            if command -v curl >/dev/null 2>&1; then
                curl -fsSL "$source_url" -o "$destination"
            elif command -v wget >/dev/null 2>&1; then
                wget -q "$source_url" -O "$destination"
            else
                echo "install.sh: curl or wget is required" >&2
                exit 1
            fi
            ;;
    esac
}

resolve_latest_version() {
    if command -v curl >/dev/null 2>&1; then
        final_url="$(curl -fsSL -o /dev/null -w '%{url_effective}' "${RELEASES_URL}/latest")"
    elif command -v wget >/dev/null 2>&1; then
        final_url="$(
            wget --server-response --spider "${RELEASES_URL}/latest" 2>&1 |
                awk '/^  Location: / { value=$2 } END { print value }'
        )"
    else
        echo "install.sh: curl or wget is required" >&2
        exit 1
    fi
    case "$final_url" in
        */tag/v*) basename "$final_url" ;;
        *)
            echo "install.sh: could not resolve the latest release from $final_url" >&2
            exit 1
            ;;
    esac
}

if [ -z "$VERSION" ]; then
    VERSION="$(resolve_latest_version)"
fi
case "$VERSION" in
    v*) ;;
    *) VERSION="v${VERSION}" ;;
esac

case "$(uname -s)" in
    Darwin)
        case "$(uname -m)" in
            arm64|aarch64) TARGET="aarch64-apple-darwin" ;;
            x86_64) TARGET="x86_64-apple-darwin" ;;
            *)
                echo "install.sh: unsupported macOS architecture $(uname -m)" >&2
                exit 1
                ;;
        esac
        ;;
    Linux)
        case "$(uname -m)" in
            x86_64|amd64) TARGET="x86_64-unknown-linux-gnu" ;;
            arm64|aarch64) TARGET="aarch64-unknown-linux-gnu" ;;
            *)
                echo "install.sh: unsupported Linux architecture $(uname -m)" >&2
                exit 1
                ;;
        esac
        ;;
    *)
        echo "install.sh: use install.ps1 on Windows" >&2
        exit 1
        ;;
esac

TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/a3s-test-install.XXXXXX")"
trap 'rm -rf "$TMP_DIR"' EXIT HUP INT TERM
DOWNLOAD_BASE="${RELEASES_URL}/download/${VERSION}"

verify_checksum() {
    artifact="$1"
    checksum_file="$2"
    expected="$(awk 'NR == 1 { print $1 }' "$checksum_file")"
    [ -n "$expected" ] || {
        echo "install.sh: checksum file is empty" >&2
        exit 1
    }
    if command -v sha256sum >/dev/null 2>&1; then
        actual="$(sha256sum "$artifact" | awk '{ print $1 }')"
    elif command -v shasum >/dev/null 2>&1; then
        actual="$(shasum -a 256 "$artifact" | awk '{ print $1 }')"
    else
        echo "install.sh: sha256sum or shasum is required" >&2
        exit 1
    fi
    if [ "$actual" != "$expected" ]; then
        echo "install.sh: checksum verification failed for $(basename "$artifact")" >&2
        exit 1
    fi
}

if [ "$INSTALL_CLI" -eq 1 ]; then
    archive_name="a3s-test-${VERSION}-${TARGET}.tar.gz"
    archive="${TMP_DIR}/${archive_name}"
    checksum_name="a3s-test-${VERSION}-${TARGET}.sha256"
    checksum="${TMP_DIR}/${checksum_name}"
    download "${DOWNLOAD_BASE}/${archive_name}" "$archive"
    download "${DOWNLOAD_BASE}/${checksum_name}" "$checksum"
    verify_checksum "$archive" "$checksum"

    mkdir -p "${TMP_DIR}/cli"
    tar -xzf "$archive" -C "${TMP_DIR}/cli"
    binary="$(find "${TMP_DIR}/cli" -type f -name a3s-test -print | head -n 1)"
    [ -n "$binary" ] || {
        echo "install.sh: CLI archive did not contain a3s-test" >&2
        exit 1
    }
    mkdir -p "$INSTALL_DIR"
    install -m 0755 "$binary" "${INSTALL_DIR}/a3s-test"
    echo "Installed CLI: ${INSTALL_DIR}/a3s-test"
fi

install_skill_at() {
    agent_name="$1"
    skills_parent="$2"

    destination="${skills_parent}/a3s-test"
    staging="${skills_parent}/.a3s-test.install.$$"
    backup="${skills_parent}/.a3s-test.backup.$$"
    mkdir -p "$skills_parent"
    rm -rf "$staging" "$backup"
    cp -R "${TMP_DIR}/skill/a3s-test" "$staging"
    if [ -e "$destination" ]; then
        mv "$destination" "$backup"
    fi
    if mv "$staging" "$destination"; then
        rm -rf "$backup"
    else
        rm -rf "$staging"
        if [ -e "$backup" ]; then
            mv "$backup" "$destination"
        fi
        echo "install.sh: failed to install Skill for $agent_name" >&2
        exit 1
    fi
    echo "Installed $agent_name Skill: $destination"
}

install_skill_for() {
    agent_name="$1"
    case "$agent_name" in
        a3s-code) skills_parent="${A3S_HOME:-${HOME}/.a3s}/skills" ;;
        codex) skills_parent="${CODEX_HOME:-${HOME}/.codex}/skills" ;;
        claude-code) skills_parent="${CLAUDE_CONFIG_DIR:-${HOME}/.claude}/skills" ;;
        cursor) skills_parent="${CURSOR_HOME:-${HOME}/.cursor}/skills" ;;
        gemini-cli) skills_parent="${GEMINI_HOME:-${HOME}/.gemini}/skills" ;;
        github-copilot) skills_parent="${COPILOT_HOME:-${HOME}/.copilot}/skills" ;;
        opencode)
            skills_parent="${OPENCODE_CONFIG_DIR:-${XDG_CONFIG_HOME:-${HOME}/.config}/opencode}/skills"
            ;;
        cline) skills_parent="${AGENTS_HOME:-${HOME}/.agents}/skills" ;;
        roo) skills_parent="${ROO_HOME:-${HOME}/.roo}/skills" ;;
        windsurf) skills_parent="${WINDSURF_HOME:-${HOME}/.codeium/windsurf}/skills" ;;
    esac
    install_skill_at "$agent_name" "$skills_parent"
}

if [ "$INSTALL_SKILL" -eq 1 ]; then
    skill_archive="${TMP_DIR}/a3s-test.skill"
    skill_checksum="${TMP_DIR}/a3s-test.skill.sha256"
    download "${DOWNLOAD_BASE}/a3s-test.skill" "$skill_archive"
    download "${DOWNLOAD_BASE}/a3s-test.skill.sha256" "$skill_checksum"
    verify_checksum "$skill_archive" "$skill_checksum"
    mkdir -p "${TMP_DIR}/skill"
    unzip -q "$skill_archive" -d "${TMP_DIR}/skill"
    [ -f "${TMP_DIR}/skill/a3s-test/SKILL.md" ] || {
        echo "install.sh: Skill archive is missing a3s-test/SKILL.md" >&2
        exit 1
    }

    if [ -n "$SKILL_DIR" ]; then
        install_skill_at custom "$SKILL_DIR"
    else
        case "$AGENT" in
            all)
                install_skill_for a3s-code
                install_skill_for codex
                install_skill_for claude-code
                install_skill_for cursor
                install_skill_for gemini-cli
                install_skill_for github-copilot
                install_skill_for opencode
                install_skill_for cline
                install_skill_for roo
                install_skill_for windsurf
                ;;
            *) install_skill_for "$AGENT" ;;
        esac
    fi
fi

if [ "$INSTALL_CLI" -eq 1 ]; then
    case ":${PATH}:" in
        *":${INSTALL_DIR}:"*) ;;
        *)
            echo "Add ${INSTALL_DIR} to PATH, then run: a3s-test --help"
            ;;
    esac
fi
