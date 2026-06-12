# 0013. Revocation policy reconciliation (§15) and rollback-target eligibility

**Status:** Accepted (sign-off by Hossain, 2026-06-11)
**Date:** 2026-06-11
**Spec references:** § 15 (runtime selection, pinning, rollback, revocation), § 8.1 (manifest / PolicyBundle), § 21.6 (revocation behavior — open)
**Amends:** Gate 1 IR / canonical encoding (ADR 0002, ADR 0006, ADR 0007, `docs/canonical-encoding.md`); proposes a clarifying amendment to spec § 15.
**Gate:** 4 preparation (must be resolved before Gate 4 Phase 1 per § 23 "Rollback and revocation policies specified").

## Context

Two § 15 problems block Gate 4 and were confirmed by direct inspection of the
code and spec.

**(1) `RevocationPolicy` is a 3-for-3 mismatch against § 15.** The enum frozen in
Gate 1 (`crates/ke-core/src/manifest.rs:83-88`) is:

```rust
pub enum RevocationPolicy {
    HaltImmediately,
    FinishPinnedThenHalt,
    FinishPinnedNoNew,
}
```

Spec § 15 ("Revocation policy options", lines 687-691) names three different
modes:

- **hard stop** — fail any workflow attempting to execute.
- **finish pinned** — allow already-started workflows to finish; block new starts.
- **audit-only** — allow execution; emit high-severity audit event.

The enum **drops audit-only entirely** and **splits finish-pinned** into two
variants (`FinishPinnedThenHalt`, `FinishPinnedNoNew`) that § 15 does not define.
`FinishPinnedNoNew` matches the spec's "finish pinned" ("allow already-started to
finish; block new starts"); `FinishPinnedThenHalt` has no spec basis. The result
is that the as-built artifact cannot even *express* the audit-only posture, which
is a real regulatory behavior (keep executing, but emit a high-severity audit
event) the platform must be able to select. Per spec § 9 the artifact/PolicyBundle
contract must be faithful to § 15 before Gate 4 attests and publishes a
PolicyBundle; this mismatch is therefore a Gate-4 blocker, not cosmetic.

**(2) Rollback has no target-eligibility check.** § 15 line 666 states
"Rollback moves a tag or policy pointer to a previous content hash; it does not
mutate artifact bytes," with no constraint on the target's lifecycle state.
Meanwhile § 15 "Platform behavior by state" (lines 683-685) makes `deprecated`
artifacts ineligible for new workflows and `revoked` artifacts ineligible
entirely. Nothing in § 15 prevents a rollback from re-pointing a live tag at a
`deprecated` (e.g. defect-deprecated) or `revoked` content hash, which would
silently re-admit a withdrawn artifact for new workflow starts — defeating the
deprecation/revocation it was rolled back over. This is a registry-authority gap
(CLAUDE.md: the registry is the only authority that transitions lifecycle state).

This ADR was authored under the AI authority boundary and is **Accepted**
(sign-off by Hossain, 2026-06-11). The canonicalization bump remains sequenced
with the other Gate-4 canon-touching ADRs.

## Decision

**Decision 1 — change the enum to match § 15 (accepted v1).** Replace the
three as-built variants with the three spec-named modes, declared in § 15 order
so the canonical discriminant order is legible:

```rust
/// Revocation behavior for already-running and new workflows (spec § 15).
/// Variant order is the canonical discriminant order; declared in § 15 order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RevocationPolicy {
    /// § 15 "hard stop": fail any workflow attempting to execute.
    HardStop,
    /// § 15 "finish pinned": allow already-started workflows to finish; block new starts.
    FinishPinned,
    /// § 15 "audit-only": allow execution; emit a high-severity audit event.
    AuditOnly,
}
```

The spec is authoritative on operational/legal semantics (CLAUDE.md: "if this
file conflicts with the spec, the spec wins"), and audit-only is a required
behavior that the as-built enum cannot represent. `FinishPinnedNoNew` maps to
`FinishPinned`; `FinishPinnedThenHalt` was an invented mode and is removed.

**Decision 2 — canonicalization version bump.** `RevocationPolicy` is a unit-only,
externally-tagged postcard enum encoded as a declaration-order varint discriminant
(`docs/canonical-encoding.md` line 42; `crates/ke-core/src/schema/defs.rs:408-414`).
Re-ordering and re-naming the variants changes (a) the discriminant that the
`production-eu` PolicyBundle fixture serializes to and (b) the JSON Schema enum
string list. This is a canonical byte-layout change, so the pinned triplet
(`crates/ke-core/src/version.rs`) bumps:

- `ir_schema_version`: `0.3.0` -> `0.4.0`.
- `canonicalization_version`: `ke-canon-3` -> `ke-canon-4`.
- `codec_version`: unchanged (`postcard-1`).

Golden vectors are regenerated under the new triplet via `bin/gen-fixtures`
(`crates/ke-core/tests/round_trip.rs` is the gate). To avoid a per-ADR bump
churn, **this triplet bump is sequenced with the other Gate-4 canon-touching
ADRs** (ADRs 0009-0012 in the Gate-4 brief): the corpus is regenerated once under
the single new triplet that lands at the start of Gate 4 Phase 1.

**Decision 3 — rollback-target eligibility (§ 15 clarifying amendment).** Add to
§ 15 "Rules": *"Rollback may only target a content hash whose current registry
lifecycle state is `published`. Rollback to a `deprecated` or `revoked` hash is
rejected by registry policy; re-instating a deprecated artifact requires an
explicit signed re-publication event, not a rollback. Rollback, like every tag
or policy-pointer move, is a signed, append-only, auditable registry event."*
This is enforced at the Gate-4 registry layer (registry authority), never in the
compiler.

## Consequences

- Desirable: the PolicyBundle contract becomes faithful to § 15, so Gate 4 can
  attest and publish a PolicyBundle that expresses all three documented revocation
  postures, including audit-only.
- Desirable: the rollback-eligibility clause closes a path by which a withdrawn
  artifact could be silently re-admitted for new workflows, preserving the meaning
  of `deprecated`/`revoked`.
- Undesirable: it modifies a Gate-1-frozen shape and bumps the canonicalization
  version, invalidating any consumer pinned to `ke-canon-3`/`0.3.0`. Mitigated by
  the version-triplet bump (a stale consumer rejects rather than misreads) and by
  regenerating all golden vectors atomically. Mitigation cost is shared because
  the bump is sequenced with the other Gate-4 ADRs.
- Migration touch-points to update atomically with the enum change:
  `crates/ke-core/src/manifest.rs:83-88` (enum), `crates/ke-core/src/schema/defs.rs:408-414`
  (`revocation_policy()` enum list), `crates/ke-core/src/examples.rs:271`
  (`RevocationPolicy::FinishPinnedNoNew` -> `FinishPinned`),
  `fixtures/artifacts/policy_production_eu/source.json:20` (`"FinishPinnedNoNew"`
  -> `"FinishPinned"`), the regenerated `fixtures/artifacts/policy_production_eu/canonical.bin`
  and `manifest.json`, the committed `crates/ke-core/schema/ir.schema.json`,
  `crates/ke-core/src/version.rs:50,58`, and the version notes in
  `docs/canonical-encoding.md`.
- Open: which revocation policy is the *default* for a production PolicyBundle
  (spec § 21.6) is a separate decision; this ADR fixes the *vocabulary*, not the
  default. The `production-eu` fixture currently selects finish-pinned and will
  continue to after the rename.

## Alternatives considered

- **Amend § 15 to bless the as-built enum** (HaltImmediately / FinishPinnedThenHalt
  / FinishPinnedNoNew, re-adding audit-only as a fourth) — rejected: it ratifies
  an undocumented mode (`ThenHalt`) into the regulatory contract and grows the
  policy surface with no stated requirement, while still requiring a canon bump to
  add audit-only.
- **Keep the enum, delete audit-only from § 15** — rejected: silently removes a
  documented regulatory behavior; violates "spec wins."
- **Rename only (HardStop/FinishPinned/AuditOnly) but keep old declaration order
  to avoid touching `canonical.bin` bytes** — rejected: leaves a misleading
  discriminant order and still changes the JSON-Schema enum strings and the
  fixture's string value, so a canon bump is unavoidable; declaring in § 15 order
  is clearer and the regen is cheap when sequenced.
- **Enforce rollback eligibility in the compiler** — rejected: lifecycle-state
  transitions and pointer moves are registry authority (CLAUDE.md); the compiler
  judges structural validity only.