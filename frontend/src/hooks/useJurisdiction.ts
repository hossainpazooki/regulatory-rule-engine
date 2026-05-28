import { useQuery, useMutation } from '@tanstack/react-query'
import { jurisdictionApi, NavigateRequest } from '@/api/jurisdiction.api'

export const jurisdictionKeys = {
  all: ['jurisdiction'] as const,
  list: () => [...jurisdictionKeys.all, 'list'] as const,
  regimes: () => [...jurisdictionKeys.all, 'regimes'] as const,
  equivalences: () => [...jurisdictionKeys.all, 'equivalences'] as const,
  navigate: () => [...jurisdictionKeys.all, 'navigate'] as const,
}

export function useJurisdictions() {
  return useQuery({
    queryKey: jurisdictionKeys.list(),
    queryFn: () => jurisdictionApi.list(),
  })
}

export function useRegimes() {
  return useQuery({
    queryKey: jurisdictionKeys.regimes(),
    queryFn: () => jurisdictionApi.listRegimes(),
  })
}

export function useEquivalences() {
  return useQuery({
    queryKey: jurisdictionKeys.equivalences(),
    queryFn: () => jurisdictionApi.listEquivalences(),
  })
}

export function useNavigate() {
  return useMutation({
    mutationFn: (request: NavigateRequest) => jurisdictionApi.navigate(request),
  })
}
