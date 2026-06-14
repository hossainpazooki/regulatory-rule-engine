# 0005. T4 conflict classes and severities for Gate 2

**Status:** Accepted (domain-reviewer sign-off by Hossain, 2026-05-30)
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
hardens; this ADR records that **accepted** policy.

**Sign-off rationale (Hossain, 2026-05-30):** contradictory executable outcomes
must block; scope/temporal overlap without encoded precedence requires review;
semantic duplicates are advisory hygiene unless they produce divergent behavior.
The future `PolicyBundle` per-environment override path (Gate 4) is retained.

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

---

## Amendment (Proposed, 2026-06-11): pin the "shared scenario" definition

**Status:** Proposed — needs domain-reviewer sign-off (the original policy block
above stays Accepted and unchanged).
**Spec references:** §12; CLAUDE.md authority boundary ("compiler: structural
validity only; never legal truth").

### Context

The accepted policy says `contradictory_outcome` fires when overlapping-scope
rules "produce incompatible decision results **for a shared scenario**." The
Gate-2 implementation did not enforce the *shared scenario* clause: `contradiction()`
reported a conflict whenever two scope-touching rules had a pair of mutually
consistent decision paths with different result strings — and "mutually
consistent" was satisfied **vacuously** when the two paths shared no branch
condition at all. Over the clean 34-rule corpus this produced **52 Blocking
`contradictory_outcome` findings** between rules that merely coincide on a broad
applicability premise (e.g. `target_jurisdiction == UK`) while branching on
disjoint variables and answering unrelated legal questions (e.g. an authorization
result vs a token-classification result). `verify().has_blocking()` was therefore
true for the clean corpus, making `draft -> structurally_verified` (spec §9)
unreachable. Reporting such pairs as contradictions is the compiler asserting
legal incompatibility between different questions — a legal judgment it must not
make.

### Decision

A `contradictory_outcome` requires a **structurally witnessed shared scenario**:
two decision paths that are (a) mutually consistent (no shared branch condition
assigned both ways) **and** (b) co-extensive — they share at least one decision-
branch variable, **or** both rules are leaf-only so the caller-verified
applicability-scope overlap alone fixes the scenario. Vacuous consistency between
paths that constrain disjoint variables is **not** a shared scenario.

Pairs that lose `ContradictoryOutcome` under this definition but still have
overlapping applicability are reported as `OverlappingScope` (Review-required),
exactly the fallback the Consequences section already anticipates.

### Consequences

- The clean corpus yields **zero** `contradictory_outcome` and is structurally
  publishable (`has_blocking() == false`); the 52 former false positives are now
  `OverlappingScope` (Review-required). Regression-guarded by
  `crates/ke-compiler/tests/t4_corpus.rs`; real detection preserved by the
  unchanged `tests/fixtures/conflicts/contradictory.yaml`.
- Bounded-detection trade-off (extends the existing limitation note): requiring a
  shared branch variable can miss a contradiction between a **leaf-only** rule and
  a **branched** rule over the same scope. This is left to Gate-3 counterexample
  generation, consistent with the original "bounded/structural, can miss
  scenario-specific contradictions" consequence. Logged, not hidden.

### Alternatives considered

- **Require a shared branch variable in all cases** — rejected: `contradictory.yaml`
  (and any genuinely-unconditional contradiction) is leaf-only with no branch
  variables, so this would disable real detection and break the positive fixture.
- **Full joint-satisfiability / SAT over the premise space** — rejected for Gate 2
  (the accepted policy is explicitly "structural/bounded — no SAT"); the
  precise-scenario recovery belongs to Gate-3 counterexamples.
