/**
 * Shared transport for the local `ke-cli serve` surface (ADR-0018).
 *
 * NON-AUTHORITATIVE (spec § 6 / § 16; CLAUDE.md authority boundary): this surface
 * is consumed for preview/verify ONLY. The frontend never signs, attests,
 * publishes, or assembles artifacts through it. The canonical compute path is the
 * authoritative registry-backed `ke-cli`; any divergence between a serve/WASM
 * preview and the canonical compute must be SURFACED, never silently treated as
 * authoritative.
 *
 * This module is the shared base every per-page local-variant (`*.serve.ts`)
 * builds on. It is used ONLY when a Gate-5 flag routes a page to the local
 * surface; with all flags off it is never reached and `main` behaves as today.
 */
import axios, { AxiosInstance } from 'axios'
import { KE_SERVE_URL } from '@/config/flags'

/**
 * Axios instance pointed at `ke-cli serve`. Mirrors `apiClient`'s contract:
 * 30s timeout, JSON content-type, and the same error-extraction shape
 * (`response.data.detail || message`). Non-authoritative preview surface.
 */
export const serveClient: AxiosInstance = axios.create({
  baseURL: KE_SERVE_URL,
  headers: {
    'Content-Type': 'application/json',
  },
  timeout: 30000,
})

/**
 * Thrown by a local-variant function for a page that has NO local serve/WASM
 * equivalent yet (SCAFFOLD-ONLY in the Gate-5 mapping table). The flag-select
 * helper (`selectQueryFn`) catches this and transparently falls back to the
 * `VITE_API_URL` path, so a flag-on scaffold page behaves exactly as today.
 *
 * It carries the page name and the reason so the fallback is auditable and the
 * not-yet-rewired boundary stays visible (never a silent empty result).
 */
export class ServeUnsupportedError extends Error {
  /** The page whose local variant is not yet wired to a real surface. */
  readonly page: string
  /** Why no local surface exists yet (e.g. "serve exposes no similarity search"). */
  readonly reason: string
  constructor(page: string, reason: string) {
    super(`serve has no local surface for ${page}: ${reason}`)
    this.name = 'ServeUnsupportedError'
    this.page = page
    this.reason = reason
  }
}

// ---------------------------------------------------------------------------
// serve DTOs - mirror crates/ke-cli/src/serve/dto.rs EXACTLY.
// (The /compile/preview and /dry-run preview DTOs are NOT re-declared here; they
//  are imported from `@/wasm` by the variants that need them, per the contract.)
// ---------------------------------------------------------------------------

export interface ServeHealthResponse {
  ok: boolean
  surface: string
}

export type AttestationTypeName =
  | 'SourceFidelity'
  | 'Interpretation'
  | 'ScenarioCoverage'
  | 'EquivalenceClaim'
  | 'PublicationApproval'

export type RegistryStatus = 'Published' | 'Deprecated' | 'Revoked' | 'Unknown'

/** Rendered timestamp-authority class string. */
export type TimestampAuthorityClass = string

export interface AttestationSummary {
  attestation_type: AttestationTypeName
  signer_key_id: string
  is_test_key: boolean
  tsa_class: TimestampAuthorityClass
  claimed_time_unix: number
}

export interface ArtifactProvenance {
  regime_id: string
  artifact_hash: number[]
  ir_schema_version: string
  codec_version: string
  canonicalization_version: string
  signer_key_id: string
  is_test_key: boolean
  attestations: AttestationSummary[]
  registry_state: RegistryStatus
  registry_event_head_hash: number[]
  exported_at_unix: number
}

export interface VerifyRequest {
  hash: string
  env?: string
  policy?: 'strict' | 'permissive'
}

export interface VerifyResponse {
  verdict: 'verified' | 'rejected'
  rejection?: string
  provenance: ArtifactProvenance
  registry_state: RegistryStatus
}

/** Structurally open until a stored-rule view is needed. */
export type ResolutionRecord = Record<string, unknown>

/**
 * Call the local serve `POST /verify` surface (ADR-0018). NON-AUTHORITATIVE:
 * verifies an artifact by hash and returns the canonical `ArtifactProvenance`
 * (`VerifyResponse.provenance`) the 5e ReviewSurface renders. The frontend never
 * signs, attests, or publishes through this — verify/read only (spec § 6/§ 16).
 */
export async function serveVerify(req: VerifyRequest): Promise<VerifyResponse> {
  const { data } = await serveClient.post<VerifyResponse>('/verify', req)
  return data
}
