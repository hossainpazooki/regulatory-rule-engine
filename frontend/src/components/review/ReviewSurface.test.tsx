/**
 * Tests the flag gate on `ReviewSurface`:
 *   - flag OFF -> renders null (host page byte-unchanged).
 *   - flag ON  -> composes ProvenancePanel + ProposalInspector, and the FOUR
 *     provenance classes render distinctly from the canonical provenance input.
 *
 * The flag is read at module load from `import.meta.env`; rather than mutate env
 * (brittle), we mock `@/config/flags` per the contract's testing guidance.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import type { ArtifactProvenance } from '@/api/serve/serveClient'
import { CLASS_ORDER, CLASS_STYLE, type ReviewProposal } from './provenance'

const provenance: ArtifactProvenance = {
  regime_id: 'us-aml',
  artifact_hash: [0x12, 0x34, 0x56],
  ir_schema_version: 'ir-1',
  codec_version: 'codec-1',
  canonicalization_version: 'canon-1',
  signer_key_id: 'key-abc',
  is_test_key: false,
  attestations: [
    {
      attestation_type: 'SourceFidelity',
      signer_key_id: 'expert-1',
      is_test_key: false,
      tsa_class: 'rfc3161',
      claimed_time_unix: 1_700_000_100,
    },
  ],
  registry_state: 'Published',
  registry_event_head_hash: [0x00],
  exported_at_unix: 1_700_000_000,
}

const proposal: ReviewProposal = {
  title: 'Tighten threshold',
  suggestion: 'set threshold to 10000',
  rationale: 'aligns with regime',
  sourceSpan: 'rules/aml.yaml L42-L48',
  mlEvidence: [{ label: 'cluster-7', detail: 'similar rules' }],
}

vi.mock('@/config/flags', () => ({ USE_REVIEW_UI: false }))

beforeEach(() => {
  vi.resetModules()
})

describe('ReviewSurface flag gate', () => {
  it('renders nothing when USE_REVIEW_UI is off', async () => {
    vi.doMock('@/config/flags', () => ({ USE_REVIEW_UI: false }))
    const { ReviewSurface } = await import('./ReviewSurface')
    const { container } = render(
      <ReviewSurface provenance={provenance} proposal={proposal} />,
    )
    expect(container).toBeEmptyDOMElement()
  })

  it('renders the surface when USE_REVIEW_UI is on', async () => {
    vi.doMock('@/config/flags', () => ({ USE_REVIEW_UI: true }))
    const { ReviewSurface } = await import('./ReviewSurface')
    render(<ReviewSurface provenance={provenance} proposal={proposal} />)
    expect(screen.getByTestId('review-surface')).toBeInTheDocument()
    expect(screen.getByTestId('provenance-panel')).toBeInTheDocument()
    expect(screen.getByTestId('proposal-inspector')).toBeInTheDocument()
  })

  it('renders the FOUR provenance classes distinctly from provenance input', async () => {
    vi.doMock('@/config/flags', () => ({ USE_REVIEW_UI: true }))
    const { ReviewSurface } = await import('./ReviewSurface')
    const { container } = render(
      <ReviewSurface provenance={provenance} proposal={proposal} />,
    )

    // each of the four class sections is present and carries its own distinct
    // color token (the visual-distinctness contract).
    const badgeClasses = CLASS_ORDER.map((klass) => {
      const badge = container.querySelector(
        `[data-provenance-section="${klass}"] [data-provenance-class="${klass}"]`,
      ) as HTMLElement
      expect(badge).not.toBeNull()
      for (const token of CLASS_STYLE[klass].colorClass.split(' ')) {
        expect(badge).toHaveClass(token)
      }
      return badge.className
    })
    expect(new Set(badgeClasses).size).toBe(4)

    // canonical-derived items are present; ai/ml come from the supplied proposal.
    const compilerSection = container.querySelector(
      '[data-provenance-section="compiler-validity"]',
    ) as HTMLElement
    expect(compilerSection).toHaveTextContent('structural validity')
    expect(screen.getByText('SourceFidelity')).toBeInTheDocument()
    const aiSection = container.querySelector(
      '[data-provenance-section="ai-suggestion"]',
    ) as HTMLElement
    expect(aiSection).toHaveTextContent('Tighten threshold')
  })

  it('shows a no-provenance placeholder when none is supplied (flag on)', async () => {
    vi.doMock('@/config/flags', () => ({ USE_REVIEW_UI: true }))
    const { ReviewSurface } = await import('./ReviewSurface')
    render(<ReviewSurface />)
    expect(screen.getByTestId('no-provenance')).toBeInTheDocument()
  })
})
