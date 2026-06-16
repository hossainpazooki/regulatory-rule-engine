/**
 * Tests that the panel renders ALL FOUR provenance-class sections as visually
 * distinct, always-present groups (empty ones show an explicit placeholder), and
 * that items are read from canonical provenance + supplied proposal items only.
 */
import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import type { ArtifactProvenance } from '@/api/serve/serveClient'
import { ProvenancePanel } from './ProvenancePanel'
import {
  CLASS_ORDER,
  CLASS_STYLE,
  classifyProposal,
  type ReviewProposal,
} from './provenance'

const provenance: ArtifactProvenance = {
  regime_id: 'us-aml',
  artifact_hash: [0x12, 0x34, 0x56],
  ir_schema_version: 'ir-1',
  codec_version: 'codec-1',
  canonicalization_version: 'canon-1',
  signer_key_id: 'key-abc',
  is_test_key: true,
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

describe('ProvenancePanel', () => {
  it('renders all four class sections, each visually distinct', () => {
    const { container } = render(<ProvenancePanel provenance={provenance} />)
    const sections = CLASS_ORDER.map((klass) =>
      container.querySelector(`[data-provenance-section="${klass}"]`),
    )
    // all four present
    expect(sections.every((s) => s !== null)).toBe(true)

    // each section's badge carries that class's distinct color token
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
  })

  it('shows an explicit empty placeholder for classes with no items', () => {
    render(<ProvenancePanel provenance={provenance} />)
    // no proposal supplied -> ml-evidence and ai-suggestion are empty, never hidden
    expect(screen.getByTestId('empty-ml-evidence')).toHaveTextContent(
      'no items in this class',
    )
    expect(screen.getByTestId('empty-ai-suggestion')).toHaveTextContent(
      'no items in this class',
    )
  })

  it('renders the canonical compiler-validity and expert-attestation items', () => {
    const { container } = render(<ProvenancePanel provenance={provenance} />)
    const compilerSection = container.querySelector(
      '[data-provenance-section="compiler-validity"]',
    ) as HTMLElement
    expect(compilerSection).toHaveTextContent('structural validity')
    expect(screen.getByText('SourceFidelity')).toBeInTheDocument()
  })

  it('surfaces the artifact test-key status loudly', () => {
    render(<ProvenancePanel provenance={provenance} />)
    expect(screen.getAllByTestId('test-key-marker').length).toBeGreaterThan(0)
  })

  it('merges supplied proposal items into ml-evidence / ai-suggestion sections', () => {
    const proposal: ReviewProposal = {
      title: 'Tighten threshold',
      suggestion: 'set threshold to 10000',
      rationale: 'aligns with regime',
      mlEvidence: [{ label: 'cluster-7', detail: 'similar rules' }],
    }
    const { container } = render(
      <ProvenancePanel
        provenance={provenance}
        proposalItems={classifyProposal(proposal)}
      />,
    )
    const aiSection = container.querySelector(
      '[data-provenance-section="ai-suggestion"]',
    ) as HTMLElement
    expect(aiSection).toHaveTextContent('Tighten threshold')
    const mlSection = container.querySelector(
      '[data-provenance-section="ml-evidence"]',
    ) as HTMLElement
    expect(mlSection).toHaveTextContent('cluster-7')
  })
})
