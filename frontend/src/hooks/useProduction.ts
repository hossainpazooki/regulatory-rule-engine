import { useQuery } from '@tanstack/react-query'
import { productionApi } from '@/api'

export const productionKeys = {
  all: ['production'] as const,
  health: () => [...productionKeys.all, 'health'] as const,
  database: () => [...productionKeys.all, 'database'] as const,
  cache: () => [...productionKeys.all, 'cache'] as const,
  config: () => [...productionKeys.all, 'config'] as const,
}

export function useHealth() {
  return useQuery({
    queryKey: productionKeys.health(),
    queryFn: productionApi.health,
    refetchInterval: 30000, // Auto-refresh every 30s
    retry: 1,
  })
}

export function useDatabaseStats() {
  return useQuery({
    queryKey: productionKeys.database(),
    queryFn: productionApi.databaseStats,
    refetchInterval: 30000,
    retry: 1,
  })
}

export function useCacheStats() {
  return useQuery({
    queryKey: productionKeys.cache(),
    queryFn: productionApi.cacheStats,
    refetchInterval: 10000, // More frequent for cache metrics
    retry: 1,
  })
}

export function useSystemConfig() {
  return useQuery({
    queryKey: productionKeys.config(),
    queryFn: productionApi.systemConfig,
    staleTime: 60000, // Config rarely changes
    retry: 1,
  })
}
