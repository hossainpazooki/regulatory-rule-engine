/**
 * Tests for the ProductionDemo local-surface variant (`healthLocal`).
 *
 * The serve `GET /healthz` HTTP call is mocked at the `serveClient` axios layer
 * so the mapping `{ ok, surface }` -> `HealthResponse` is exercised in isolation
 * (no live server, honoring the non-authoritative preview boundary). The
 * canonical `VITE_API_URL` path is untouched and is not exercised here.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest'

const getMock = vi.fn()

vi.mock('./serveClient', () => ({
  serveClient: { get: (...args: unknown[]) => getMock(...args) },
}))

import { healthLocal } from './production.serve'

beforeEach(() => {
  vi.clearAllMocks()
})

describe('healthLocal', () => {
  it('calls GET /healthz on the serve client', async () => {
    getMock.mockResolvedValue({ data: { ok: true, surface: 'ke-cli serve (preview)' } })
    await healthLocal()
    expect(getMock).toHaveBeenCalledWith('/healthz')
  })

  it('maps { ok: true } to status "healthy"', async () => {
    getMock.mockResolvedValue({ data: { ok: true, surface: 'preview' } })
    const result = await healthLocal()
    expect(result).toEqual({ status: 'healthy' })
  })

  it('maps { ok: false } to status "unhealthy"', async () => {
    getMock.mockResolvedValue({ data: { ok: false, surface: 'preview' } })
    const result = await healthLocal()
    expect(result).toEqual({ status: 'unhealthy' })
  })

  it('propagates a transport error (no silent success)', async () => {
    getMock.mockRejectedValue(new Error('ECONNREFUSED'))
    await expect(healthLocal()).rejects.toThrow('ECONNREFUSED')
  })
})
