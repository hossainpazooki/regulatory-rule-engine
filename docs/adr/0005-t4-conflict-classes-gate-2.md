# 0005. T4 conflict classes and severities for Gate 2

**Status:** Proposed (needs domain-reviewer sign-off per spec §23 "Before Gate 2")
**Date:** 2026-05-30
**Spec references:** §11 (T4), §12 (T4 taxonomy), §19 (Gate 2)
**Brief references:** `dev/briefs/gate-2-parser-compiler-verification.md`
**Gate:** 2

## Context

Spec §12 defines eight T4 conflict classes. Gate 2 implements an **initial
subset** (spec §19 "conflict taxonomy initial classes") without `ke-search` and
without a runtime, so classes requiring corpus indexing, source-text comparison,
or scenario execution are deferred. The spec's readiness checklist requires the
chosen classes and severities to be accepted by a domain reviewer before Gate 2
hardens; this ADR records the proposal for that sign-off.

## Decision

Gate 2 implements exactly these four classes, with these default severities
(spec §12 severity levels: Blocking / Review-required / Advisory):

| Class | Severity | Gate 2 detection |
| ----- | -------- | ---------------- |
| `contradictory_outcome` | **Blocking** | Two rules whose applicability scopes overlap produce incompatible decision results for a shared scenario. Detected structurally/bounded over the finite premise space — no SAT. |
| `overlapping_scope` | **Review-required** | Two rules whose applicability conditions can be jointly satisfied (premise-key intersection) with no encoded precedence. |
| `temporal_overlap` | **Review-required** | Two rules with overlapping effective windows `[from,to)` **and** overlapping scope. |
| `duplicate_rule` | **Advisory** | Two rules equal under the semantic normal form (`SemanticRule`) but differing in metadata/source. |

Severities are defaults attached to the class; a future `PolicyBundle` may
override per environment (Gate 4). Each `Conflict` finding carries the spec §12
fields it can populate at Gate 2: rule ids, class, severity, the involved legal
source refs, and a suggested-resolution class. **Counterexample scenario and
trace comparison are left `None`** until the Gate 3 preview runtime exists.

## Consequences

- Desirable: a tractable, reviewable T4 that needs neither `ke-search` nor a
  runtime, yet covers the highest-value conflicts (contradiction, undeclared
  overlap, temporal collision, duplication).
- Undesirable: `contradictory_outcome` is bounded/structural, so it can miss
  contradictions that only manifest on specific scenarios; those surface in Gate
  3 once counterexamples can be generated. This limitation is logged, not hidden.
- Corpus note: corpus rules have **no `effective_to`** (open-ended windows), so
  any two same-scope rules technically overlap temporally; `temporal_overlap` on
  the corpus is therefore expected and Review-required, not Blocking.

## Alternatives considered

- **Implement all eight classes now** — rejected: `source_span_divergence` needs
  `ke-search`; `equivalence_matrix_conflict` needs the equivalence-matrix
  artifact (Gate 4); `obligation_collision` and `missing_precedence` are better
  with a runtime/counterexamples (Gate 3). Spec §19 explicitly scopes Gate 2 to
  "initial classes."
- **Make `duplicate_rule` Blocking** — rejected: duplicates are often intentional
  (regime variants); Advisory with a clear finding is the right default.
