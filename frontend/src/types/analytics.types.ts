// Types derived from backend/analytics/schemas.py

export type EmbeddingType = 'semantic' | 'structural' | 'entity' | 'legal' | 'graph' | 'all'
export type ClusterAlgorithm = 'kmeans' | 'dbscan' | 'hierarchical'
export type ConflictType = 'semantic' | 'structural' | 'temporal' | 'jurisdiction'
export type ConflictSeverity = 'high' | 'medium' | 'low'
export type CoverageImportance = 'high' | 'medium' | 'low'

export interface CompareRulesRequest {
  rule1_id: string
  rule2_id: string
  include_graph?: boolean
  weights?: Record<string, number>
}

export interface ComparisonResult {
  rule1_id: string
  rule2_id: string
  rule1_name?: string
  rule2_name?: string
  overall_similarity: number
  similarity_by_type: Record<string, number>
  structural_comparison: Record<string, unknown>
  shared_entities: string[]
  shared_legal_sources: string[]
  conflict_indicators: string[]
}

export interface ClusterRequest {
  embedding_type?: EmbeddingType
  n_clusters?: number
  algorithm?: ClusterAlgorithm
  rule_ids?: string[]
}

export interface ClusterInfo {
  cluster_id: number
  size: number
  rule_ids: string[]
  centroid_rule_id?: string
  cohesion_score: number
  keywords: string[]
}

export interface ClusterAnalysis {
  num_clusters: number
  algorithm: ClusterAlgorithm
  embedding_type: EmbeddingType
  silhouette_score: number
  clusters: ClusterInfo[]
  total_rules: number
}

export interface ConflictSearchRequest {
  rule_ids?: string[]
  conflict_types?: ConflictType[]
  threshold?: number
}

export interface ConflictInfo {
  rule1_id: string
  rule2_id: string
  rule1_name?: string
  rule2_name?: string
  conflict_type: ConflictType
  severity: ConflictSeverity
  description: string
  similarity_score: number
  conflicting_aspects: string[]
  resolution_hints: string[]
}

export interface ConflictReport {
  total_rules_analyzed: number
  conflicts_found: number
  conflicts: ConflictInfo[]
  high_severity_count: number
  medium_severity_count: number
  low_severity_count: number
}

export interface SimilarityExplanation {
  primary_reason: string
  shared_entities: string[]
  shared_legal_sources: string[]
  structural_similarity?: string
  semantic_alignment?: string
}

export interface SimilarRule {
  rule_id: string
  rule_name?: string
  jurisdiction?: string
  overall_score: number
  scores_by_type: Record<string, number>
  explanation?: SimilarityExplanation
}

export interface SimilarRulesRequest {
  rule_id: string
  embedding_type?: EmbeddingType
  top_k?: number
  min_score?: number
  include_explanation?: boolean
}

export interface SimilarRulesResponse {
  query_rule_id: string
  query_rule_name?: string
  similar_rules: SimilarRule[]
  total_candidates: number
}

export interface FrameworkCoverage {
  framework: string
  total_articles: number
  covered_articles: number
  coverage_percentage: number
  rules_per_article: Record<string, number>
  rule_count: number
}

export interface CoverageGap {
  framework: string
  article: string
  importance: CoverageImportance
  recommendation: string
}

export interface CoverageReport {
  total_rules: number
  total_legal_sources: number
  coverage_by_framework: Record<string, FrameworkCoverage>
  uncovered_sources: string[]
  coverage_gaps: CoverageGap[]
  overall_coverage_percentage: number
}
