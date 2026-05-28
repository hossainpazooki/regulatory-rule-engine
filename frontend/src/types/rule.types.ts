// Types derived from backend/rules/schemas.py

export interface DecideRequest {
  instrument_type?: string
  activity?: string
  jurisdiction?: string
  authorized?: boolean
  actor_type?: string
  issuer_type?: string
  is_credit_institution?: boolean
  is_authorized_institution?: boolean
  reference_asset?: string
  is_significant?: boolean
  reserve_value_eur?: number
  extra?: Record<string, unknown>
  rule_id?: string
}

export interface TraceStep {
  node: string
  condition: string
  result: boolean
  value_checked?: unknown
}

export interface Obligation {
  id: string
  description?: string
  source?: string
  deadline?: string
}

export interface DecisionResponse {
  rule_id: string
  applicable: boolean
  decision?: string
  trace: TraceStep[]
  obligations: Obligation[]
  source?: string
  notes?: string
}

export interface DecideResponse {
  results: DecisionResponse[]
  summary?: string
}

export interface RuleInfo {
  rule_id: string
  version: string
  description?: string
  effective_from?: string
  effective_to?: string
  tags: string[]
  source?: string
}

export interface RulesListResponse {
  rules: RuleInfo[]
  total: number
}

export interface RuleDetail {
  rule_id: string
  version: string
  description?: string
  effective_from?: string
  effective_to?: string
  tags: string[]
  source?: Record<string, unknown>
  applies_if?: Record<string, unknown>
  decision_tree?: Record<string, unknown>
  interpretation_notes?: string
}

export interface RuleVersion {
  id: string
  rule_id: string
  version: number
  content_hash: string
  effective_from?: string
  effective_to?: string
  created_at: string
  created_by?: string
  superseded_by?: number
  superseded_at?: string
  jurisdiction_code?: string
  regime_id?: string
}

export interface RuleVersionList {
  rule_id: string
  versions: RuleVersion[]
  total: number
}

export interface RuleEvent {
  id: string
  sequence_number?: number
  rule_id: string
  version: number
  event_type: string
  event_data: Record<string, unknown>
  timestamp: string
  actor?: string
  reason?: string
}

export interface RuleEventList {
  rule_id: string
  events: RuleEvent[]
  total: number
}

// Decision tree node types for visualization
export interface TreeNode {
  id: string
  label: string
  type: 'condition' | 'outcome'
  children?: TreeNode[]
  condition?: string
  result?: string
  consistency?: 'consistent' | 'inconsistent' | 'unknown'
  isTracePath?: boolean
}
