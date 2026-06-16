/**
 * Tests that the inspector renders an AI proposal as an ai-suggestion-class item
 * with a prominent non-authoritative banner, surfaces rationale + source span +
 * ML evidence, and offers NO accept/sign/publish affordance (authority boundary).
 */
import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { ProposalInspector } from './ProposalInspector'
import type { ReviewProposal } from './provenance'

const proposal: ReviewProposal = {
  title: 'Tighten threshold',
  suggestion: 'set threshold to 10000',
  rationale: 'aligns with the regime text',
  sourceSpan: 'rules/aml.yaml L42-L48',
  mlEvidence: [{ label: 'cluster-7', detail: 'similar rules cluster' }],
}

describe('ProposalInspector', () => {
  it('renders a prominent non-authoritative, not-attested banner', () => {
    render(<ProposalInspector proposal={proposal} />)
    const banner = screen.getByTestId('non-authoritative-banner')
    expect(banner).toHaveTextContent(/non-authoritative, not attested/i)
    expect(banner).toHaveAttribute('role', 'alert')
  })

  it('classifies the proposal under the ai-suggestion provenance class', () => {
    const { container } = render(<ProposalInspector proposal={proposal} />)
    expect(
      container.querySelector('[data-provenance-class="ai-suggestion"]'),
    ).not.toBeNull()
  })

  it('surfaces the suggestion, rationale, and source span', () => {
    render(<ProposalInspector proposal={proposal} />)
    expect(screen.getByText('set threshold to 10000')).toBeInTheDocument()
    expect(screen.getByText(/aligns with the regime text/)).toBeInTheDocument()
    expect(screen.getByText('rules/aml.yaml L42-L48')).toBeInTheDocument()
  })

  it('renders ML evidence as an advisory ml-evidence-class block', () => {
    const { container } = render(<ProposalInspector proposal={proposal} />)
    expect(
      container.querySelector('[data-provenance-class="ml-evidence"]'),
    ).not.toBeNull()
    expect(screen.getByText('cluster-7')).toBeInTheDocument()
  })

  it('offers NO accept/sign/publish affordance (inspection only)', () => {
    render(<ProposalInspector proposal={proposal} />)
    for (const btn of screen.queryAllByRole('button')) {
      expect(btn.textContent ?? '').not.toMatch(/accept|sign|publish|attest/i)
    }
  })
})
