import { describe, it, expect } from 'vitest'
import { findSimilarLocal } from './analytics.serve'
import { ServeUnsupportedError } from './serveClient'
import type { SimilarRulesRequest } from '@/types'

describe('findSimilarLocal (SimilaritySearch SCAFFOLD-ONLY variant)', () => {
  const req: SimilarRulesRequest = {
    rule_id: 'rule-1',
    embedding_type: 'all',
    top_k: 10,
    min_score: 0.3,
    include_explanation: true,
  }

  it('throws ServeUnsupportedError (no local similarity surface yet)', async () => {
    await expect(findSimilarLocal(req)).rejects.toBeInstanceOf(ServeUnsupportedError)
  })

  it('carries the page name and a reason, never returns an empty result', async () => {
    await findSimilarLocal(req).then(
      () => {
        throw new Error('findSimilarLocal must reject, not resolve with empty data')
      },
      (err: unknown) => {
        expect(err).toBeInstanceOf(ServeUnsupportedError)
        const e = err as ServeUnsupportedError
        expect(e.page).toContain('SimilaritySearch')
        expect(e.reason).toMatch(/similarity/i)
      },
    )
  })
})
