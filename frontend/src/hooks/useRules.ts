import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { rulesApi } from '@/api'
import type { DecideRequest, DecideResponse, RulesListResponse } from '@/types'
import { USE_LOCAL_KE_API, USE_WASM_PREVIEW } from '@/config/flags'
import {
  listRulesLocal,
  decideLocal,
  compilePreviewLocal,
  dryRunLocal,
} from '@/api/serve/rules.serve'
import {
  ServeUnsupportedError,
  serveVerify,
  type VerifyRequest,
} from '@/api/serve/serveClient'

/**
 * Gate-5 flag select for the rules-list source (spec § 7.4).
 *
 * DEFAULT-OFF: when `USE_LOCAL_KE_API` is off (the default), this returns the
 * canonical `rulesApi.list` queryFn UNCHANGED — Home fetches via `VITE_API_URL`
 * exactly as on `main`. When the flag is on, it tries the local serve variant
 * and, because Home is SCAFFOLD-ONLY (`ServeUnsupportedError`), transparently
 * falls back to the canonical path — so flag-on Home still behaves as today.
 * The local serve surface is NON-AUTHORITATIVE (spec § 6/§ 16).
 */
function selectRulesListFn(): () => Promise<RulesListResponse> {
  if (!USE_LOCAL_KE_API) return () => rulesApi.list()
  return async () => {
    try {
      return await listRulesLocal()
    } catch (err) {
      if (err instanceof ServeUnsupportedError) {
        // Not-yet-rewired: fall back to the untouched VITE_API_URL path.
        return rulesApi.list()
      }
      throw err
    }
  }
}

export const ruleKeys = {
  all: ['rules'] as const,
  lists: () => [...ruleKeys.all, 'list'] as const,
  list: () => [...ruleKeys.lists()] as const,
  details: () => [...ruleKeys.all, 'detail'] as const,
  detail: (id: string) => [...ruleKeys.details(), id] as const,
  versions: (id: string) => [...ruleKeys.detail(id), 'versions'] as const,
  events: (id: string) => [...ruleKeys.detail(id), 'events'] as const,
  tree: (id: string) => [...ruleKeys.detail(id), 'tree'] as const,
}

export function useRules() {
  return useQuery({
    queryKey: ruleKeys.list(),
    queryFn: selectRulesListFn(),
  })
}

export function useRule(ruleId: string) {
  return useQuery({
    queryKey: ruleKeys.detail(ruleId),
    queryFn: () => rulesApi.get(ruleId),
    enabled: !!ruleId,
  })
}

export function useRuleVersions(ruleId: string) {
  return useQuery({
    queryKey: ruleKeys.versions(ruleId),
    queryFn: () => rulesApi.getVersions(ruleId),
    enabled: !!ruleId,
  })
}

export function useRuleEvents(ruleId: string) {
  return useQuery({
    queryKey: ruleKeys.events(ruleId),
    queryFn: () => rulesApi.getEvents(ruleId),
    enabled: !!ruleId,
  })
}

export function useRuleTree(ruleId: string) {
  return useQuery({
    queryKey: ruleKeys.tree(ruleId),
    queryFn: () => rulesApi.getTree(ruleId, 'json'),
    enabled: !!ruleId,
  })
}

/**
 * Gate-5 flag select for the KEWorkbench `decide` source (spec § 7.4).
 *
 * DEFAULT-OFF: when neither `USE_WASM_PREVIEW` nor `USE_LOCAL_KE_API` is set (the
 * default), this is the canonical `rulesApi.decide` UNCHANGED — KEWorkbench's
 * "Run Trace" hits `VITE_API_URL` exactly as on `main`. With a flag on, it tries
 * the local variant and, because `decideLocal` is SCAFFOLD-ONLY (the decide DTO
 * carries facts but the local dry-run surface needs inline YAML source — the
 * stored-rule→source path is off-WASM/G5-1), transparently falls back to the
 * canonical `POST /decide`. So flag-on KEWorkbench still behaves as today; the
 * genuine local affordances are `compilePreviewLocal`/`dryRunLocal` (inline
 * source). The local surface is NON-AUTHORITATIVE (spec § 6/§ 16).
 */
function decideFn(request: DecideRequest): Promise<DecideResponse> {
  if (!USE_WASM_PREVIEW && !USE_LOCAL_KE_API) return rulesApi.decide(request)
  return decideLocal(request).catch((err) => {
    if (err instanceof ServeUnsupportedError) return rulesApi.decide(request)
    throw err
  })
}

export function useDecision() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (request: DecideRequest) => decideFn(request),
    onSuccess: () => {
      // Optionally invalidate related queries
      queryClient.invalidateQueries({ queryKey: ruleKeys.all })
    },
  })
}

/**
 * Gate-5 (5d) local compile-preview of INLINE YAML source. Mutation over
 * `compilePreviewLocal`, which routes to the WASM adapter (`USE_WASM_PREVIEW`) or
 * serve `POST /compile/preview`. NON-AUTHORITATIVE preview (spec § 6/§ 16); the
 * KEWorkbench pane that calls this is hidden unless a Gate-5 flag is on.
 */
export function useCompilePreview() {
  return useMutation({
    mutationFn: (source: string) => compilePreviewLocal(source),
  })
}

/**
 * Gate-5 (5d) local dry-run of INLINE YAML `source` against `facts`. Mutation
 * over `dryRunLocal` (WASM adapter or serve `POST /dry-run`). NON-AUTHORITATIVE.
 */
export function useDryRun() {
  return useMutation({
    mutationFn: ({ source, facts }: { source: string; facts: unknown }) =>
      dryRunLocal(source, facts),
  })
}

/**
 * Gate-5 (5d) artifact verify against the local serve `POST /verify` surface
 * (ADR-0018). Returns the canonical `VerifyResponse` whose `.provenance` feeds the
 * 5e ReviewSurface. NON-AUTHORITATIVE: verify/read only, never signs or publishes.
 */
export function useVerify() {
  return useMutation({
    mutationFn: (req: VerifyRequest) => serveVerify(req),
  })
}
