#!/usr/bin/env bash
#
# equivalence-harness.sh — Gate 3 fuzzed Rust↔Python runtime equivalence.
#
# WHAT THIS IS FOR
# ----------------
# Gate 3 adds a Rust preview executor (`ke-runtime`) that must be observationally
# equivalent to the platform's Python `RuleRuntime`. This script is the proof:
# it generates N≥1000 scenarios across the corpus, runs each through BOTH
# runtimes, and compares the normalized result (outcome, obligation id-set,
# normalized trace). Equivalence boundary: spec §20, pinned in ADR 0008.
#
#   Rust:   gen-scenarios  → ke-runtime executor → {rust: normalized}   (one JSONL)
#   Python: py_reference_runtime.py → RuleLoader → RuleCompiler → RuleRuntime.infer
#           → {python: normalized}, compared line-by-line to {rust}.
#
# It also (re)emits the committed trace fixtures (fixtures/traces/golden.json)
# from the Python oracle — only when the run is clean, so a committed fixture is
# always a verified agreement. `cargo test -p ke-runtime` then asserts the Rust
# runtime reproduces them.
#
# INVARIANT (spec §4.5): the platform checkout MUST be at the SHA recorded in
# fixtures/rules/SOURCE.md — the revision the corpus was snapshotted from — with
# src/rules/data clean. Otherwise Rust would be compared against a different
# corpus/runtime than the one on disk. Fails fast otherwise.
#
# USAGE
#   ./scripts/equivalence-harness.sh
#   KE_FUZZ=40 KE_SEED=12345 ./scripts/equivalence-harness.sh
#   PLATFORM_REPO=/path/to/platform ./scripts/equivalence-harness.sh
#
# Designed for bash, including Git Bash / MINGW64 on Windows (native python.exe
# paths are translated with cygpath).

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
platform="${PLATFORM_REPO:-${repo_root}/../institutional-defi-platform-api}"

seed="${KE_SEED:-7242087531}"
fuzz="${KE_FUZZ:-30}"
# Trace-fixture rules: a small, varied allowlist (obligations, nested decisions,
# numeric threshold, applicability). Emitted to fixtures/traces/golden.json.
trace_rules="mica_art38_reserve_assets,mica_art59_casp_authorization,mica_art54_emt_redemption,fca_crypto_promotion_authorization"

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

# --- 3. resolve cargo + build the generator -------------------------------

if ! command -v cargo >/dev/null 2>&1; then
  [ -f "${HOME}/.cargo/env" ] && . "${HOME}/.cargo/env"  # shellcheck disable=SC1091
  export PATH="${HOME}/.cargo/bin:${PATH}"
fi
command -v cargo >/dev/null 2>&1 \
  || fatal "cargo not found — install rustup or add ~/.cargo/bin to PATH"

( cd "${repo_root}" && cargo build -q -p ke-runtime --bin gen-scenarios )
gen="${repo_root}/target/debug/gen-scenarios"
[ -x "${gen}" ] || gen="${repo_root}/target/debug/gen-scenarios.exe"
[ -x "${gen}" ] || fatal "gen-scenarios binary not found after build"

# --- 4. generate scenarios (Rust side) ------------------------------------

tmp="$(mktemp -d)"
trap 'rm -rf "${tmp}"' EXIT
scenarios="${tmp}/scenarios.jsonl"

"${gen}" --seed "${seed}" --fuzz "${fuzz}" "${repo_root}"/fixtures/rules/*.yaml > "${scenarios}"
n="$(wc -l < "${scenarios}" | tr -d ' ')"
log "generated ${n} scenarios (seed=${seed}, fuzz/rule=${fuzz})"
[ "${n}" -ge 1000 ] || fatal "only ${n} scenarios (< 1000); raise KE_FUZZ"

# --- 5. Python reference + compare ----------------------------------------

driver="${repo_root}/scripts/py_reference_runtime.py"
corpus="$(host_path "${repo_root}/fixtures/rules")"
golden="$(host_path "${repo_root}/fixtures/traces/golden.json")"

# Run with cwd = platform root and PYTHONPATH set so `from src...` resolves.
if ( cd "${platform}" \
     && PYTHONPATH="$(host_path "${platform}")" \
        "${py}" "$(host_path "${driver}")" "${corpus}" \
            --trace-rules "${trace_rules}" \
            --emit-traces "${golden}" \
        < "${scenarios}" ); then
  status=0
else
  status=$?
fi

# --- 6. summary -----------------------------------------------------------

log "----"
log "platform SHA: ${head_sha}"
log "scenarios:    ${n} (seed=${seed}, fuzz/rule=${fuzz})"
if [ "${status}" -ne 0 ]; then
  fatal "Rust ≢ Python — see divergences above"
fi
log "PASS: Rust ≡ Python over ${n} scenarios; trace fixtures refreshed"
