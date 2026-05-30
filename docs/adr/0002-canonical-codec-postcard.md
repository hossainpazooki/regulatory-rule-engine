# 0002. Canonical wire codec — postcard

**Status:** Accepted
**Date:** 2026-05-30
**Spec references:** § 8 (artifact contract), § 8.3 (golden test vectors)
**Brief references:** `docs/gate-1-canonical-ir.md` § 4.1
**Gate:** 1

## Context

Canonical bytes are the input to BLAKE3 content addressing (Gate 4) and the
basis of the cross-language Rust ⇄ Python round-trip. The codec must be
deterministic: no map-order ambiguity, no float reformatting, no
implementation-defined field ordering. The choice is load-bearing because
`codec_version` travels in the manifest and a change to it is a breaking
artifact-format change.

Candidates:

- **postcard** — a `serde`-based, schema-led, length-prefixed binary format.
  Field order follows the Rust struct declaration order deterministically; there
  is no self-describing map-key reordering to police; it has no float-canonical
  pitfalls because we forbid floats in the IR (ADR 0003). Already present in the
  workspace `Cargo.toml`.
- **canonical CBOR** — self-describing and widely supported, but canonical-CBOR
  ordering rules (deterministic map key ordering, shortest-form integers,
  preferred float encodings) are subtle; enforcing them faithfully would mean
  re-implementing half of RFC 8949 § 4.2 and auditing it for cross-language
  agreement.
- **bespoke encoder** — maximal control, maximal maintenance and audit burden.

## Decision

Adopt **postcard** as the v1 canonical codec. `codec_version = "postcard-1"`.

The canonical profile (`docs/canonical-encoding.md`) layers explicit ordering
and normalization rules *on top of* postcard rather than relying on postcard's
defaults alone: struct field order = declaration order (§ 4.2); sets/maps are
encoded as length-prefixed sequences sorted by canonical-encoded key/element
bytes (§ 4.3–4.4); `Option` is `0x00` / `0x01 + payload` (§ 4.5); integers are
fixed-width per declared type, no varints in audited positions (§ 4.6); strings
are NFC-validated (§ 4.7). These rules are enforced and re-checked on decode so
non-canonical bytes are rejected with specific errors.

## Consequences

- Desirable: deterministic by construction; field order is the struct order, so
  the schema is the single source of layout truth; no float canonicalization
  problem; minimal new dependencies.
- Desirable: the explicit profile layer means the contract is documented and
  testable independent of the postcard version.
- Undesirable: postcard is **not** self-describing, so a decoder must know the
  exact schema/version triplet — enforced via the manifest version fields and a
  strict decoder. Cross-language consumers target the JSON Schema (ADR-adjacent,
  brief § 5), not the raw bytes, and use a generated/parallel decoder.
- Undesirable: reordering a struct field is a `canonicalization_version` bump.
  This is called out explicitly in `docs/canonical-encoding.md` and guarded by a
  schema-hash test (brief § 12).

## Alternatives considered

Canonical CBOR was rejected for the ordering-rule complexity and cross-language
audit cost. A bespoke encoder was rejected as unjustified maintenance burden
when postcard plus a thin profile layer meets every determinism requirement.
