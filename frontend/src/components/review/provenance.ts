/**
 * The provenance-class model + classifier for the 5e AI-provenance review UI
 * (spec § 13). This is the data layer of the review surface.
 *
 * NON-AUTHORITATIVE / read-from-canonical-only (CLAUDE.md authority boundary,
 * spec § 5/§ 6/§ 16): every display item here is DERIVED from canonical artifact
 * provenance — the `VerifyResponse.provenance` from serve `POST /verify`, or the
 * WASM `read_provenance(...)` JSON via the adapter. NOTHING is invented
 * client-side. ML-evidence and AI-suggestion items exist ONLY when explicitly
 * supplied as proposal metadata (see `ReviewProposal` / `ProposalInspector`);
 * they are never fabricated from the artifact. If a field is absent, its class
 * is omitted — never guessed.
 *
 * The four classes encode the authority taxonomy:
 *   - compiler-validity   : structural validity only — NEVER legal truth.
 *   - ml-evidence         : ML/analytics-derived evidence (advisory).
 *   - ai-suggestion       : an AI proposal — NEVER authoritative, never attested.
 *   - expert-attestation  : a typed, signed expert attestation — the ONLY legal
 *                           authority bound to an artifact hash.
 */
import type { ComponentType, SVGProps } from 'react'
import {
  CheckBadgeIcon,
  ChartBarIcon,
  SparklesIcon,
  ShieldCheckIcon,
} from '@heroicons/react/24/outline'
import type { ArtifactProvenance, AttestationSummary } from '@/api/serve/serveClient'

export type ProvenanceClass =
  | 'compiler-validity'
  | 'ml-evidence'
  | 'ai-suggestion'
  | 'expert-attestation'

export interface ProvenanceItem {
  klass: ProvenanceClass
  label: string
  detail: string
  /** Surfaced loudly when true (test key — not production). */
  isTestKey?: boolean
  attestationType?: AttestationSummary['attestation_type']
}

type IconType = ComponentType<SVGProps<SVGSVGElement>>

/**
 * Fixed, stable visual identity per class. Defined ONCE here so every renderer
 * (and Playwright baselines) stay in lockstep. Do not randomize or compute these
 * at render time. Each token pairs a distinct color family + icon + accessible
 * label so the four classes are visually and semantically separable.
 */
export const CLASS_STYLE: Record<
  ProvenanceClass,
  { colorClass: string; icon: IconType; aria: string; title: string }
> = {
  'compiler-validity': {
    // emerald — deterministic, structural
    colorClass: 'bg-emerald-500/20 text-emerald-300 border-emerald-500/40',
    icon: CheckBadgeIcon,
    aria: 'Compiler validity (structural only)',
    title: 'Compiler validity',
  },
  'ml-evidence': {
    // sky — analytic/advisory
    colorClass: 'bg-sky-500/20 text-sky-300 border-sky-500/40',
    icon: ChartBarIcon,
    aria: 'Machine-learning evidence (advisory)',
    title: 'ML evidence',
  },
  'ai-suggestion': {
    // amber — caution, non-authoritative
    colorClass: 'bg-amber-500/20 text-amber-300 border-amber-500/40',
    icon: SparklesIcon,
    aria: 'AI suggestion (non-authoritative, not attested)',
    title: 'AI suggestion',
  },
  'expert-attestation': {
    // violet — the legal authority
    colorClass: 'bg-violet-500/20 text-violet-300 border-violet-500/40',
    icon: ShieldCheckIcon,
    aria: 'Expert attestation (typed, signed legal authority)',
    title: 'Expert attestation',
  },
}

/** Stable display order for the four-class taxonomy. */
export const CLASS_ORDER: ProvenanceClass[] = [
  'compiler-validity',
  'ml-evidence',
  'ai-suggestion',
  'expert-attestation',
]

/** A reference into ML evidence supplied alongside an AI proposal. */
export interface MlEvidenceRef {
  label: string
  detail: string
}

/**
 * An AI suggestion under review, supplied BY THE PAGE (never invented by a
 * component). It carries the suggestion, its source-span mapping, the AI
 * rationale, and any ML evidence references. The review UI renders this as an
 * `ai-suggestion`-class item — inspection only, no accept/sign/publish.
 */
export interface ReviewProposal {
  /** Short title for the suggestion. */
  title: string
  /** The proposed edit / text the AI suggests. */
  suggestion: string
  /** The AI's stated rationale. */
  rationale: string
  /** Human-readable source-span mapping (e.g. "rules/aml.yaml L42-L48"). */
  sourceSpan?: string
  /** ML evidence the proposal leans on, if any (rendered as ml-evidence items). */
  mlEvidence?: MlEvidenceRef[]
}

/** Render `[u8;32]` byte arrays as a short hex prefix for display. */
function shortHash(bytes: number[] | undefined): string {
  if (!bytes || bytes.length === 0) return '(none)'
  const hex = bytes
    .slice(0, 6)
    .map((b) => (b & 0xff).toString(16).padStart(2, '0'))
    .join('')
  return `${hex}...`
}

/**
 * Map canonical artifact provenance -> display items. Pure, deterministic, no
 * fetch.
 *
 * - compiler-validity: ALWAYS exactly one item, drawn from the artifact's
 *   structural identity fields.
 * - expert-attestation: one item per `AttestationSummary` on the artifact.
 * - ml-evidence / ai-suggestion: NOT produced here (they are not present in the
 *   canonical artifact). They come only from `classifyProposal` when the page
 *   supplies a `ReviewProposal`.
 */
export function classifyProvenance(p: ArtifactProvenance): ProvenanceItem[] {
  const items: ProvenanceItem[] = []

  items.push({
    klass: 'compiler-validity',
    label: `${p.regime_id} - structural validity`,
    detail: [
      `signer ${p.signer_key_id}`,
      `ir ${p.ir_schema_version}`,
      `codec ${p.codec_version}`,
      `canon ${p.canonicalization_version}`,
      `hash ${shortHash(p.artifact_hash)}`,
    ].join(' / '),
    isTestKey: p.is_test_key,
  })

  for (const att of p.attestations) {
    items.push({
      klass: 'expert-attestation',
      label: att.attestation_type,
      detail: [
        `signer ${att.signer_key_id}`,
        `tsa ${att.tsa_class}`,
        `claimed ${att.claimed_time_unix}`,
      ].join(' / '),
      isTestKey: att.is_test_key,
      attestationType: att.attestation_type,
    })
  }

  return items
}

/**
 * Map a page-supplied `ReviewProposal` -> display items. The suggestion itself
 * becomes an `ai-suggestion` item (always present); each ML evidence reference
 * becomes an `ml-evidence` item (present only when supplied). Pure; invents
 * nothing beyond formatting the props it is given.
 */
export function classifyProposal(proposal: ReviewProposal): ProvenanceItem[] {
  const items: ProvenanceItem[] = []

  items.push({
    klass: 'ai-suggestion',
    label: proposal.title,
    detail: proposal.sourceSpan
      ? `${proposal.suggestion} (${proposal.sourceSpan})`
      : proposal.suggestion,
  })

  for (const ev of proposal.mlEvidence ?? []) {
    items.push({
      klass: 'ml-evidence',
      label: ev.label,
      detail: ev.detail,
    })
  }

  return items
}
