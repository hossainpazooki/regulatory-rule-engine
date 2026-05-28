// Types for D3 visualizations

import type { EmbeddingType } from './analytics.types'

// UMAP point for scatter plot
export interface UMAPPoint {
  rule_id: string
  rule_name?: string
  x: number
  y: number
  z?: number
  jurisdiction?: string
  cluster_id?: number
  metadata: Record<string, unknown>
}

export interface UMAPProjectionRequest {
  embedding_type?: EmbeddingType
  n_components?: 2 | 3
  n_neighbors?: number
  min_dist?: number
  rule_ids?: string[]
}

export interface UMAPProjectionResponse {
  points: UMAPPoint[]
  embedding_type: EmbeddingType
  n_components: number
  total_rules: number
}

// Force graph types
export interface GraphNode {
  id: string
  label: string
  type: 'rule' | 'condition' | 'outcome' | 'obligation'
  jurisdiction?: string
  cluster?: number
  x?: number
  y?: number
  fx?: number | null
  fy?: number | null
}

export interface GraphLink {
  source: string | GraphNode
  target: string | GraphNode
  type: 'implies' | 'requires' | 'conflicts' | 'similar' | 'child'
  weight?: number
}

export interface GraphData {
  nodes: GraphNode[]
  links: GraphLink[]
}

// Tree visualization types
export interface D3TreeNode {
  id: string
  name: string
  type: 'condition' | 'outcome'
  children?: D3TreeNode[]
  consistency?: 'consistent' | 'inconsistent' | 'unknown'
  isTracePath?: boolean
  condition?: string
  result?: string
}

// Chart types for analytics
export interface BarChartData {
  label: string
  value: number
  color?: string
}

export interface TimeSeriesPoint {
  date: Date
  value: number
  label?: string
}

// Color scales
export type JurisdictionColorScale = Record<string, string>

export const JURISDICTION_COLORS: JurisdictionColorScale = {
  EU: '#3b82f6',
  UK: '#ef4444',
  US: '#22c55e',
  US_SEC: '#16a34a',
  US_CFTC: '#15803d',
  CH: '#f97316',
  SG: '#a855f7',
  HK: '#ec4899',
  JP: '#14b8a6',
  default: '#6b7280',
}

export const CLUSTER_COLORS = [
  '#3b82f6',
  '#ef4444',
  '#22c55e',
  '#f97316',
  '#a855f7',
  '#ec4899',
  '#14b8a6',
  '#eab308',
  '#6366f1',
  '#84cc16',
]
