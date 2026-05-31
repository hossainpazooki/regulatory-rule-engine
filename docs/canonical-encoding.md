# Canonical encoding profile

**Status:** Authored in Gate 1. Authoritative for `ke-core` canonical bytes.
**Spec references:** § 8 (artifact contract), § 8.3 (golden test vectors),
§ 8.4 (effective dates and jurisdiction time).
**ADRs:** [0001 jurisdiction time-zone](adr/0001-jurisdiction-time-zone.md),
[0002 codec — postcard](adr/0002-canonical-codec-postcard.md),
[0003 decimal scalars](adr/0003-decimal-scalar-representation.md).

This document specifies the deterministic encoding `ke-core` applies before the
bytes are content-addressed with BLAKE3 (wired in Gate 4 `ke-artifact`). Every
byte the encoder emits is a pure function of the input value plus the pinned
**version triplet** — no wall-clock time, no environment, no map iteration
order, no floating-point reformatting.

Implementation: `crates/ke-core/src/canonical/` (`encode.rs` normalizes,
`decode.rs` re-validates, `ordering.rs` holds the shared rules). Entry points:
`encode_rule` / `decode_rule`, `encode_policy` / `decode_policy`,
`encode_manifest` / `decode_manifest`.

## Version triplet

Reproduced in the manifest (§ 8.1) and in every decode error so a mismatch is
immediately diagnosable. Defined in `crates/ke-core/src/version.rs`:

| Field | Value | Meaning |
| ----- | ----- | ------- |
| `ir_schema_version` | `0.2.0` | IR shape version (semver). Bumped from `0.1.0` in Gate 2 — ADR 0006 made `effective_window` optional. |
| `codec_version` | `postcard-1` | Wire codec (ADR 0002). |
| `canonicalization_version` | `ke-canon-2` | This profile's version. Bumped from `ke-canon-1` in Gate 2 (ADR 0006 changed the `effective_window` byte layout). |

**Any change to the byte layout — including reordering a struct field — is a
breaking change that requires bumping `canonicalization_version`.**

## Wire codec

**postcard** (ADR 0002): `serde`-driven, length-prefixed, not self-describing.
Field order in the canonical encoding is the **declaration order of the Rust
struct** (§ 4.2). The decoder therefore must know the exact schema/version;
non-canonical detection happens on top of postcard via the rules below.

`serde` enums are **externally tagged** (the postcard-friendly default;
`#[serde(untagged)]` is forbidden because postcard cannot drive `deserialize_any`).
This applies to `ConditionOrGroup`, `DecisionEntry`, `ScalarValue`, and the
provenance/policy enums.

## Encoding rules

### Field ordering (§ 4.2)
Struct field order = declaration order. Part of the contract; reordering bumps
`canonicalization_version`.

### Set ordering (§ 4.4)
Sequences that represent **sets** (rule `tags`, policy attestation sets) are
sorted by the **lexicographic byte order of each element's canonical postcard
encoding**, and duplicates are rejected. `in` / `not_in` operand lists are
*ordered sequences*, not sets — author order is preserved.

### Map ordering (§ 4.3)
Maps (e.g. the policy `minimum_attestation_count_per_type`) are encoded as
length-prefixed sequences of entries sorted by the canonical encoding of the
key. Same rule as sets; duplicates rejected.

### Optional fields (§ 4.5)
`Option<T>::None` → one `0x00` byte; `Some(x)` → `0x01` followed by canonical
`x` (postcard-native). Every field is present in the byte stream — no
missing-key sentinel. Empty string is distinct from `None`.

### Numeric representation (§ 4.6, ADR 0003)
Integers are fixed-width per declared type. **Floats are not representable** in
the IR. All rule numbers are exact decimals `mantissa × 10^(-scale)`
(`ScalarValue::Decimal { mantissa: i128, scale: i8 }`). Canonical decimal form:
non-negative scale, no trailing zeros, and `mantissa == 0 ⇒ scale == 0`. The
encoder folds negative scale into the mantissa and strips trailing zeros (e.g.
`{200, 3}` → `{20, 2}`, `{5, 0}` stays); overflow during folding is an error.

### String normalization (§ 4.7)
All strings are **NFC-normalized UTF-8**. The encoder rejects non-NFC input;
the decoder rejects non-NFC bytes.

### Dates and jurisdiction time (§ 4.8, ADR 0001)
`JurisdictionDate { year: i16, month: u8, day: u8 }` — no timestamps in the IR.
Structural validation only: month 1–12, day 1–31, year ≥ 1900 (calendar
correctness and the closed-open `[from, to)` window are Gate 3 runtime
concerns). Time zone is an IANA name + pinned `tz_data_version` on
`EffectiveWindow`; the encoder rejects zones outside its allow-list (seeded
from the corpus; widened in Gate 3 against a pinned tz-data snapshot).

## Decoding and rejection of non-canonical bytes (§ 8.3)

`decode_*` postcard-decodes, rejects trailing bytes, then re-validates every
invariant above. Each violation maps to a specific `CanonicalDecodeError`
variant so non-canonical input is diagnosable:

| Violation | Error variant |
| --------- | ------------- |
| bytes left after the payload | `TrailingBytes` |
| set/map not in canonical order | `UnsortedSet` |
| duplicate set/map element | `DuplicateSetElement` |
| non-NFC string | `NonNfcString` |
| out-of-range date | `InvalidDate` |
| time zone outside the allow-list | `UnknownTimeZone` |
| decimal with trailing zeros / negative scale | `NonCanonicalDecimal` |
| malformed postcard | `Codec` |

Note: postcard is positional, so "wrong struct field order" is not a
byte-detectable mutation — field order is enforced *structurally* by the schema
and `canonicalization_version`, not by a decode check. "Missing version field"
is a manifest-level concern (the version triplet lives in `Manifest`, validated
by `decode_manifest`).

## JSON Schema determinism (§ 5 of the brief, spec § 8)

`ke-core` emits a deterministic JSON Schema (the authoritative shape for
downstream model generation). Rules:

1. Top-level keys in fixed order: `$schema`, `$id`, `title`, `description`,
   `type`, `properties`, `required`, `additionalProperties`, `$defs`.
2. `properties` / `required` in Rust field declaration order.
3. `$defs` ordered lexicographically by name; reference names are `PascalCase`
   of the Rust type (nested `Outer_Inner`).
4. Enum values in declaration order (matching the canonical discriminant order),
   not alphabetically.
5. No timestamps or environment data in `$id` / `description`; schema metadata
   version = `ir_schema_version`.

Emission is a pure function (`schema::emit_schema_string`). The committed
`crates/ke-core/schema/ir.schema.json` is regenerated by
`cargo run -p ke-core --bin emit-schema`; CI fails on any `git diff`. Determinism
relies on `serde_json`'s `preserve_order` feature.

## Golden fixtures (§ 8.3)

Golden artifacts live under `fixtures/artifacts/<id>/` as a triple:
`canonical.bin` (authoritative bytes), `source.json` and `manifest.json`
(regenerated review views). Generated by
`scripts/generate-golden-fixtures.sh` (synthetic mode for Gate 1; see
`fixtures/artifacts/MANIFEST.md` for the provenance ledger). The generator is
idempotent — re-running produces byte-identical output. The platform-driven
cross-corpus path is deferred until the recorded `fixtures/rules/SOURCE.md` SHA
is reconciled with the platform `HEAD`.
