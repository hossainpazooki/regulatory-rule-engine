import { describe, it, expect } from 'vitest'
import { getUMAPProjectionLocal, clusterLocal } from './analytics.serve'
import { ServeUnsupportedError } from './serveClient'
import type { UMAPProjectionRequest, ClusterRequest } from '@/types'

// EmbeddingExplorer's two data calls are SCAFFOLD-ONLY (Gate-5 contract mapping
// table): ke-cli serve (ADR-0018) has no UMAP-projection or clustering endpoint
// and there is no WASM-preview equivalent. Both local variants MUST throw
// ServeUnsupportedError (carrying the page + reason) so the hook falls back to the
// untouched VITE_API_URL path — they MUST NOT silently return empty/fabricated
// analytics, which would hide the not-yet-rewired boundary (spec § 6/§ 16).

describe('getUMAPProjectionLocal (EmbeddingExplorer SCAFFOLD-ONLY variant)', () => {
  const req: UMAPProjectionRequest = { embedding_type: 'semantic', n_components: 2 }

  it('throws ServeUnsupportedError (no local UMAP-projection surface yet)', async () => {
    await expect(getUMAPProjectionLocal(req)).rejects.toBeInstanceOf(ServeUnsupportedError)
  })

  it('carries the EmbeddingExplorer page name and a projection reason, never resolves with empty data', async () => {
    await getUMAPProjectionLocal(req).then(
      () => {
        throw new Error('getUMAPProjectionLocal must reject, not resolve with empty data')
      },
      (err: unknown) => {
        expect(err).toBeInstanceOf(ServeUnsupportedError)
        const e = err as ServeUnsupportedError
        expect(e.page).toContain('EmbeddingExplorer')
        expect(e.reason).toMatch(/projection|UMAP/i)
      },
    )
  })
})

describe('clusterLocal (EmbeddingExplorer cluster-stats variant)', () => {
  const req: ClusterRequest = {}

  it('throws ServeUnsupportedError (no local clustering surface yet)', async () => {
    await expect(clusterLocal(req)).rejects.toBeInstanceOf(ServeUnsupportedError)
  })

  it('carries a reason and never resolves with empty data', async () => {
    await clusterLocal(req).then(
      () => {
        throw new Error('clusterLocal must reject, not resolve with empty data')
      },
      (err: unknown) => {
        expect(err).toBeInstanceOf(ServeUnsupportedError)
        const e = err as ServeUnsupportedError
        expect(e.reason).toMatch(/clustering|cluster/i)
      },
    )
  })
})
