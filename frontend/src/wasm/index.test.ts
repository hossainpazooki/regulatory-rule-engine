/**
 * Smoke tests for the WASM preview adapter. These do NOT load a real `.wasm`
 * (none is built in unit CI) — the raw `./ke_wasm.js` module is mocked so the
 * adapter's JSON parsing, init guard, and error mapping are exercised in
 * isolation. The load-bearing parity proof lives Rust-side in
 * `crates/ke-wasm/tests/parity.rs`; this just keeps `npm test` green and pins
 * the adapter's JSON-shape handling.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest'

// Mock the generated bindings. `compile_preview` / `dry_run` return JSON
// strings (mirroring the Rust surface); the mocks let us drive success and the
// thrown-error path without a compiled artifact.
const compilePreviewMock = vi.fn()
const dryRunMock = vi.fn()
const initMock = vi.fn(() => Promise.resolve({}))

vi.mock('./ke_wasm.js', () => ({
  default: initMock,
  compile_preview: (source: string) => compilePreviewMock(source),
  dry_run: (source: string, facts: string) => dryRunMock(source, facts),
}))

import { compilePreview, dryRun, ensureWasm, KeWasmComputeError } from './index'
import { deepEqual } from './parity'

beforeEach(() => {
  vi.clearAllMocks()
})

describe('compilePreview', () => {
  it('parses the CompilePreviewResponse JSON shape', async () => {
    compilePreviewMock.mockReturnValue(
      JSON.stringify({
        rules: [{ rule_id: 'r1' }],
        report: { has_blocking: false, findings: [], conflicts: [] },
      }),
    )
    const result = await compilePreview('rule_id: r1')
    expect(result.rules).toHaveLength(1)
    expect(result.report.has_blocking).toBe(false)
    expect(Array.isArray(result.report.findings)).toBe(true)
  })

  it('maps a thrown compile_error JsError to KeWasmComputeError', async () => {
    compilePreviewMock.mockImplementation(() => {
      throw new Error(JSON.stringify({ error: 'compile_error', detail: 'ParseError(..)' }))
    })
    await expect(compilePreview('!!bad')).rejects.toBeInstanceOf(KeWasmComputeError)
    await expect(compilePreview('!!bad')).rejects.toMatchObject({ kind: 'compile_error' })
  })
})

describe('dryRun', () => {
  it('stringifies facts and parses the DryRunResponse JSON shape', async () => {
    dryRunMock.mockReturnValue(
      JSON.stringify({
        evaluations: [
          {
            applicable: true,
            decision: 'compliant',
            obligations: [],
            applicability_steps: [],
            decision_path: [],
          },
        ],
      }),
    )
    const result = await dryRun('rule_id: r1', { jurisdiction: 'EU' })
    expect(dryRunMock).toHaveBeenCalledWith('rule_id: r1', JSON.stringify({ jurisdiction: 'EU' }))
    expect(result.evaluations[0].applicable).toBe(true)
    expect(result.evaluations[0].decision).toBe('compliant')
  })

  it('maps a thrown facts_error JsError to KeWasmComputeError', async () => {
    dryRunMock.mockImplementation(() => {
      throw new Error(JSON.stringify({ error: 'facts_error', detail: 'facts must be an object' }))
    })
    await expect(dryRun('rule_id: r1', [1, 2, 3])).rejects.toMatchObject({ kind: 'facts_error' })
  })
})

describe('ensureWasm', () => {
  it('is idempotent: repeated calls do not re-initialise', async () => {
    // The init guard memoises the first init() across the whole module, so by
    // the time this runs init() may already be cached from an earlier test.
    // Either way, two calls here must not ADD more than one init() call.
    const before = initMock.mock.calls.length
    await ensureWasm()
    await ensureWasm()
    expect(initMock.mock.calls.length - before).toBeLessThanOrEqual(1)
  })
})

describe('deepEqual (G5-2 parity check)', () => {
  it('is order-insensitive over object keys', () => {
    expect(deepEqual({ a: 1, b: 2 }, { b: 2, a: 1 })).toBe(true)
  })
  it('detects a value difference (a mismatch would be surfaced)', () => {
    expect(deepEqual({ a: 1 }, { a: 2 })).toBe(false)
  })
  it('compares arrays positionally', () => {
    expect(deepEqual([1, 2, 3], [1, 2, 3])).toBe(true)
    expect(deepEqual([1, 2], [2, 1])).toBe(false)
  })
})
