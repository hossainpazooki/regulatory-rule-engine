// SCAFFOLD-ONLY (not-yet-rewired): the credit document-ingestion pipeline
// (classification + field extraction) has NO equivalent on the `ke-cli serve`
// surface (ADR-0018 exposes only /healthz, /resolve, /verify, /compile/preview,
// /dry-run, /events) and NO WASM-preview equivalent. There is therefore no local
// compute path for `creditApi.uploadDocument`. Per the Gate-5 contract (§ B.2),
// this variant is flag-and-fallback wired but its body throws so the hook
// transparently falls back to the untouched VITE_API_URL path. It MUST NOT
// silently return empty / fabricated data.

import type { AxiosResponse } from 'axios'
import type { ClassificationResult } from '@/api/credit.api'

/**
 * Thrown by a SCAFFOLD-ONLY local variant that has no serve/WASM surface yet.
 * The flag-select helper (Workstream D `selectQueryFn`) catches this and retries
 * with the canonical `VITE_API_URL` fallback, so a flag-on scaffold page behaves
 * exactly as it does on `main`.
 *
 * NOTE: Workstream B's `serveClient.ts` is the eventual canonical home for this
 * shared error. It does not exist at the time this DocumentIngestion-only variant
 * is authored; this self-contained definition is intentionally minimal and
 * reconciles to the shared one at integration. The `name` is stable so callers
 * may branch on it without importing the class.
 */
export class ServeUnsupportedError extends Error {
  readonly page: string
  readonly reason: string
  constructor(page: string, reason: string) {
    super(`[serve unsupported: ${page}] ${reason}`)
    this.name = 'ServeUnsupportedError'
    this.page = page
    this.reason = reason
  }
}

/**
 * LOCAL variant of `creditApi.uploadDocument` — IDENTICAL return type
 * (`Promise<AxiosResponse<ClassificationResult>>`).
 *
 * SCAFFOLD-ONLY: there is no `ke-cli serve` or WASM-preview surface for document
 * classification, so this throws `ServeUnsupportedError`. The hook falls back to
 * the canonical `VITE_API_URL` path, preserving today's behavior with the flag on.
 */
export function uploadDocumentLocal(
  _data: { filename: string; content_type: string; raw_text: string }
): Promise<AxiosResponse<ClassificationResult>> {
  return Promise.reject(
    new ServeUnsupportedError(
      'DocumentIngestion',
      'ke-cli serve has no document-classification surface (ADR-0018); ' +
        'no local compute equivalent for creditApi.uploadDocument. ' +
        'Falls back to VITE_API_URL.'
    )
  )
}
