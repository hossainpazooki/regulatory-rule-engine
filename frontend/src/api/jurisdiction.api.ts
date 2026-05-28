import { apiClient } from './client'

// V4 Navigate Request - matches backend/core/api/routes_navigate.py
export interface NavigateRequest {
  issuer_jurisdiction: string
  target_jurisdictions: string[]
  instrument_type: string
  activity: string
  investor_types?: string[]
  facts?: Record<string, unknown>
  token_standard?: string
  underlying_chain?: string
  is_defi_integrated?: boolean
  defi_protocol?: string
}

// Jurisdiction role in cross-border scenario
export interface JurisdictionRoleResponse {
  jurisdiction: string
  regime_id: string
  role: string
}

// Howey test analysis result
export interface HoweyAnalysis {
  is_security: boolean
  investment_of_money: boolean
  common_enterprise: boolean
  expectation_of_profit: boolean
  efforts_of_others: boolean
  analysis_notes: string[]
}

// GENIUS Act analysis result
export interface GeniusAnalysis {
  is_compliant_stablecoin: boolean
  reserve_requirements_met: boolean
  issuer_requirements: string[]
}

// Token compliance result
export interface TokenComplianceResult {
  standard: string
  classification: string
  requires_sec_registration: boolean
  genius_act_applicable: boolean
  sec_jurisdiction: boolean
  cftc_jurisdiction: boolean
  compliance_requirements: string[]
  regulatory_risks: string[]
  recommended_actions: string[]
  howey_analysis?: HoweyAnalysis
  genius_analysis?: GeniusAnalysis
}

// Protocol risk assessment
export interface ProtocolRiskResult {
  protocol_id: string
  risk_tier: string
  overall_score: number
  consensus_score: number
  decentralization_score: number
  settlement_score: number
  operational_score: number
  security_score: number
  risk_factors: string[]
  strengths: string[]
  regulatory_notes: string[]
}

// DeFi risk scoring
export interface DefiRiskResult {
  protocol_id: string
  category: string
  overall_grade: string
  overall_score: number
  smart_contract_grade: string
  smart_contract_score: number
  economic_grade: string
  economic_score: number
  oracle_grade: string
  oracle_score: number
  governance_grade: string
  governance_score: number
  regulatory_flags: string[]
  critical_risks: string[]
  high_risks: string[]
  strengths: string[]
}

// Audit trail entry
export interface AuditTrailEntry {
  timestamp: string
  action: string
  details: Record<string, unknown>
}

// V4 Navigate Response - matches backend/core/api/routes_navigate.py
export interface NavigateResponse {
  status: 'actionable' | 'blocked' | 'requires_review'
  applicable_jurisdictions: JurisdictionRoleResponse[]
  jurisdiction_results: Array<{
    jurisdiction: string
    regime_id: string
    role: string
    status: string
    rules_evaluated: number
    obligations: Array<{
      id: string
      description: string
      deadline?: string
    }>
    warnings: string[]
  }>
  conflicts: Array<{
    type: string
    jurisdictions: string[]
    description: string
    severity: string
    resolution_hint?: string
  }>
  pathway: Array<{
    step: number
    jurisdiction: string
    action: string
    dependencies: string[]
    timeline_days?: number
  }>
  cumulative_obligations: Array<{
    obligation_id: string
    jurisdiction: string
    description: string
    category: string
  }>
  estimated_timeline: string
  audit_trail: AuditTrailEntry[]
  // Market risk enhancements
  token_compliance?: TokenComplianceResult
  protocol_risk?: ProtocolRiskResult
  defi_risk?: DefiRiskResult
}

// Jurisdiction info from list endpoint
export interface JurisdictionInfo {
  code: string
  name: string
  authority: string
}

// Regime info
export interface RegimeInfo {
  id: string
  jurisdiction_code: string
  name: string
  effective_date: string
}

// Equivalence determination
export interface EquivalenceInfo {
  id: string
  from_jurisdiction: string
  to_jurisdiction: string
  scope: string
  status: string
  notes: string
}

export const jurisdictionApi = {
  // Navigate across jurisdictions (v4 spec)
  navigate: async (request: NavigateRequest): Promise<NavigateResponse> => {
    const { data } = await apiClient.post<NavigateResponse>('/navigate', request)
    return data
  },

  // List available jurisdictions
  list: async (): Promise<JurisdictionInfo[]> => {
    const { data } = await apiClient.get<{ jurisdictions: JurisdictionInfo[] }>('/navigate/jurisdictions')
    return data.jurisdictions
  },

  // List all regulatory regimes
  listRegimes: async (): Promise<RegimeInfo[]> => {
    const { data } = await apiClient.get<{ regimes: RegimeInfo[] }>('/navigate/regimes')
    return data.regimes
  },

  // List all equivalence determinations
  listEquivalences: async (): Promise<EquivalenceInfo[]> => {
    const { data } = await apiClient.get<{ equivalences: EquivalenceInfo[] }>('/navigate/equivalences')
    return data.equivalences
  },
}
