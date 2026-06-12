# Gate 4 — implementation log

Phase-by-phase record of the Gate 4 implementation (signed, content-addressed
artifact + attestations + registry lifecycle + PyO3 binding + cross-language
contract test), written as each phase lands. Authoritative contract:
`dev/briefs/gate-4-artifact-registry-attestation.md`. Continues the Gate 1–3
doc-each-phase convention so Hossain's manual gate review is fast and auditable.

**Branch:** `migration/gate-4-artifact` (off accepted Gate 3 + the Phase-0
prerequisite changeset).
**Toolchain:** Rust 1.85 (`x86_64-pc-windows-gnu`).
**Canonical triplet:** `ir_schema_version 0.4.0` / `postcard-1` / `ke-canon-4`
(post-ADR-0013 canon-4 landing; Phase 1 adds **no further canon bump** — the
13-field `Manifest` is untouched and all Gate 1–3 golden bytes remain valid).

**Locked decisions** (ADRs 0009–0014, brief §3/§4): fixed-seed deterministic
test keys for every committed signature (no `OsRng`/`getrandom` — toolchain
constraint, carried from Gate 3); content hash = BLAKE3 zero-then-patch;
signature = ed25519 over the hash-patched envelope prefix; registry persistence
v1 = S3 (ADR 0012, implementation Phase 3); attestations are typed and bind to
the artifact hash (schema in `docs/attestation-schema.md`, behavior Phase 2).

**Gate 4 status: Phase 1 complete (2026-06-12) — Phases 2–6 not started.**
Phase 1 turned a compiled IR into a signed, content-addressed `.kew` artifact
with committed golden vectors. Attestation/consistency *behavior*, the registry
state machine, PyO3, and the contract test are still ahead. No commits made —
Hossain owns the history.

---

## Phase 0 — prerequisite ADRs + attestation schema + canon-4 landing ✅ (2026-06-11)

The Before-Gate-4 checklist (spec §23) items that gate Phase 1, all closed
2026-06-11:

- **ADRs 0009–0015 authored;** 0009–0014 **Accepted** by Hossain 2026-06-11
  (see `docs/adr/README.md`):
  - `0009-expert-key-authority-and-revocation.md` — expert key authority, key
    lifecycle, revocation (spec §21.1, §21.6, §20).
  - `0010-trusted-timestamp-authority.md` — TSA for typed attestations
    (§21.5, §10); deterministic mock TSA is dev/test-only.
  - `0011-t2t3-publication-policy-and-sidecar-deployment.md` — T2/T3
    publication policy + sidecar deployment (§21.2, §21.3, §11).
  - `0012-s3-registry-and-pep503-index-layout.md` — S3 registry layout +
    PEP 503 package-index layout (§14; resolved-persistence §21).
  - `0013-revocation-policy-reconciliation.md` — §15 reconciliation; the only
    canon-bumping prerequisite (authorized `0.3.0/ke-canon-3 →
    0.4.0/ke-canon-4`).
  - `0014-audit-contract-ownership.md` — §18 audit-contract ownership +
    pre-freeze static field model (the two named version slots that Phase 1
    carries as `AuditVersions`).
  - `0015-temporal-orchestration-ownership.md` — **Proposed** (restates
    existing spec policy: orchestration stays Python, the work moves to Rust;
    not load-bearing for Phase 1).
- **Typed attestation schema finalized** — `docs/attestation-schema.md` (§10
  bound fields); ADRs 0009/0011 and the schema bind to the post-0013
  `RevocationPolicy` names.
- **T4 remediation** (hard prerequisite, not in the original §23 list): the
  `contradictory_outcome` detector fixed so `verify()` over the clean corpus
  yields `has_blocking() == false` (ADR 0005 amendment +
  `crates/ke-compiler/tests/t4_corpus.rs`). Without it no artifact could reach
  `draft → structurally_verified` (§9) and Gate 4 had nothing to attest.
- **ADR-0013 canon-4 landing executed** in a single atomic changeset: enum
  reconciled, version triplet bumped to `0.4.0 / postcard-1 / ke-canon-4`,
  golden corpus regenerated once via the generators, workspace green.

Still open from §23 (not blocking Phase 1, tracked for later phases): the
platform-repo consumer brief exists here only as
`dev/briefs/gate-4-platform-consumption-OUTLINE.md`; the full brief is a
separate platform-repo deliverable (§14) and must be reviewed before the
platform-side Gate-4 PR.

**Verification:** docs only; `cargo test --workspace` green after the canon-4
landing (goldens regenerated through the generators, never hand-edited).

---

## Phase 1 — `ke-artifact` core: assembly, content addressing, compiler signature, signed golden vectors ✅ (2026-06-12)

Plan of record: `~/.claude/plans/ethereal-bubbling-dragonfly.md` Rev 2
(incorporates Hossain's two review corrections — the byte-range/verify
semantics and the `test-keys` feature gating — both reflected below).

### What was built

- `crates/ke-artifact/src/lib.rs` — module tree, re-exports, and
  `ArtifactError` (thiserror): identifiable variants per spec §8.3 —
  `Canonical` / `CanonicalDecode` (wrapping ke-core's profile errors; the
  canonical profile is **reused via `ke_core::canonical` public fns, not
  duplicated**), `TrailingBytes`, `HashMismatch{expected,got}`,
  `SignatureInvalid`, `EnvelopeTruncated`.
- `crates/ke-artifact/src/artifact.rs` — the `Artifact` assembly (spec §8.1).
  Field **declaration order is the byte contract** (postcard = declaration
  order, no framing): envelope = `manifest` (13 Gate-1-frozen fields),
  `compiled_ir: Vec<RuleIR>`, `source_span_index`, `audit_versions`
  (ADR 0014 static slots), `consistency_block: Option<ConsistencyBlock>`
  (`None` in Phase 1); outside the envelope = `compiler_signature`,
  `attestations: Vec<Attestation>` (empty in Phase 1),
  `registry_state_metadata` (`Draft`). `SourceSpanIndex` is built from
  `compiled_ir`, sorted by `rule_id`. `ConsistencyBlock` and `Attestation`
  (+ `AttestationScope`, `TimestampEnvelope`) are **Phase-2 shells** — shapes
  per `docs/attestation-schema.md` §3, behavior deferred; `Attestation` sits
  outside the envelope so its shape may still evolve in Phase 2 without
  breaking content addresses.
- `crates/ke-artifact/src/hash.rs` — `content_hash` (zero-then-patch),
  `verify_hash` (extracts the envelope prefix, **re-zeroes the 32-byte slot,
  then** recomputes), `artifact_hash_offset` generalized over all four
  `ArtifactKind`s.
- `crates/ke-artifact/src/sign.rs` — `sign_envelope` / `verify_signature`
  (ed25519 over the hash-patched envelope prefix) + the fixed-seed `test_keys`
  module (see key hygiene below).
- `crates/ke-artifact/src/bin/gen-golden-artifacts.rs` — documented generator
  (mirrors ke-core's `gen-fixtures` writer pattern): builds signed artifacts
  from the existing fixture inputs, writes
  `fixtures/artifacts/<id>/artifact.kew` + `signature.json` (review view only,
  never authoritative) and its own ledger `fixtures/artifacts/GOLDEN.md`.
  Two generators, two ledgers — ke-core's `MANIFEST.md` is untouched.
- **Four test suites** (`crates/ke-artifact/tests/`): `golden.rs` (6 — byte
  stability, hash recompute, sig verify, the negative naive-hash assertion,
  key-id hygiene), `hash_offset.rs` (3 — all four `ArtifactKind`s),
  `sign.rs` (3 — RFC-8032 determinism; tamper → fail), `non_canonical.rs`
  (4 — §8.3 identifiable errors). 16 tests total in the crate.

### The byte-range contract (load-bearing — Hossain review correction #1)

- A `.kew` file is `postcard::to_stdvec(&Artifact)`. The **envelope
  serialization is the literal byte prefix** `[0, envelope_len)`;
  `envelope_len` is recovered by decoding a private `EnvelopeView` (the five
  envelope fields only) with `postcard::take_from_bytes` and measuring the
  cursor.
- `artifact_hash` = BLAKE3 over the envelope prefix **with the 32-byte hash
  slot zeroed**, then patched in at
  `offset = postcard::to_stdvec(&manifest.artifact_kind).len()` (idempotence
  proven in `ke-core/tests/artifact_hash_offset.rs`).
- **Consequence — the trap:** `blake3(final .kew bytes) ≠ artifact_hash` *by
  construction*. Every verifier must re-zero the slot within the envelope
  prefix before recomputing. `golden.rs` **negatively asserts** the naive
  whole-file check fails (and that BLAKE3 over the *patched* prefix also
  fails), so the semantics can't later be "fixed" by weakening the design.
- `compiler_signature` = ed25519 over the **hash-patched** envelope prefix.
  Order: encode-zeroed → hash prefix → patch → sign prefix → append
  signature + empty attestations + Draft metadata → write `.kew`.
- `docs/adr/0012-s3-registry-and-pep503-index-layout.md` §5 step 3 carries an
  **Erratum (2026-06-12)** correcting its verify wording from the naive
  whole-file recompute to the re-zero-slot-within-envelope-prefix procedure —
  so the Phase-4 platform consumer doesn't implement the naive check and
  reject every artifact.

### Key hygiene (Hossain review correction #2)

`cfg(test)` is invisible to bin targets, so the fixed-seed key module is gated
`#[cfg(any(test, feature = "test-keys"))]` with `test-keys = []` declared in
`ke-artifact`'s `[features]` and `required-features = ["test-keys"]` on the
`gen-golden-artifacts` bin. Keys come from `SigningKey::from_bytes(&FIXED_SEED)`
only — **no `OsRng`/`getrandom` anywhere** (windows-gnu toolchain blocker,
carried from Gate 3). Every committed signature carries
**`key_id = "test-fixed-seed-1"`** so golden-vector signatures can never be
mistaken for ADR-0009 production-key signatures; `golden.rs` asserts the
`test-` prefix on every committed `signature.json`.

### Golden vectors

`fixtures/artifacts/` (generator-written only): `rule_reserve_assets`
(`artifact_hash bcebbd1f…ccb87`, `envelope_len 862`) and
`rule_significant_thresholds` (`a0a06ee4…f66bf`, `envelope_len 598`), each as
`artifact.kew` + `signature.json`, ledgered in `fixtures/artifacts/GOLDEN.md`.
**`policy_production_eu` (PolicyBundle) is skipped** — the documented
generator/ledger decision: the Phase-1 `Artifact` envelope is RuleIR-oriented
(`compiled_ir: Vec<RuleIR>`), so only the two RegimePack rule fixtures carry
signed artifacts. A PolicyBundle-shaped artifact is future work, not silently
absent.

### Gate evidence (integration agent, re-run independently 2026-06-12; quoted verbatim)

> All 7 gate checks re-run independently this session, all pass with zero fixes
> needed. (1) cargo fmt --all -- --check: clean. cargo clippy --workspace
> --all-targets -- -D warnings: 'Finished dev profile', clean. cargo test
> --workspace: 34 'test result: ok' blocks, 89 passed / 0 failed / 0 ignored;
> load-bearing ke-core suites confirmed by name in output:
> golden_rule_files_are_byte_stable, golden_policy_file_is_byte_stable,
> schema_determinism, t4_corpus, artifact_hash_offset (Manifest/encoding
> untouched). (2) cargo build -p ke-artifact with NO features: succeeds;
> fixed-seed module absent from normal surface. (3) grep -rn 'OsRng|getrandom'
> crates/ke-artifact/: 7 hits, all doc comments, zero code usage.
> (4) Generator idempotence: snapshot git status + sha256 of all 5 fixture
> files, re-ran cargo run -p ke-artifact --features test-keys --bin
> gen-golden-artifacts, both diffs empty -> IDEMPOTENT_CONFIRMED.
> (5) tests/golden.rs:112 naive_whole_file_hash_fails_by_construction asserts
> blake3(raw .kew) != artifact_hash AND blake3(patched prefix) !=
> artifact_hash; golden.rs:147 committed_key_id_is_loudly_a_test_key asserts
> starts_with("test-") and == TEST_KEY_ID; both signature.json files on disk
> carry key_id test-fixed-seed-1. (6) ADR 0012 §5 step 3 line 177:
> '**Erratum (2026-06-12):**' with struck naive wording and the
> re-zero-slot-within-envelope-prefix procedure. (7) Independent out-of-suite
> recompute (Python blake3; offset=1 verified against source —
> ArtifactKind::RegimePack is variant 0 of a unit enum, postcard varint =
> 1 byte, matching ke-core hash_offset = postcard::to_stdvec(&kind).len()):
> rule_reserve_assets envelope_len=862 recomputed
> bcebbd1f89619efbab253e9fb463fa089b0d487a28064006ec6fd7a43a0ccb87 match=True
> naive=False; rule_significant_thresholds envelope_len=598 recomputed
> a0a06ee4cd592d557d42e9f1a0c5177a64a4c080f0677ef73a706542798f66bf match=True
> naive=False.

Test counts (same report): `cargo test --workspace` = **89 passed, 0 failed,
0 ignored across 34 suites**; `ke-artifact` alone = **16 passed** (golden.rs 6,
hash_offset.rs 3, non_canonical.rs 4, sign.rs 3), 0 failed. No commits made;
`fixtures/` written only by the generator; ke-core's `MANIFEST.md` untouched.

### What Phase 1 deliberately EXCLUDES (deferred, not done)

- **Attestation behavior** — signing, verification, the §10 rejection rules
  R1–R8. `Attestation` is a shape-only shell; `attestations` is empty in every
  committed artifact. → Phase 2.
- **`ConsistencyBlock` behavior** — T0–T4 evidence carriage, policy mode,
  overrides. Stub type only; `None` in every committed artifact. → Phase 2.
- **PolicyBundle golden artifact** — skipped per the generator/ledger decision
  above (envelope is RuleIR-oriented in Phase 1).
- **Registry state machine + S3** (Phase 3), **PyO3 / `ke-artifact-py`**
  (Phase 4), **`contract-test.sh`** (Phase 5), per the brief's phase plan.
- §17 per-branch `interpretation_notes` stays an open pre-attestation triage
  item (flagged, not blocking — attestations don't bind yet).

### Known residue (carried forward, needs a small follow-up)

The ADR 0012 verify-wording erratum is scoped to §5 step 3 only; the naive
whole-file-hash wording **recurs in ADR 0012 §2 (~line 82) and Consequences
(~line 222)** and should get the same correction before Phase 4's platform
consumer is written against that ADR.

Phase 1 is ready for Hossain review/commit on `migration/gate-4-artifact`.
