/**
 * Gate-5 feature flags (spec § 7.4). ALL DEFAULT-OFF.
 *
 * With every flag off, the frontend behaves byte-for-byte as it does on `main`:
 * every page fetches via `apiClient` against `VITE_API_URL`. These flags only
 * select an alternative *data source* (`ke-cli serve` / WASM preview) or reveal
 * an additive, guarded UI surface — they never change render output or page/route
 * contracts. The local serve surface and WASM adapter are NON-AUTHORITATIVE
 * (spec § 6/§ 16): preview/verify only, never signing/publishing.
 */

/**
 * Parse a Vite env var as a boolean flag. Default-OFF: only the exact string
 * "true" (case-insensitive, trimmed) is true; everything else (undefined, "",
 * "false", "0", "1", anything) is false. A real boolean is honored directly.
 */
export function flagOn(raw: string | boolean | undefined): boolean {
  if (typeof raw === 'boolean') return raw
  if (typeof raw !== 'string') return false
  return raw.trim().toLowerCase() === 'true'
}

/** Route a page's REST call to `ke-cli serve` instead of VITE_API_URL. */
export const USE_LOCAL_KE_API: boolean = flagOn(import.meta.env.VITE_USE_LOCAL_KE_API)
/** Use the frontend/src/wasm adapter for compile-preview / dry-run. */
export const USE_WASM_PREVIEW: boolean = flagOn(import.meta.env.VITE_USE_WASM_PREVIEW)
/** Show the 5e AI-provenance review UI. */
export const USE_REVIEW_UI: boolean = flagOn(import.meta.env.VITE_USE_REVIEW_UI)

/**
 * Base URL for the local ke-cli serve surface. Default 'http://localhost:8787'.
 * Reads import.meta.env.VITE_KE_SERVE_URL when set. Used ONLY when
 * USE_LOCAL_KE_API or a local variant is active.
 */
export const KE_SERVE_URL: string =
  import.meta.env.VITE_KE_SERVE_URL || 'http://localhost:8787'
