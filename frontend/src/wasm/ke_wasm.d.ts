/**
 * Type sidecar for `ke_wasm.js` (TS resolves this `.d.ts` as that module's
 * types). This is the PINNED contract for the wasm-bindgen `--target web`
 * output; the Build phase overwrites `ke_wasm.js` with the real glue (and emits
 * a matching `.d.ts` — verify the shape still matches this before overwriting).
 * Until then, `ke_wasm.js` is the committed scaffold stub (throws if called).
 *
 * All four functions are NON-AUTHORITATIVE preview surfaces (spec § 6 / § 16):
 * compile/dry-run/verify in the browser never sign, attest, publish, or mutate
 * a registry.
 */

/** Compile + verify YAML for preview. Returns a JSON string mirroring the
 *  native `CompilePreviewResponse`. Throws on a compile error (the thrown value
 *  carries the `{"error":"compile_error","detail":...}` body). */
export function compile_preview(source: string): string

/** Evaluate inline YAML against facts for preview. Returns a JSON string
 *  mirroring the native `DryRunResponse`. Throws on compile/facts errors. */
export function dry_run(source: string, facts_json: string): string

/** UNCHANGED, already shipped (ADR-0016). Verify a `.kew` artifact. */
export function verify_artifact(
  kew: Uint8Array,
  keydir_json: string,
  context_json: string,
  policy_json: string,
  registry_json: string,
  exported_at_unix: bigint,
): string

/** UNCHANGED, already shipped (ADR-0016). Read provenance from a `.kew`. */
export function read_provenance(
  kew: Uint8Array,
  registry_json: string,
  exported_at_unix: bigint,
): string

/** wasm-bindgen `--target web` init. Idempotent via the adapter's guard. */
export default function init(
  module_or_path?: string | URL | Request | BufferSource | WebAssembly.Module,
): Promise<unknown>
