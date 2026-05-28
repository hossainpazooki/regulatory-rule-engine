import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { creditApi } from '@/api'

export const creditKeys = {
  all: ['credit'] as const,
  status: (appId: string) => [...creditKeys.all, 'status', appId] as const,
  result: (appId: string) => [...creditKeys.all, 'result', appId] as const,
  queue: () => [...creditKeys.all, 'queue'] as const,
}

export function useApplicationStatus(appId: string | null) {
  return useQuery({
    queryKey: creditKeys.status(appId ?? ''),
    queryFn: () => creditApi.getStatus(appId!).then((r) => r.data),
    enabled: !!appId,
    refetchInterval: 2000,
  })
}

export function useApplicationResult(appId: string | null) {
  return useQuery({
    queryKey: creditKeys.result(appId ?? ''),
    queryFn: () => creditApi.getResult(appId!).then((r) => r.data),
    enabled: !!appId,
  })
}

export function useReviewQueue() {
  return useQuery({
    queryKey: creditKeys.queue(),
    queryFn: () => creditApi.getQueue().then((r) => r.data),
  })
}

export function useUploadDocument() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: creditApi.uploadDocument,
    onSuccess: () => qc.invalidateQueries({ queryKey: creditKeys.all }),
  })
}

export function useCreateApplication() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: creditApi.createApplication,
    onSuccess: () => qc.invalidateQueries({ queryKey: creditKeys.all }),
  })
}

export function useAnalyzeApplication() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (appId: string) => creditApi.analyzeApplication(appId).then((r) => r.data),
    onSuccess: () => qc.invalidateQueries({ queryKey: creditKeys.all }),
  })
}
