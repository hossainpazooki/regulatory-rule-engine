# 0003. Decimal scalar representation — mantissa/scale

**Status:** Accepted
**Date:** 2026-05-30
**Spec references:** § 8 (artifact contract)
**Brief references:** `docs/gate-1-canonical-ir.md` § 4.6, § 9.4, § 12
**Gate:** 1

## Context

Rule conditions compare against numeric values — thresholds, quantities,
percentages, holder counts, market-cap limits. In the Python source these arrive
as `Any` (`int`, `float`, `str`, `bool`, `list`) on `ConditionSpec.value`
(`src/rules/service.py`). Floats cannot appear in a *canonical* encoding: IEEE-754
reformatting and rounding differ across languages and break byte-stable hashing
and cross-language parity. A regulatory threshold like "EUR 5,000,000" or a fee
of "0.20%" must encode and compare exactly.

## Decision

Numbers in the IR are **decimal scalars** represented as
`{ mantissa: i128, scale: i8 }`, denoting the value `mantissa × 10^(-scale)`.

- Floats are **forbidden** in the IR. The encoder rejects any float-shaped input
  with a specific error.
- Integers encode as `scale = 0` (e.g. `0` → `{ mantissa: 0, scale: 0 }`,
  `5_000_000` → `{ mantissa: 5000000, scale: 0 }`).
- Decimal fractions encode with the matching scale
  (e.g. `0.20` → `{ mantissa: 20, scale: 2 }`).
- The encoder rejects values whose magnitude overflows `i128` or whose scale
  exceeds the declared precision.

`ScalarValue` (in `crates/ke-core/src/ir/condition.rs`) is the typed sum
`Str(String) | Bool(bool) | Decimal { mantissa, scale } | List(Vec<ScalarValue>)`.
The `List` arm carries `in` / `not_in` operands.

## Consequences

- Desirable: exact, language-neutral numeric encoding; byte-stable hashing;
  no float canonicalization problem (this is also why ADR 0002's postcard choice
  is clean).
- Desirable: `i128` mantissa covers every magnitude in the regulatory corpus
  (holder counts, EUR billions, basis points) with room to spare.
- Undesirable: the Python boundary must use `decimal.Decimal` with the recorded
  `scale`, never `float`. Float coercion at the boundary is a contract violation
  that must produce a specific error. This is pinned by a Gate 4 cross-language
  contract test (brief § 12).
- Undesirable: Gate 2's YAML→AST lowering must parse numeric literals into
  mantissa/scale rather than `f64`; a literal like `5e6` is normalized to
  `{ 5000000, 0 }` at parse time, not stored as a float.

## Alternatives considered

- **`f64` in the IR** — rejected outright: non-deterministic across languages,
  defeats canonical hashing.
- **Arbitrary-precision rational (`num-rational`)** — rejected as overkill;
  regulatory values are decimal, not arbitrary rationals, and a big-rational type
  complicates the JSON Schema and the Python boundary.
- **Decimal-as-string** (`"0.20"`) — rejected: pushes parsing/normalization onto
  every consumer and reintroduces ambiguity (`"0.20"` vs `"0.2"` vs `".2"`),
  which the mantissa/scale pair eliminates.
