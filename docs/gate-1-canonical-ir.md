# Gate 1 — Canonical IR and artifact foundation

**Status:** implemented in `crates/ke-core/` — see
[`gate-1-implementation-log.md`](gate-1-implementation-log.md) for the
phase-by-phase record and verification evidence. Merged to `main` (see the
status table in [docs/STATUS.md](STATUS.md); Gates 1–3 have long been on `main`).
**Authoritative spec sections:** § 5, § 6, § 8, § 8.3, § 8.4, § 11, § 19 (Gate 1), § 22 (Gate 1 brief outline).
**Predecessor:** Gate 0 (repo synthesis) — green pending CI confirmation.
**Successor:** Gate 2 (parser, compiler, T0/T1/T4 verification).

> **Implementation note (2026-05-30):** The IR was ported from the platform's
> *authoring tree* in `src/rules/service.py` (`Rule`, `ConditionGroupSpec`,
> `DecisionNode`, `DecisionLeaf`, …), not the flattened jump-table `RuleIR` in
> `src/production/schemas.py`. The flattening in `compiler.py` is Gate 2's
> AST→IR lowering (a Gate 1 non-goal), so Gate 1 freezes the un-lowered tree
> shapes. Golden fixtures are synthetic (Rust-authored); the platform-driven
> cross-corpus path is deferred until `fixtures/rules/SOURCE.md`'s recorded SHA
> (`f73b940`) is reconciled with the platform `HEAD`.

This brief defines the contract Gate 1 implementation must honour. No compiler
behaviour, runtime, signing, or platform binding is built here. Gate 1
freezes data shapes so every later gate has a stable target.

---

## 1. Design principles

1. **Determinism over convenience.** Every byte the compiler emits must be a
   pure function of the input AST plus pinned version triplet
   (`ir_schema_version`, `codec_version`, `canonicalization_version`). No
   wall-clock time, no environment variables, no map iteration order, no
   floating-point reformatting in the encode path.
2. **Authority separation visible in the type system.** Candidate, structurally
   verified, attested, and published rules are distinguishable at the IR layer
   — never by a string field that can be flipped. Gate 1 introduces the typed
   markers; transition logic lands in Gate 4.
3. **Source-span preservation is mandatory.** Every IR node that represents a
   decision node, obligation, threshold, exception, or discretionary term
   carries a `SourceSpan` reference. Gate 2 enforces coverage; Gate 1 makes
   the carrier shape exist so coverage is testable.
4. **Jurisdiction-time is a first-class concept.** Effective windows are not
   UTC instants. The IR encodes `effective_from`, `effective_to`, and
   `jurisdiction_time_zone` with a closed-open semantic explicitly stated in
   the schema docstring. See § 8.4.
5. **Cross-language parity is by canonical bytes, not by struct field
   matching.** The PyO3 binding consumes canonical bytes from `ke-artifact`,
   not Rust struct layout. Therefore Rust-side struct names and casing can
   differ from Python expectations as long as canonical encoding agrees.
6. **Schema-first, types-second.** The JSON Schema emitted by `ke-core` is the
   authoritative documentation for downstream consumers (platform model
   generation, frontend type generation, fixture validators). Rust types are
   one of the encoders that target it, not the source of truth.
7. **No premature semantics.** Gate 1 ports `RuleIR`, `CompiledCheck`,
   `DecisionEntry`, `ObligationSpec`, `ConditionGroupSpec`, `DecisionNode`,
   `DecisionLeaf` as shapes. It does not normalize, fold, or simplify them.

### Non-goals (explicit)

- No YAML parser or AST. Gate 2.
- No AST→IR lowering. Gate 2.
- No compiler verification (T0/T1/T4). Gate 2.
- No preview executor. Gate 3.
- No signing, attestations, registry, content addressing as production
  surfaces. Gate 4. (Canonical encoding is defined and tested at the byte
  level; BLAKE3 wrapping happens in `ke-artifact` in Gate 4.)
- No PyO3 binding. Gate 4.
- No WASM. Gate 5.
- No frontend rewire. Gate 5.

---

## 2. IR types — initial port list

Ported from `institutional-defi-platform-api/src/production/{compiler.py,schemas.py}`
into `crates/ke-core/src/ir/`. Module layout:

```text
crates/ke-core/src/
├── lib.rs
├── ir/
│   ├── mod.rs
│   ├── rule.rs              # RuleIR, ProvenanceMarker
│   ├── condition.rs         # ConditionGroupSpec, Condition, Operator
│   ├── decision.rs          # DecisionNode, DecisionLeaf, DecisionEntry
│   ├── obligation.rs        # ObligationSpec
│   ├── check.rs             # CompiledCheck (T0/T1 artifact placeholder)
│   ├── source_span.rs       # SourceSpan, DocumentRef
│   └── time.rs              # JurisdictionDate, EffectiveWindow, TimeZone
├── canonical/
│   ├── mod.rs               # encode/decode entrypoints
│   ├── encode.rs            # canonical encoder
│   ├── decode.rs            # strict decoder (rejects non-canonical)
│   └── ordering.rs          # field-/map-/set-ordering rules
├── schema/
│   ├── mod.rs               # JSON Schema generator entrypoint
│   ├── emit.rs              # deterministic emit
│   └── defs.rs              # $defs ordering + reference naming
└── version.rs               # ir_schema_version / codec_version / canonicalization_version
```

`crates/ke-core/schema/ir.schema.json` is the generated artifact (committed
under `crates/ke-core/schema/` and rebuilt by the build script on demand).

### Required field set (no semantics yet, just shape)

`RuleIR` carries: `rule_id`, `rule_version`, `description?`, `tags?`,
`applies_if?: ConditionGroupSpec`, `decision_tree: DecisionEntry`,
`obligations: [ObligationSpec]`, `source: DocumentRef`,
`interpretation_notes?`, `effective_window: EffectiveWindow`,
`provenance: ProvenanceMarker`.

`ConditionGroupSpec`: discriminated union (`all`/`any`) of
`Condition | ConditionGroupSpec`.

`Condition`: `field`, `operator` (typed enum, see ordering rules below),
`value` (typed sum: string | number | bool | list).

`DecisionEntry`: discriminated union `DecisionNode | DecisionLeaf`.
`DecisionNode`: `node_id`, `condition: Condition`, `true_branch:
DecisionEntry`, `false_branch: DecisionEntry`, `source_span?`.
`DecisionLeaf`: `result`, `obligations?`, `notes?`, `source_span?`.

`ObligationSpec`: `id`, `description?`, `deadline?`, `source_span?`.

`SourceSpan`: `document_id`, `article?`, `section?`, `paragraph?`,
`pages?: [u32]`, `byte_range?: {start, end}`, `text_hash?` (BLAKE3 of the
referenced text segment — populated only when legal source storage decision
is resolved; structurally optional in Gate 1).

`EffectiveWindow`: `effective_from: JurisdictionDate`,
`effective_to?: JurisdictionDate`, `jurisdiction_time_zone: TimeZone`,
`effective_time_policy?: EffectiveTimePolicy` (free-form key + version, see
§ 8.4 — required only when a regime declares a non-standard convention).

`ProvenanceMarker`: a typed marker enum used to make the candidate vs
attested distinction explicit even in `ke-core`. Values:
`Candidate { proposal_id?: String }`,
`StructurallyVerified`,
`MlChecked { policy_version: String }`,
`ExpertAttested { attestation_count: u16 }`,
`Published { environment: String }`,
`Deprecated`,
`Revoked`. Gate 1 defines the enum; Gate 4 enforces state-machine
transitions and binds attestations.

### Operator enum (closed)

`Eq`, `NotEq`, `In`, `NotIn`, `Gt`, `Lt`, `Gte`, `Lte`, `Exists`. Canonical
JSON serialization uses the YAML form (`"=="`, `"!="`, `"in"`, `"not_in"`,
`">"`, `"<"`, `">="`, `"<="`, `"exists"`) for round-trip with the existing
corpus. No regex, no fuzzy operator strings.

---

## 3. Artifact manifest draft

`ke-artifact::Manifest` lands in Gate 4, but its **shape** is frozen in Gate 1
so canonical encoding tests can be written. Per spec § 8.1:

```rust
pub struct Manifest {
    pub artifact_kind: ArtifactKind,       // RegimePack | EquivalenceMatrix | TestCorpus | PolicyBundle
    pub artifact_hash: [u8; 32],           // BLAKE3 of canonical bytes; self-referential, computed last
    pub regime_id: String,
    pub effective_from: JurisdictionDate,
    pub effective_to: Option<JurisdictionDate>,
    pub compiler_version: SemVer,
    pub compiler_build_hash: [u8; 32],
    pub ir_schema_version: SchemaVersion,
    pub codec_version: CodecVersion,
    pub canonicalization_version: CanonicalizationVersion,
    pub corpus_root_hash: [u8; 32],
    pub source_corpus_hash: [u8; 32],
    pub attestation_policy_version: String,
}
```

Self-referential `artifact_hash` field handling: encoded with all 32 zero
bytes during the first encode pass; BLAKE3 is computed over the resulting
bytes; the hash bytes are then patched into the manifest position at a
fixed byte offset. Decode verifies by zeroing the same bytes before
recomputing. The fixed-offset trick requires the manifest to be the first
field of `Artifact` and `artifact_hash` to be the first variable-length-free
field after a known prefix. This is a Gate 4 implementation concern;
Gate 1 only needs to define the field order so the offset is determinable.

---

## 4. Canonical serialization profile

Canonical bytes are the input to BLAKE3 (Gate 4) and the basis of the
cross-language round-trip (Rust ⇄ Python). Profile choices for Gate 1:

### 4.1 Wire codec

**postcard** (already in `Cargo.toml` workspace deps). Rationale: schema-led,
length-prefixed, no map-order ambiguity, no float reformatting risk, has
working PyO3-friendly counterparts. Alternative considered: CBOR — rejected
because canonical-CBOR ordering rules are subtle and we would re-implement
half the spec to enforce them.

`codec_version` is `"postcard-1"` for v1.

### 4.2 Field ordering

Struct field order in the canonical encoding is the **declaration order in
the Rust struct**. The declaration order is part of the contract and a
breaking change requires bumping `canonicalization_version`. Documented in
`docs/canonical-encoding.md`.

### 4.3 Map ordering

Maps are encoded as length-prefixed sequences of `(key, value)` pairs sorted
by **lexicographic byte order of the canonical-encoded key**. We do not rely
on the encoder's `BTreeMap` because we want the rule to be explicit at the
profile layer.

### 4.4 Set ordering

Sets (e.g., `tags`) are encoded as sequences sorted by canonical-encoded
element bytes. Duplicates are a decode error.

### 4.5 Optional fields

`Option<T>::None` is encoded as a single zero byte; `Option<T>::Some(x)` as
`0x01` followed by canonical `x`. No "missing key" sentinel — every field
is present in the byte stream.

### 4.6 Numeric representation

Integers: signed/unsigned fixed-width per declared type. Variable-length
varints are **not** used in v1 because they make cross-language equivalence
harder to audit. Floats: forbidden in the IR. All "numbers" that appear in
rules (thresholds, quantities) are decimal scalars represented as
`{ mantissa: i128, scale: i8 }`. Scale is fixed by the regime's declared
precision; the encoder rejects values that overflow the declared scale.

### 4.7 String normalization

All strings are **NFC-normalized UTF-8**. The encoder rejects non-NFC input
with a specific error. Empty strings are distinct from `None` and round-trip
as such.

### 4.8 Date and time representation

`JurisdictionDate` is `{ year: i16, month: u8, day: u8 }`. No timestamps in
the IR. Time-zone is a separate field on `EffectiveWindow`. The closed-open
semantic `[effective_from, effective_to)` is enforced only at runtime
(Gate 3); Gate 1 encodes the fields and validates structural correctness
(month 1–12, day 1–31, year ≥ a sentinel like 1900).

### 4.9 Versioning fields

`ir_schema_version` (semver-ish: `major.minor.patch`), `codec_version`
(opaque string), `canonicalization_version` (opaque string). All three are
present in the manifest and reproduced in any artifact-decode error so
mismatches are immediately diagnosable.

---

## 5. JSON Schema determinism rules

The JSON Schema is emitted by a `crates/ke-core/build.rs` build script (or
an idempotent `cargo run --bin ke-core-emit-schema`) into
`crates/ke-core/schema/ir.schema.json`. Determinism rules:

1. **Top-level keys** in fixed order: `$schema`, `$id`, `title`,
   `description`, `type`, `properties`, `required`, `additionalProperties`,
   `$defs`.
2. **`properties` ordering** matches Rust field declaration order (same rule
   as canonical encoding §4.2).
3. **`required` ordering** matches `properties` order, filtered.
4. **`$defs` ordering** is lexicographic by definition name.
5. **Reference naming** uses `PascalCase` of the Rust type name. Nested
   types use `Outer_Inner`.
6. **Enum representation** uses `{ "enum": [v1, v2, ...] }` with values
   ordered by appearance in the Rust source (declaration order), **not**
   alphabetically — to match canonical encoding's field-order rule.
7. **No timestamps, no environment data** in `$id` or `description`.
8. **Schema metadata version** is `ir_schema_version` exactly, so generated
   schemas and runtime artifacts agree.
9. **CI determinism gate:** the schema is regenerated in CI and `git diff
   --exit-code crates/ke-core/schema/ir.schema.json` must be clean. Schema
   drift is a hard failure.

---

## 6. Effective-date and jurisdiction-time model — options

Spec § 8.4 mandates jurisdiction-local dates plus an explicit zone. Two
implementation options:

| Option | Representation | Pros | Cons |
|--------|----------------|------|------|
| **A. IANA zone string** | `jurisdiction_time_zone: String` (`"Europe/Berlin"`) | Trivial; future-proof to DST rules; matches Python `zoneinfo`. | Requires a Rust IANA database in `ke-runtime` (Gate 3); embedding tz data into WASM bloats the bundle. |
| **B. Fixed offset + tz tag** | `{ offset_minutes: i16, tz_tag: String }` | No tz-database dependency; trivially WASM-portable. | Offset is wrong half the year for any zone with DST; legal effective windows are rare DST edge cases but exist (Brazil DST history). |

**Recommendation (subject to ADR):** Option A with a fallback. Store the
IANA name; refuse to encode an unknown zone; ship a pinned tz-data snapshot
under `crates/ke-core/tzdata/` with `tz_data_version` recorded in the
manifest. Runtime resolution lives in Gate 3 once `ke-runtime` exists; the
IR side only stores the name and version. ADR pending in
`docs/adr/0001-jurisdiction-time-zone.md` before Gate 1 implementation
starts.

**Effective-time policy escape hatch (§ 8.4):** `effective_time_policy` is a
typed key + version, not free-form, even though Gate 1's enum starts with a
single variant `MidnightLocal` (the default). Future regimes can add
variants (e.g., `MarketOpenLocal { exchange: String }`) without breaking
canonicalization.

---

## 7. Runtime policy draft (informational, not implemented)

`PolicyBundle` is one of the four artifact kinds. Its shape, sketched now so
the IR and canonical encoding don't have to be revisited at Gate 4:

```text
PolicyBundle {
  environment: String,                       // e.g., "production-eu"
  verification_policy: {
    t2_t3_mode: enum { Strict, ReviewOverride, Advisory, Disabled },
    required_attestation_types: [enum {
      SourceFidelity, Interpretation, ScenarioCoverage,
      EquivalenceClaim, PublicationApproval,
    }],
    minimum_attestation_count_per_type: map<AttestationType, u8>,
  },
  revocation_policy: enum {
    HaltImmediately, FinishPinnedThenHalt, FinishPinnedNoNew,
  },
  effective_window: EffectiveWindow,
}
```

Open decisions (spec § 21): expert key authority, T2/T3 mode default,
revocation default. None of those block Gate 1; they block Gate 4. The
fields exist now so the Gate 1 canonical encoding can round-trip an empty
`PolicyBundle` artifact and prove the wire shape is stable.

---

## 8. Fixture and golden-file strategy

### 8.1 Source corpus

`fixtures/rules/*.yaml`, snapshotted from
`institutional-defi-platform-api/src/rules/data/` via
`scripts/bootstrap.sh`. Platform commit SHA is recorded in
`fixtures/rules/SOURCE.md`. Differential and equivalence harnesses (Gates 2
and 3) verify the platform checkout still points at this SHA before running.

### 8.2 Golden artifact bytes

Stored under `fixtures/artifacts/`. Each entry is a triple:

```text
fixtures/artifacts/<artifact_id>/
├── source.json        # decoded canonical form, pretty-printed (for review)
├── canonical.bin      # canonical bytes (the authoritative artifact)
└── manifest.json      # decoded manifest, for fast-eye review
```

`canonical.bin` is the artifact under test. `source.json` and
`manifest.json` are convenience views; CI must regenerate them from
`canonical.bin` and fail if they drift. Only `canonical.bin` is allowed to
be hand-curated (and even then only via the fixture-generation script).

### 8.3 Fixture generation script

`scripts/generate-golden-fixtures.sh` (Gate 1 new) drives the workflow:

1. Resolves `${PLATFORM_REPO}`; verifies `fixtures/rules/SOURCE.md` SHA
   matches platform `HEAD` (otherwise refuses to run).
2. For every Rust IR fixture, asks the Python pipeline to emit the same IR
   shape, converts to canonical bytes via the Gate 1 encoder, writes to
   `fixtures/artifacts/<id>/canonical.bin`.
3. Regenerates `source.json` and `manifest.json` views.
4. Writes a `fixtures/artifacts/MANIFEST.md` provenance ledger listing each
   golden id, its `ir_schema_version`/`codec_version`/`canonicalization_version`,
   and the platform commit SHA.
5. Exits non-zero on any encoder error, schema mismatch, or unresolved
   platform sibling.

The script is idempotent — running it twice produces identical bytes — and
this idempotence is itself a CI check.

### 8.4 Golden test vector coverage (spec § 8.3)

Gate 1 lands these golden tests:

- Canonical serialization (input AST → bytes). **Round-trip:** encode →
  decode → re-encode → byte-equal.
- Artifact hash computation. Even though `ke-artifact` is Gate 4, Gate 1 can
  compute BLAKE3 over `canonical.bin` and record it as an expected value;
  Gate 4 wires it into `Manifest::artifact_hash`.
- Rejection of non-canonical encodings. Mutated bytes (wrong field order,
  unsorted map, non-NFC string, float-shaped integer, missing version
  field) must produce specific decode errors enumerated in the
  `CanonicalDecodeError` enum.

Cross-language Rust/Python decoding (spec § 8.3, fifth bullet) is **not** a
Gate 1 deliverable — it requires `ke-artifact-py`, which is Gate 4. Gate 1
records the bytes; Gate 4 proves they decode in Python.

---

## 9. Rust/Python compatibility assumptions

1. **Bytes-level contract, not struct-level.** The Python side will consume
   canonical bytes; mismatched struct field names or casing are non-events
   so long as `canonical.bin` decodes.
2. **Schema is the bridge.** The JSON Schema generated in §5 is what
   platform-side `pydantic` / `msgspec` consumers target. Schema
   determinism (§5.9) is therefore a cross-language correctness invariant,
   not a cosmetic CI gate.
3. **Versions travel with the artifact.** `ir_schema_version`,
   `codec_version`, `canonicalization_version` are in the manifest and in
   any decode error. A Python consumer on an older schema rejects
   immediately rather than silently re-interpreting fields.
4. **Decimal scalars, not floats.** Python side must use `decimal.Decimal`
   with the recorded `scale`. Float coercion at the boundary is a contract
   violation that produces a specific error.
5. **No private fields.** Anything not in the JSON Schema is not part of
   the contract. Rust-side helper fields used during compile but stripped
   before encoding must be in non-IR structs (e.g.,
   `crates/ke-compiler/src/internal/`), not on `RuleIR`.
6. **No timezone fallback.** Python consumers must use `zoneinfo` with the
   recorded `tz_data_version`. A mismatched tzdata version is a runtime
   error, not a silent re-resolution. (See § 6.)

---

## 10. Phase plan

**Phase 0 — ADRs (before any code):**
- `docs/adr/0001-jurisdiction-time-zone.md` — § 6 option A vs B.
- `docs/adr/0002-canonical-codec-postcard.md` — confirm or reject postcard.
- `docs/adr/0003-decimal-scalar-representation.md` — mantissa/scale shape.

**Phase 1 — type scaffolding:**
- Create `crates/ke-core/src/ir/` module layout per § 2.
- Define types with `serde` derive, no canonical encoding yet.
- Compile-only — no behaviour, no schema emission yet.

**Phase 2 — canonical encoding:**
- Implement `crates/ke-core/src/canonical/encode.rs` and `decode.rs`.
- Field-order, map-order, set-order, optional, numeric, string rules per §4.
- Round-trip test on a synthetic IR.

**Phase 3 — JSON Schema emission:**
- `crates/ke-core/build.rs` (or `bin/emit-schema.rs`) writing
  `crates/ke-core/schema/ir.schema.json`.
- Determinism test: emit twice, byte-compare.

**Phase 4 — fixture generation:**
- `scripts/generate-golden-fixtures.sh`.
- Populate `fixtures/artifacts/`.
- Round-trip + hash + non-canonical-rejection golden tests in
  `crates/ke-core/tests/`.

**Phase 5 — exit criteria:**
- `cargo test -p ke-core` green.
- `cargo run --bin ke-core-emit-schema` produces a byte-stable file.
- Regenerating golden fixtures produces identical bytes.
- All three ADRs merged.

---

## 11. Acceptance criteria mapping (spec § 19 Gate 1)

| Spec acceptance | Gate 1 phase | Test target |
|-----------------|--------------|-------------|
| Python-emitted artifact → canonical Rust → semantic match | Phase 4 | golden fixture round-trip |
| Golden fixture bytes → decode → re-encode → hash stable | Phase 4 | `tests/round_trip.rs` |
| JSON Schema generation is deterministic | Phase 3 | `tests/schema_determinism.rs` + CI `git diff --exit-code` gate |

---

## 12. Risks

- **Decimal scalar choice locks downstream Python.** If Python `decimal.Decimal`
  contexts on the platform side use different defaults, the contract breaks
  at the boundary. Mitigation: ADR 0003 + a contract test in Gate 4.
- **tzdata version drift.** Gate 1 freezes a snapshot; Gate 3 must not silently
  upgrade it. Mitigation: `tz_data_version` in manifest + CI gate.
- **Field-order-as-contract.** Reordering a Rust struct field becomes a
  breaking change to `canonicalization_version`. Mitigation: explicit
  callout in `docs/canonical-encoding.md` plus a `cargo test` that hashes
  the schema and compares against a pinned value.
- **Self-referential `artifact_hash`.** Patch-after-encode is error-prone.
  Mitigation: dedicated `tests/artifact_hash_offset.rs` that re-derives the
  offset and proves the patch is idempotent. Real wiring lands in Gate 4.

---

## 13. Out-of-scope reminders

Repeating the non-goals from § 1 here so they cannot be quietly absorbed
into Gate 1 implementation:

- No YAML parser.
- No AST→IR lowering.
- No T0/T1/T4 verification.
- No preview executor.
- No signing or registry.
- No PyO3 binding.
- No WASM.
- No frontend rewire.
- No production artifact promotion.

Any of these creeping into Gate 1 is a spec violation and must be backed
out before merge.
