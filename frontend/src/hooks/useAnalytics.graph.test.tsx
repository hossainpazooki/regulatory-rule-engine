/**
 * Gate-5 flag-select behavior for the GraphVisualizer graph hooks (spec § 7.4).
 *
 * In the test environment `VITE_USE_LOCAL_KE_API` is unset, so `USE_LOCAL_KE_API`
 * is DEFAULT-OFF. This asserts the byte-unchanged-`main` contract: with the flag
 * off, `useRuleGraph` / `useNetworkGraph` fetch via the canonical `analyticsApi`
 * (`VITE_API_URL`) and the local serve variants are never touched.
 *
 * Scoped to the GraphVisualizer-owned hooks only; the other analytics hooks have
 * their own sibling-owned coverage.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import React from 'react'
import type { GraphData } from '@/types'

const getGraphMock = vi.fn<[string | undefined], Promise<GraphData>>()
const getNetworkGraphMock = vi.fn<[number | undefined], Promise<GraphData>>()
const getGraphLocalMock = vi.fn<[string | undefined], Promise<GraphData>>()
const getNetworkGraphLocalMock = vi.fn<[number | undefined], Promise<GraphData>>()

vi.mock('@/api', () => ({
  analyticsApi: {
    getGraph: (ruleId?: string) => getGraphMock(ruleId),
    getNetworkGraph: (minSimilarity?: number) => getNetworkGraphMock(minSimilarity),
  },
}))
vi.mock('@/api/serve/graph.serve', () => ({
  getGraphLocal: (ruleId?: string) => getGraphLocalMock(ruleId),
  getNetworkGraphLocal: (minSimilarity?: number) => getNetworkGraphLocalMock(minSimilarity),
}))

import { useRuleGraph, useNetworkGraph } from './useAnalytics'

const sampleGraph: GraphData = {
  nodes: [{ id: 'n1', label: 'Rule 1', type: 'rule' }],
  links: [],
}

function wrapper({ children }: { children: React.ReactNode }) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>
}

describe('GraphVisualizer graph hooks — flag DEFAULT-OFF', () => {
  beforeEach(() => {
    getGraphMock.mockReset()
    getNetworkGraphMock.mockReset()
    getGraphLocalMock.mockReset()
    getNetworkGraphLocalMock.mockReset()
    getGraphMock.mockResolvedValue(sampleGraph)
    getNetworkGraphMock.mockResolvedValue(sampleGraph)
  })

  it('useRuleGraph fetches via canonical analyticsApi.getGraph (VITE_API_URL path)', async () => {
    const { result } = renderHook(() => useRuleGraph('rule-1'), { wrapper })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toEqual(sampleGraph)
    expect(getGraphMock).toHaveBeenCalledTimes(1)
    expect(getGraphMock).toHaveBeenCalledWith('rule-1')
    expect(getGraphLocalMock).not.toHaveBeenCalled()
  })

  it('useNetworkGraph fetches via canonical analyticsApi.getNetworkGraph (VITE_API_URL path)', async () => {
    const { result } = renderHook(() => useNetworkGraph(0.75), { wrapper })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toEqual(sampleGraph)
    expect(getNetworkGraphMock).toHaveBeenCalledTimes(1)
    expect(getNetworkGraphMock).toHaveBeenCalledWith(0.75)
    expect(getNetworkGraphLocalMock).not.toHaveBeenCalled()
  })

  it('never touches the local serve variants when the flag is off', async () => {
    const r1 = renderHook(() => useRuleGraph(undefined), { wrapper })
    const r2 = renderHook(() => useNetworkGraph(), { wrapper })
    await waitFor(() => expect(r1.result.current.isSuccess).toBe(true))
    await waitFor(() => expect(r2.result.current.isSuccess).toBe(true))
    expect(getGraphLocalMock).not.toHaveBeenCalled()
    expect(getNetworkGraphLocalMock).not.toHaveBeenCalled()
  })
})
