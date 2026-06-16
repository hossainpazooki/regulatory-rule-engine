/**
 * The single entry component for the 5e AI-provenance review UI (spec § 13).
 * The page mounts ONLY this; it composes `ProvenancePanel` + `ProposalInspector`.
 *
 * Flag-gated: returns `null` whenever `USE_REVIEW_UI` is off, so with the flag
 * off nothing renders and the host page is byte-unchanged. The provenance and
 * proposal are supplied by the page from CANONICAL sources (serve `/verify` or
 * WASM `read_provenance`); this component reads them, classifies them into the
 * four provenance classes, and presents them — inventing nothing and offering no
 * accept/sign/publish affordance (authority boundary).
 *
 * Out of scope by contract: source-coverage visualization, counterexample
 * exploration, semantic-diff. Not built here.
 */
import { USE_REVIEW_UI } from '@/config/flags'
import type { ArtifactProvenance } from '@/api/serve/serveClient'
import { ProvenancePanel } from './ProvenancePanel'
import { ProposalInspector } from './ProposalInspector'
import { classifyProposal, type ReviewProposal } from './provenance'

interface ReviewSurfaceProps {
  provenance?: ArtifactProvenance
  proposal?: ReviewProposal
}

export function ReviewSurface({ provenance, proposal }: ReviewSurfaceProps) {
  if (!USE_REVIEW_UI) return null

  const proposalItems = proposal ? classifyProposal(proposal) : []

  return (
    <div data-testid="review-surface" className="space-y-6">
      <header>
        <h3 className="text-lg font-semibold text-slate-100">AI-provenance review</h3>
        <p className="text-xs text-slate-400">
          Preview/verify surface (spec § 6/§ 16) - non-authoritative. The four
          provenance classes are read from the canonical artifact; the compiler
          asserts structural validity only, never legal truth.
        </p>
      </header>

      {provenance ? (
        <ProvenancePanel provenance={provenance} proposalItems={proposalItems} />
      ) : (
        <p data-testid="no-provenance" className="text-sm text-slate-500 italic">
          No artifact provenance loaded. Verify an artifact to inspect its provenance.
        </p>
      )}

      {proposal ? <ProposalInspector proposal={proposal} /> : null}
    </div>
  )
}
