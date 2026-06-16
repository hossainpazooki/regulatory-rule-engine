import { useQuery } from '@tanstack/react-query'
import { productionApi } from '@/api'
import { USE_LOCAL_KE_API } from '@/config/flags'
import { healthLocal } from '@/api/serve/production.serve'
import { selectQueryFn } from './useLocalVariant'

export const productionKeys = {
  all: ['production'] as const,
  health: () => [...productionKeys.all, 'health'] as const,
  database: () => [...productionKeys.all, 'database'] as const,
  cache: () => [...productionKeys.all, 'cache'] as const,
  config: () => [...productionKeys.all, 'config'] as const,
}

/**
 * Service health for ProductionDemo.
 *
 * Gate-5 (spec § 7.4): when `USE_LOCAL_KE_API` is ON, the source is the local
 * `ke-cli serve` `GET /healthz` (`healthLocal`, REWIRED — a genuine local
 * equivalent, ADR-0018), mapped to the page's `HealthResponse`. With the flag
 * OFF (the default) `selectQueryFn` returns `productionApi.health` UNCHANGED, so
 * ProductionDemo fetches via `VITE_API_URL` byte-identically to `main`. The
 * query key, refetch interval, retry, and return shape are unchanged — the page
 * and its tests are untouched. The serve surface is NON-AUTHORITATIVE liveness
 * only (spec § 6/§ 16).
 */
export function useHealth() {
  return useQuery({
    queryKey: productionKeys.health(),
    queryFn: selectQueryFn(USE_LOCAL_KE_API, healthLocal, productionApi.health),
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
