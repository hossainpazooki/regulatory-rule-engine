#!/usr/bin/env bash
#
# generate-golden-fixtures.sh — produce the golden artifacts under
# fixtures/artifacts/ (brief docs/gate-1-canonical-ir.md § 8.3).
#
# Modes:
#   --synthetic   (default) Generate Rust-authored example artifacts via the
#                 ke-core `gen-fixtures` binary. Self-contained; no platform
#                 checkout required. This is Gate 1's mode.
#   --platform    Generate from the platform Python pipeline. Enforces that the
#                 platform checkout matches the SHA recorded in
#                 fixtures/rules/SOURCE.md before running. NOT IMPLEMENTED in
#                 Gate 1 — the cross-corpus Python-driven path is deferred until
#                 the recorded SHA is reconciled with the platform HEAD.
#
# Idempotent: running twice produces byte-identical output. CI asserts this and
# fails on any `git diff` of the generated paths.
#
# Platform repo resolution (spec § 4.5): ${PLATFORM_REPO:-../institutional-defi-platform-api}.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mode="synthetic"

for arg in "$@"; do
  case "$arg" in
    --synthetic) mode="synthetic" ;;
    --platform)  mode="platform" ;;
    -h|--help)
      sed -n '2,30p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *) echo "unknown argument: $arg" >&2; exit 2 ;;
  esac
done

recorded_sha() {
  # Pull the 40-hex platform commit out of fixtures/rules/SOURCE.md.
  grep -oE '[0-9a-f]{40}' "${repo_root}/fixtures/rules/SOURCE.md" | head -n1
}

check_platform_sha() {
  local platform="${PLATFORM_REPO:-${repo_root}/../institutional-defi-platform-api}"
  if [ ! -d "${platform}/.git" ]; then
    echo "FATAL: platform checkout not found at ${platform}" >&2
    echo "       set PLATFORM_REPO or place a sibling institutional-defi-platform-api." >&2
    exit 1
  fi
  local want have
  want="$(recorded_sha)"
  have="$(git -C "${platform}" rev-parse HEAD)"
  if [ "${want}" != "${have}" ]; then
    echo "FATAL: platform HEAD ${have} != recorded SOURCE.md SHA ${want}" >&2
    echo "       re-bootstrap the corpus or check out the recorded commit before generating." >&2
    exit 1
  fi
  echo "platform SHA verified: ${have}"
}

case "${mode}" in
  synthetic)
    echo "[synthetic] generating Rust-authored golden fixtures…"
    recorded="$(recorded_sha || true)"
    echo "[synthetic] note: platform-driven cross-corpus bytes are deferred for Gate 1."
    echo "[synthetic] recorded corpus SHA (fixtures/rules/SOURCE.md): ${recorded:-<none>}"
    ( cd "${repo_root}" && cargo run -q -p ke-core --bin gen-fixtures )
    echo "[synthetic] done. Re-run to confirm idempotence (bytes must not change)."
    ;;
  platform)
    check_platform_sha
    echo "FATAL: --platform generation is not implemented in Gate 1." >&2
    echo "       The Python-driven cross-corpus path lands once the recorded" >&2
    echo "       SOURCE.md SHA is reconciled with the platform HEAD. Use --synthetic." >&2
    exit 1
    ;;
esac
