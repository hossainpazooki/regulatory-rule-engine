import { apiClient } from './client'
import type {
  DecideRequest,
  DecideResponse,
  RulesListResponse,
  RuleDetail,
  RuleVersionList,
  RuleEventList,
} from '@/types'

// Rules endpoints
export const rulesApi = {
  // List all rules
  list: async (): Promise<RulesListResponse> => {
    const { data } = await apiClient.get<RulesListResponse>('/rules')
    return data
  },

  // Get rule detail
  get: async (ruleId: string): Promise<RuleDetail> => {
    const { data } = await apiClient.get<RuleDetail>(`/rules/${ruleId}`)
    return data
  },

  // Get rule versions
  getVersions: async (ruleId: string): Promise<RuleVersionList> => {
    const { data } = await apiClient.get<RuleVersionList>(`/rules/${ruleId}/versions`)
    return data
  },

  // Get rule events (audit trail)
  getEvents: async (ruleId: string): Promise<RuleEventList> => {
    const { data } = await apiClient.get<RuleEventList>(`/rules/${ruleId}/events`)
    return data
  },

  // Run decision
  decide: async (request: DecideRequest): Promise<DecideResponse> => {
    const { data } = await apiClient.post<DecideResponse>('/decide', request)
    return data
  },

  // Get decision tree visualization
  getTree: async (ruleId: string, format: string = 'json'): Promise<unknown> => {
    const { data } = await apiClient.get(`/ke/charts/decision-tree/${ruleId}`, {
      params: { format },
    })
    return data
  },
}
