# `frontend/src/wasm/` — browser preview/dry-run + verify bindings

This directory holds the **hand-written, typed adapter** over the `ke-wasm`
crate's browser bindings, plus the G5-2 parity cross-check. Everything here is
**NON-AUTHORITATIVE** (spec § 6 / § 16, CLAUDE.md authority boundary): the
browser never signs, attests, publishes, assembles, or mutates a registry. The
canonical compute is `ke-cli serve`; the WASM result is preview-only.

## Files

| File | Status | What it is |
|------|--------|------------|
| `index.ts` | hand-written | Typed adapter: `compilePreview`, `dryRun`, `ensureWasm`, the `*Dto` interfaces, and `KeWasmComputeError`. Import THIS, never the raw bindings. |
| `parity.ts` | hand-written | G5-2 browser leg: `deepEqual` + `compilePreviewWithParity` / `dryRunWithParity`. Cross-checks the local preview against the canonical endpoint when reachable; surfaces any mismatch. |
| `index.test.ts` | hand-written | Vitest smoke test (mocks the raw bindings — no compiled `.wasm` needed). |
| `ke_wasm.d.ts` | pinned contract | Type surface for `ke_wasm.js`. The shape is fixed here; the JS/wasm are generated. Tracked. |
| `ke_wasm.js` | **committed stub → generated** | A committed scaffold STUB (every export throws if called) so the adapter + smoke test resolve their import before the real `.wasm` exists. The Build phase OVERWRITES it with wasm-bindgen `--target web` glue. Tracked; do NOT hand-edit beyond the stub. |
| `ke_wasm_bg.wasm` | **generated** | The compiled wasm module. Git-ignored; CI regenerates. Do NOT hand-edit. |

## Generating the bindings (Build phase)

The crate pins `wasm-bindgen = "=0.2.95"`. The `wasm-bindgen-cli` version **MUST
match exactly** (classic footgun):

```bash
# once, to install the matching CLI:
cargo install wasm-bindgen-cli --version 0.2.95

# build the wasm module, then generate the web-target bindings into this dir:
cargo build -p ke-wasm --target wasm32-unknown-unknown
wasm-bindgen --target web \
  --out-dir frontend/src/wasm \
  target/wasm32-unknown-unknown/debug/ke_wasm.wasm
```

This emits `ke_wasm.js`, `ke_wasm_bg.wasm`, and a `ke_wasm.d.ts`. The committed
`ke_wasm.d.ts` here is the pinned contract; regeneration should produce a
matching shape (verify before overwriting). The CI job
`.github/workflows/wasm-build.yml` runs the same build + `wasm-bindgen` step.

## Generated-artifact policy (flag to Hossain)

- `ke_wasm.js` is committed as a **scaffold stub** (throws if invoked) so the
  adapter and unit tests resolve their import with no build; the Build phase /
  CI overwrites it with the real wasm-bindgen glue. `ke_wasm.d.ts` is the
  committed pinned type contract.
- `ke_wasm_bg.wasm` (and its bindgen `.wasm.d.ts`) are **git-ignored** (binary /
  large); CI regenerates them from the pinned `wasm-bindgen-cli`.
- If you prefer to commit the `.wasm` too (zero-build frontend dev), remove the
  `.gitignore` entries and commit it — but then the `wasm-bindgen-cli` pin
  becomes a review checkpoint on every regeneration.

## Public function signatures (what the Build phase + tests consume)

Rust (`crates/ke-wasm/src/lib.rs`):

- `compile_preview(source: &str) -> Result<String, JsError>` — `#[wasm_bindgen]`
- `dry_run(source: &str, facts_json: &str) -> Result<String, JsError>` — `#[wasm_bindgen]`
- `compile_preview_impl(source: &str) -> Result<String, String>` — pure, native-callable (parity test)
- `dry_run_impl(source: &str, facts_json: &str) -> Result<String, String>` — pure, native-callable
- `verify_artifact(...)`, `read_provenance(...)` — UNCHANGED (ADR-0016)

TypeScript (`index.ts`):

- `compilePreview(source: string): Promise<CompilePreviewResult>`
- `dryRun(source: string, facts: unknown): Promise<DryRunResult>`
- `ensureWasm(): Promise<void>`
