/**
 * SCAFFOLD-ONLY flag-wiring test for the CrossBorderNavigator data layer.
 *
 * Asserts the Gate-5 contract for this page: with `USE_LOCAL_KE_API` OFF (the
 * default) the hooks call the canonical `jurisdictionApi.*` (`VITE_API_URL`)
 * path — byte-unchanged from `main`. Because the page is SCAFFOLD-ONLY (no local
 * serve/wasm equivalent), the flag-on path ALSO falls through to the same
 * canonical call: there is no fabricated local surface. We mock the flag module
 * per-case (rather than mutating `import.meta.env`) and confirm the exact
 * canonical function is invoked either way.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import React from 'react'

const listMock = vi.fn()
const navigateMock = vi.fn()

vi.mock('@/api/jurisdiction.api', () => ({
  jurisdictionApi: {
    list: (...args: unknown[]) => listMock(...args),
    navigate: (...args: unknown[]) => navigateMock(...args),
    listRegimes: vi.fn(),
    listEquivalences: vi.fn(),
  },
}))

// Default-OFF flag module. Overridden per-describe via vi.doMock + dynamic import.
vi.mock('@/config/flags', () => ({ USE_LOCAL_KE_API: false }))

function wrapper({ children }: { children: React.ReactNode }) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  })
  return (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  )
}

describe('useJurisdiction flag wiring (SCAFFOLD-ONLY)', () => {
  beforeEach(() => {
    listMock.mockReset().mockResolvedValue([
      { code: 'EU', name: 'European Union', authority: 'ESMA' },
    ])
    navigateMock.mockReset().mockResolvedValue({
      status: 'actionable',
      applicable_jurisdictions: [],
      jurisdiction_results: [],
      conflicts: [],
      pathway: [],
      cumulative_obligations: [],
      estimated_timeline: '—',
      audit_trail: [],
    })
  })

  afterEach(() => {
    vi.resetModules()
  })

  it('useJurisdictions hits the canonical VITE_API_URL path when the flag is OFF', async () => {
    const { useJurisdictions } = await import('./useJurisdiction')
    const { result } = renderHook(() => useJurisdictions(), { wrapper })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(listMock).toHaveBeenCalledTimes(1)
  })

  it('useJurisdictions still falls through to the canonical path when the flag is ON (no local surface)', async () => {
    vi.resetModules()
    vi.doMock('@/config/flags', () => ({ USE_LOCAL_KE_API: true }))
    const { useJurisdictions } = await import('./useJurisdiction')
    const { result } = renderHook(() => useJurisdictions(), { wrapper })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    // SCAFFOLD-ONLY: flag-on must NOT invent a local call; canonical is still used.
    expect(listMock).toHaveBeenCalledTimes(1)
  })

  it('useNavigate posts via the canonical path with the flag OFF', async () => {
    const { useNavigate } = await import('./useJurisdiction')
    const { result } = renderHook(() => useNavigate(), { wrapper })
    const req = {
      issuer_jurisdiction: 'CH',
      target_jurisdictions: ['EU'],
      instrument_type: 'e-money-token',
      activity: 'public_offer',
    }
    result.current.mutate(req)
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(navigateMock).toHaveBeenCalledTimes(1)
    expect(navigateMock).toHaveBeenCalledWith(req)
  })

  it('useNavigate falls through to the canonical path when the flag is ON (no local surface)', async () => {
    vi.resetModules()
    vi.doMock('@/config/flags', () => ({ USE_LOCAL_KE_API: true }))
    const { useNavigate } = await import('./useJurisdiction')
    const { result } = renderHook(() => useNavigate(), { wrapper })
    const req = {
      issuer_jurisdiction: 'CH',
      target_jurisdictions: ['EU'],
      instrument_type: 'e-money-token',
      activity: 'public_offer',
    }
    result.current.mutate(req)
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(navigateMock).toHaveBeenCalledTimes(1)
    expect(navigateMock).toHaveBeenCalledWith(req)
  })
})
