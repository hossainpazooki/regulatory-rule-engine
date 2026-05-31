#!/usr/bin/env bash
#
# differential-test.sh — Gate 2 differential compile (Rust vs Python).
#
# WHAT THIS IS FOR
# ----------------
# Gate 2 replaces the platform's Python rule compiler with a Rust one. This
# script is the proof that the replacement is faithful: for every rule in the
# corpus it compiles the SAME YAML two ways —
#
#   Python:  platform RuleLoader  → Rule          → JSON (model_dump)
#   Rust:    ke-compile           → RuleIR        → semantic normal form
#
# — reduces both sides to a representation-independent "semantic normal form"
# (decision paths, applicability predicate, obligations, source, dates), and
# diffs them. If they differ on any rule, the Rust compiler is not yet a safe
# drop-in and the script exits non-zero. This is spec §19's Gate 2 acceptance
# criterion ("compiled by Rust and Python → normalized IRs are semantically
# equivalent over every corpus rule").
#
# INVARIANT (spec §4.5): the platform checkout MUST be at the SHA recorded in
# fixtures/rules/SOURCE.md — the exact revision the corpus was snapshotted from
# — not whatever HEAD happens to be. Otherwise Rust would be compared against a
# different corpus than the one on disk. The script fails fast if the SHA, the
# checkout, or the platform's rule data is not in the expected state.
#
# USAGE
#   ./scripts/differential-test.sh
#   PLATFORM_REPO=/path/to/platform ./scripts/differential-test.sh
#
# Designed to run in a bash terminal, including Git Bash / MINGW64 on Windows
# (native python.exe paths are translated with cygpath).

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
platform="${PLATFORM_REPO:-${repo_root}/../institutional-defi-platform-api}"

log()   { printf '%s\n' "$*"; }
fatal() { printf 'FATAL: %s\n' "$*" >&2; exit 1; }

# --- 1. platform checkout + SHA gate --------------------------------------

recorded_sha="$(grep -oE '[0-9a-f]{40}' "${repo_root}/fixtures/rules/SOURCE.md" | head -n1 || true)"
[ -n "${recorded_sha}" ] || fatal "no 40-hex SHA found in fixtures/rules/SOURCE.md"
[ -d "${platform}/.git" ] || fatal "platform checkout not found at ${platform}
       set PLATFORM_REPO or place a sibling institutional-defi-platform-api."

head_sha="$(git -C "${platform}" rev-parse HEAD)"
if [ "${head_sha}" != "${recorded_sha}" ]; then
  fatal "platform HEAD ${head_sha}
       != recorded SOURCE.md SHA ${recorded_sha}
       run: git -C \"${platform}\" checkout ${recorded_sha}"
fi
git -C "${platform}" diff --quiet -- src/rules/data \
  || fatal "${platform}/src/rules/data has uncommitted changes"
log "platform SHA verified: ${head_sha}"

# --- 2. resolve python (prefer the platform venv) -------------------------

py=""
for cand in \
  "${platform}/.venv/bin/python" \
  "${platform}/.venv/Scripts/python.exe" \
  python3 python; do
  if [ -x "${cand}" ] || command -v "${cand}" >/dev/null 2>&1; then
    py="${cand}"
    break
  fi
done
[ -n "${py}" ] || fatal "no python interpreter found (install the platform venv)"

# Is this a native Windows interpreter (so MSYS paths must be translated)?
win_py=0
case "${py}" in *.exe) win_py=1 ;; esac
log "python: ${py}$( [ "${win_py}" -eq 1 ] && echo ' (native windows)')"

# Translate an MSYS/Git-Bash path to a Windows path for native python.exe.
host_path() {
  if [ "${win_py}" -eq 1 ] && command -v cygpath >/dev/null 2>&1; then
    cygpath -w "$1"
  else
    printf '%s' "$1"
  fi
}

# --- 3. build the Rust dev tool -------------------------------------------

# rustup installs cargo under ~/.cargo/bin, which a fresh shell (MINGW64, CI)
# may not have on PATH.
if ! command -v cargo >/dev/null 2>&1; then
  [ -f "${HOME}/.cargo/env" ] && . "${HOME}/.cargo/env"  # shellcheck disable=SC1091
  export PATH="${HOME}/.cargo/bin:${PATH}"
fi
command -v cargo >/dev/null 2>&1 \
  || fatal "cargo not found — install rustup or add ~/.cargo/bin to PATH"

# Package is `ke-compiler`; the binary it produces is `ke-compile`.
( cd "${repo_root}" && cargo build -q -p ke-compiler )
ke_compile="${repo_root}/target/debug/ke-compile"
[ -x "${ke_compile}" ] || ke_compile="${repo_root}/target/debug/ke-compile.exe"
[ -x "${ke_compile}" ] || fatal "ke-compile binary not found after build"

# --- 4. per-rule-file Python dump + Rust diff ------------------------------

# Python one-liner: load a rule file via the platform RuleLoader and emit a
# JSON array of Rule.model_dump(mode="json"). Run with cwd = platform root and
# PYTHONPATH set so `from src...` resolves.
dump_py='
import json, sys
from src.rules.service import RuleLoader
rules = RuleLoader().load_file(sys.argv[1])
print(json.dumps([r.model_dump(mode="json") for r in rules]))
'

tmp="$(mktemp -d)"
trap 'rm -rf "${tmp}"' EXIT

fail=0
count=0
for yaml in "${repo_root}"/fixtures/rules/*.yaml; do
  name="$(basename "${yaml}")"
  [ "${name}" = "schema.yaml" ] && continue
  count=$((count + 1))
  json="${tmp}/${name}.json"

  # Python side (paths translated for native windows python).
  if ! ( cd "${platform}" \
         && PYTHONPATH="$(host_path "${platform}")" \
            "${py}" -c "${dump_py}" "$(host_path "${yaml}")" ) \
         > "${json}" 2>"${tmp}/py.err"; then
    log "FAIL  ${name}  (python load)"
    sed 's/^/        /' "${tmp}/py.err" >&2
    fail=$((fail + 1))
    continue
  fi

  # Rust side + semantic diff (run once; show captured output only on failure).
  if "${ke_compile}" diff "${yaml}" "${json}" >"${tmp}/diff.out" 2>&1; then
    log "ok    ${name}"
  else
    log "FAIL  ${name}  (semantic divergence)"
    sed 's/^/        /' "${tmp}/diff.out" >&2
    fail=$((fail + 1))
  fi
done

# --- 5. summary -----------------------------------------------------------

log "----"
log "platform SHA: ${head_sha}"
log "checked ${count} rule file(s); ${fail} with divergence(s)"
[ "${fail}" -eq 0 ] || exit 1
log "PASS: Rust ≡ Python over the whole corpus"
