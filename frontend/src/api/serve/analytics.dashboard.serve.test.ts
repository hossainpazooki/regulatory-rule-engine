/**
 * SCAFFOLD-ONLY contract test for the AnalyticsDashboard local-surface variants
 * (`analyticsApi.cluster` / `getCoverage` / `findConflicts`).
 *
 * AnalyticsDashboard is classified SCAFFOLD-ONLY in the Gate-5 mapping table:
 * `ke-cli serve` (ADR-0018) has no clustering / coverage / conflict-detection
 * surface and there is no WASM equivalent (these are ML/analytics-derived, off the
 * ATLAS artifact path). The honest, mandated behavior is that each `*Local`
 * variant THROWS `ServeUnsupportedError` (naming the page) so the hook falls back
 * to the untouched `VITE_API_URL` path — it must NOT silently return
 * empty/fabricated analytics. These tests pin exactly that.
 *
 * Lives in its own file (not the shared `analytics.serve.test.ts`) because the
 * `analyticsApi` variant module is shared across several pages' builders.
 */

import { describe, it, expect } from 'vitest'
import { ServeUnsupportedError } from './serveClient'
import { clusterLocal, getCoverageLocal, findConflictsLocal } from './analytics.serve'

describe('AnalyticsDashboard local-surface variants (SCAFFOLD-ONLY)', () => {
  it('clusterLocal rejects with ServeUnsupportedError naming the page', async () => {
    await expect(clusterLocal()).rejects.toBeInstanceOf(ServeUnsupportedError)
    await expect(clusterLocal({ n_clusters: 3 })).rejects.toMatchObject({
      name: 'ServeUnsupportedError',
      page: 'AnalyticsDashboard (analyticsApi.cluster)',
    })
  })

  it('getCoverageLocal rejects with ServeUnsupportedError naming the page', async () => {
    await expect(getCoverageLocal()).rejects.toBeInstanceOf(ServeUnsupportedError)
    await expect(getCoverageLocal()).rejects.toMatchObject({
      name: 'ServeUnsupportedError',
      page: 'AnalyticsDashboard (analyticsApi.getCoverage)',
    })
  })

  it('findConflictsLocal rejects with ServeUnsupportedError naming the page', async () => {
    await expect(findConflictsLocal()).rejects.toBeInstanceOf(ServeUnsupportedError)
    await expect(findConflictsLocal({ threshold: 0.8 })).rejects.toMatchObject({
      name: 'ServeUnsupportedError',
      page: 'AnalyticsDashboard (analyticsApi.findConflicts)',
    })
  })

  it('never resolves with a value (no silent empty/fabricated analytics)', async () => {
    const settled = await Promise.allSettled([
      clusterLocal(),
      getCoverageLocal(),
      findConflictsLocal(),
    ])
    expect(settled.every((s) => s.status === 'rejected')).toBe(true)
  })
})
