import { useQuery, useMutation } from '@tanstack/react-query'
import { jurisdictionApi, NavigateRequest } from '@/api/jurisdiction.api'
import { USE_LOCAL_KE_API } from '@/config/flags'

export const jurisdictionKeys = {
  all: ['jurisdiction'] as const,
  list: () => [...jurisdictionKeys.all, 'list'] as const,
  regimes: () => [...jurisdictionKeys.all, 'regimes'] as const,
  equivalences: () => [...jurisdictionKeys.all, 'equivalences'] as const,
  navigate: () => [...jurisdictionKeys.all, 'navigate'] as const,
}

/**
 * SCAFFOLD-ONLY (not-yet-rewired) flag wiring for the CrossBorderNavigator page.
 *
 * Per the Gate-5 contract page→surface mapping, CrossBorderNavigator is
 * SCAFFOLD-ONLY: the `ke-cli serve` surface (ADR-0018) exposes only
 * `/healthz`, `/resolve`, `/verify`, `/compile/preview`, `/dry-run`, `/events` —
 * there is NO local equivalent for the jurisdiction navigation calls
 * (`GET /navigate/jurisdictions`, `POST /navigate`). So there is nothing to
 * route to yet.
 *
 * The contract still requires the flag + fallback to be wired so the page is
 * ready to be rewired the moment a local surface exists, WITHOUT inventing a
 * fake local call. `selectLocalOrFallback` is that wiring: when
 * `USE_LOCAL_KE_API` is OFF (the default) it returns the canonical
 * `apiClient`-backed function byte-unchanged; when it is ON, since no local
 * jurisdiction variant exists, it transparently falls through to the same
 * canonical function. The page therefore behaves EXACTLY as today under both
 * flag states — honest scaffold, no fabricated local surface.
 *
 * When Workstream B lands a real `jurisdiction.serve.ts` variant (or the serve
 * surface grows a `/navigate` endpoint), swap the `local` argument here for that
 * variant; the canonical call stays as the fallback. Browser consumption of the
 * local surface is NON-AUTHORITATIVE preview/verify only (spec § 6/§ 16).
 */
function selectLocalOrFallback<T>(
  flagOn: boolean,
  local: (() => Promise<T>) | null,
  fallback: () => Promise<T>,
): () => Promise<T> {
  // No local jurisdiction surface exists yet (SCAFFOLD-ONLY). Even with the flag
  // on, fall through to the canonical VITE_API_URL path so behavior is identical
  // to today. `local` is intentionally `null` until a real variant is wired.
  if (!flagOn || local === null) return fallback
  return local
}

export function useJurisdictions() {
  return useQuery({
    queryKey: jurisdictionKeys.list(),
    // SCAFFOLD-ONLY: `local` is null — flag-on still uses the canonical fallback.
    queryFn: selectLocalOrFallback(
      USE_LOCAL_KE_API,
      null,
      () => jurisdictionApi.list(),
    ),
  })
}

export function useRegimes() {
  return useQuery({
    queryKey: jurisdictionKeys.regimes(),
    queryFn: () => jurisdictionApi.listRegimes(),
  })
}

export function useEquivalences() {
  return useQuery({
    queryKey: jurisdictionKeys.equivalences(),
    queryFn: () => jurisdictionApi.listEquivalences(),
  })
}

export function useNavigate() {
  return useMutation({
    // SCAFFOLD-ONLY: no local `POST /navigate` equivalent on the serve surface;
    // the canonical apiClient path is the untouched fallback under any flag state.
    mutationFn: (request: NavigateRequest) =>
      selectLocalOrFallback(
        USE_LOCAL_KE_API,
        null,
        () => jurisdictionApi.navigate(request),
      )(),
  })
}
