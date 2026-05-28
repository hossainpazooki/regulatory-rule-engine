import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { rulesApi } from '@/api'
import type { DecideRequest } from '@/types'

export const ruleKeys = {
  all: ['rules'] as const,
  lists: () => [...ruleKeys.all, 'list'] as const,
  list: () => [...ruleKeys.lists()] as const,
  details: () => [...ruleKeys.all, 'detail'] as const,
  detail: (id: string) => [...ruleKeys.details(), id] as const,
  versions: (id: string) => [...ruleKeys.detail(id), 'versions'] as const,
  events: (id: string) => [...ruleKeys.detail(id), 'events'] as const,
  tree: (id: string) => [...ruleKeys.detail(id), 'tree'] as const,
}

export function useRules() {
  return useQuery({
    queryKey: ruleKeys.list(),
    queryFn: () => rulesApi.list(),
  })
}

export function useRule(ruleId: string) {
  return useQuery({
    queryKey: ruleKeys.detail(ruleId),
    queryFn: () => rulesApi.get(ruleId),
    enabled: !!ruleId,
  })
}

export function useRuleVersions(ruleId: string) {
  return useQuery({
    queryKey: ruleKeys.versions(ruleId),
    queryFn: () => rulesApi.getVersions(ruleId),
    enabled: !!ruleId,
  })
}

export function useRuleEvents(ruleId: string) {
  return useQuery({
    queryKey: ruleKeys.events(ruleId),
    queryFn: () => rulesApi.getEvents(ruleId),
    enabled: !!ruleId,
  })
}

export function useRuleTree(ruleId: string) {
  return useQuery({
    queryKey: ruleKeys.tree(ruleId),
    queryFn: () => rulesApi.getTree(ruleId, 'json'),
    enabled: !!ruleId,
  })
}

export function useDecision() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (request: DecideRequest) => rulesApi.decide(request),
    onSuccess: () => {
      // Optionally invalidate related queries
      queryClient.invalidateQueries({ queryKey: ruleKeys.all })
    },
  })
}
