/**
 * Inspects a single AI proposal under review. The proposal (suggestion +
 * source-span mapping + rationale + ML evidence references) is supplied as a
 * prop by the page; the component invents nothing.
 *
 * Authority boundary (CLAUDE.md, spec § 5/§ 13): this is INSPECTION ONLY. There
 * is no accept / sign / publish affordance. An AI suggestion is rendered as an
 * `ai-suggestion`-class item with a prominent "AI suggestion - non-authoritative,
 * not attested" banner. Any ML evidence is rendered as `ml-evidence`-class
 * context, never promoted to authority.
 */
import { ProvenanceClassBadge } from './ProvenanceClassBadge'
import type { ReviewProposal } from './provenance'

interface ProposalInspectorProps {
  proposal: ReviewProposal
}

export function ProposalInspector({ proposal }: ProposalInspectorProps) {
  return (
    <section
      data-testid="proposal-inspector"
      className="border border-amber-500/40 rounded-lg p-4 space-y-3"
    >
      <div className="flex items-center justify-between gap-2">
        <ProvenanceClassBadge klass="ai-suggestion" />
      </div>

      <p
        data-testid="non-authoritative-banner"
        role="alert"
        className="text-sm font-semibold text-amber-300"
      >
        AI suggestion - non-authoritative, not attested. Inspection only; this UI
        cannot accept, sign, or publish.
      </p>

      <div>
        <h4 className="text-sm font-medium text-slate-100">{proposal.title}</h4>
        <pre className="mt-1 whitespace-pre-wrap break-words text-sm text-slate-200 bg-slate-900/60 rounded p-2">
          {proposal.suggestion}
        </pre>
      </div>

      {proposal.sourceSpan ? (
        <p className="text-xs text-slate-400">
          source span: <code>{proposal.sourceSpan}</code>
        </p>
      ) : null}

      <div>
        <h5 className="text-xs uppercase tracking-wide text-slate-500">Rationale</h5>
        <p className="text-sm text-slate-300">{proposal.rationale}</p>
      </div>

      {proposal.mlEvidence && proposal.mlEvidence.length > 0 ? (
        <div className="space-y-2">
          <div className="flex items-center gap-2">
            <ProvenanceClassBadge klass="ml-evidence" />
            <span className="text-xs text-slate-500">supporting evidence (advisory)</span>
          </div>
          <ul className="space-y-1">
            {proposal.mlEvidence.map((ev, idx) => (
              <li key={idx} className="text-sm text-slate-300">
                <span className="font-medium">{ev.label}</span>
                <span className="text-xs text-slate-400"> - {ev.detail}</span>
              </li>
            ))}
          </ul>
        </div>
      ) : null}
    </section>
  )
}
