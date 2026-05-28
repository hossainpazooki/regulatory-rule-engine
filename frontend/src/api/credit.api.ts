import { apiClient } from './client'

// Types
export interface CreditApplication {
  app_id: string
  borrower_name: string
  deal_amount_usd: number
  document_ids: string[]
  industry: string
  borrower_type: 'corporate' | 'fund' | 'sov' | 'sme'
  status: string
  created_at: string
}

export interface DocumentUpload {
  filename: string
  content_type: string
  raw_text: string
  document_id: string
}

export interface ClassificationResult {
  document_id: string
  predicted_type: string
  confidence: number
  extracted_fields: Record<string, unknown>
}

export interface SynthesisOutput {
  recommendation: 'approve' | 'decline' | 'refer'
  confidence: number
  escalate: boolean
  escalation_reason: string | null
  citations: Array<{ source: string; text: string }>
  agent_outputs: Record<string, unknown>
}

export interface PipelineStatus {
  app_id: string
  phase: string
  phases_completed: string[]
  phases_remaining: string[]
}

export interface HITLDecision {
  reviewer_id: string
  decision: 'approve' | 'decline' | 'override'
  notes: string
  overrides?: Record<string, unknown>
}

// API calls
export const creditApi = {
  createApplication: (data: Partial<CreditApplication>) =>
    apiClient.post<CreditApplication>('/credit/applications', data),

  uploadDocument: (data: { filename: string; content_type: string; raw_text: string }) =>
    apiClient.post<ClassificationResult>('/credit/documents/upload', data),

  analyzeApplication: (appId: string) =>
    apiClient.post<SynthesisOutput>(`/credit/applications/${appId}/analyze`),

  getStatus: (appId: string) =>
    apiClient.get<PipelineStatus>(`/credit/applications/${appId}/status`),

  getResult: (appId: string) =>
    apiClient.get<SynthesisOutput>(`/credit/applications/${appId}/result`),

  submitReview: (appId: string, decision: HITLDecision) =>
    apiClient.post(`/credit/applications/${appId}/review`, decision),

  getQueue: () =>
    apiClient.get<Array<Record<string, unknown>>>('/credit/queue'),
}
