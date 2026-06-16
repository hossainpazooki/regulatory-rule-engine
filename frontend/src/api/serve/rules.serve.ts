// Local-surface variants of `rulesApi` (`src/api/rules.api.ts`).
//
// Two pages consume `rulesApi` against the local surface:
//   - Home   → `listRulesLocal`  (SCAFFOLD-ONLY: serve has no GET /rules)
//   - KEWorkbench → `decideLocal` + `compilePreviewLocal` (REWIRED: WASM/serve)
//
// NON-AUTHORITATIVE preview surface (spec § 6 / § 16). Every variant returns the
// IDENTICAL TypeScript shape as the corresponding `rulesApi` method, computed via
// the local surface. Selection between local and the untouched `VITE_API_URL`
// fallback happens in the hook layer (`selectQueryFn`); with all flags off these
// are never reached and `main` behaves exactly as today.
import type {
  RulesListResponse,
  DecideRequest,
  DecideResponse,
} from '@/types'
import { ServeUnsupportedError, serveClient } from './serveClient'
import { USE_WASM_PREVIEW } from '@/config/flags'
import {
  compilePreview,
  dryRun,
  type CompilePreviewResult,
  type DryRunResult,
} from '@/wasm'

// ---------------------------------------------------------------------------
// Home — SCAFFOLD-ONLY
// ---------------------------------------------------------------------------

/**
 * LOCAL variant of `rulesApi.list` — identical return type (`RulesListResponse`).
 *
 * SCAFFOLD-ONLY: the `ke-cli serve` surface (ADR-0018) exposes only /healthz,
 * /resolve, /verify, /compile/preview, /dry-run, /events. There is NO list-rules
 * endpoint and no WASM equivalent, so this always throws `ServeUnsupportedError`.
 * The hook catches it and re-runs the canonical `rulesApi.list` fallback, so
 * enabling `VITE_USE_LOCAL_KE_API` does not change Home's behavior. Do NOT
 * silently return an empty list — that would hide the not-yet-rewired boundary.
 */
export async function listRulesLocal(): Promise<RulesListResponse> {
  throw new ServeUnsupportedError(
    'Home (rulesApi.list)',
    'ke-cli serve exposes no GET /rules (and no WASM list equivalent); rule listing is not yet rewired',
  )
}

// ---------------------------------------------------------------------------
// KEWorkbench — REWIRED (decide + compile-preview)
// ---------------------------------------------------------------------------

/**
 * LOCAL variant of `rulesApi.decide` — identical return type (`DecideResponse`).
 *
 * SCAFFOLD-ONLY (honest classification): the `decide` DTO carries scenario FACTS
 * plus an optional stored `rule_id`, but every local dry-run surface (WASM
 * `dryRun` and serve `POST /dry-run`'s `source` path) evaluates INLINE YAML
 * `source`. The by-`hash`/by-`rule_id` dry-run that would turn a stored rule into
 * evaluable bytes for the browser is intentionally the OFF-WASM, registry-backed
 * G5-1 path (see `src/wasm/index.ts`) — it is NOT reachable from the browser
 * today. Rather than fabricate source for a stored rule, this throws
 * `ServeUnsupportedError` so the hook falls back to the canonical `POST /decide`;
 * flag-on KEWorkbench's "Run Trace" therefore behaves exactly as today.
 *
 * The genuine local KEWorkbench affordances are the additive
 * `compilePreviewLocal` / `dryRunLocal` (inline-source preview) below.
 * `request` is accepted to preserve the `rulesApi.decide` signature exactly.
 */
export async function decideLocal(_request: DecideRequest): Promise<DecideResponse> {
  void _request
  throw new ServeUnsupportedError(
    'KEWorkbench (rulesApi.decide)',
    'local dry-run evaluates inline YAML source; resolving a stored rule_id → evaluable source for browser preview is the off-WASM G5-1 path (use canonical POST /decide). Use compilePreviewLocal / dryRunLocal for inline-source preview.',
  )
}

/**
 * LOCAL preview compute for the additive KEWorkbench compile-preview pane.
 * REWIRED (additive): there is no existing `rulesApi` method for this — it is a
 * new, flag-gated affordance. With `USE_WASM_PREVIEW` / `USE_LOCAL_KE_API` off
 * the pane is hidden and this is never called.
 *
 * Surface selection:
 *   - `USE_WASM_PREVIEW` on → WASM adapter `compilePreview(source)` in-browser.
 *   - otherwise (serve)     → serve `POST /compile/preview`.
 *
 * Returns the shared `CompilePreviewResult` (`@/wasm`). NON-AUTHORITATIVE: this
 * compiles + verifies for preview only and signs/stores NOTHING; any divergence
 * from the canonical compile must be surfaced by the caller, never silently used.
 */
export async function compilePreviewLocal(source: string): Promise<CompilePreviewResult> {
  if (USE_WASM_PREVIEW) {
    return compilePreview(source)
  }
  const { data } = await serveClient.post<CompilePreviewResult>('/compile/preview', {
    source,
  })
  return data
}

/**
 * LOCAL preview dry-run over INLINE YAML source (the affordance the compile pane
 * exposes once a user supplies source). Distinct from `decideLocal`, which takes
 * the scenario-facts DTO and has no stored-rule→source browser path. Returns the
 * shared `DryRunResult` (`@/wasm`).
 *
 * Surface selection mirrors `compilePreviewLocal`. NON-AUTHORITATIVE preview.
 */
export async function dryRunLocal(
  source: string,
  facts: unknown,
): Promise<DryRunResult> {
  if (USE_WASM_PREVIEW) {
    return dryRun(source, facts)
  }
  const { data } = await serveClient.post<DryRunResult>('/dry-run', { source, facts })
  return data
}
