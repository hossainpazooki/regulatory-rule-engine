# 0001. Jurisdiction time-zone representation in the IR

**Status:** Accepted
**Date:** 2026-05-30
**Spec references:** § 8.4 (effective dates and jurisdiction time)
**Brief references:** `docs/gate-1-canonical-ir.md` § 6, § 9.6
**Gate:** 1

## Context

Regulatory effective windows are *legal-date* concepts, not UTC instants
(spec § 8.4). A scenario timestamp must be converted into the rule's
jurisdiction-local zone before the closed-open window `[effective_from,
effective_to)` is evaluated. Gate 1 freezes the IR shape that carries the zone;
Gate 3 (`ke-runtime`) implements the actual date-window resolution.

Two representations were considered (brief § 6):

- **Option A — IANA zone string** (`"Europe/Berlin"`): future-proof to DST and
  historical rule changes; matches Python `zoneinfo`. Cost: a Rust IANA tz
  database is needed at resolution time, and embedding tz data into the WASM
  bundle adds weight.
- **Option B — fixed offset + tag** (`{ offset_minutes, tz_tag }`): no tz-database
  dependency, trivially WASM-portable. Cost: a fixed offset is wrong for half
  the year in any DST zone; legal effective windows occasionally land on real
  DST edge cases (e.g. Brazil's DST history).

## Decision

Adopt **Option A**. The IR stores the IANA zone name as a string plus a pinned
`tz_data_version`. The encoder refuses to encode an unknown zone name. Gate 1
stores *only* the name and the version — it does **not** embed a tz database and
does **not** resolve dates. The embedded tz-data snapshot and resolution logic
land in Gate 3 alongside `ke-runtime`; `tz_data_version` is recorded in the
artifact manifest so a consumer on a mismatched snapshot rejects rather than
silently re-resolving (brief § 9.6).

`TimeZone` is therefore `{ name: String, tz_data_version: String }` in
`crates/ke-core/src/ir/time.rs`.

## Consequences

- Desirable: faithful to legal dates and DST; cross-language parity with Python
  `zoneinfo`; the WASM target stays lean because Gate 1 carries no tz database.
- Desirable: version pinning makes tz-data drift a hard, diagnosable error
  rather than a silent behavioral change.
- Undesirable: Gate 3 owes a Rust IANA database (or a pinned snapshot under
  `crates/ke-core/tzdata/`) and a validation list of accepted zone names. Until
  then, Gate 1's "refuse unknown zone" check validates against a small allow-list
  seeded from the corpus, widened in Gate 3.
- Undesirable: a Python consumer must use the recorded `tz_data_version`; a
  mismatch is a runtime error, not a fallback (brief § 9.6).

## Alternatives considered

Option B (fixed offset + tag) was rejected: it cannot represent DST or
historical zone changes, and legal effective windows do hit those edges. The
WASM-size argument that favors B is addressed under A by deferring the tz
database out of `ke-core` until Gate 3.
