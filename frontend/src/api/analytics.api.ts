import { apiClient } from './client'
import type {
  CompareRulesRequest,
  ComparisonResult,
  ClusterRequest,
  ClusterAnalysis,
  ConflictSearchRequest,
  ConflictReport,
  SimilarRulesRequest,
  SimilarRulesResponse,
  CoverageReport,
  UMAPProjectionRequest,
  UMAPProjectionResponse,
  GraphData,
} from '@/types'

export const analyticsApi = {
  // Compare two rules
  compare: async (request: CompareRulesRequest): Promise<ComparisonResult> => {
    const { data } = await apiClient.post<ComparisonResult>('/analytics/rules/compare', request)
    return data
  },

  // Cluster rules
  cluster: async (request: ClusterRequest = {}): Promise<ClusterAnalysis> => {
    const { data } = await apiClient.post<ClusterAnalysis>('/analytics/rule-clusters', request)
    return data
  },

  // Find conflicts
  findConflicts: async (request: ConflictSearchRequest = {}): Promise<ConflictReport> => {
    const { data } = await apiClient.post<ConflictReport>('/analytics/find-conflicts', request)
    return data
  },

  // Find similar rules
  findSimilar: async (request: SimilarRulesRequest): Promise<SimilarRulesResponse> => {
    const { data } = await apiClient.get<SimilarRulesResponse>(
      `/analytics/rules/${request.rule_id}/similar`,
      {
        params: {
          embedding_type: request.embedding_type,
          top_k: request.top_k,
          min_score: request.min_score,
          include_explanation: request.include_explanation,
        },
      }
    )
    return data
  },

  // Get coverage report
  getCoverage: async (): Promise<CoverageReport> => {
    const { data } = await apiClient.get<CoverageReport>('/analytics/coverage')
    return data
  },

  // Get UMAP projection
  getUMAPProjection: async (request: UMAPProjectionRequest = {}): Promise<UMAPProjectionResponse> => {
    const { data } = await apiClient.post<UMAPProjectionResponse>('/analytics/umap-projection', request)
    return data
  },

  // Get rule graph
  getGraph: async (ruleId?: string): Promise<GraphData> => {
    const endpoint = ruleId ? `/analytics/graph/${ruleId}` : '/analytics/graph'
    const { data } = await apiClient.get<GraphData>(endpoint)
    return data
  },

  // Get network graph (all rules)
  getNetworkGraph: async (minSimilarity = 0.7): Promise<GraphData> => {
    const { data } = await apiClient.get<GraphData>('/analytics/network', {
      params: { min_similarity: minSimilarity },
    })
    return data
  },
}
