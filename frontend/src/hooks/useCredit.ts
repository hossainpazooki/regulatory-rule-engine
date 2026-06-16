import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { creditApi } from '@/api'
import { USE_LOCAL_KE_API } from '@/config/flags'
import { uploadDocumentLocal, ServeUnsupportedError } from '@/api/serve/credit.serve'

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

/**
 * Upload + classify a credit document.
 *
 * Gate-5 (spec § 7.4): when `USE_LOCAL_KE_API` is ON we first attempt the LOCAL
 * variant (`uploadDocumentLocal`). DocumentIngestion is SCAFFOLD-ONLY — the local
 * variant has no serve/WASM surface and throws `ServeUnsupportedError`, which we
 * catch and transparently fall back to the canonical `VITE_API_URL` path. With
 * the flag OFF (the default) the `mutationFn` is `creditApi.uploadDocument`
 * unchanged — behavior is byte-identical to `main`. The non-local fallback path
 * is NON-AUTHORITATIVE here only in that the local surface does not exist;
 * authoritative upload remains the external backend (spec § 6/§ 16).
 */
export function useUploadDocument() {
  const qc = useQueryClient()
  const mutationFn = USE_LOCAL_KE_API
    ? async (data: { filename: string; content_type: string; raw_text: string }) => {
        try {
          return await uploadDocumentLocal(data)
        } catch (err) {
          if (err instanceof ServeUnsupportedError) {
            // SCAFFOLD-ONLY: no local surface yet — fall back to VITE_API_URL,
            // identical to flag-off behavior.
            return creditApi.uploadDocument(data)
          }
          throw err
        }
      }
    : creditApi.uploadDocument
  return useMutation({
    mutationFn,
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
