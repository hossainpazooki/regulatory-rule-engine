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
    queryFn: () => analyticsApi.cluster(request),
  })
}

export function useConflicts(request: ConflictSearchRequest = {}) {
  return useQuery({
    queryKey: analyticsKeys.conflict(request),
    queryFn: () => analyticsApi.findConflicts(request),
  })
}

export function useCoverage() {
  return useQuery({
    queryKey: analyticsKeys.coverage(),
    queryFn: () => analyticsApi.getCoverage(),
  })
}

export function useUMAPProjection(request: UMAPProjectionRequest = {}) {
  return useQuery({
    queryKey: analyticsKeys.umapProjection(request),
    queryFn: () => analyticsApi.getUMAPProjection(request),
  })
}

export function useRuleGraph(ruleId?: string) {
  return useQuery({
    queryKey: analyticsKeys.ruleGraph(ruleId || 'all'),
    queryFn: () => analyticsApi.getGraph(ruleId),
    enabled: true,
  })
}

export function useNetworkGraph(minSimilarity = 0.7) {
  return useQuery({
    queryKey: analyticsKeys.network(minSimilarity),
    queryFn: () => analyticsApi.getNetworkGraph(minSimilarity),
  })
}

export function useSimilarRules(request: SimilarRulesRequest) {
  return useQuery({
    queryKey: analyticsKeys.similar(request.rule_id, request.embedding_type || 'all'),
    queryFn: () => analyticsApi.findSimilar(request),
    enabled: !!request.rule_id,
  })
}

export function useCompareRules() {
  return useMutation({
    mutationFn: (request: CompareRulesRequest) => analyticsApi.compare(request),
  })
}
