// REWIRED (ProductionDemo): the `ke-cli serve` surface (ADR-0018) exposes
// `GET /healthz` -> `{ ok: boolean, surface: string }` (see
// crates/ke-cli/src/serve/dto.rs `HealthResponse`). That is a genuine local
// equivalent for `productionApi.health`, so this variant is a real rewire (not
// SCAFFOLD-ONLY) behind `USE_LOCAL_KE_API`.
//
// NON-AUTHORITATIVE preview surface (spec § 6 / § 16): this is liveness of the
// local preview server only; it never signs, attests, publishes, or assembles
// artifacts. With `USE_LOCAL_KE_API` OFF the hook uses the untouched
// `productionApi.health` (`VITE_API_URL`) fallback and ProductionDemo behaves
// exactly as on `main`.

import { serveClient } from './serveClient'
import type { HealthResponse } from '@/api/production.api'

/** `GET /healthz` body (dto.rs `HealthResponse`). */
interface ServeHealthResponse {
  ok: boolean
  surface: string
}

/**
 * LOCAL variant of `productionApi.health` — IDENTICAL return type
 * (`Promise<HealthResponse>`).
 *
 * Maps the serve `GET /healthz` `{ ok, surface }` onto the page's
 * `HealthResponse` status enum: `ok === true` -> `'healthy'`, else
 * `'unhealthy'`. The ProductionDemo page reads only `healthData?.status`, so the
 * mapped value renders identically to the fallback's `'healthy'`/`'unhealthy'`
 * states. (`'degraded'` has no serve signal and is therefore never synthesized.)
 */
export async function healthLocal(): Promise<HealthResponse> {
  const { data } = await serveClient.get<ServeHealthResponse>('/healthz')
  return { status: data.ok ? 'healthy' : 'unhealthy' }
}
