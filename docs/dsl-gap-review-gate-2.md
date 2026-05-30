# DSL gap review (Gate 2 readiness)

**Status:** complete — **one IR extension required** (`effective_window`
optionality, ADR 0006); no *DSL/operator* extension required.

> **Correction (2026-05-30):** an initial pass concluded "no IR extension
> required." Lowering the corpus then found `fca_crypto.yaml`'s rules carry no
> effective window, which the mandatory Gate-1 `effective_window` could not
> represent. That gap was surfaced to Hossain and resolved by making
> `effective_window` optional (ADR 0006) rather than synthesizing data. The DSL
> operators/structure still need no extension.
**Spec references:** §20 (DSL ontology mismatch risk), §23 (Before Gate 2 checklist).
**Date:** 2026-05-30
**Method:** static analysis of the snapshotted corpus (`fixtures/rules/*.yaml`,
recorded SHA `f73b940`) across the represented regimes — MiCA, GENIUS Act, FCA
(UK), FINMA, MAS, plus RWA authorization — checking each rule against the Gate 1
DSL/IR (`ke-core::ir`).

Spec §20 requires walking ≥3 regimes (MiCA, FSMA UK, GENIUS) for rules that
resist conditional encoding **before** hardening the compiler. This is that walk.

## What the corpus exercises

- **Operators** (all within the closed `Operator` set): `==` (151×), `in` (35×),
  `>=` (7×), `<=` (3×), `>` (1×), `<` (1×), `!=` (1×). **Not used** in the corpus:
  `not_in`, `exists` — exercised only by synthetic fixtures.
- **Nesting**: `all`/`any` groups nest (36 group occurrences). Covered by
  `ConditionGroupSpec` / `ConditionOrGroup`.
- **Numeric thresholds**, including fractional: `0`, `0.90`, `1.0`, `2`, `5`,
  `1000`, `250000`, `1000000`, `3000000`. All representable as exact decimals
  (`ScalarValue::Decimal { mantissa, scale }`, ADR 0003) — e.g. `0.90 → {9,1}`.
- **Effective dates**: `effective_from` on every rule; **no `effective_to`** in
  the corpus (open-ended windows). Relevant to T4 `temporal_overlap` (ADR 0005).

## The real gap: standards-based provisions → boolean facts

Discretionary / standards-based regulation does not encode as conditionals.
Across all regimes the corpus handles this the same way: it **externalizes the
judgment as a pre-computed boolean fact** the caller must supply, rather than
deriving it from source text. Examples:

- MiCA "white paper fair, clear and not misleading" → `whitepaper_compliant: bool`.
- MiCA significant-ART "enhanced requirements" → `enhanced_requirements_met: bool`.
- GENIUS issuer "authorized" status → `issuer_authorized: bool`.
- FCA / MAS / FINMA "fit and proper", "adequate systems and controls" → boolean
  flags (`fit_and_proper`, etc.).

This is the spec §20 ontology mismatch made concrete. The conclusion:

- **The DSL needs no new constructs.** Every corpus rule encodes within the Gate
  1 IR. The mismatch is *semantic*, not *syntactic*: the DSL cannot mechanically
  derive a discretionary standard, so the standard becomes an input fact plus an
  `interpretation_notes` entry explaining the modeling choice, ultimately bound
  by expert attestation (Gate 4).
- Therefore **no Gate-1-frozen `ke-core` change is needed**, and the §20
  checkpoint is cleared without an IR-extension ADR.

## Findings feeding later passes

- `fca_crypto.yaml` carries **no `interpretation_notes`** on any rule; where its
  rules use thresholds/exceptions, T1 (ADR 0004) will flag missing notes. Correct
  behavior — a verification signal, not a blocker.
- Open-ended windows (no `effective_to`) make corpus-wide `temporal_overlap`
  expected; T4 fixtures use explicit windows to test the class precisely.
- `not_in` / `exists` are unexercised by the corpus, so parser/lowering coverage
  for them is proven by synthetic fixtures, not the differential harness.

## Recommendation

Proceed with Gate 2 compiler hardening on the current Gate 1 DSL/IR. Revisit the
DSL only if a *new* regime introduces a construct the boolean-fact pattern cannot
express (e.g. quant! over collections, temporal sequencing) — at which point an
IR-extension ADR is required before encoding it.
