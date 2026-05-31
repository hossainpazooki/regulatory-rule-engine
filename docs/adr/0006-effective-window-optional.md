# 0006. `effective_window` is optional (always-effective rules)

**Status:** Accepted
**Date:** 2026-05-30
**Spec references:** §8.4 (effective dates), §19 (Gate 2)
**Amends:** Gate 1 IR (ADR 0001, `docs/canonical-encoding.md`)
**Gate:** 2 (discovered during parser/lowering)

## Context

Gate 1 modeled `RuleIR.effective_window` as a **required** `EffectiveWindow`.
Gate 2 lowering of the real corpus found that legitimate rules may carry **no
effective window at all**: `fixtures/rules/fca_crypto.yaml`'s five rules have no
`effective_from`/`effective_to`, matching the platform's `Rule.effective_from:
date | None` (an unspecified window means "always effective"). A mandatory
`effective_window` cannot represent these rules, so lowering would either reject
them (breaking the "every YAML rule" differential) or invent a date.

This is the §20 / Gate-2-checkpoint case: an IR gap that must be reconciled in
the open, not papered over in lowering. Hossain chose to amend the IR rather
than synthesize a sentinel date.

## Decision

`RuleIR.effective_window` becomes `Option<EffectiveWindow>`. `None` means the
rule states no effective window (always effective); `Some(w)` is unchanged. This
amends the Gate 1 freeze, so the pinned version triplet bumps:

- `ir_schema_version`: `0.1.0` → `0.2.0` (schema field becomes nullable, dropped
  from `required`).
- `canonicalization_version`: `ke-canon-1` → `ke-canon-2` (the canonical byte
  layout changes — the field now carries an `Option` presence byte).
- `codec_version`: unchanged (`postcard-1`).

Gate 1's committed JSON Schema and golden fixtures are regenerated under the new
triplet. Lowering emits `None` when the YAML has no effective dates; when dates
are present it builds `Some(EffectiveWindow{..})`, defaulting the (YAML-absent)
`jurisdiction_time_zone` to `UTC` as a Gate-2 placeholder — proper
jurisdiction→zone resolution is Gate 3 (`ke-runtime`) and the zone is not part of
the semantic-equivalence comparison.

## Consequences

- Desirable: the IR faithfully represents always-effective rules; Rust `None`
  matches the platform's `None` directly, so the semantic normal form compares
  cleanly without a sentinel convention.
- Desirable: keeps the change small and auditable — one field, one version bump,
  regenerated artifacts.
- Undesirable: it modifies a Gate-1-frozen shape. Mitigated by the version-triplet
  bump (any consumer on `0.1.0`/`ke-canon-1` rejects rather than silently
  misreads) and by regenerating all Gate 1 golden vectors atomically.
- Manifest-level `effective_from` (artifact effective range) is **unchanged** —
  this ADR is about rule-level windows only.

## Alternatives considered

- **Sentinel default in lowering** (`effective_from = 1900-01-01`) — rejected:
  invents source data and forces the semantic form to special-case the sentinel.
- **Flag + exclude from parity** — rejected: breaks the "every YAML rule"
  differential and leaves `fca_crypto` permanently uncovered.
