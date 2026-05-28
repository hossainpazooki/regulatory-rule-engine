import { apiClient } from './client'

// Response types for production endpoints

export interface HealthResponse {
  status: 'healthy' | 'degraded' | 'unhealthy'
}

export interface DatabaseStats {
  rules_count: number
  compiled_rules_count: number
  verification_stats: Record<string, number>
  reviews_count: number
  premise_keys_count: number
}

export interface CacheStats {
  size: number
  hits: number
  misses: number
  hit_rate: number
}

export interface SystemConfig {
  features: {
    rate_limiting: boolean
    rate_limit: string
    audit_logging: boolean
    tracing: boolean
    auth_required: boolean
  }
  observability: {
    log_format: string
    log_level: string
    service_name: string
  }
}

// Production API endpoints
export const productionApi = {
  // Get service health status
  health: async (): Promise<HealthResponse> => {
    const { data } = await apiClient.get<HealthResponse>('/health')
    return data
  },

  // Get database statistics
  databaseStats: async (): Promise<DatabaseStats> => {
    const { data } = await apiClient.get<DatabaseStats>('/v2/status')
    return data
  },

  // Get IR cache statistics
  cacheStats: async (): Promise<CacheStats> => {
    const { data } = await apiClient.get<CacheStats>('/v2/cache/stats')
    return data
  },

  // Get system configuration (feature flags)
  systemConfig: async (): Promise<SystemConfig> => {
    const { data } = await apiClient.get<SystemConfig>('/v2/config')
    return data
  },
}