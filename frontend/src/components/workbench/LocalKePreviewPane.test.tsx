/**
 * Gate-5 (5d) gate + wiring tests for the KEWorkbench local-surface preview pane.
 *   - both surface flags OFF (default) -> renders null (KEWorkbench byte-unchanged).
 *   - USE_WASM_PREVIEW on -> the preview pane renders.
 *   - a verify result feeds its CANONICAL provenance into the 5e ReviewSurface
 *     (with USE_REVIEW_UI on).
 *
 * Flags are read at render from imported constants; per the repo's testing
 * convention we `vi.doMock('@/config/flags', …)` + dynamic-import the component.
 * The three mutation hooks are stubbed so no QueryClient is needed.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import type { ArtifactProvenance } from '@/api/serve/serveClient'

const provenance: ArtifactProvenance = {
  regime_id: 'mica_2023',
  artifact_hash: [0x12, 0x34, 0x56],
  ir_schema_version: 'ir-1',
  codec_version: 'codec-1',
  canonicalization_version: 'canon-1',
  signer_key_id: 'key-abc',
  is_test_key: false,
  attestations: [],
  registry_state: 'Published',
  registry_event_head_hash: [0x00],
  exported_at_unix: 1_700_000_000,
}

function stubMutation(overrides: Record<string, unknown> = {}) {
  return { mutate: vi.fn(), isPending: false, data: undefined, error: null, ...overrides }
}

vi.mock('@/config/flags', () => ({
  USE_WASM_PREVIEW: false,
  USE_LOCAL_KE_API: false,
  USE_REVIEW_UI: false,
}))

beforeEach(() => {
  vi.resetModules()
})

describe('LocalKePreviewPane', () => {
  it('renders nothing when both surface flags are off (main unchanged)', async () => {
    vi.doMock('@/config/flags', () => ({
      USE_WASM_PREVIEW: false,
      USE_LOCAL_KE_API: false,
      USE_REVIEW_UI: false,
    }))
    vi.doMock('@/hooks', () => ({
      useCompilePreview: () => stubMutation(),
      useDryRun: () => stubMutation(),
      useVerify: () => stubMutation(),
    }))
    const { LocalKePreviewPane } = await import('./LocalKePreviewPane')
    const { container } = render(<LocalKePreviewPane />)
    expect(container).toBeEmptyDOMElement()
  })

  it('renders the preview pane when USE_WASM_PREVIEW is on', async () => {
    vi.doMock('@/config/flags', () => ({
      USE_WASM_PREVIEW: true,
      USE_LOCAL_KE_API: false,
      USE_REVIEW_UI: false,
    }))
    vi.doMock('@/hooks', () => ({
      useCompilePreview: () => stubMutation(),
      useDryRun: () => stubMutation(),
      useVerify: () => stubMutation(),
    }))
    const { LocalKePreviewPane } = await import('./LocalKePreviewPane')
    render(<LocalKePreviewPane />)
    expect(screen.getByTestId('local-ke-preview')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /compile preview/i })).toBeInTheDocument()
  })

  it('feeds verify provenance into the review surface (USE_REVIEW_UI on)', async () => {
    vi.doMock('@/config/flags', () => ({
      USE_WASM_PREVIEW: false,
      USE_LOCAL_KE_API: true,
      USE_REVIEW_UI: true,
    }))
    vi.doMock('@/hooks', () => ({
      useCompilePreview: () => stubMutation(),
      useDryRun: () => stubMutation(),
      useVerify: () =>
        stubMutation({
          data: { verdict: 'verified', provenance, registry_state: 'Published' },
        }),
    }))
    const { LocalKePreviewPane } = await import('./LocalKePreviewPane')
    render(<LocalKePreviewPane />)
    expect(screen.getByTestId('verify-result')).toBeInTheDocument()
    expect(screen.getByTestId('review-surface')).toBeInTheDocument()
  })
})
