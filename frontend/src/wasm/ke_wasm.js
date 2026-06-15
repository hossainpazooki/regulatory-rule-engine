/**
 * SCAFFOLD STUB — replaced by the real wasm-bindgen `--target web` glue during
 * the Build phase / CI (see README.md):
 *
 *   wasm-bindgen --target web --out-dir frontend/src/wasm \
 *     target/wasm32-unknown-unknown/debug/ke_wasm.wasm
 *
 * This stub exists ONLY so the typed adapter (`index.ts`) and its vitest smoke
 * test resolve their dynamic import before the real `.wasm` is built. Every
 * export throws if actually invoked: calling preview/dry-run/verify against the
 * stub is a build-misconfiguration, never a silent no-op. The smoke test mocks
 * this module via `vi.mock('./ke_wasm.js')`, so these bodies do not run in unit
 * tests; they only fire if a real app forgot to generate the bindings.
 *
 * NON-AUTHORITATIVE: even the real glue this replaces cannot sign/attest/publish
 * (spec § 6 / § 16) — it is preview-only.
 */

const NOT_BUILT =
  'ke-wasm bindings not generated: run `wasm-bindgen --target web --out-dir ' +
  'frontend/src/wasm target/wasm32-unknown-unknown/debug/ke_wasm.wasm` ' +
  '(see frontend/src/wasm/README.md). This is the scaffold stub.'

export default function init() {
  return Promise.reject(new Error(NOT_BUILT))
}

export function compile_preview() {
  throw new Error(NOT_BUILT)
}

export function dry_run() {
  throw new Error(NOT_BUILT)
}

export function verify_artifact() {
  throw new Error(NOT_BUILT)
}

export function read_provenance() {
  throw new Error(NOT_BUILT)
}
