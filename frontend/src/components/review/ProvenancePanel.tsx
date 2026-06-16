/**
 * Renders canonical artifact provenance grouped into the FOUR provenance
 * classes. Calls `classifyProvenance` (read-from-canonical-only) and lays out
 * four always-present, labeled sections — empty sections show an explicit
 * "no items in this class" line rather than being hidden, so the four-class
 * taxonomy is always legible (spec § 13).
 *
 * Optional `proposalItems` (ml-evidence / ai-suggestion derived from a
 * page-supplied `ReviewProposal`) are merged in for grouping; they are never
 * fabricated here.
 */
import type { ArtifactProvenance } from '@/api/serve/serveClient'
import { ProvenanceClassBadge } from './ProvenanceClassBadge'
import {
  CLASS_ORDER,
  CLASS_STYLE,
  classifyProvenance,
  type ProvenanceClass,
  type ProvenanceItem,
} from './provenance'

interface ProvenancePanelProps {
  provenance: ArtifactProvenance
  /** Additional items (ml-evidence / ai-suggestion) from a reviewed proposal. */
  proposalItems?: ProvenanceItem[]
}

export function ProvenancePanel({ provenance, proposalItems = [] }: ProvenancePanelProps) {
  const items = [...classifyProvenance(provenance), ...proposalItems]
  const byClass = (klass: ProvenanceClass) => items.filter((i) => i.klass === klass)

  return (
    <section data-testid="provenance-panel" className="space-y-4">
      <p className="text-xs text-slate-400">
        Read from canonical artifact provenance (serve <code>/verify</code> or WASM{' '}
        <code>read_provenance</code>). Non-authoritative preview - nothing below is
        invented client-side.
      </p>

      {CLASS_ORDER.map((klass) => {
        const classItems = byClass(klass)
        const style = CLASS_STYLE[klass]
        return (
          <div
            key={klass}
            data-provenance-section={klass}
            className="border border-slate-700 rounded-lg p-3"
          >
            <div className="mb-2">
              <ProvenanceClassBadge klass={klass} />
            </div>

            {classItems.length === 0 ? (
              <p data-testid={`empty-${klass}`} className="text-xs text-slate-500 italic">
                no items in this class
              </p>
            ) : (
              <ul className="space-y-2">
                {classItems.map((item, idx) => (
                  <li
                    key={`${klass}-${idx}`}
                    className="flex flex-col gap-1 text-sm text-slate-200"
                  >
                    <span className="flex items-center gap-2 font-medium">
                      {item.label}
                      {item.isTestKey ? (
                        <ProvenanceClassBadge klass={klass} isTestKey />
                      ) : null}
                    </span>
                    <span className="text-xs text-slate-400">{item.detail}</span>
                  </li>
                ))}
              </ul>
            )}
            <span className="sr-only">{style.aria}</span>
          </div>
        )
      })}
    </section>
  )
}
