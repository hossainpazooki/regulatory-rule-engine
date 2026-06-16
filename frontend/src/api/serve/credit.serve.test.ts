/**
 * Tests for the SCAFFOLD-ONLY DocumentIngestion local variant. The contract
 * mandates it throws `ServeUnsupportedError` (never silently returns empty), so
 * the hook can transparently fall back to the untouched VITE_API_URL path.
 */
import { describe, it, expect } from 'vitest'
import { uploadDocumentLocal, ServeUnsupportedError } from './credit.serve'

describe('uploadDocumentLocal (SCAFFOLD-ONLY)', () => {
  it('rejects with ServeUnsupportedError carrying the page name', async () => {
    await expect(
      uploadDocumentLocal({ filename: 'x.pdf', content_type: 'application/pdf', raw_text: 'hi' })
    ).rejects.toBeInstanceOf(ServeUnsupportedError)

    const err = await uploadDocumentLocal({
      filename: 'x.pdf',
      content_type: 'application/pdf',
      raw_text: 'hi',
    }).catch((e) => e)
    expect(err).toBeInstanceOf(ServeUnsupportedError)
    expect(err.name).toBe('ServeUnsupportedError')
    expect(err.page).toBe('DocumentIngestion')
    expect(err.reason).toMatch(/no.*surface|fall.?back/i)
  })

  it('never resolves to a fabricated result', async () => {
    const resolved = await uploadDocumentLocal({
      filename: 'x.pdf',
      content_type: 'application/pdf',
      raw_text: 'hi',
    }).then(
      () => 'RESOLVED',
      () => 'REJECTED'
    )
    expect(resolved).toBe('REJECTED')
  })
})
