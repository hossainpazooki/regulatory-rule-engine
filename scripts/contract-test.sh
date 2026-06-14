#!/usr/bin/env bash
#
# contract-test.sh — Gate 4 Phase 4b three-language artifact contract.
#
# WHAT THIS IS FOR
# ----------------
# "The artifact is the contract" (ADR 0016) means a `.kew` must yield the SAME
# verdict and the SAME canonical provenance in every consumer language. This
# script proves it: for each committed golden artifact it runs the verify
# surface three ways —
#
#   Rust:    ke-artifact `contract-verify` example (the pure 4a verify_artifact)
#   Python:  the ke-artifact-py wheel (PyO3 binding over the same fns)
#   WASM:    @platform/atlas-artifact via node (wasm-bindgen over the same fns)
#
# — over ONE shared set of verifier inputs (scripts/contract-inputs/*.json) and
# asserts the three emit byte-identical `{verdict, registry_state, content_hash,
# provenance}` JSON. The bindings add NO crypto and NO policy: they wrap the same
# Rust functions, so agreement is the whole point.
#
# A leg whose toolchain is absent is SKIPPED WITH A LOUD MESSAGE (never a silent
# pass). The Rust leg is always present (it only needs cargo); Python needs the
# installed wheel in an importable interpreter; WASM needs the built package +
# node. The script passes iff every PRESENT leg agrees on every golden, and at
# least the Rust leg ran.
#
# INVARIANT (spec §4.5): the platform checkout MUST be at the SHA recorded in
# fixtures/rules/SOURCE.md (the revision the corpus + goldens were snapshotted
# from). The goldens are content-addressed off that corpus, so a SHA drift means
# a different contract. The script fails fast if the SHA is wrong — UNLESS the
# platform checkout is simply absent, in which case it warns and proceeds (the
# contract test does not itself invoke the platform; the SHA gate guards against
# a *wrong* checkout, which only matters when one is present).
#
# RE-RUNNABLE: no global state; rebuilds the Rust leg, re-imports the wheel,
# re-loads the wasm package each run.
#
# USAGE
#   ./scripts/contract-test.sh
#   PLATFORM_REPO=/path/to/platform ./scripts/contract-test.sh
#
# Designed for bash, including Git Bash / MINGW64 on Windows.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
platform="${PLATFORM_REPO:-${repo_root}/../institutional-defi-platform-api}"
inputs="${repo_root}/scripts/contract-inputs"

log()   { printf '%s\n' "$*"; }
warn()  { printf 'WARN: %s\n' "$*" >&2; }
fatal() { printf 'FATAL: %s\n' "$*" >&2; exit 1; }

# The export timestamp the provenance records — fixed so the canonical JSON is
# byte-stable across runs and across the three languages.
EXPORTED_AT=1750000000

# ---------------------------------------------------------------------------
# 1. platform checkout + SHA gate (spec §4.5)
# ---------------------------------------------------------------------------

recorded_sha="$(grep -oE '[0-9a-f]{40}' "${repo_root}/fixtures/rules/SOURCE.md" | head -n1 || true)"
[ -n "${recorded_sha}" ] || fatal "no 40-hex SHA found in fixtures/rules/SOURCE.md"

if [ -d "${platform}/.git" ]; then
  head_sha="$(git -C "${platform}" rev-parse HEAD)"
  if [ "${head_sha}" != "${recorded_sha}" ]; then
    fatal "platform HEAD ${head_sha}
       != recorded SOURCE.md SHA ${recorded_sha}
       run: git -C \"${platform}\" checkout ${recorded_sha}"
  fi
  log "platform SHA verified: ${head_sha}"
else
  warn "platform checkout not found at ${platform} (set PLATFORM_REPO to enable the SHA gate)"
  warn "proceeding: the contract test does not invoke the platform; goldens are committed"
fi

# Shared verifier inputs must exist (regenerate via the emit-contract-inputs example).
for f in keydir.json context.json policy.json registry.json; do
  [ -f "${inputs}/${f}" ] || fatal "missing ${inputs}/${f}
       regenerate: cargo run -p ke-artifact --features test-keys --example emit-contract-inputs"
done

# ---------------------------------------------------------------------------
# 2. resolve cargo (rustup bin may be off PATH in a fresh MINGW64/CI shell)
# ---------------------------------------------------------------------------

if ! command -v cargo >/dev/null 2>&1; then
  [ -f "${HOME}/.cargo/env" ] && . "${HOME}/.cargo/env"  # shellcheck disable=SC1091
  export PATH="${HOME}/.cargo/bin:${PATH}"
fi
command -v cargo >/dev/null 2>&1 \
  || fatal "cargo not found — install rustup or add ~/.cargo/bin to PATH"

# Build the Rust contract-verify example once (needs the test-keys feature so the
# strict policy mirrors the goldens' attested types).
log "building the Rust verify leg (ke-artifact --example contract-verify)…"
( cd "${repo_root}" && cargo build -q -p ke-artifact --features test-keys --example contract-verify )
rust_bin="${repo_root}/target/debug/examples/contract-verify"
[ -x "${rust_bin}" ] || rust_bin="${repo_root}/target/debug/examples/contract-verify.exe"
[ -x "${rust_bin}" ] || fatal "contract-verify example binary not found after build"

# ---------------------------------------------------------------------------
# 3. detect the optional Python + WASM legs (loud skip if absent)
# ---------------------------------------------------------------------------

# Python leg: an interpreter that can `import ke_artifact_py` (the installed
# wheel). Prefer the platform venv, then python3/python.
py=""
py_ok=0
for cand in \
  "${platform}/.venv/bin/python" \
  "${platform}/.venv/Scripts/python.exe" \
  python3 python; do
  if [ -x "${cand}" ] || command -v "${cand}" >/dev/null 2>&1; then
    if "${cand}" -c "import ke_artifact_py" >/dev/null 2>&1; then
      py="${cand}"; py_ok=1; break
    fi
  fi
done
if [ "${py_ok}" -eq 1 ]; then
  log "python leg: ${py} (ke_artifact_py importable)"
else
  warn "PYTHON LEG SKIPPED — no interpreter can 'import ke_artifact_py'."
  warn "  build+install the wheel: maturin build --features pyo3 (Linux/CI authoritative)"
  warn "  then pip install the wheel into the interpreter this script can see."
fi

# WASM leg: node + a nodejs-target build of the @platform/atlas-artifact crate.
#
# The browser-facing package (crates/ke-wasm/pkg, "type":"module") is the
# bundler/ESM artifact COMPASS consumes; node cannot load the *nodejs*-target
# CJS glue from under that ESM package.json. So the node leg uses a dedicated
# nodejs-target build under pkg-node/ with its own {"type":"commonjs"} marker.
# Same wasm-bindgen crate, same fns — only the JS module system differs.
wasm_ok=0
node_bin=""
wasm_pkg="${repo_root}/crates/ke-wasm/pkg-node"
if command -v node >/dev/null 2>&1; then
  node_bin="node"
  # Build the nodejs-target package if the wasm-bindgen-cli is present.
  if command -v wasm-bindgen >/dev/null 2>&1; then
    wb_ver="$(wasm-bindgen --version | awk '{print $2}')"
    if [ "${wb_ver}" != "0.2.95" ]; then
      warn "wasm-bindgen-cli ${wb_ver} != crate pin 0.2.95 — version lock-step REQUIRED; skipping wasm build"
    else
      log "building the WASM nodejs-target package (wasm-bindgen ${wb_ver})…"
      ( cd "${repo_root}" \
        && cargo build -q -p ke-wasm --target wasm32-unknown-unknown --release \
        && wasm-bindgen --target nodejs \
             --out-dir "${wasm_pkg}" --out-name ke_wasm \
             target/wasm32-unknown-unknown/release/ke_wasm.wasm )
      # CJS marker so node treats the nodejs-target glue as CommonJS regardless
      # of any parent "type":"module".
      printf '{"type":"commonjs"}\n' > "${wasm_pkg}/package.json"
    fi
  fi
  if [ -f "${wasm_pkg}/ke_wasm.js" ] && [ -f "${wasm_pkg}/ke_wasm_bg.wasm" ]; then
    wasm_ok=1
    log "wasm leg: node + ${wasm_pkg}"
  else
    warn "WASM LEG SKIPPED — node present but ${wasm_pkg}/ke_wasm.js not built."
    warn "  install the matching CLI: cargo install wasm-bindgen-cli --version 0.2.95"
    warn "  (or drop the prebuilt 0.2.95 binary on PATH); the CLI version MUST equal the crate pin."
  fi
else
  warn "WASM LEG SKIPPED — node not found on PATH."
fi

# ---------------------------------------------------------------------------
# 4. per-golden, per-leg verify + cross-language diff
# ---------------------------------------------------------------------------

tmp="$(mktemp -d)"
trap 'rm -rf "${tmp}"' EXIT

# A tiny node driver for the WASM leg (nodejs target). Reads argv, prints the
# same JSON line the Rust/Python legs print. Written per-run (re-runnable).
wasm_driver="${tmp}/wasm-verify.cjs"
cat > "${wasm_driver}" <<'NODE'
const { readFileSync } = require("node:fs");
const [, , modPath, kewPath, keydir, ctx, policy, registry, exportedAt] =
  process.argv;
// The nodejs-target wasm-bindgen output is a CommonJS module (require /
// module.exports) shipped under a {"type":"commonjs"} marker; load it directly.
const mod = require(modPath);
const verify_artifact = mod.verify_artifact;
const kew = new Uint8Array(readFileSync(kewPath));
const out = verify_artifact(
  kew,
  readFileSync(keydir, "utf8"),
  readFileSync(ctx, "utf8"),
  readFileSync(policy, "utf8"),
  readFileSync(registry, "utf8"),
  BigInt(exportedAt),
);
process.stdout.write(out);
NODE

run_rust() {
  "${rust_bin}" "$1" \
    "${inputs}/keydir.json" "${inputs}/context.json" \
    "${inputs}/policy.json" "${inputs}/registry.json" "${EXPORTED_AT}"
}

run_python() {
  "${py}" - "$1" "${inputs}/keydir.json" "${inputs}/context.json" \
    "${inputs}/policy.json" "${inputs}/registry.json" "${EXPORTED_AT}" <<'PY'
import json, sys
import ke_artifact_py as k
kew_path, keydir, ctx, policy, registry, exported_at = sys.argv[1:7]
with open(kew_path, "rb") as f:
    kew = f.read()
def read(p):
    with open(p, "r", encoding="utf-8") as fh:
        return fh.read()
out = k.verify_artifact(
    kew, read(keydir), read(ctx), read(policy), read(registry), int(exported_at)
)
# k.verify_artifact returns a native dict; emit compact JSON matching Rust/WASM.
sys.stdout.write(json.dumps(out, separators=(",", ":")))
PY
}

# On Git Bash / MINGW64, node needs native Windows paths, not the /c/... MSYS
# form `pwd` yields. Convert when cygpath is available; pass through otherwise.
winpath() {
  if command -v cygpath >/dev/null 2>&1; then cygpath -w "$1"; else printf '%s' "$1"; fi
}

run_wasm() {
  # nodejs-target package: require its .js and call the named export. The module
  # path is passed as argv so the driver `require`s the built package.
  "${node_bin}" "$(winpath "${wasm_driver}")" \
    "$(winpath "${wasm_pkg}/ke_wasm.js")" "$(winpath "$1")" \
    "$(winpath "${inputs}/keydir.json")" "$(winpath "${inputs}/context.json")" \
    "$(winpath "${inputs}/policy.json")" "$(winpath "${inputs}/registry.json")" \
    "${EXPORTED_AT}"
}

# Normalize a JSON line to a canonical, key-sorted compact form so trivial
# whitespace/key-order differences across emitters don't masquerade as a
# contract break (the provenance field order is already stable; this guards the
# top-level wrapper).
norm() {
  if command -v python3 >/dev/null 2>&1; then
    python3 -c 'import sys,json; print(json.dumps(json.load(sys.stdin),sort_keys=True,separators=(",",":")))'
  elif command -v python >/dev/null 2>&1; then
    python  -c 'import sys,json; print(json.dumps(json.load(sys.stdin),sort_keys=True,separators=(",",":")))'
  else
    cat
  fi
}

fail=0
checked=0
for dir in "${repo_root}"/fixtures/artifacts/*/; do
  kew="${dir}artifact.kew"
  [ -f "${kew}" ] || continue
  name="$(basename "${dir}")"
  checked=$((checked + 1))

  rust_out="$(run_rust "${kew}" | norm)" \
    || { log "FAIL  ${name}  (rust leg errored)"; fail=$((fail + 1)); continue; }

  # The reference verdict is the Rust leg's (the canonical surface).
  rust_verdict="$(printf '%s' "${rust_out}" | sed -E 's/.*"verdict":"([^"]*)".*/\1/')"

  legs="rust"
  agree=1

  if [ "${py_ok}" -eq 1 ]; then
    if py_out="$(run_python "${kew}" 2>"${tmp}/py.err" | norm)"; then
      legs="${legs}+python"
      if [ "${py_out}" != "${rust_out}" ]; then
        agree=0
        log "MISMATCH ${name}: python != rust"
        printf '  rust  : %s\n' "${rust_out}" >&2
        printf '  python: %s\n' "${py_out}" >&2
      fi
    else
      log "FAIL  ${name}  (python leg errored)"; sed 's/^/        /' "${tmp}/py.err" >&2
      fail=$((fail + 1))
    fi
  fi

  if [ "${wasm_ok}" -eq 1 ]; then
    if wasm_out="$(run_wasm "${kew}" 2>"${tmp}/wasm.err" | norm)"; then
      legs="${legs}+wasm"
      if [ "${wasm_out}" != "${rust_out}" ]; then
        agree=0
        log "MISMATCH ${name}: wasm != rust"
        printf '  rust: %s\n' "${rust_out}" >&2
        printf '  wasm: %s\n' "${wasm_out}" >&2
      fi
    else
      log "FAIL  ${name}  (wasm leg errored)"; sed 's/^/        /' "${tmp}/wasm.err" >&2
      fail=$((fail + 1))
    fi
  fi

  if [ "${agree}" -eq 1 ]; then
    log "ok    ${name}  [${legs}]  verdict=${rust_verdict}"
  else
    fail=$((fail + 1))
  fi
done

# ---------------------------------------------------------------------------
# 5. summary
# ---------------------------------------------------------------------------

log "----"
log "checked ${checked} golden artifact(s)"
[ "${py_ok}"  -eq 1 ] || log "NOTE: python leg was SKIPPED (wheel not importable) — see warnings above"
[ "${wasm_ok}" -eq 1 ] || log "NOTE: wasm leg was SKIPPED (package not built / no node) — see warnings above"
[ "${checked}" -gt 0 ] || fatal "no golden artifacts found under fixtures/artifacts/*/"
[ "${fail}" -eq 0 ] || exit 1
log "PASS: every present leg agrees on verdict + canonical provenance over all goldens"
