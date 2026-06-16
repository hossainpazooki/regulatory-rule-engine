import { describe, it, expect, vi } from 'vitest'
import { selectQueryFn } from './useLocalVariant'
import { ServeUnsupportedError } from '@/api/serve/serveClient'

describe('selectQueryFn (Gate-5 flag-select helper)', () => {
  it('flag OFF: returns the fallback fn UNCHANGED (no wrapping); local is never called', async () => {
    const local = vi.fn().mockResolvedValue('local')
    const fallback = vi.fn().mockResolvedValue('fallback')
    const fn = selectQueryFn(false, local, fallback)
    expect(fn).toBe(fallback) // byte-identical to today
    await expect(fn()).resolves.toBe('fallback')
    expect(local).not.toHaveBeenCalled()
  })

  it('flag ON: uses the local variant when it succeeds', async () => {
    const local = vi.fn().mockResolvedValue('local')
    const fallback = vi.fn().mockResolvedValue('fallback')
    const fn = selectQueryFn(true, local, fallback)
    await expect(fn()).resolves.toBe('local')
    expect(fallback).not.toHaveBeenCalled()
  })

  it('flag ON: falls back transparently on ServeUnsupportedError (SCAFFOLD-ONLY)', async () => {
    const local = vi.fn().mockRejectedValue(new ServeUnsupportedError('X', 'no surface'))
    const fallback = vi.fn().mockResolvedValue('fallback')
    const fn = selectQueryFn(true, local, fallback)
    await expect(fn()).resolves.toBe('fallback')
    expect(local).toHaveBeenCalledOnce()
    expect(fallback).toHaveBeenCalledOnce()
  })

  it('flag ON: a genuine local-surface error PROPAGATES (never silently swallowed)', async () => {
    const boom = new Error('serve 500')
    const local = vi.fn().mockRejectedValue(boom)
    const fallback = vi.fn().mockResolvedValue('fallback')
    const fn = selectQueryFn(true, local, fallback)
    await expect(fn()).rejects.toBe(boom)
    expect(fallback).not.toHaveBeenCalled()
  })
})
