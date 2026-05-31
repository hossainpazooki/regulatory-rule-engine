#!/usr/bin/env bash
#
# differential-test.sh — Gate 2 differential compile (Rust vs Python).
#
# For every YAML in fixtures/rules/, load it with the platform `RuleLoader`
# (Python) and compile it with `ke-compile` (Rust), then compare both at the
# semantic-normal-form level. Exits non-zero on any divergence.
#
# Platform access (spec §4.5): resolves ${PLATFORM_REPO:-../institutional-defi-platform-api}.
# INVARIANT: the platform checkout MUST be at the SHA recorded in
# fixtures/rules/SOURCE.md (not whatever HEAD happens to be). Fails fast
# otherwise so Rust is never compared against a different corpus revision.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
platform="${PLATFORM_REPO:-${repo_root}/../institutional-defi-platform-api}"

# --- platform checkout + SHA gate -----------------------------------------

recorded_sha="$(grep -oE '[0-9a-f]{40}' "${repo_root}/fixtures/rules/SOURCE.md" | head -n1)"
if [ -z "${recorded_sha}" ]; then
  echo "FATAL: no recorded SHA in fixtures/rules/SOURCE.md" >&2
  exit 1
fi
if [ ! -d "${platform}/.git" ]; then
  echo "FATAL: platform checkout not found at ${platform}" >&2
  echo "       set PLATFORM_REPO or place a sibling institutional-defi-platform-api." >&2
  exit 1
fi
head_sha="$(git -C "${platform}" rev-parse HEAD)"
if [ "${head_sha}" != "${recorded_sha}" ]; then
  echo "FATAL: platform HEAD ${head_sha}" >&2
  echo "       != recorded SOURCE.md SHA ${recorded_sha}" >&2
  echo "       run: git -C \"${platform}\" checkout ${recorded_sha}" >&2
  exit 1
fi
if ! git -C "${platform}" diff --quiet -- src/rules/data; then
  echo "FATAL: ${platform}/src/rules/data has uncommitted changes" >&2
  exit 1
fi
echo "platform SHA verified: ${head_sha}"

# --- resolve python (prefer the platform venv) ----------------------------

py=""
for cand in "${platform}/.venv/bin/python" "${platform}/.venv/Scripts/python.exe" python3 python; do
  if command -v "${cand}" >/dev/null 2>&1 || [ -x "${cand}" ]; then
    py="${cand}"
    break
  fi
done
if [ -z "${py}" ]; then
  echo "FATAL: no python interpreter found" >&2
  exit 1
fi

# --- build the Rust dev tool ----------------------------------------------

( cd "${repo_root}" && cargo build -q -p ke-compiler )
ke_compile="${repo_root}/target/debug/ke-compile"

# Python: load one rule file via RuleLoader → JSON list of Rule.model_dump.
dump_py='
import json, sys
from src.rules.service import RuleLoader
loader = RuleLoader()
rules = loader.load_file(sys.argv[1])
print(json.dumps([r.model_dump(mode="json") for r in rules]))
'

tmp="$(mktemp -d)"
trap 'rm -rf "${tmp}"' EXIT

fail=0
count=0
for yaml in "${repo_root}"/fixtures/rules/*.yaml; do
  [ "$(basename "${yaml}")" = "schema.yaml" ] && continue
  count=$((count + 1))
  json="${tmp}/$(basename "${yaml}").json"
  if ! ( cd "${platform}" && "${py}" -c "${dump_py}" "${yaml}" ) > "${json}" 2>"${tmp}/py.err"; then
    echo "FAIL (python load): $(basename "${yaml}")" >&2
    cat "${tmp}/py.err" >&2
    fail=$((fail + 1))
    continue
  fi
  if ! "${ke_compile}" diff "${yaml}" "${json}"; then
    fail=$((fail + 1))
  fi
done

echo "----"
echo "platform SHA: ${head_sha}"
echo "checked ${count} rule file(s); ${fail} with divergence(s)"
[ "${fail}" -eq 0 ] || exit 1
