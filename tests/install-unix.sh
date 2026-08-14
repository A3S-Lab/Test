#!/bin/sh

set -eu

ROOT="$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)"
TMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/a3s-test-installer-test.XXXXXX")"
trap 'rm -rf "$TMP_ROOT"' EXIT HUP INT TERM

case "$(uname -s):$(uname -m)" in
    Darwin:arm64|Darwin:aarch64) TARGET="aarch64-apple-darwin" ;;
    Darwin:x86_64) TARGET="x86_64-apple-darwin" ;;
    Linux:x86_64|Linux:amd64) TARGET="x86_64-unknown-linux-gnu" ;;
    Linux:arm64|Linux:aarch64) TARGET="aarch64-unknown-linux-gnu" ;;
    *)
        echo "unsupported installer test host" >&2
        exit 1
        ;;
esac

VERSION="v0.10.0"
RELEASE_DIR="${TMP_ROOT}/releases/download/${VERSION}"
PAYLOAD_NAME="a3s-test-${VERSION}-${TARGET}"
mkdir -p "${RELEASE_DIR}" "${TMP_ROOT}/payload/${PAYLOAD_NAME}" "${TMP_ROOT}/skill/a3s-test"

printf '#!/bin/sh\nprintf "a3s-test fixture\\n"\n' > "${TMP_ROOT}/payload/${PAYLOAD_NAME}/a3s-test"
chmod +x "${TMP_ROOT}/payload/${PAYLOAD_NAME}/a3s-test"
tar -czf "${RELEASE_DIR}/${PAYLOAD_NAME}.tar.gz" -C "${TMP_ROOT}/payload" "${PAYLOAD_NAME}"

printf '%s\n' '# A3S Test fixture Skill' > "${TMP_ROOT}/skill/a3s-test/SKILL.md"
(
    cd "${TMP_ROOT}/skill"
    zip -X -q -r "${RELEASE_DIR}/a3s-test.skill" a3s-test
)

checksum() {
    file="$1"
    output="$2"
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$file" > "$output"
    else
        shasum -a 256 "$file" > "$output"
    fi
}

checksum \
    "${RELEASE_DIR}/${PAYLOAD_NAME}.tar.gz" \
    "${RELEASE_DIR}/${PAYLOAD_NAME}.sha256"
checksum \
    "${RELEASE_DIR}/a3s-test.skill" \
    "${RELEASE_DIR}/a3s-test.skill.sha256"

HOME_DIR="${TMP_ROOT}/home"
BIN_DIR="${TMP_ROOT}/bin"
export HOME="$HOME_DIR"
export CODEX_HOME="${TMP_ROOT}/codex"
export A3S_TEST_RELEASES_URL="file://${TMP_ROOT}/releases"
unset A3S_HOME CLAUDE_CONFIG_DIR CURSOR_HOME GEMINI_HOME COPILOT_HOME
unset OPENCODE_CONFIG_DIR AGENTS_HOME CLINE_HOME ROO_HOME WINDSURF_HOME
PATH="/usr/bin:/bin"
export PATH

"${ROOT}/scripts/install.sh" \
    --version "$VERSION" \
    --install-dir "$BIN_DIR"

test -x "${BIN_DIR}/a3s-test"
test -f "${CODEX_HOME}/skills/a3s-test/SKILL.md"
test ! -e "${TMP_ROOT}/a3s/skills/a3s-test"
test ! -e "${TMP_ROOT}/claude/skills/a3s-test"

NO_AGENT_HOME="${TMP_ROOT}/no-agent-home"
(
    unset CODEX_HOME
    HOME="$NO_AGENT_HOME"
    export HOME
    "${ROOT}/scripts/install.sh" --version "$VERSION" --skill-only
)
test -f "${NO_AGENT_HOME}/.agents/skills/a3s-test/SKILL.md"

(
    unset CODEX_HOME
    HOME="$NO_AGENT_HOME"
    export HOME
    "${ROOT}/scripts/install.sh" \
        --version "$VERSION" \
        --cli-only \
        --install-dir "${TMP_ROOT}/cli-only-bin"
)
test -x "${TMP_ROOT}/cli-only-bin/a3s-test"

export A3S_HOME="${TMP_ROOT}/a3s"
export CLAUDE_CONFIG_DIR="${TMP_ROOT}/claude"
export CURSOR_HOME="${TMP_ROOT}/cursor"
export GEMINI_HOME="${TMP_ROOT}/gemini"
export COPILOT_HOME="${TMP_ROOT}/copilot"
export OPENCODE_CONFIG_DIR="${TMP_ROOT}/opencode"
export AGENTS_HOME="${TMP_ROOT}/agents"
export CLINE_HOME="${TMP_ROOT}/cline"
export ROO_HOME="${TMP_ROOT}/roo"
export WINDSURF_HOME="${TMP_ROOT}/windsurf"

"${ROOT}/scripts/install.sh" \
    --version "$VERSION" \
    --agent universal \
    --skill-only
test -f "${AGENTS_HOME}/skills/a3s-test/SKILL.md"

printf '%s\n' stale > "${CODEX_HOME}/skills/a3s-test/stale.txt"
"${ROOT}/scripts/install.sh" \
    --version "$VERSION" \
    --agent all \
    --skill-only

for skills_parent in \
    "${A3S_HOME}/skills" \
    "${CODEX_HOME}/skills" \
    "${CLAUDE_CONFIG_DIR}/skills" \
    "${CURSOR_HOME}/skills" \
    "${GEMINI_HOME}/skills" \
    "${COPILOT_HOME}/skills" \
    "${OPENCODE_CONFIG_DIR}/skills" \
    "${AGENTS_HOME}/skills" \
    "${CLINE_HOME}/skills" \
    "${ROO_HOME}/skills" \
    "${WINDSURF_HOME}/skills"
do
    test -f "${skills_parent}/a3s-test/SKILL.md"
done
test ! -e "${CODEX_HOME}/skills/a3s-test/stale.txt"

CUSTOM_SKILLS="${TMP_ROOT}/custom-skills"
"${ROOT}/scripts/install.sh" \
    --version "$VERSION" \
    --agent codex \
    --skill-dir "$CUSTOM_SKILLS" \
    --skill-only
test -f "${CUSTOM_SKILLS}/a3s-test/SKILL.md"

echo "Unix installer test passed"
