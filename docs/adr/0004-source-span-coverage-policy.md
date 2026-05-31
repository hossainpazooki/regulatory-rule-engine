# 0004. Source-span coverage policy (T1) and span/provenance separation

**Status:** Accepted
**Date:** 2026-05-30
**Spec references:** §11 (verification model — T1), §13 (AI edit provenance), §8.1
**Brief references:** `dev/briefs/gate-2-parser-compiler-verification.md`
**Gate:** 2

## Context

Gate 2 introduces a YAML parser and the T1 verification pass. Two distinct
"span"/"source" notions collide if not separated, and T1's coverage rule must be
defined so the existing corpus is checkable rather than spuriously rejected.

The corpus carries a **mandatory rule-level `source:`** (a `DocumentRef`) but
generally **no per-decision-node** legal references. A naive "every decision node
must carry its own `SourceSpan`" reading of spec §11 would reject the entire
corpus, contradicting the Gate 2 acceptance that compilation succeed over it.

## Decision

Distinguish **three** concepts; never conflate the first two into one type.

1. **YAML parse spans** — a parser-local `YamlSpan` (byte offsets + line/column)
   recording *where in the authoring YAML* a node came from. Used only for
   diagnostics and file-traceability (jumping from a verification finding back to
   the YAML). Lives in `crates/ke-compiler/src/ast/span.rs`. **Dropped during
   lowering** — it never enters `ke-core::RuleIR`.

2. **Legal source refs** — `ke-core::ir::SourceSpan` / `DocumentRef`: *which
   regulation/document provision* supports a node (document id, article, section,
   paragraph, optional pages/byte-range/text-hash). Semantic provenance; the
   basis of expert attestation (Gate 4). Carries **no** YAML byte offsets.

3. **T1 coverage rule** — a decision node / obligation is **covered** if it, or
   its nearest ancestor up to the **mandatory rule-level `source:`**, carries a
   legal source reference (inheritance). Per-node `SourceSpan` stays optional
   (the Gate 1 shape). Finer-grained, node-level references are encouraged and a
   missing finer reference may produce a **warning**, but inheritance from the
   rule-level `source:` means a well-formed rule is never **blocked** by T1 for
   coverage alone.

T1 additionally requires `interpretation_notes` where source text does not
mechanically imply the encoded condition — for Gate 2, scoped to rules carrying
numeric thresholds (`Gt/Lt/Gte/Lte` on a decimal) or `not_in` exceptions
(spec §17). Absence is a blocking T1 finding.

## Consequences

- Desirable: the legal-provenance contract (attestation, audit reconstruction)
  is never polluted by editor/file coordinates; `SourceSpan` means one thing.
- Desirable: the corpus passes T1 coverage by inheritance, so Gate 2 can verify
  it without fixture surgery.
- Desirable: `YamlSpan` still powers precise diagnostics and §13 provenance
  (mapping a node back to its YAML origin).
- Undesirable: inheritance is coarse — a rule with one rule-level `source:`
  "covers" every node even if individual nodes cite different provisions. Whole-
  document and per-node coverage tightening is deferred until the legal-source-
  storage decision (spec §21.4) is resolved; until then node-level refs are
  advisory.
- Known: some corpus rules (e.g. `fca_crypto.yaml`) carry no
  `interpretation_notes`; T1 will correctly flag them where thresholds/exceptions
  are present. This is a verification signal, not a Gate 2 blocker.

## Alternatives considered

- **One span type for both YAML and legal provenance** — rejected: it corrupts
  the provenance contract and makes `SourceSpan` ambiguous to every downstream
  consumer (attestation, platform verification).
- **Require explicit per-node `SourceSpan`** — rejected for Gate 2: would reject
  the corpus and presupposes the unresolved legal-source-storage decision.
