/**
 * SCAFFOLD-ONLY contract test for the GraphVisualizer local-surface variants.
 *
 * GraphVisualizer is classified SCAFFOLD-ONLY in the Gate-5 mapping table:
 * `ke-cli serve` (ADR-0018) exposes no rule-graph / network-graph endpoint and
 * there is no WASM-preview equivalent (Node2Vec embeddings are ML-derived, off
 * the ATLAS artifact path). The honest, mandated behavior is that each `*Local`
 * variant THROWS `ServeUnsupportedError` (carrying the page name) so the hook
 * (`useRuleGraph` / `useNetworkGraph`) falls back to the untouched `VITE_API_URL`
 * path — it must NOT silently return an empty graph. These tests pin exactly that.
 */

import { describe, it, expect } from 'vitest'
import { ServeUnsupportedError } from './serveClient'
import { getGraphLocal, getNetworkGraphLocal } from './graph.serve'

describe('GraphVisualizer local-surface variants (SCAFFOLD-ONLY)', () => {
  it('getGraphLocal rejects with ServeUnsupportedError naming the page', async () => {
    await expect(getGraphLocal()).rejects.toBeInstanceOf(ServeUnsupportedError)
    await expect(getGraphLocal('rule-123')).rejects.toMatchObject({
      name: 'ServeUnsupportedError',
      page: 'GraphVisualizer (analyticsApi.getGraph)',
    })
  })

  it('getNetworkGraphLocal rejects with ServeUnsupportedError naming the page', async () => {
    await expect(getNetworkGraphLocal()).rejects.toBeInstanceOf(ServeUnsupportedError)
    await expect(getNetworkGraphLocal(0.85)).rejects.toMatchObject({
      name: 'ServeUnsupportedError',
      page: 'GraphVisualizer (analyticsApi.getNetworkGraph)',
    })
  })

  it('never resolves with a value (no silent empty/fabricated graph)', async () => {
    const settled = await Promise.allSettled([
      getGraphLocal(),
      getGraphLocal('r1'),
      getNetworkGraphLocal(),
      getNetworkGraphLocal(0.9),
    ])
    expect(settled.every((s) => s.status === 'rejected')).toBe(true)
  })

  it('carries a reason explaining the not-yet-rewired boundary', async () => {
    await expect(getGraphLocal()).rejects.toMatchObject({
      reason: expect.stringContaining('not yet rewired'),
    })
  })
})
