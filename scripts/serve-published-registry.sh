#!/usr/bin/env bash
#
# serve-published-registry.sh — stand up a local `ke serve` over a registry that
# actually contains a PUBLISHED artifact, so the consumer (COMPASS) verify loop
# returns `verified / Published` instead of the empty-registry `Unknown`.
#
# WHAT THIS IS FOR
# ----------------
# The live-verifier loop needs three things on the ATLAS side: a browser build
# of `@platform/atlas-artifact` (see docs/publish-atlas-artifact.md), and a
# running `ke serve` whose registry has artifacts in the `Published` lifecycle
# state. This script provides the latter, reproducibly:
#
#   1. builds `ke` with --features test-keys (compiler / registry-root / expert
#      signing keys are fixed-seed TEST keys — nothing signs with a production
#      key; production key authority is the still-open ADR-0009 decision);
#   2. seeds a local-FS registry with TWO artifacts under a fixed KE_NOW:
#        - mica_stablecoin  -> driven compile -> ml-check -> attest -> publish,
#          ending in `Published` (the happy-path artifact to verify);
#        - fca_crypto       -> compile ONLY, left at `StructurallyVerified`
#          (maps to RegistryStatus::Unknown), so the consumer can prove its
#          fail-closed path: a non-Published artifact must NOT verify as
#          Published (ADR-0019);
#   3. prints both content hashes + ready-to-paste curl examples;
#   4. exec's `ke serve` against that registry (foreground; Ctrl-C to stop).
#
# Local-FS registry objects are NON-AUTHORITATIVE (ADR 0012 §6) — this is a
# dev/consumer harness, never the authoritative registry. `ke serve` is
# read-only: it never signs, attests, publishes, or mutates lifecycle state.
#
# USAGE
#   ./scripts/serve-published-registry.sh
#   PORT=8787 ./scripts/serve-published-registry.sh
#   KE_REGISTRY_DIR=/path/to/keep ./scripts/serve-published-registry.sh
#
# Requires `ke serve` to be built with --features test-keys, or /verify returns
# HTTP 500 by design (it needs a verifying-key directory). This script builds it.
#
# Designed to run in a bash terminal, including Git Bash / MINGW64 on Windows.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

log()   { printf '%s\n' "$*" >&2; }
fatal() { printf 'FATAL: %s\n' "$*" >&2; exit 1; }

# Deterministic clock for every seeding command (2025-06-15T15:06:40Z) so the
# published content hash is stable across runs.
export KE_NOW="${KE_NOW:-1750000000}"

HOST="${HOST:-127.0.0.1}"
PORT="${PORT:-9999}"

# The published (happy-path) rule and the compile-only (fail-closed) rule.
PUB_YAML="fixtures/rules/mica_stablecoin.yaml"
PUB_REGIME="mica_2023"
PUB_ENV="local"
UNPUB_YAML="fixtures/rules/fca_crypto.yaml"
UNPUB_REGIME="fca_crypto_2024"

# 1. Build `ke` with the test-keys feature.
log "[build] cargo build -p ke-cli --features test-keys"
( cd "${repo_root}" && cargo build -q -p ke-cli --features test-keys ) \
  || fatal "build failed"
ke_bin="${repo_root}/target/debug/ke"
[ -x "${ke_bin}" ] || ke_bin="${repo_root}/target/debug/ke.exe"
[ -x "${ke_bin}" ] || fatal "ke binary not found under target/debug"

# Registry dir: a fresh tmp dir by default; override with KE_REGISTRY_DIR to keep.
registry="${KE_REGISTRY_DIR:-$(mktemp -d "${TMPDIR:-/tmp}/ke-published-registry.XXXXXX")}"
mkdir -p "${registry}"
log "[registry] ${registry}"

assert_state() {
  local out="$1" want="$2" what="$3"
  printf '%s\n' "${out}" | grep -q "state=${want}" \
    || fatal "${what}: expected state=${want}, got:\n${out}"
}

hash_of() { printf '%s\n' "$1" | sed -n 's/.*hash=\([0-9a-f]\{64\}\).*/\1/p'; }

# --- Seed the PUBLISHED artifact: compile -> ml-check -> attest -> publish. ---
out="$("${ke_bin}" --registry "${registry}" compile "${repo_root}/${PUB_YAML}" --regime "${PUB_REGIME}")" \
  || fatal "compile (published) failed"
assert_state "${out}" "StructurallyVerified" "compile(published)"
PUB_HASH="$(hash_of "${out}")"
[ -n "${PUB_HASH}" ] || fatal "no content hash printed for published rule:\n${out}"

"${ke_bin}" --registry "${registry}" ml-check --hash "${PUB_HASH}" >/dev/null \
  || fatal "ml-check failed"
"${ke_bin}" --registry "${registry}" attest --hash "${PUB_HASH}" \
  --type source_fidelity --type scenario_coverage --type publication_approval >/dev/null \
  || fatal "attest failed"
out="$("${ke_bin}" --registry "${registry}" publish --hash "${PUB_HASH}" --env "${PUB_ENV}")" \
  || fatal "publish failed"
assert_state "${out}" "Published" "publish"

# Confirm Published via query --tag (the canonical resolve path).
out="$("${ke_bin}" --registry "${registry}" query --tag "${PUB_ENV}/current")" \
  || fatal "query --tag failed"
printf '%s\n' "${out}" | grep -q "state:[[:space:]]*Published" \
  || fatal "query --tag ${PUB_ENV}/current: expected Published, got:\n${out}"
log "[seed] PUBLISHED  ${PUB_HASH} (tag ${PUB_ENV}/current)"

# --- Seed the UNPUBLISHED artifact: compile only (-> StructurallyVerified). ---
out="$("${ke_bin}" --registry "${registry}" compile "${repo_root}/${UNPUB_YAML}" --regime "${UNPUB_REGIME}")" \
  || fatal "compile (unpublished) failed"
assert_state "${out}" "StructurallyVerified" "compile(unpublished)"
UNPUB_HASH="$(hash_of "${out}")"
[ -n "${UNPUB_HASH}" ] || fatal "no content hash printed for unpublished rule:\n${out}"
log "[seed] UNPUBLISHED ${UNPUB_HASH} (StructurallyVerified -> RegistryStatus::Unknown)"

base="http://${HOST}:${PORT}"
log ""
log "Registry seeded. Starting ke serve on ${base} (read-only, non-authoritative)."
log "Verify keys are fixed-seed TEST keys — NOT production-trusted."
log ""
log "  # happy path -> verified / Published:"
log "  curl -s -X POST ${base}/verify -H 'content-type: application/json' -d '{\"hash\":\"${PUB_HASH}\"}'"
log ""
log "  # fail-closed -> NOT Published (non-published artifact):"
log "  curl -s -X POST ${base}/verify -H 'content-type: application/json' -d '{\"hash\":\"${UNPUB_HASH}\"}'"
log ""
log "  curl -s ${base}/healthz"
log ""

# 4. Hand the process over to the server (foreground; Ctrl-C to stop).
exec "${ke_bin}" --registry "${registry}" serve --host "${HOST}" --port "${PORT}"
