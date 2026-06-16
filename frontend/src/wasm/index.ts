/**
 * Typed, hand-written adapter over the raw wasm-bindgen bindings
 * (`./ke_wasm.js`). This is the surface the frontend imports — never the raw
 * `compile_preview`/`dry_run` strings directly.
 *
 * NON-AUTHORITATIVE (spec § 6 / § 16; CLAUDE.md authority boundary): every
 * function here computes a PREVIEW in the browser. WASM never signs, attests,
 * publishes, assembles, or mutates a registry. The canonical compute is the
 * `ke-cli serve` endpoint; the WASM result is preview-only and, when a canonical
 * endpoint is reachable, is checked against it (see `parity.ts`, G5-2) — any
 * mismatch is SURFACED, never silently used.
 *
 * The two new fns (`compilePreview`, `dryRun`) wrap the byte-identical twins of
 * the native `POST /compile/preview` and `POST /dry-run` (`source` path)
 * handlers. The native by-`hash` dry-run path is intentionally NOT bound in the
 * browser (it needs the canonical registry backend, off-WASM, G5-1); use the
 * canonical endpoint for stored artifacts.
 */

/**
 * The raw wasm-bindgen module surface. We load it via a DYNAMIC import (in
 * `ensureWasm`) rather than a static `import … from './ke_wasm.js'` so the
 * adapter type-checks and unit-tests cleanly BEFORE the Build phase emits
 * `ke_wasm.js` — a static import would fail vite's transform-time module
 * resolution when the generated file is absent. The dynamic specifier is built
 * from a variable so the bundler treats it as external (the real glue is loaded
 * at runtime); the smoke test mocks `./ke_wasm.js` via `vi.mock`.
 */
interface RawKeWasm {
  default: (
    module_or_path?: string | URL | Request | BufferSource | WebAssembly.Module,
  ) => Promise<unknown>
  compile_preview: (source: string) => string
  dry_run: (source: string, facts_json: string) => string
}

let raw: RawKeWasm | null = null

async function loadRaw(): Promise<RawKeWasm> {
  if (!raw) {
    // Dynamic import (not a top-level static import) so the adapter loads
    // before the Build phase emits `ke_wasm.js`. `@vite-ignore` stops vite from
    // resolving the (possibly-absent) target at transform time; vitest's
    // `vi.mock('./ke_wasm.js')` still intercepts this literal specifier.
    raw = (await import(/* @vite-ignore */ './ke_wasm.js')) as RawKeWasm
  }
  return raw
}

// ---------------------------------------------------------------------------
// DTOs — mirror the Rust serde response shapes (ke-wasm src/lib.rs, which in
// turn mirror ke-cli serve::dto). Keep these in lockstep with that Rust file.
// ---------------------------------------------------------------------------

export interface FindingDto {
  tier: 'T0' | 'T1'
  rule_id: string
  code: string
  message: string
  blocking: boolean
}

export interface ConflictDto {
  class: string
  severity: string
  message: string
}

export interface VerificationReportDto {
  has_blocking: boolean
  findings: FindingDto[]
  conflicts: ConflictDto[]
}

/**
 * The compiled rule IR as emitted by `ke_core::ir::RuleIR` (serde). This is the
 * COMPILED intermediate representation — distinct from the API's `RuleDetail`
 * (which is the platform's stored-rule view). Kept structurally open here; the
 * preview UI reads only the fields it renders. (No existing frontend `RuleIR`
 * type to import — flagged in the README.)
 */
export type RuleIR = Record<string, unknown>

export interface CompilePreviewResult {
  rules: RuleIR[]
  report: VerificationReportDto
}

/** One step of an applicability / decision trace, as serialized by the runtime. */
export type NormStep = Record<string, unknown>

export interface EvaluationNormalized {
  applicable: boolean
  decision: string | null
  obligations: string[]
  applicability_steps: NormStep[]
  decision_path: NormStep[]
}

export interface DryRunResult {
  evaluations: EvaluationNormalized[]
}

/**
 * The structured error a thrown WASM `JsError` carries — the SAME
 * `{error, detail}` shape the native handler returns as a 422. The adapter maps
 * the thrown error back to this so callers see an identical error shape whether
 * they hit WASM preview or the canonical HTTP endpoint.
 */
export interface KeWasmError {
  error: 'compile_error' | 'facts_error'
  detail: string
}

/** A typed error thrown by `compilePreview` / `dryRun` on a compute failure. */
export class KeWasmComputeError extends Error {
  readonly kind: KeWasmError['error']
  readonly detail: string
  constructor(body: KeWasmError) {
    super(`${body.error}: ${body.detail}`)
    this.name = 'KeWasmComputeError'
    this.kind = body.error
    this.detail = body.detail
  }
}

// ---------------------------------------------------------------------------
// init guard
// ---------------------------------------------------------------------------

let initPromise: Promise<void> | null = null

/** Idempotent wasm-bindgen `init()`. Safe to call before every compute. */
export async function ensureWasm(): Promise<void> {
  if (!initPromise) {
    initPromise = loadRaw()
      .then((m) => m.default())
      .then(() => undefined)
  }
  return initPromise
}

/**
 * Map a thrown WASM error to `KeWasmComputeError`. The thrown `JsError`'s
 * message is the JSON `{error, detail}` body; if it does not parse (e.g. an
 * unexpected internal throw), wrap the raw message verbatim.
 */
function toComputeError(err: unknown): KeWasmComputeError {
  const message = err instanceof Error ? err.message : String(err)
  try {
    const body = JSON.parse(message) as Partial<KeWasmError>
    if (
      (body.error === 'compile_error' || body.error === 'facts_error') &&
      typeof body.detail === 'string'
    ) {
      return new KeWasmComputeError(body as KeWasmError)
    }
  } catch {
    // fall through to a generic compile_error wrapper
  }
  return new KeWasmComputeError({ error: 'compile_error', detail: message })
}

// ---------------------------------------------------------------------------
// Public compute surface
// ---------------------------------------------------------------------------

/**
 * Compile + verify YAML `source` for PREVIEW. Resolves to the parsed
 * `CompilePreviewResult`; rejects with a `KeWasmComputeError` carrying the
 * `{error:'compile_error', detail}` body on a compile failure.
 */
export async function compilePreview(source: string): Promise<CompilePreviewResult> {
  await ensureWasm()
  const m = await loadRaw()
  let json: string
  try {
    json = m.compile_preview(source)
  } catch (err) {
    throw toComputeError(err)
  }
  return JSON.parse(json) as CompilePreviewResult
}

/**
 * Evaluate inline YAML `source` against `facts` for PREVIEW. `facts` is
 * `JSON.stringify`'d at the JS edge to reach the SAME `facts_from_json` call the
 * native handler uses. Rejects with a `KeWasmComputeError`
 * (`compile_error` | `facts_error`) on failure.
 */
export async function dryRun(source: string, facts: unknown): Promise<DryRunResult> {
  await ensureWasm()
  const m = await loadRaw()
  const factsJson = JSON.stringify(facts)
  let json: string
  try {
    json = m.dry_run(source, factsJson)
  } catch (err) {
    throw toComputeError(err)
  }
  return JSON.parse(json) as DryRunResult
}
