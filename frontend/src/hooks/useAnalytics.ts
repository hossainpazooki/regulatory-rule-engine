import { useQuery, useMutation } from '@tanstack/react-query'
import { analyticsApi } from '@/api'
import type {
  CompareRulesRequest,
  ClusterRequest,
  ConflictSearchRequest,
  SimilarRulesRequest,
  UMAPProjectionRequest,
  EmbeddingType,
} from '@/types'
import { USE_LOCAL_KE_API } from '@/config/flags'
import { ServeUnsupportedError } from '@/api/serve/serveClient'
import {
  clusterLocal,
  getCoverageLocal,
  findConflictsLocal,
  getUMAPProjectionLocal,
  findSimilarLocal,
} from '@/api/serve/analytics.serve'
import { getGraphLocal, getNetworkGraphLocal } from '@/api/serve/graph.serve'

/**
 * Gate-5 flag-select (spec § 7.4). When `flagOn`, run the local-surface variant;
 * if it throws `ServeUnsupportedError` (a SCAFFOLD-ONLY page, which every
 * AnalyticsDashboard call is — see the contract mapping table), transparently
 * re-run the canonical `VITE_API_URL` fallback so behavior is identical to `main`.
 * When `flagOn` is false the fallback is returned unchanged — the local module is
 * never invoked. This mirrors Workstream D's `selectQueryFn`; it lives here only
 * because AnalyticsDashboard owns its own hook wiring (it is not one of the two
 * REWIRED pages D edits). NON-AUTHORITATIVE: the local surface never signs/
 * publishes; for these scaffold calls it never even returns data.
 */
function selectAnalyticsQueryFn<T>(
  flagOn: boolean,
  localFn: () => Promise<T>,
  fallbackFn: () => Promise<T>
): () => Promise<T> {
  if (!flagOn) return fallbackFn
  return async () => {
    try {
      return await localFn()
    } catch (err) {
      if (err instanceof ServeUnsupportedError) return fallbackFn()
      throw err
    }
  }
}

export const analyticsKeys = {
  all: ['analytics'] as const,
  clusters: () => [...analyticsKeys.all, 'clusters'] as const,
  cluster: (params: ClusterRequest) => [...analyticsKeys.clusters(), params] as const,
  conflicts: () => [...analyticsKeys.all, 'conflicts'] as const,
  conflict: (params: ConflictSearchRequest) => [...analyticsKeys.conflicts(), params] as const,
  coverage: () => [...analyticsKeys.all, 'coverage'] as const,
  umap: () => [...analyticsKeys.all, 'umap'] as const,
  umapProjection: (params: UMAPProjectionRequest) => [...analyticsKeys.umap(), params] as const,
  graph: () => [...analyticsKeys.all, 'graph'] as const,
  ruleGraph: (ruleId: string) => [...analyticsKeys.graph(), ruleId] as const,
  network: (minSimilarity: number) => [...analyticsKeys.graph(), 'network', minSimilarity] as const,
  similar: (ruleId: string, type: EmbeddingType) =>
    [...analyticsKeys.all, 'similar', ruleId, type] as const,
}

export function useClusters(request: ClusterRequest = {}) {
  return useQuery({
    queryKey: analyticsKeys.cluster(request),
    queryFn: selectAnalyticsQueryFn(
      USE_LOCAL_KE_API,
      () => clusterLocal(request),
      () => analyticsApi.cluster(request)
    ),
  })
}

export function useConflicts(request: ConflictSearchRequest = {}) {
  return useQuery({
    queryKey: analyticsKeys.conflict(request),
    queryFn: selectAnalyticsQueryFn(
      USE_LOCAL_KE_API,
      () => findConflictsLocal(request),
      () => analyticsApi.findConflicts(request)
    ),
  })
}

export function useCoverage() {
  return useQuery({
    queryKey: analyticsKeys.coverage(),
    queryFn: selectAnalyticsQueryFn(
      USE_LOCAL_KE_API,
      () => getCoverageLocal(),
      () => analyticsApi.getCoverage()
    ),
  })
}

export function useUMAPProjection(request: UMAPProjectionRequest = {}) {
  return useQuery({
    queryKey: analyticsKeys.umapProjection(request),
    // Gate-5 (spec § 7.4): when USE_LOCAL_KE_API is on, try the local variant;
    // EmbeddingExplorer is SCAFFOLD-ONLY (serve has no UMAP-projection surface and
    // no WASM equivalent), so getUMAPProjectionLocal throws ServeUnsupportedError
    // and the selector falls back to the untouched VITE_API_URL path. With the flag
    // off this is the canonical fallback unchanged — identical to `main`.
    queryFn: selectAnalyticsQueryFn(
      USE_LOCAL_KE_API,
      () => getUMAPProjectionLocal(request),
      () => analyticsApi.getUMAPProjection(request)
    ),
  })
}

export function useRuleGraph(ruleId?: string) {
  return useQuery({
    queryKey: analyticsKeys.ruleGraph(ruleId || 'all'),
    // GraphVisualizer (single-rule view) — SCAFFOLD-ONLY. Flag-on tries the
    // local variant, which throws ServeUnsupportedError, so this falls back to
    // the untouched VITE_API_URL path; flag-off returns the fallback unchanged.
    queryFn: selectAnalyticsQueryFn(
      USE_LOCAL_KE_API,
      () => getGraphLocal(ruleId),
      () => analyticsApi.getGraph(ruleId)
    ),
    enabled: true,
  })
}

export function useNetworkGraph(minSimilarity = 0.7) {
  return useQuery({
    queryKey: analyticsKeys.network(minSimilarity),
    // GraphVisualizer (network view) — SCAFFOLD-ONLY. Same flag-and-fallback as
    // useRuleGraph; behavior is identical to `main` with the flag on or off.
    queryFn: selectAnalyticsQueryFn(
      USE_LOCAL_KE_API,
      () => getNetworkGraphLocal(minSimilarity),
      () => analyticsApi.getNetworkGraph(minSimilarity)
    ),
  })
}

export function useSimilarRules(request: SimilarRulesRequest) {
  return useQuery({
    queryKey: analyticsKeys.similar(request.rule_id, request.embedding_type || 'all'),
    // Gate-5 (spec § 7.4): when USE_LOCAL_KE_API is on, try the local variant;
    // SimilaritySearch is SCAFFOLD-ONLY (serve has no similarity surface), so
    // findSimilarLocal throws ServeUnsupportedError and the selector falls back to
    // the untouched VITE_API_URL path. With the flag off this is the fallback
    // unchanged — identical to `main`.
    queryFn: selectAnalyticsQueryFn(
      USE_LOCAL_KE_API,
      () => findSimilarLocal(request),
      () => analyticsApi.findSimilar(request)
    ),
    enabled: !!request.rule_id,
  })
}

export function useCompareRules() {
  return useMutation({
    mutationFn: (request: CompareRulesRequest) => analyticsApi.compare(request),
  })
}
