/**
 * Gate-5 (5d) tests for the local-surface preview/verify mutation hooks:
 * `useCompilePreview` / `useDryRun` / `useVerify` reach the local serve/WASM
 * functions with the right arguments. The hooks are pure mutations (no flag
 * default to assert here — the flag gating lives in the consuming pane, covered
 * by LocalKePreviewPane.test.tsx).
 */
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import React from 'react'

const compilePreviewLocalMock = vi.fn()
const dryRunLocalMock = vi.fn()
const serveVerifyMock = vi.fn()

vi.mock('@/api', () => ({ rulesApi: { list: vi.fn(), decide: vi.fn() } }))
vi.mock('@/api/serve/rules.serve', () => ({
  listRulesLocal: vi.fn(),
  decideLocal: vi.fn(),
  compilePreviewLocal: (s: string) => compilePreviewLocalMock(s),
  dryRunLocal: (s: string, f: unknown) => dryRunLocalMock(s, f),
}))
vi.mock('@/api/serve/serveClient', () => ({
  ServeUnsupportedError: class ServeUnsupportedError extends Error {},
  serveVerify: (r: unknown) => serveVerifyMock(r),
}))

import { useCompilePreview, useDryRun, useVerify } from './useRules'

function wrapper({ children }: { children: React.ReactNode }) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } })
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>
}

beforeEach(() => {
  compilePreviewLocalMock.mockReset()
  dryRunLocalMock.mockReset()
  serveVerifyMock.mockReset()
})

describe('Gate-5 preview/verify hooks', () => {
  it('useCompilePreview calls compilePreviewLocal with the source', async () => {
    const report = { has_blocking: false, findings: [], conflicts: [] }
    compilePreviewLocalMock.mockResolvedValue({ rules: [], report })
    const { result } = renderHook(() => useCompilePreview(), { wrapper })
    const out = await result.current.mutateAsync('rule_id: x')
    expect(compilePreviewLocalMock).toHaveBeenCalledWith('rule_id: x')
    expect(out.report.has_blocking).toBe(false)
  })

  it('useDryRun calls dryRunLocal with source + facts', async () => {
    dryRunLocalMock.mockResolvedValue({ evaluations: [] })
    const { result } = renderHook(() => useDryRun(), { wrapper })
    await result.current.mutateAsync({ source: 'rule_id: x', facts: { a: 1 } })
    expect(dryRunLocalMock).toHaveBeenCalledWith('rule_id: x', { a: 1 })
  })

  it('useVerify calls serveVerify and returns the provenance', async () => {
    serveVerifyMock.mockResolvedValue({
      verdict: 'verified',
      provenance: { regime_id: 'mica_2023' },
      registry_state: 'Published',
    })
    const { result } = renderHook(() => useVerify(), { wrapper })
    const out = await result.current.mutateAsync({ hash: 'abc123' })
    expect(serveVerifyMock).toHaveBeenCalledWith({ hash: 'abc123' })
    expect(out.verdict).toBe('verified')
    expect(out.provenance.regime_id).toBe('mica_2023')
  })
})
