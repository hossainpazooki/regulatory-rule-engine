/**
 * G5-2 browser-side surfaced-difference leg (optional).
 *
 * When `VITE_USE_WASM_PREVIEW` is on AND a canonical `ke-cli serve` endpoint is
 * reachable (`VITE_USE_LOCAL_KE_API`), the browser computes the preview LOCALLY
 * (WASM) and ALSO requests the canonical `/compile/preview` or `/dry-run` for
 * the SAME input, then deep-equals the two JSON payloads.
 *
 * Parity holds by construction (the WASM fns reuse the SAME pure functions the
 * native handlers call), so any inequality is an unexpected drift: it is
 * SURFACED to the UI as a non-authoritative-mismatch banner and logged — NEVER
 * silently used or published. The canonical native result is the source of
 * truth; the WASM result is preview-only.
 *
 * This module is intentionally framework-agnostic: it returns a structured
 * `ParityOutcome` the calling component renders. It performs no network calls
 * itself beyond the caller-supplied `fetchCanonical`, and is a no-op when the
 * canonical endpoint is unavailable (preview proceeds, parity simply unchecked).
 */

import { compilePreview, dryRun } from './index'
import type { CompilePreviewResult, DryRunResult } from './index'

/** A stable, order-insensitive structural deep-equality over JSON values. */
export function deepEqual(a: unknown, b: unknown): boolean {
  if (a === b) return true
  if (a === null || b === null) return a === b
  if (typeof a !== typeof b) return false
  if (typeof a !== 'object') return a === b

  if (Array.isArray(a) || Array.isArray(b)) {
    if (!Array.isArray(a) || !Array.isArray(b)) return false
    if (a.length !== b.length) return false
    return a.every((x, i) => deepEqual(x, b[i]))
  }

  const ao = a as Record<string, unknown>
  const bo = b as Record<string, unknown>
  const ak = Object.keys(ao).sort()
  const bk = Object.keys(bo).sort()
  if (ak.length !== bk.length) return false
  if (!ak.every((k, i) => k === bk[i])) return false
  return ak.every((k) => deepEqual(ao[k], bo[k]))
}

export interface ParityOutcome<T> {
  /** The local (WASM, preview-only) result — always present on success. */
  local: T
  /** The canonical (`ke-cli serve`) result, if the endpoint was reachable. */
  canonical: T | null
  /** True when canonical was reached AND differed from local (SURFACE this). */
  mismatch: boolean
  /** Non-null when canonical was unreachable (parity simply unchecked). */
  unchecked: string | null
}

/**
 * Compute a compile preview locally and, when reachable, cross-check it against
 * the canonical endpoint. `fetchCanonical` is supplied by the caller (it owns
 * the `VITE_USE_LOCAL_KE_API` base URL + auth) and should resolve to the
 * canonical `CompilePreviewResponse` JSON, or reject/return null if unavailable.
 */
export async function compilePreviewWithParity(
  source: string,
  fetchCanonical: ((source: string) => Promise<CompilePreviewResult>) | null,
): Promise<ParityOutcome<CompilePreviewResult>> {
  const local = await compilePreview(source)
  return crossCheck(local, fetchCanonical ? () => fetchCanonical(source) : null)
}

/** Dry-run variant of {@link compilePreviewWithParity}. */
export async function dryRunWithParity(
  source: string,
  facts: unknown,
  fetchCanonical: ((source: string, facts: unknown) => Promise<DryRunResult>) | null,
): Promise<ParityOutcome<DryRunResult>> {
  const local = await dryRun(source, facts)
  return crossCheck(local, fetchCanonical ? () => fetchCanonical(source, facts) : null)
}

async function crossCheck<T>(
  local: T,
  fetchCanonical: (() => Promise<T>) | null,
): Promise<ParityOutcome<T>> {
  if (!fetchCanonical) {
    return { local, canonical: null, mismatch: false, unchecked: 'canonical endpoint disabled' }
  }
  let canonical: T
  try {
    canonical = await fetchCanonical()
  } catch (err) {
    const reason = err instanceof Error ? err.message : String(err)
    return { local, canonical: null, mismatch: false, unchecked: reason }
  }
  const mismatch = !deepEqual(local, canonical)
  if (mismatch) {
    // SURFACE, never silently use. The component renders a
    // non-authoritative-mismatch banner; the canonical result is source-of-truth.
    console.warn('[ke-wasm parity] non-authoritative preview differs from canonical compute', {
      local,
      canonical,
    })
  }
  return { local, canonical, mismatch, unchecked: null }
}
