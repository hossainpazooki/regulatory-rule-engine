/**
 * Local-variant tests for `rulesApi` (Home + KEWorkbench).
 *
 * - Home (`listRulesLocal`) is SCAFFOLD-ONLY: `ke-cli serve` has no list-rules
 *   surface, so it must throw `ServeUnsupportedError` (never a placeholder) so the
 *   hook falls back to the untouched `VITE_API_URL` path.
 * - KEWorkbench (`decideLocal`) is also SCAFFOLD-ONLY by classification: the
 *   decide DTO carries facts but the local dry-run surface evaluates inline YAML
 *   source, so it throws and the hook falls back to canonical `POST /decide`.
 * - KEWorkbench's genuine local affordances are `compilePreviewLocal` /
 *   `dryRunLocal` over INLINE source. With `USE_WASM_PREVIEW` off (the test
 *   default), they take the serve `POST` path; we mock `serveClient` so no real
 *   server is needed and no `.wasm` is loaded (convention B honored).
 */
import { describe, it, expect, vi, beforeEach } from 'vitest'

// Mock the WASM adapter so importing the variant module never touches `ke_wasm.js`
// and so we can assert the serve (non-WASM) path is taken when the flag is off.
const compilePreviewMock = vi.fn()
const dryRunMock = vi.fn()
vi.mock('@/wasm', () => ({
  compilePreview: (src: string) => compilePreviewMock(src),
  dryRun: (src: string, facts: unknown) => dryRunMock(src, facts),
}))

// Mock the shared serve transport so `*Local` serve calls hit a fake axios post.
const postMock = vi.fn()
vi.mock('./serveClient', async () => {
  const actual = await vi.importActual<typeof import('./serveClient')>('./serveClient')
  return {
    ...actual,
    serveClient: { post: (...args: unknown[]) => postMock(...args) },
  }
})

import {
  listRulesLocal,
  decideLocal,
  compilePreviewLocal,
  dryRunLocal,
} from './rules.serve'
import { ServeUnsupportedError } from './serveClient'

beforeEach(() => {
  vi.clearAllMocks()
})

describe('listRulesLocal (Home — SCAFFOLD-ONLY)', () => {
  it('throws ServeUnsupportedError instead of returning a placeholder', async () => {
    await expect(listRulesLocal()).rejects.toBeInstanceOf(ServeUnsupportedError)
  })

  it('carries the page name and a not-yet-rewired reason', async () => {
    await expect(listRulesLocal()).rejects.toMatchObject({
      name: 'ServeUnsupportedError',
      page: expect.stringContaining('Home'),
    })
  })
})

describe('decideLocal (KEWorkbench — SCAFFOLD-ONLY fall-through)', () => {
  it('throws ServeUnsupportedError so the hook falls back to canonical decide', async () => {
    await expect(
      decideLocal({ instrument_type: 'art', jurisdiction: 'EU' }),
    ).rejects.toBeInstanceOf(ServeUnsupportedError)
  })

  it('names KEWorkbench and never hits the network', async () => {
    await expect(decideLocal({})).rejects.toMatchObject({
      page: expect.stringContaining('KEWorkbench'),
    })
    expect(postMock).not.toHaveBeenCalled()
    expect(dryRunMock).not.toHaveBeenCalled()
  })
})

describe('compilePreviewLocal (KEWorkbench — REWIRED, additive)', () => {
  it('takes the serve POST /compile/preview path when USE_WASM_PREVIEW is off', async () => {
    const report = { has_blocking: false, findings: [], conflicts: [] }
    postMock.mockResolvedValue({ data: { rules: [{ rule_id: 'r1' }], report } })

    const result = await compilePreviewLocal('rule_id: r1')

    expect(postMock).toHaveBeenCalledWith('/compile/preview', { source: 'rule_id: r1' })
    expect(compilePreviewMock).not.toHaveBeenCalled() // wasm path NOT taken
    expect(result.rules).toHaveLength(1)
    expect(result.report.has_blocking).toBe(false)
  })
})

describe('dryRunLocal (KEWorkbench — REWIRED, additive)', () => {
  it('takes the serve POST /dry-run path when USE_WASM_PREVIEW is off', async () => {
    postMock.mockResolvedValue({
      data: {
        evaluations: [
          {
            applicable: true,
            decision: 'compliant',
            obligations: [],
            applicability_steps: [],
            decision_path: [],
          },
        ],
      },
    })

    const result = await dryRunLocal('rule_id: r1', { jurisdiction: 'EU' })

    expect(postMock).toHaveBeenCalledWith('/dry-run', {
      source: 'rule_id: r1',
      facts: { jurisdiction: 'EU' },
    })
    expect(dryRunMock).not.toHaveBeenCalled() // wasm path NOT taken
    expect(result.evaluations[0].applicable).toBe(true)
  })
})
