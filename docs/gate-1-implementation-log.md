# Gate 1 — implementation log

A phase-by-phase record of the Gate 1 implementation, written as each phase
lands. The authoritative contract is `docs/gate-1-canonical-ir.md` (the brief);
this log records what was actually built, decisions made in flight, and the
verification evidence for each phase. It exists so Hossain's manual gate review
(he owns commits/merges) is fast and auditable.

**Branch:** `migration/gate-0-repo-synthesis` (Gate 1 work; Hossain branches/merges).
**Toolchain:** Rust 1.85.0 (`x86_64-pc-windows-gnu`), pinned by `rust-toolchain.toml`.
**Decisions in effect:** local toolchain verification; synthetic golden fixtures
now, Python-driven cross-corpus bytes deferred until the platform SHA
(`fixtures/rules/SOURCE.md` = `f73b940`) is reconciled with platform `HEAD`.

---

## Phase 0 — ADRs ✅

Three ADRs written in `docs/adr/`, all **Accepted**:

- `0001-jurisdiction-time-zone.md` — Option A: IANA zone name + pinned
  `tz_data_version`; tz database and date resolution deferred to Gate 3.
- `0002-canonical-codec-postcard.md` — postcard; `codec_version = "postcard-1"`,
  with an explicit ordering/normalization profile layered on top.
- `0003-decimal-scalar-representation.md` — numbers are `{ mantissa: i128,
  scale: i8 }`; floats forbidden in the IR; integers at `scale = 0`.

`docs/adr/README.md` index reconciled: the brief's three Gate 1 ADRs take
0001–0003; the previously-speculated registry/key/TSA/sidecar ADRs are marked
"anticipated (Gate 4), numbers assigned when authored."

**Verification:** `cargo check --workspace` green against the empty workspace
(Gate 0 acceptance re-confirmed on the freshly installed toolchain).

---

## Phase 1 — IR types ✅

Module tree under `crates/ke-core/src/ir/` exactly per brief § 2 (`rule`,
`condition`, `decision`, `obligation`, `check`, `source_span`, `time`), plus
`version.rs` (the pinned triplet) and `manifest.rs` (Manifest + PolicyBundle
shapes frozen for Gate 4). Types are `serde`-derived shapes only — no
semantics. Every field is `pub` so the canonical walker lives entirely in
`canonical/`, keeping `ir/` a pure shape layer.

Key shape decisions:
- Ported from the platform **authoring tree** (`src/rules/service.py`), not the
  flattened `RuleIR` in `schemas.py` (that flattening is Gate 2). `CompiledCheck`
  is a documented placeholder, not a field of `RuleIR`.
- Discriminated unions (`ConditionOrGroup`, `DecisionEntry`, `ScalarValue`) are
  **externally tagged** — `serde(untagged)` is incompatible with postcard.
- `ScalarValue::Decimal { mantissa: i128, scale: i8 }`; no float arm (ADR 0003).
- `DecisionEntry` boxes both variants (clippy `large_enum_variant`; boxing is
  serde-transparent).

Deps added: `postcard`, `blake3`, `unicode-normalization` (+ `unicode-normalization`
to the workspace).

**Verification:** `cargo check --lib -p ke-core` green.

## Phase 2 — Canonical encoding ✅

`crates/ke-core/src/canonical/` — `mod.rs` (entry points + `CanonicalError` /
`CanonicalDecodeError`), `encode.rs` (in-place normalize then postcard
serialize), `decode.rs` (postcard decode, reject trailing bytes, re-validate
every invariant), `ordering.rs` (set/map ordering, NFC, decimal canonical form,
date/time-zone validators). Profile: postcard + declaration-order fields, sets
sorted by canonical-encoded element bytes (dup-rejecting), `Option` as
`0x00`/`0x01`, decimals with no trailing zeros, NFC strings, structurally-valid
dates, allow-listed IANA zones. Full prose in `docs/canonical-encoding.md`.

**Verification:** covered by Phase 4 tests (round-trip + non-canonical rejection).

## Phase 3 — JSON Schema emission ✅

`crates/ke-core/src/schema/` — `defs.rs` (29 `$defs`, lexicographically ordered,
`PascalCase` refs, enum values in declaration order), `emit.rs` (fixed top-level
key order, version-pinned `$id`, no clock/env), `mod.rs`. Emitted via
`cargo run -p ke-core --bin emit-schema` into the committed
`crates/ke-core/schema/ir.schema.json`. Determinism relies on `serde_json`'s
`preserve_order` feature (enabled for `ke-core`).

Design choice: a `bin/emit-schema` + determinism test, **not** a `build.rs` that
writes into the source tree (avoids the write-outside-`OUT_DIR` anti-pattern and
makes determinism directly testable).

**Verification:** `tests/schema_determinism.rs` (emit twice identical; committed
file matches fresh emit; top-level key order fixed).

## Phase 4 — Fixtures + tests ✅

- `crates/ke-core/src/examples.rs` — shared synthetic IRs (2 rules + 1 policy)
  exercising nested `all`/`any`, decimals, tag-set sorting, obligations,
  effective window, multiple provenance markers; used by both the generator and
  the tests.
- `crates/ke-core/src/bin/gen-fixtures.rs` — writes the
  `fixtures/artifacts/<id>/{canonical.bin,source.json,manifest.json}` triples
  and the `MANIFEST.md` provenance ledger (deterministic, idempotent).
- `scripts/generate-golden-fixtures.sh` — wraps the generator; `--synthetic`
  (default, Gate 1) and `--platform` (enforces the `SOURCE.md`-SHA guard, then
  reports the Python path is deferred).
- Tests: `round_trip.rs` (encode→decode→re-encode byte-stable; golden files
  stable), `schema_determinism.rs`, `non_canonical.rs` (8 specific rejections),
  `artifact_hash_offset.rs` (offset derivable + zero-then-patch idempotent).

In-flight correction: the round-trip test initially asserted `decode == original`,
which is wrong for non-canonical inputs (the encoder *normalizes*, e.g. sorts
tags). Fixed to assert byte-stability of the encoded fixed point.

**Verification:** `cargo test -p ke-core` → 19 passed. Generator idempotence
confirmed by regenerating and diffing (identical bytes).

## Phase 5 — Docs + verification ✅

- Rewrote `docs/canonical-encoding.md` into the authoritative profile (codec,
  ordering, numeric/string/date rules, rejection table, JSON-Schema determinism,
  fixtures).
- Updated `docs/gate-1-canonical-ir.md` status header + an implementation note
  on the service.py-vs-schemas.py shape choice and synthetic fixtures.
- Added a Gate 1 note to the `docs/attestation-schema.md` placeholder (the
  attestation/policy *enum shapes* are now frozen in `ke-core`).
- Unrelated deployment docs (`EKS …`, `Local Kubernetes …`,
  `Production Enhancements`) were intentionally left untouched — out of Gate 1
  scope.

### Final verification (Gate 1 acceptance, brief § 11)

| Check | Command | Result |
| ----- | ------- | ------ |
| format | `cargo fmt --all -- --check` | clean |
| lint | `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| tests | `cargo test -p ke-core` | 19 passed |
| schema determinism | emit twice + committed-file match | green (`tests/schema_determinism.rs`) |
| fixture idempotence | regenerate + sha256 compare | identical bytes |
| workspace | `cargo check --workspace` | green |

### Deferred / handoff notes for Gate 2+

- Platform-driven cross-corpus golden bytes: deferred until `SOURCE.md`'s SHA
  (`f73b940`) is reconciled with platform `HEAD` (currently `224dcab`). The
  `--platform` script path holds the guard and a clear "not implemented" exit.
- The IANA time-zone allow-list in `canonical/ordering.rs` is corpus-seeded;
  Gate 3 widens it against a pinned tz-data snapshot (ADR 0001).
- `CompiledCheck` and the `Manifest` `artifact_hash` patch are shape-only; Gate 2
  fills check semantics, Gate 4 wires BLAKE3 authority.
