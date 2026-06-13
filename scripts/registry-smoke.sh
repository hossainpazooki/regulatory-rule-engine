#!/usr/bin/env bash
#
# registry-smoke.sh — Gate 4 Phase 3a end-to-end smoke for the `ke` registry.
#
# WHAT THIS IS FOR
# ----------------
# Phase 3a adds the registry lifecycle as an append-only, hash-chained,
# registry-root-signed event log on a local-FS backend, driven by `ke compile`
# (-> draft -> structurally_verified) and read back by `ke query`. This script
# proves the binary actually does that, end to end, against the real corpus
# fixtures — not just the unit/integration tests.
#
# It:
#   1. builds `ke` with --features test-keys (the compiler + registry-root
#      signing keys are fixed-seed TEST keys; Phase 3a does not sign with
#      production keys);
#   2. compiles two real corpus rules into a tmp local-FS registry with a fixed
#      KE_NOW (deterministic), asserting each reaches `structurally_verified`;
#   3. `ke query --hash <printed hash>` for each and asserts the state line;
#   4. re-runs the whole compile into a SECOND tmp registry and diffs the event
#      trees — byte-identical (determinism via fixed keys + KE_NOW).
#
# Local-FS registry objects are NON-AUTHORITATIVE (ADR 0012 §6); this is a
# dev/test harness only.
#
# USAGE
#   ./scripts/registry-smoke.sh
#
# Designed to run in a bash terminal, including Git Bash / MINGW64 on Windows.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

log()   { printf '%s\n' "$*"; }
fatal() { printf 'FATAL: %s\n' "$*" >&2; exit 1; }

# Deterministic clock for every compile (2025-06-15T15:06:40Z).
export KE_NOW=1750000000

# Two corpus rules that compile cleanly (no blocking findings) with regimes.
FIXTURES=(
  "fixtures/rules/mica_stablecoin.yaml:mica_2023"
  "fixtures/rules/fca_crypto.yaml:fca_cryptoassets"
)

cleanup() {
  [ -n "${tmp1:-}" ] && rm -rf "${tmp1}" 2>/dev/null || true
  [ -n "${tmp2:-}" ] && rm -rf "${tmp2}" 2>/dev/null || true
}
trap cleanup EXIT

# 1. Build the `ke` binary with the test-keys feature.
log "[build] cargo build -p ke-cli --features test-keys"
( cd "${repo_root}" && cargo build -q -p ke-cli --features test-keys ) \
  || fatal "build failed"

# Locate the built binary (cargo target dir; .exe on Windows).
ke_bin="${repo_root}/target/debug/ke"
[ -x "${ke_bin}" ] || ke_bin="${repo_root}/target/debug/ke.exe"
[ -x "${ke_bin}" ] || fatal "ke binary not found under target/debug"

# Run a compile into a registry dir; echo the printed content hash.
# Asserts the printed final state is structurally_verified.
compile_one() {
  local registry="$1" yaml="$2" regime="$3"
  local out hash state
  out="$("${ke_bin}" --registry "${registry}" compile "${repo_root}/${yaml}" --regime "${regime}")" \
    || fatal "compile ${yaml} failed"
  # Output line: "compiled: hash=<hex> state=StructurallyVerified"
  hash="$(printf '%s\n' "${out}" | sed -n 's/.*hash=\([0-9a-f]\{64\}\).*/\1/p')"
  state="$(printf '%s\n' "${out}" | sed -n 's/.*state=\([A-Za-z]*\).*/\1/p')"
  [ -n "${hash}" ] || fatal "no content hash printed for ${yaml}: ${out}"
  [ "${state}" = "StructurallyVerified" ] \
    || fatal "${yaml} reached state '${state}', expected StructurallyVerified"
  printf '%s\n' "${hash}"
}

# Assert `ke query --hash` prints the expected state line.
query_assert_state() {
  local registry="$1" hash="$2" want="$3"
  local out
  out="$("${ke_bin}" --registry "${registry}" query --hash "${hash}")" \
    || fatal "query --hash ${hash} failed"
  printf '%s\n' "${out}" | grep -q "state:[[:space:]]*${want}" \
    || fatal "query state mismatch for ${hash}: wanted ${want}, got:\n${out}"
}

# 2 + 3. First registry: compile each fixture, query each by hash.
tmp1="$(mktemp -d "${TMPDIR:-/tmp}/ke-registry-smoke-1.XXXXXX")"
log "[registry] ${tmp1}"
declare -a hashes1=()
for entry in "${FIXTURES[@]}"; do
  yaml="${entry%%:*}"; regime="${entry##*:}"
  log "[compile] ${yaml} (regime ${regime})"
  h="$(compile_one "${tmp1}" "${yaml}" "${regime}")"
  log "          hash=${h} state=StructurallyVerified"
  query_assert_state "${tmp1}" "${h}" "StructurallyVerified"
  log "[query]   ${h} -> StructurallyVerified OK"
  hashes1+=("${h}")
done

# Confirm the non-authoritative marker is present (ADR 0012 §6).
[ -f "${tmp1}/NON_AUTHORITATIVE" ] || fatal "missing NON_AUTHORITATIVE marker"
log "[marker]  NON_AUTHORITATIVE present (ADR 0012 §6)"

# 4. Determinism: re-compile into a second registry and diff the event trees.
tmp2="$(mktemp -d "${TMPDIR:-/tmp}/ke-registry-smoke-2.XXXXXX")"
log "[registry] ${tmp2} (determinism re-run)"
for entry in "${FIXTURES[@]}"; do
  yaml="${entry%%:*}"; regime="${entry##*:}"
  compile_one "${tmp2}" "${yaml}" "${regime}" >/dev/null
done

# The events/ and artifacts/ subtrees must be byte-identical between runs
# (same input + fixed keys + fixed KE_NOW => identical content hash, identical
# signed events). The NON_AUTHORITATIVE marker is identical too.
if diff -r "${tmp1}/events" "${tmp2}/events" >/dev/null \
   && diff -r "${tmp1}/artifacts" "${tmp2}/artifacts" >/dev/null; then
  log "[determinism] event + artifact trees byte-identical across runs OK"
else
  fatal "registry trees differ between runs — non-determinism detected"
fi

log ""
log "registry-smoke: PASS (${#hashes1[@]} artifacts compiled -> structurally_verified, queried, deterministic)"
