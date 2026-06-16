/**
 * Gate-5 flag-select behavior for the Home rules-list source (spec § 7.4).
 *
 * In the test environment `VITE_USE_LOCAL_KE_API` is unset, so `USE_LOCAL_KE_API`
 * is DEFAULT-OFF. This asserts the byte-unchanged-`main` contract: with the flag
 * off, `useRules` fetches via the canonical `rulesApi.list` (`VITE_API_URL`) and
 * the local serve variant is never touched.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import React from 'react'
import type { RulesListResponse } from '@/types'

const listMock = vi.fn<[], Promise<RulesListResponse>>()
const listRulesLocalMock = vi.fn<[], Promise<RulesListResponse>>()

vi.mock('@/api', () => ({
  rulesApi: { list: () => listMock() },
}))
vi.mock('@/api/serve/rules.serve', () => ({
  listRulesLocal: () => listRulesLocalMock(),
}))

import { useRules } from './useRules'

const sample: RulesListResponse = { rules: [], total: 7 }

function wrapper({ children }: { children: React.ReactNode }) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>
}

describe('useRules — flag DEFAULT-OFF', () => {
  beforeEach(() => {
    listMock.mockReset()
    listRulesLocalMock.mockReset()
    listMock.mockResolvedValue(sample)
  })

  it('fetches via the canonical rulesApi.list (VITE_API_URL path)', async () => {
    const { result } = renderHook(() => useRules(), { wrapper })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toEqual(sample)
    expect(listMock).toHaveBeenCalledTimes(1)
  })

  it('never touches the local serve variant when the flag is off', async () => {
    const { result } = renderHook(() => useRules(), { wrapper })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(listRulesLocalMock).not.toHaveBeenCalled()
  })
})
