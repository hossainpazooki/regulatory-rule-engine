/**
 * Tests for the provenance classifier (`classifyProvenance` / `classifyProposal`)
 * and the class-style map. Asserts that items are DERIVED from canonical
 * provenance only and that ml-evidence / ai-suggestion are never fabricated from
 * the artifact.
 */
import { describe, it, expect } from 'vitest'
import type { ArtifactProvenance } from '@/api/serve/serveClient'
import {
  CLASS_ORDER,
  CLASS_STYLE,
  classifyProposal,
  classifyProvenance,
  type ReviewProposal,
} from './provenance'

function makeProvenance(overrides: Partial<ArtifactProvenance> = {}): ArtifactProvenance {
  return {
    regime_id: 'us-aml',
    artifact_hash: [0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0],
    ir_schema_version: 'ir-1',
    codec_version: 'codec-1',
    canonicalization_version: 'canon-1',
    signer_key_id: 'key-abc',
    is_test_key: false,
    attestations: [],
    registry_state: 'Published',
    registry_event_head_hash: [0x00],
    exported_at_unix: 1_700_000_000,
    ...overrides,
  }
}

describe('classifyProvenance', () => {
  it('always emits exactly one compiler-validity item from structural fields', () => {
    const items = classifyProvenance(makeProvenance())
    const compiler = items.filter((i) => i.klass === 'compiler-validity')
    expect(compiler).toHaveLength(1)
    expect(compiler[0].detail).toContain('key-abc')
    expect(compiler[0].detail).toContain('ir-1')
  })

  it('does NOT fabricate ml-evidence or ai-suggestion from the artifact', () => {
    const items = classifyProvenance(makeProvenance())
    expect(items.some((i) => i.klass === 'ml-evidence')).toBe(false)
    expect(items.some((i) => i.klass === 'ai-suggestion')).toBe(false)
  })

  it('emits one expert-attestation item per attestation, preserving type', () => {
    const items = classifyProvenance(
      makeProvenance({
        attestations: [
          {
            attestation_type: 'SourceFidelity',
            signer_key_id: 'expert-1',
            is_test_key: false,
            tsa_class: 'rfc3161',
            claimed_time_unix: 1_700_000_100,
          },
          {
            attestation_type: 'PublicationApproval',
            signer_key_id: 'expert-2',
            is_test_key: true,
            tsa_class: 'none',
            claimed_time_unix: 1_700_000_200,
          },
        ],
      }),
    )
    const attest = items.filter((i) => i.klass === 'expert-attestation')
    expect(attest).toHaveLength(2)
    expect(attest[0].attestationType).toBe('SourceFidelity')
    expect(attest[1].attestationType).toBe('PublicationApproval')
    expect(attest[1].isTestKey).toBe(true)
  })

  it('propagates artifact test-key status onto the compiler-validity item', () => {
    const items = classifyProvenance(makeProvenance({ is_test_key: true }))
    const compiler = items.find((i) => i.klass === 'compiler-validity')
    expect(compiler?.isTestKey).toBe(true)
  })
})

describe('classifyProposal', () => {
  const proposal: ReviewProposal = {
    title: 'Tighten threshold',
    suggestion: 'set threshold to 10000',
    rationale: 'aligns with regime text',
    sourceSpan: 'rules/aml.yaml L42-L48',
    mlEvidence: [{ label: 'cluster-7', detail: 'similar rules cluster' }],
  }

  it('renders the suggestion as a single ai-suggestion item', () => {
    const items = classifyProposal(proposal)
    const ai = items.filter((i) => i.klass === 'ai-suggestion')
    expect(ai).toHaveLength(1)
    expect(ai[0].detail).toContain('rules/aml.yaml')
  })

  it('renders ml-evidence ONLY when supplied', () => {
    const withEvidence = classifyProposal(proposal)
    expect(withEvidence.some((i) => i.klass === 'ml-evidence')).toBe(true)

    const withoutEvidence = classifyProposal({
      title: 't',
      suggestion: 's',
      rationale: 'r',
    })
    expect(withoutEvidence.some((i) => i.klass === 'ml-evidence')).toBe(false)
  })
})

describe('CLASS_STYLE', () => {
  it('defines a distinct color token for each of the four classes', () => {
    const colors = CLASS_ORDER.map((k) => CLASS_STYLE[k].colorClass)
    expect(new Set(colors).size).toBe(4)
  })

  it('defines a distinct icon for each of the four classes', () => {
    const icons = CLASS_ORDER.map((k) => CLASS_STYLE[k].icon)
    expect(new Set(icons).size).toBe(4)
  })

  it('covers exactly the four-class taxonomy', () => {
    expect(CLASS_ORDER).toEqual([
      'compiler-validity',
      'ml-evidence',
      'ai-suggestion',
      'expert-attestation',
    ])
  })
})
