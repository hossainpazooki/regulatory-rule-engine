#!/usr/bin/env bash
#
# lifecycle-smoke.sh — Gate 4 Phase 3b end-to-end smoke for the `ke` lifecycle.
#
# WHAT THIS IS FOR
# ----------------
# Phase 3b drives an artifact through the rest of the spec-9 lifecycle via CLI
# commands: ml-check (dev stand-in T2/T3) -> attest (expert) -> publish ->
# deprecate -> revoke, on top of the Phase-3a compile (-> structurally_verified)
# and query. This script proves the `ke` binary actually does that, end to end,
# against a real corpus rule — not just the integration tests.
#
# It:
#   1. builds `ke` with --features test-keys (the compiler, registry-root, and
#      expert signing keys are fixed-seed TEST keys; nothing signs with a
#      production key);
#   2. compiles one real corpus rule into a tmp local-FS registry with a fixed
#      KE_NOW (deterministic), asserting structurally_verified;
#   3. ml-check -> ml_checked; attest (source_fidelity + scenario_coverage +
#      publication_approval) -> expert_attested; publish --env staging ->
#      published; query --tag staging/current asserts Published; deprecate ->
#      deprecated; revoke --policy auditonly --reason "smoke" -> revoked;
#   4. re-runs the whole lifecycle into a SECOND tmp registry and diffs the
#      WHOLE tree (events/ artifacts/ tags/ consistency/ revocations/) —
#      byte-identical (determinism via fixed keys + KE_NOW).
#
# Local-FS registry objects are NON-AUTHORITATIVE (ADR 0012 §6); this is a
# dev/test harness only. The revocation policy is RECORDED, not enforced —
# runtime enforcement is platform/Gate 6.
#
# USAGE
#   ./scripts/lifecycle-smoke.sh
#
# Designed to run in a bash terminal, including Git Bash / MINGW64 on Windows.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# log goes to stderr so functions can return a value on stdout cleanly.
log()   { printf '%s\n' "$*" >&2; }
fatal() { printf 'FATAL: %s\n' "$*" >&2; exit 1; }

# Deterministic clock for every command (2025-06-15T15:06:40Z).
export KE_NOW=1750000000

# One corpus rule that compiles cleanly (no blocking findings).
YAML="fixtures/rules/mica_stablecoin.yaml"
REGIME="mica_2023"
ENV="staging"

cleanup() {
  [ -n "${tmp1:-}" ] && rm -rf "${tmp1}" 2>/dev/null || true
  [ -n "${tmp2:-}" ] && rm -rf "${tmp2}" 2>/dev/null || true
}
trap cleanup EXIT

# 1. Build the `ke` binary with the test-keys feature.
log "[build] cargo build -p ke-cli --features test-keys"
( cd "${repo_root}" && cargo build -q -p ke-cli --features test-keys ) \
  || fatal "build failed"

ke_bin="${repo_root}/target/debug/ke"
[ -x "${ke_bin}" ] || ke_bin="${repo_root}/target/debug/ke.exe"
[ -x "${ke_bin}" ] || fatal "ke binary not found under target/debug"

# Assert a command's stdout contains a state token "state=<WANT>".
assert_state() {
  local out="$1" want="$2" what="$3"
  printf '%s\n' "${out}" | grep -q "state=${want}" \
    || fatal "${what}: expected state=${want}, got:\n${out}"
}

# Drive the full lifecycle in one registry; echo the artifact hash on success.
drive_lifecycle() {
  local registry="$1"
  local out hash

  # compile -> structurally_verified
  out="$("${ke_bin}" --registry "${registry}" compile "${repo_root}/${YAML}" --regime "${REGIME}")" \
    || fatal "compile failed"
  assert_state "${out}" "StructurallyVerified" "compile"
  hash="$(printf '%s\n' "${out}" | sed -n 's/.*hash=\([0-9a-f]\{64\}\).*/\1/p')"
  [ -n "${hash}" ] || fatal "no content hash printed: ${out}"
  log "[compile]   ${hash} -> StructurallyVerified OK"

  # ml-check -> ml_checked
  out="$("${ke_bin}" --registry "${registry}" ml-check --hash "${hash}")" \
    || fatal "ml-check failed"
  assert_state "${out}" "MlChecked" "ml-check"
  log "[ml-check]  -> MlChecked OK (dev stand-in consistency sidecar)"

  # attest (three types) -> expert_attested
  out="$("${ke_bin}" --registry "${registry}" attest --hash "${hash}" \
          --type source_fidelity --type scenario_coverage --type publication_approval)" \
    || fatal "attest failed"
  assert_state "${out}" "ExpertAttested" "attest"
  log "[attest]    -> ExpertAttested OK (3 attestations)"

  # publish -> published + tag pointer
  out="$("${ke_bin}" --registry "${registry}" publish --hash "${hash}" --env "${ENV}")" \
    || fatal "publish failed"
  assert_state "${out}" "Published" "publish"
  log "[publish]   -> Published OK (tag ${ENV}/current)"

  # query --tag asserts Published
  out="$("${ke_bin}" --registry "${registry}" query --tag "${ENV}/current")" \
    || fatal "query --tag failed"
  printf '%s\n' "${out}" | grep -q "state:[[:space:]]*Published" \
    || fatal "query --tag ${ENV}/current: expected Published, got:\n${out}"
  printf '%s\n' "${out}" | grep -q "${hash}" \
    || fatal "query --tag ${ENV}/current did not resolve to ${hash}:\n${out}"
  log "[query]     --tag ${ENV}/current -> Published, resolves to ${hash} OK"

  # deprecate -> deprecated
  out="$("${ke_bin}" --registry "${registry}" deprecate --hash "${hash}")" \
    || fatal "deprecate failed"
  assert_state "${out}" "Deprecated" "deprecate"
  log "[deprecate] -> Deprecated OK"

  # revoke (auditonly) -> revoked + revocation sidecar (severity=high)
  out="$("${ke_bin}" --registry "${registry}" revoke --hash "${hash}" \
          --policy auditonly --reason "smoke")" \
    || fatal "revoke failed"
  assert_state "${out}" "Revoked" "revoke"
  printf '%s\n' "${out}" | grep -q "severity=high" \
    || fatal "revoke --policy auditonly: expected severity=high, got:\n${out}"
  log "[revoke]    -> Revoked OK (policy auditonly RECORDED, severity=high; NOT enforced)"

  # Confirm the sidecars exist with the expected content.
  [ -f "${registry}/consistency/${hash}.json" ] \
    || fatal "missing consistency sidecar for ${hash}"
  grep -q "local-dev-standin" "${registry}/consistency/${hash}.json" \
    || fatal "consistency sidecar is not the dev stand-in"
  [ -f "${registry}/revocations/${hash}.json" ] \
    || fatal "missing revocation sidecar for ${hash}"
  grep -q '"severity": "high"' "${registry}/revocations/${hash}.json" \
    || fatal "revocation sidecar severity is not high for auditonly"
  [ -f "${registry}/NON_AUTHORITATIVE" ] || fatal "missing NON_AUTHORITATIVE marker"

  printf '%s\n' "${hash}"
}

# 2 + 3. First registry: full lifecycle.
tmp1="$(mktemp -d "${TMPDIR:-/tmp}/ke-lifecycle-smoke-1.XXXXXX")"
log "[registry] ${tmp1}"
hash1="$(drive_lifecycle "${tmp1}")"

# 4. Determinism: re-run the whole lifecycle into a second registry, diff the
#    WHOLE tree (events/ artifacts/ tags/ consistency/ revocations/).
tmp2="$(mktemp -d "${TMPDIR:-/tmp}/ke-lifecycle-smoke-2.XXXXXX")"
log "[registry] ${tmp2} (determinism re-run)"
hash2="$(drive_lifecycle "${tmp2}")"

[ "${hash1}" = "${hash2}" ] \
  || fatal "content hash differs between runs: ${hash1} vs ${hash2}"

det_ok=1
for sub in events artifacts tags consistency revocations; do
  diff -r "${tmp1}/${sub}" "${tmp2}/${sub}" >/dev/null || det_ok=0
done
[ "${det_ok}" = "1" ] \
  || fatal "registry trees differ between runs — non-determinism detected"
log "[determinism] events+artifacts+tags+consistency+revocations byte-identical OK"

log ""
log "lifecycle-smoke: PASS (compile -> ml-check -> attest -> publish -> query -> deprecate -> revoke; deterministic)"
