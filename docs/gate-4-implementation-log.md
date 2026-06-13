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

**Gate 4 status: Phases 1–2 complete (2026-06-12) — Phases 3–6 not started.**
Phase 1 turned a compiled IR into a signed, content-addressed `.kew` artifact
with committed golden vectors. Phase 2 added typed-attestation *behavior*
(payload-prefix signing, verification, the R1–R8 rejection matrix) and
**froze the `Attestation` shape** with the first attested goldens — content
addresses unchanged. The registry state machine, PyO3, and the contract test
are still ahead. No commits made — Hossain owns the history.

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

---

## Phase 2 — typed attestations, MockTsa, key directory, rejection rules (2026-06-12)

**Status: delivered.** Behavior filled into the Phase-1 shapes at the freeze
point — committed Phase-1 goldens encoded `attestations = []` (0x00) and
`consistency_block = None` (0x00), so the shell amendments below changed no
committed bytes; with attested goldens now committed, the `Attestation` shape
is frozen.

### What was built

- **`attestation.rs`** — `Attestation`/`AttestationScope` moved here (old paths
  re-exported). Signing reuses the proven prefix pattern: `signature: [u8;64]`
  is the last field, so the signed bytes are the postcard payload prefix,
  recovered via an `AttestationPayloadView` + `take_from_bytes` (the
  `EnvelopeView` technique). `sign_attestation` / `verify_attestation` /
  `verify_attestation_set` with an explicit `PolicyContext { environment,
  now_unix, supported_policy_versions, current_legal_source_hash }` — no clock
  syscalls; fully deterministic.
- **Shell amendments:** `signer_role` is now the typed `SignerRole`
  (`DomainExpert | PublicationApprover | Registry`, ADR 0009);
  `TimestampEnvelope` → ADR-0010 `TimestampToken { class, token,
  claimed_time_unix }` with `TimestampAuthorityClass` (`Rfc3161External` /
  `Rfc3161Internal` / `Mock`) **inside the signed payload and re-derived from
  the token at verification** (mismatch ⇒ typed rejection);
  `test_corpus_hash: Option<[u8;32]>` slot added — the proposed §10 amendment
  (attestation-schema §6A), frozen as a slot, **not yet authoritative**, `None`
  in all fixtures.
- **`tsa.rs`** — deterministic offline `MockTsa` (own fixed seed, authority
  `test-mock-tsa-1`; token = ed25519 over class-discriminant ‖ payload_hash ‖
  claimed_time LE; payload_hash = the attested `artifact_hash`). An ungated
  `MOCK_TSA_PUBLIC_KEY` lets `derive_class` run without the `test-keys`
  feature; a unit test pins it to the gated seed. `Rfc3161*` verification
  returns typed `TsaUnsupported` until vendor onboarding (ADR 0010 blocker —
  production publish stays dark regardless).
- **`keydir.rs`** — ADR-0009 `KeyDirectoryEntry`/`KeyDirectory` (roles,
  authorized types, validity window, status, revocation back-pointer). The
  registry-signed directory object + root custody is Phase 3.
- **`consistency.rs`** — `ConsistencyBlock` moved here + builder/validation.
  No ke-compiler dependency; the `VerificationReport → ConsistencyBlock`
  adapter is ke-cli (Phase 3). `consistency_block` stays `None` in committed
  goldens (T2/T3 evidence is platform-owned, ADR 0011).
- **Rejection matrix** — `AttestationRejection` with one typed variant per
  rule: R1a–d (`KeyUnknown/KeyExpired/KeyRevoked/KeyUnauthorizedForType`),
  R2 `NotBoundToArtifact`, R3 `PolicyVersionUnsupported`, R4 `Expired`,
  R5 `LegalSourceHashChanged`, R6 `RequiredTypeMissing`, R7
  `CoAttestationAbsent`, R8 `MockTsaNonLocal`, plus
  `AttestationSignatureInvalid`, `TimestampClassMismatch`, `TsaUnsupported`.
  `tests/attestation.rs` has **one named test per variant**, each asserting
  the exact variant — the brief §7 "specific policy error" criterion,
  mechanically. Mapping notes: regime/compiler-version mismatch folds into R2,
  ir-schema drift into R3 (schema §7 footnote); R4 expiry is half-open from
  00:00 UTC of the expiration date; `KeyUnknown` surfaces before the signature
  check (lookup is a prerequisite).
- **Attested golden vectors** — the generator appends a deterministic
  3-attestation set (SourceFidelity + ScenarioCoverage under `DomainExpert`,
  PublicationApproval under `PublicationApprover`; expert key
  `test-expert-fixed-seed-1`; MockTsa at fixed `claimed_time` 1_750_000_000;
  `attestations.json` review views). **The §9 append property is now pinned:**
  `.kew` files grew post-envelope only (946→1750, 682→1486 bytes) while both
  Phase-1 content addresses are unchanged and **hardcoded as regression pins**
  in `tests/golden.rs` (`bcebbd1f…/862`, `a0a06ee4…/598`); `signature.json`
  files are untouched in git, proving the compiler signature never moved.

### Gate evidence (integration runner, independently re-verified)

`cargo test --workspace` = **117 passed, 0 failed across 35 suites**
(`ke-artifact` = 44: 7 unit + 17 attestation + 8 golden + 3 hash_offset +
6 non_canonical + 3 sign). fmt + clippy (`-D warnings`, both feature sets)
clean; `cargo build -p ke-artifact` without features clean (fixed-seed
material absent from the normal surface); `OsRng`/`getrandom` grep:
doc-comments only; generator double-run **byte-identical** (sha256 over all
13 fixture files); key hygiene: `test-fixed-seed-1` (compiler),
`test-expert-fixed-seed-1` (expert), `test-mock-tsa-1` (TSA) all asserted.

### What Phase 2 deliberately EXCLUDES (deferred, not done)

- **Registry-signed key directory + root-key custody** and **anti-backdating
  monotonic ordering against the registry event log** — need the Phase-3 event
  log (ADR 0009 §3, ADR 0010 §4).
- **RFC 3161 token parsing** — blocked on vendor onboarding (ADR 0010);
  `TsaUnsupported` is the honest stand-in.
- **ConsistencyBlock evidence path** — T2/T3 is platform-owned (ADR 0011);
  the ke-compiler adapter lands with ke-cli in Phase 3.
- **`test_corpus_hash` binding semantics** — slot frozen, enforcement waits on
  the §10 spec amendment.
- Registry state machine + S3 (Phase 3), PyO3 (Phase 4), contract-test.sh
  (Phase 5).

### Residue from Phase 1, resolved

The ADR 0012 naive-hash wording recurrence (§2 + Consequences) was corrected
alongside the Phase-1 docs commit — all three spots now state the
re-zero-the-slot procedure.

Phase 2 is ready for Hossain review/commit on `migration/gate-4-artifact`.

---

## Phase 3a — registry core: signed/hash-chained event log, local-FS backend, `ke compile`→draft/structurally_verified, `ke query` (2026-06-13)

Phase 3 stands up the §9 registry lifecycle. **Split decision (confirmed):**
**3a now** = registry-core library + local-FS backend + `ke compile`→
`draft`/`structurally_verified` + `ke query`. **3b next** = `attest`/`publish`/
`deprecate`/`revoke`/`rollback` commands + revocation-policy behavior. All of
Phase 3a lives in `ke-cli` (now a `[lib]` + `ke` `[bin]`); the platform is not
linked here.

### What was built

- **State lives in the event log, not the artifact (ADR 0012 §2).**
  `ke-artifact`'s `RegistryStateMetadata` stays the inert `Draft` marker; a
  new registry-local `LifecycleState { Draft, StructurallyVerified, MlChecked,
  ExpertAttested, Published, Deprecated, Revoked }` is **derived** by walking
  the log. `current_state(&[LifecycleEvent])` returns the highest-`seq` event's
  `new_state`, validating as it goes.
- **Append-only, hash-chained, registry-root-signed events**
  (`registry/event.rs`). `LifecycleEvent { artifact_hash, seq, prior_state,
  new_state, event_kind, authority_key_id, authority_role, timestamp,
  prev_event_hash, signature }`. `signature` is the **last** field; the signed
  bytes are the postcard **payload prefix** of all prior fields, recovered with
  `take_from_bytes` via a private `EventPayloadView` (the proven
  attestation/envelope technique). `prev_event_hash = blake3(canonical bytes of
  the prior event **including** its signature)`; first event = `None`.
  ed25519 by the **registry-root** key over the prefix.
- **Transition authority = preconditions, not per-actor signing**
  (ADR 0012 §2; §9). Every event is registry-root-signed (`SignerRole::
  Registry`) and records the triggering authority in `authority_role`. The §9
  rules ("only compiler/CI → structurally_verified", "only registry policy →
  published", …) are enforced by `can_transition(from, to, &Preconditions)`
  before append. **3a executes only `draft` + `structurally_verified`**; the
  remaining edges (`→ml_checked` needs a consistency block; `→expert_attested`
  needs a valid attestation set; `→published` needs `prior == expert_attested`;
  `→deprecated`/`→revoked`) are table entries exercised by tests and used by 3b.
- **Backend trait seam** (`registry/backend.rs`). `RegistryBackend`
  (put_artifact / append_event / read_events / put_pointer / read_pointer /
  list_manifests), sync, `io::Result`-style errors wrapped in `RegistryError`.
  `LocalFsBackend` mirrors the ADR-0012 paths under one root
  (`artifacts/<hash>/{artifact.kew,manifest.json,schema.json}`,
  `events/<hash>/<seq:04>-<kind>.json`, `tags/<env>/<tag>.json`) and writes a
  **`NON_AUTHORITATIVE`** marker at the root (ADR 0012 §6). S3 slots behind the
  same trait in a later gate.
- **Resolution + §18 record** (ADR 0012 §5, ADR 0014). `resolve(&backend,
  selector, now)` for `ByHash` / `ByTag{env,tag}` / `ByRegime{regime_id,
  effective, env}`. `ByRegime` filters `list_manifests` by regime AND the
  closed-open `[effective_from, effective_to)` window (`to=None` open-ended;
  date-only, `tz=None` honored) intersected with `state==Published`; `>1` =>
  `Ambiguous`, `0` => `NotFound`. The §18 `ResolutionRecord { artifact_hash,
  registry_state_at_resolution, resolving_event_key, selector_desc,
  attestation_policy_version, resolution_timestamp_unix }`. Verification re-uses
  the re-zero hash procedure (`ke_artifact::verify_hash`, **never**
  `blake3(raw .kew)`). `is_rollback_eligible(state) == matches Published`
  (ADR 0013 predicate now; command is 3b).
- **Clock-free core.** Every registry fn takes `now_unix: u64`; no
  `SystemTime`/clock syscalls in `registry/`. The CLI sources time from
  `--now`/`KE_NOW` else system time **at the CLI edge only**.
- **`ke` CLI (clap v4 derive).** Global `--registry <dir>` (or
  `KE_REGISTRY_BACKEND=local` + `KE_REGISTRY_DIR`), `--now`/`KE_NOW`.
  `ke compile <yaml> --regime <id> [--env]` (compile → verify, abort if
  blocking → assemble with the test compiler key → put_artifact → append draft
  → if structural preconditions hold, append structurally_verified → print
  hash+state). `ke query (--hash | --tag <env>/<tag> | --regime --effective
  --env)`. `verify`/`attest`/`publish`/`deprecate`/`revoke`/`rollback` are
  declared but exit 2 with a Phase-3b message (surface visible).
  clap is pinned `default-features = false` (drops the `anstyle-wincon →
  windows-sys` color stack the windows-gnu dlltool cannot build).
- **Registry test key (loud, no getrandom).** `registry/event.rs` gates
  `test_keys` on `any(test, feature="test-keys")`: `REGISTRY_ROOT_KEY_ID =
  "test-registry-fixed-seed-1"`, fixed 32-byte seed, signing key; plus an
  **ungated** `REGISTRY_ROOT_PUBLIC_KEY` const for verification (mirrors
  `tsa.rs MOCK_TSA_PUBLIC_KEY`). A gated unit test pins the const to the
  seed-derived key. **Signing is feature-gated** so `cargo build -p ke-cli`
  (no features) stays clean — the core types, `current_state`, `resolve`, and
  the precondition table are all ungated; only the signing entry points
  (`sign_event`, `ke compile`) need the feature.

### Canonical-event-head-hash pin

`tests/registry.rs::canonical_event_head_hash_is_pinned` builds the fixed
registry (fixed keys + `KE_NOW=1_750_000_000`) by compiling
`fixtures/rules/mica_stablecoin.yaml` and asserts the blake3 of the highest-seq
event's canonical bytes equals the hardcoded constant
**`3ded38e468316b59cf8afe2cd46fe36bb13632ca2b159085324dd3102282ce3e`** — any
accidental change to the event encoding/signing shape flips this and fails.

### Gate evidence (independently re-verified 2026-06-13)

- `cargo test -p ke-cli` = **11 passed** (2 unit: the registry-root public-key
  pin + key_id `test-` prefix; 9 integration: draft→structurally_verified flow,
  chain-tamper → typed error, seq-gap → typed error, transition-precondition
  rejection, ByHash/ByTag/ByRegime resolution + §18 fields + pre-window/unknown
  NotFound, rollback-eligibility, bad-sig rejection incl. non-registry-key
  forgery, the canonical-event-head-hash pin, determinism re-run).
- `cargo test --workspace` = **128 passed, 0 failed across 38 suites**.
- fmt clean; clippy (`-D warnings`) clean **both** without features and with
  `--features test-keys --all-targets`; `cargo build -p ke-cli` (no features)
  clean, **0 warnings**.
- `bash scripts/registry-smoke.sh` → **PASS**: compiles two real corpus rules
  (`mica_stablecoin`/`mica_2023`, `fca_crypto`/`fca_cryptoassets`) into a tmp
  local-FS registry at `KE_NOW=1750000000`, each reaches
  `structurally_verified`, `ke query --hash` confirms the state, the
  `NON_AUTHORITATIVE` marker is present, and a re-run into a second tmp produces
  **byte-identical** `events/` + `artifacts/` trees (determinism).
- Hygiene: `OsRng`/`getrandom`/`tokio`/`aws` appear only in doc/Cargo comments
  asserting the prohibition; `cargo tree -p ke-cli` pulls none of them. Every
  event carries `authority_key_id = "test-registry-fixed-seed-1"` (asserted
  `test-` prefixed). Local-FS objects flagged non-authoritative (ADR 0012 §6).

### What Phase 3a deliberately EXCLUDES (deferred to 3b / later)

- `attest`/`publish`/`deprecate`/`revoke`/`rollback` commands + revocation-policy
  behavior (3b). The lifecycle edges past `structurally_verified` exist as
  `can_transition` table entries + are exercised by tests (a test hand-appends a
  `published` chain via core to drive ByTag/ByRegime), but no CLI executes them.
- Real S3 backend + Object-Lock/versioning (later gate; trait seam ready).
- Registry-root key HSM custody + signed key-directory object + root rotation
  (ADR 0009, infra).
- Full anti-backdating skew-bound check (monotonic `now_unix` + the hash chain
  are present; the bound is 3b).
- PyO3 (Phase 4), `contract-test.sh` (Phase 5), `ke serve` (Gate 5).

Phase 3a is ready for Hossain review/commit on `migration/gate-4-artifact`.

---

## Phase 3b — lifecycle commands: `ml-check`, `attest`, `publish`, `deprecate`, `revoke`, `rollback` (2026-06-13)

Phase 3b drives a `structurally_verified` artifact through the **rest of the §9
lifecycle** via CLI commands, filling the 3a exit-2 stubs. All six commands keep
the 3a precedent: signing stays behind the `test-keys` cargo feature, and a
no-feature build returns a typed "requires `--features test-keys`" error per
command. Every lifecycle event remains **registry-root-signed**
(`SignerRole::Registry`) and records the triggering authority in
`authority_role` — the `LifecycleEvent` shape is **unchanged**, so the 3a
canonical-event-head-hash pin still holds. Branch: `migration/gate-4-artifact`
(continues; Phase 3a is committed).

### What was built

- **`ke ml-check --hash <h>` — dev stand-in (loudly non-authoritative).**
  Real T2/T3 is platform-owned (ADR 0011); this command is a development
  stand-in only. Precondition: prior == `StructurallyVerified`. It builds a dev
  `ConsistencyBlock` via `ConsistencyBlockBuilder` with
  `execution_environment = "local-dev-standin"` (clearly non-authoritative),
  writes it to a **new registry sidecar** `consistency/<hash>.json` (**not** the
  artifact envelope), and appends an `ml_checked` event. The
  `consistency_block_present` precondition reads that sidecar object's presence.
- **`ke attest --hash <h> --type <t>` (repeatable) — expert attestations
  outside the envelope.** For each `--type`, builds an `Attestation` (expert key
  via `test_keys::expert_signing_key`, `key_id = TEST_EXPERT_KEY_ID`; fields
  pulled from the manifest: `regime_id`, `effective_from`/`to`,
  `ir_schema_version`, `compiler_version`; `legal_source_hash =
  manifest.source_corpus_hash`; `scope = WholeArtifact`; MockTsa-stamped at
  `now_unix`; `attestation_policy_version = "ap-1"`; `test_corpus_hash = None`),
  `sign_attestation`, then `decode_artifact` → `with_attestations(all)` →
  re-write `artifact.kew`. The post-envelope append property is **asserted**:
  `artifact_hash` is unchanged before/after (decode-and-compare; the §9
  immutability + Phase-1/2 content-address pins hold). When the set verifies
  under the policy context, appends `expert_attested` (precondition: prior ==
  `MlChecked` **and** `verify_attestation_set` ok).
- **`ke publish --hash <h> --env <env> [--tag <tag>] [--policy <bundle.json>]`
  — the policy gate.** Default `VerificationPolicy` requires `SourceFidelity` +
  `ScenarioCoverage` + `PublicationApproval`, count ≥1 each; `--policy` loads a
  `PolicyBundle` JSON (serde) and uses its `verification_policy`. Runs
  `verify_attestation_set`; on a missing required type it **fails with a typed
  error** (`AttestationSetRejected`, carrying the rejections — the policy gate).
  On pass + prior == `ExpertAttested`, appends `published` and writes the tag
  pointer via `put_pointer(env, tag, hash, event_ref)`.
- **`ke deprecate --hash <h>`.** Precondition prior == `Published`; appends
  `deprecated`.
- **`ke revoke --hash <h> --policy <hardstop|finishpinned|auditonly>
  [--reason <s>]`.** Precondition prior ∈ {`Published`, `Deprecated`}; appends a
  standard `revoked` event **plus** a `revocations/<hash>.json` sidecar
  `{policy, reason, event_ref, severity}` (severity = `high` for `AuditOnly`).
  The registry **records** policy + severity; **runtime enforcement**
  (fail/block/audit-emit) is platform/Gate 6 — documented as a boundary, not
  implemented here.
- **`ke rollback --env <env> [--tag current] --to <hash>` (ADR 0013).**
  Requires `is_rollback_eligible(current_state(--to)) == Published` (rejects
  `Deprecated`/`Revoked` with a typed `RollbackIneligible{state}` error); moves
  the tag pointer → `--to` and appends a `tag_moved` event.
- **Backend additions (additive to the trait + `LocalFsBackend`):**
  `put_consistency`/`read_consistency` (`consistency/<hash>.json`),
  `put_revocation`/`read_revocation` (`revocations/<hash>.json`). `put_pointer`
  already existed. New `RegistryError` arms: `RollbackIneligible{state}`,
  `AttestationSetRejected` (carries the rejections), `PolicyLoad`.

### Event shape unchanged — the 3a pin still holds

All new metadata (the dev consistency block, the revocation policy/reason/
severity) lives in **sidecar objects**, never in `LifecycleEvent`.
`LifecycleEvent` was not altered, so
`tests/registry.rs::canonical_event_head_hash_is_pinned` stays green. A new
`tests/lifecycle.rs` adds its own pins for the **published** and **revoked**
event-head hashes (hardcoded hex), so any accidental change to the lifecycle
event encoding flips them and fails.

### Two placement decisions, flagged for follow-up

- **(i) §8.1-vs-§9 `consistency_block` placement tension.** §8.1 lists
  `consistency_block` as an *in-envelope* component, but the §9 lifecycle
  attaches T2/T3 evidence *after* compile. These can't both hold for T2/T3: the
  in-envelope slot is part of the hashed/signed bytes, so populating it
  post-compile would change `artifact_hash` and break immutability + the
  Phase-1/2 content-address pins. **Resolution adopted in 3b:** the in-envelope
  `consistency_block` slot is reserved for *compile-time* T0/T1/T4 evidence only
  (a Phase-1 slot left `None`); **T2/T3 evidence is a registry sidecar**
  (`consistency/<hash>.json`), never the envelope. The envelope field stays
  `None`. **Recommendation:** raise a follow-up ADR if the envelope slot should
  ever carry compile-time evidence — 3b makes no envelope/contract change beyond
  the sidecar.
- **(ii) S3-WORM note on the `.kew` re-write.** `attest` re-writes
  `artifact.kew` in place to append attestations after the envelope. This is
  fine on local-FS. Under a future **S3-WORM** (Object-Lock) backend, an
  in-place re-write is not allowed — attestations would have to become
  **separate objects** behind the same `RegistryBackend` trait. Flagged, trait
  seam ready, not built.

### Gate evidence (integration runner, independently re-verified 2026-06-13; quoted verbatim)

> All Phase 3b integration gates independently re-run this session on branch
> migration/gate-4-artifact; every check passed with no fixes required.
>
> 1. FMT: `cargo fmt --all -- --check` -> exit 0 (clean).
>
> 2. CLIPPY (-D warnings, both feature sets):
>    - `cargo clippy --workspace --all-targets -- -D warnings` -> exit 0.
>    - `cargo clippy -p ke-cli --features test-keys --all-targets -- -D warnings` -> exit 0.
>
> 3. TESTS:
>    - `cargo test --workspace` -> exit 0, every binary green (sample totals: ke-artifact 7+17+8+3+6+3; ke-cli unit 2, lifecycle 5, registry 9; ke-compiler 2+2+3+1+4+2+4; ke-runtime unit 27 + metamorphic 5 + property 3 + trace 1; ke-core round-trip/non-canonical/etc all green; 0 failed across the workspace).
>    - `cargo test -p ke-cli --features test-keys` -> lifecycle 5 passed, registry 9 passed (incl. the 3a pin `canonical_event_head_hash_is_pinned`), unit 2 passed, 0 failed. The 3a event-head pin still holds -> LifecycleEvent shape unchanged.
>
> 4. NO-FEATURE GATE: `cargo build -p ke-cli` (no features) -> exit 0. Each of the six commands returns its typed "requires --features test-keys" error (verbatim, e.g. ml-check: "error: `ke ml-check` requires the `test-keys` feature ... Build with `--features test-keys`. Production signing keys are an infra/ADR-0009 concern."), all exit 1. Confirmed for ml-check/attest(valid type)/publish/deprecate/revoke/rollback.
>
> 5. FORBIDDEN-DEPS GREP over crates/ke-cli/src for OsRng|getrandom|tokio|aws_|aws-sdk|.await|async fn|async {: 6 hits, ALL doc-comment lines (// or //!). Zero code usage; no async.
>
> 6. lifecycle.rs PINS hardcoded (grep confirmed at lines 407, 413): published head 24ca20b500735f2fe3840c89f3a9e9ebc39faf98508834c86cfd6422f7614328; revoked head c7429bba9673837c21749fb99de690ea0c1b8cc5bd9e1a513b3e283686ed6b74. Required test names all present: full_lifecycle_happy_path, publish_rejected_when_required_type_missing, rollback_to_published_ok_and_to_revoked_ineligible, revoke_auditonly_records_high_severity, published_and_revoked_event_heads_are_pinned; attest-hash-unchanged assertion at lines 159-174 (decode before/after, assert artifact_hash == hash_before == registry hash).
>
> 7. SMOKE: `bash scripts/lifecycle-smoke.sh` -> exit 0, "lifecycle-smoke: PASS"; its internal twice-run determinism check (events/artifacts/tags/consistency/revocations byte-identical) passed. I then ran my OWN independent twice-run into persistent dirs: same content hash 4fa59822189929b1f814dffd68b2f10786cf042eef6a78ec0981f025a9f9c5c2 both runs; diff -r of all five subtrees -> IDENTICAL each. NON_AUTHORITATIVE marker present and reads the ADR-0012 section 6 non-authoritative notice.
>
> 8. BOUNDARY CHECKS (independent decode of a fresh run):
>    - tags/staging/current.json -> target_hash_hex = published hash, event_ref published@seq4.
>    - revoked event (events/<h>/0006-revoked.json): standard fields only (event_kind=revoked, new_state=Revoked, authority_role=Registry, authority_key_id=test-registry-fixed-seed-1); NO policy/severity/reason field -> LifecycleEvent shape untouched.
>    - revocations/<h>.json sidecar = {policy: AuditOnly, reason, event_ref: revoked@seq6, severity: high} -> policy/severity RECORDED, no enforcement.
>    - consistency/<h>.json sidecar: execution_environment=local-dev-standin, policy_mode=Advisory, loudly non-authoritative.
>    - CONTENT-ADDRESS proof via a throwaway example calling verify_hash (the proper re-zero recompute, NOT blake3 of raw .kew): before attest = 7360 bytes / envelope_len 7276 / 0 attestations / hash 4fa598...c5c2; after attest = 8164 bytes (file grew 804B) / envelope_len STILL 7276 / 3 attestations / verify_hash STILL 4fa598...c5c2; consistency_block_is_none=true in BOTH. Attestations appended post-envelope; content address unchanged; in-envelope consistency_block stays None (dev block is sidecar-only).

**Test counts** (same report): `cargo test -p ke-cli --features test-keys` =
**16 passed, 0 failed** (unit 2 + lifecycle 5 + registry 9 — registry includes
the 3a `canonical_event_head_hash_is_pinned`). `cargo test --workspace` = **0
failed across all crates**: ke-cli 16, ke-artifact 44 (7+17+8+3+6+3),
ke-compiler 17, ke-runtime 36 (27+5+3+1), ke-core 19, ke-wasm 0; doc-tests 0.
`tests/lifecycle.rs` = 5 named tests covering the full happy path, the policy-
gate rejection, rollback-ineligibility, AuditOnly high-severity, the attest-
hash-unchanged assertion, and the published+revoked hardcoded hex pins
(`24ca20b5…4328` published head, `c7429bba…6b74` revoked head).

### Implemented vs recorded-not-enforced vs deferred (honest boundary)

- **Implemented (this repo, local-FS, behind `test-keys`):** the six lifecycle
  commands, the dev consistency sidecar, expert attestation + the
  `verify_attestation_set` publish gate, the revocation-policy + severity
  sidecar, ADR-0013 rollback eligibility, the no-feature typed-error surface.
- **Recorded, not enforced (boundary):** the revocation policy/severity is
  *recorded* in the sidecar; **runtime enforcement** (fail/block/audit-emit) is
  platform/Gate 6. The `ml-check` consistency block is a **dev stand-in**
  (`local-dev-standin`), explicitly non-authoritative.

### What Phase 3b deliberately EXCLUDES (deferred, not done)

- **Real T2/T3 sidecar evidence** — platform-owned (ADR 0011); `ml-check` ships
  only a loudly-marked dev stand-in.
- **Runtime revocation enforcement** — platform/Gate 6; the registry only
  records state + policy + severity.
- **Registry-root HSM custody + signed key-directory object + root rotation** —
  ADR 0009, infra.
- **Real S3 backend + Object-Lock/versioning + attestations-as-separate-objects
  under WORM** — trait seam ready; the local-FS `.kew` re-write is the dev path.
- **PyO3 / `ke-artifact-py`** (Phase 4), **`contract-test.sh`** (Phase 5),
  **`ke serve`** (Gate 5).
- The §8.1-vs-§9 `consistency_block` placement is **flagged for a possible
  follow-up ADR**, not resolved here.

Phase 3b is ready for Hossain review/commit on `migration/gate-4-artifact`.
**Phase 3 (registry lifecycle) is complete (3a + 3b).**

---

## Phase 4a — consumer-agnostic verify surface + provenance export (ADR 0016 rescope; pure Rust core) (2026-06-13)

Phase 4a delivers the **pure, CI-testable core** of the rescoped Phase 4. The
brief's original Phase 4 was **Python-only** (`ke-artifact-py` PyO3 wheel for the
hypothetical `institutional-defi-platform-api` consumer). **ADR 0016**
(`docs/adr/0016-phase4-consumer-agnostic-verification.md`, **Accepted** sign-off
by Hossain 2026-06-13) rescopes it: Phase 4 delivers **one** consumer-agnostic
verification surface + a provenance export carrying registry state, with **both**
bindings (PyO3 + WASM) sitting thinly over that single surface — splitting the
binding effort doubles the cross-language contract surface. ADR 0016 also pulls
`ke-wasm` verification **into Gate 4** (COMPASS, a live consumer today, needs
in-browser verify + revocation now; it reuses the Phase 1–2 surface verbatim) and
keeps `ke-cli serve` (REST/WS) in Gate 5. Spec refs §6 (WASM stays
preview/verify-only — never signs/publishes), §14, §16. Approving the plan was
the gate-discipline sign-off for the rescope; ADR 0016 + the brief §5 amendment +
the `docs/adr/README.md` index entry (line 59) were written **with** the code.

**This is bindings-prep + provenance export, NOT new crypto.** The verify path
already existed as pure, RNG-free, I/O-free functions in `ke-artifact`
(`decode_artifact`, `verify_hash`, `verify_signature`, `verify_attestation_set`);
ed25519 *verify* is deterministic — only signing/`test_keys` touch RNG and stay
feature-gated. 4a wraps those into one call and a serde provenance struct.

**Confirmed split:** **4a now** = ADR 0016 + the pure Rust core (no new
toolchain). **4b next** = PyO3 wheel + `ke-wasm` wasm-bindgen verifier + the
`@platform/atlas-artifact` npm package + the 3-language `contract-test.sh`.

### What was built

- **`crates/ke-artifact/src/verify.rs` (new) — the pure consumer surface,
  re-exported from `lib.rs`.** One entry point folds the four existing pure
  verifiers plus registry state into a single verdict:
  - `verify_artifact(kew: &[u8], keydir: &KeyDirectory, ctx: &PolicyContext,
    registry: RegistryEvidence) -> VerificationOutcome`. Order, first failure
    short-circuits to `Rejected`: `decode_artifact` (→ `RejectionReason::Decode`)
    → `verify_hash` (→ `HashMismatch`) → `verify_signature` over the envelope
    prefix `[0, envelope_len)` with the compiler key (→
    `CompilerSignatureInvalid`) → `verify_attestation_set` (→
    `Attestations(Vec<AttestationRejection>)`) → if `registry.status !=
    Published` ⇒ `NotPublished{status}` → if `registry.live_event_head` is `Some`
    and `!= event_head_hash` ⇒ `StaleEventHead{embedded, live}`; otherwise
    `Verified`. **It always builds provenance** (even on rejection) and performs
    **no I/O and no RNG** — registry state arrives as data, so the surface stays
    WASM-ready.
  - `enum Verdict { Verified, Rejected(RejectionReason) }`;
    `enum RejectionReason { HashMismatch, CompilerSignatureInvalid,
    Attestations(Vec<AttestationRejection>), NotPublished{status},
    StaleEventHead{embedded, live}, Decode(String) }`;
    `struct VerificationOutcome { verdict, provenance, registry_state }`.
  - `enum RegistryStatus { Published, Deprecated, Revoked, Unknown }` — an
    **ke-artifact-local mirror** of the registry lifecycle state, so the crate
    stays backend-free.
  - `struct RegistryEvidence { status, event_head_hash: [u8;32],
    live_event_head: Option<[u8;32]> }` — status + head as-of-export;
    `live_event_head` (if `Some`) is a freshly-fetched head for **staleness
    detection** against an offline export.
- **`ArtifactProvenance` canonical export (in `ke-artifact`, plain serde so both
  4b bindings emit/read it).** `regime_id`, `artifact_hash`, the version triplet
  (`ir_schema_version` / `codec_version` / `canonicalization_version`),
  `signer_key_id` + **`is_test_key: bool`** (`signer_key_id.starts_with("test-")`
  — surfaces that `test-*` keys are not production), `attestations:
  Vec<AttestationSummary>`, `registry_state: RegistryStatus`,
  `registry_event_head_hash: [u8;32]`, `exported_at_unix: u64`.
  `to_canonical_json()` → one stable JSON (serde_json, key order = struct field
  order). `AttestationSummary { attestation_type, signer_key_id, is_test_key,
  tsa_class, claimed_time_unix }` carries **no signature bytes**.
  `artifact_provenance(artifact, registry, exported_at_unix) -> ArtifactProvenance`
  is pure.
- **`crates/ke-cli/src/commands/export_provenance.rs` + an `export-provenance`
  subcommand — the only registry-touching part.** Reads the artifact `.kew`
  (`decode_artifact`) and the event log: `registry::current_state` → mapped to
  `ke_artifact::RegistryStatus` (`Published`/`Deprecated`/`Revoked` else
  `Unknown`) via `status_for`; `registry::head_event` → `chain_hash()` =
  `event_head_hash`; builds `RegistryEvidence{status, event_head_hash,
  live_event_head: None}`, calls `artifact_provenance`, prints canonical
  serde_json and optionally writes `artifacts/<hash>/provenance.json`.
  `exported_at` comes from `--now`/`KE_NOW`. The ke-cli→ke-artifact dependency is
  one-way: the lifecycle-state→`RegistryStatus` mapping happens **at this
  boundary**, never the reverse.

### The layering (the load-bearing discipline)

`verify_artifact` and `artifact_provenance` are **pure / RNG-free / backend-free**
— no `std::fs`, no `std::net`, no `tokio`/`reqwest`, no `OsRng`/`getrandom` in
`verify.rs` or its transitive callees (`decode_artifact`, `verify_hash`,
`verify_signature`, `verify_attestation_set`, `KeyDirectory::lookup`,
`VerifyingKey::from_bytes`); ed25519 verify is deterministic.
**`ke-artifact` has no `ke-cli` dependency** so the future WASM binding needs no
filesystem. **All registry reading is confined to `ke-cli`** (the
`export-provenance` producer). This is what makes 4b's PyO3 and WASM bindings
*thin over one surface*.

### The COMPASS correctness fix (proven, not asserted)

COMPASS today surfaces ATLAS provenance **"surfaced, not re-verified,"** reads a
sibling `fixtures/` dir absent on Vercel, and **has no revocation channel** — so
it can show a **revoked** pack as authoritative. The export embeds **registry
state + the event-head hash** so an offline consumer (1) **refuses non-`Published`
packs** and (2) **detects staleness** against a live head. Both are closed by
named tests, not prose:

- `rejected_when_revoked` — `RegistryStatus::Revoked` with otherwise-valid crypto
  ⇒ `Verdict::Rejected(NotPublished{Revoked})`. This is the exact COMPASS bug the
  rescope closes.
- `stale_event_head` — `live_event_head` ≠ the embedded `event_head_hash` ⇒
  rejected with `StaleEventHead{embedded, live}`.

### Gate evidence (integration runner, independently re-verified 2026-06-13; quoted verbatim)

> All seven gate items verified independently this session; no fixes were needed
> (builder work was already correct). Working tree left unchanged (only mtime
> touches, zero content diffs).
>
> 1. STATIC GATES: `cargo fmt --all -- --check` -> clean (no output).
>    `cargo clippy --workspace --all-targets -- -D warnings` -> "Finished `dev`
>    profile ... in 1.02s" (clean). `cargo clippy -p ke-artifact -p ke-cli
>    --all-targets --features test-keys -- -D warnings` (after forcing rebuild
>    via touch) -> "Checking ke-artifact ... Checking ke-cli ... Finished"
>    (clean).
>
> 2. TESTS: `cargo test --workspace` -> NO FAILURES; summed 141 passed, 0 failed
>    across all suites. New 4a suites both ran under default `cargo test
>    --workspace` (feature unification via the self dev-dependency with
>    features=["test-keys"]): verify_surface.rs = 6 passed, export_provenance.rs =
>    2 passed. Prior suites intact: ke-artifact lib 7, attestation 17, golden 8,
>    hash_offset 3, non_canonical 6, sign 3; ke-cli lib 2, lifecycle 5 (incl. 3a
>    event-head pin), registry 9; ke-compiler lowering/parser_spans/python_import/
>    t4_conflicts/t4_corpus/verify_t0_t1; ke-core round_trip 6, non_canonical 8,
>    schema_determinism 3, artifact_hash_offset 2; ke-runtime lib 27, metamorphic
>    5, property 3, trace_fixtures 1.
>
> 3. PURE-BUILD: `cargo build -p ke-artifact` (no features) -> "Finished"
>    (verify surface compiles with no feature, no RNG).
>
> 4. RNG-FREE GATE: grep getrandom|OsRng|rand:: in
>    crates/ke-artifact/src/verify.rs -> ZERO. Crate-wide hits only in sign.rs
>    (lines 11,53-54) and tsa.rs (line 17) -- all DOC COMMENTS asserting absence;
>    no executable RNG anywhere. verify.rs grep for std::fs|std::net|File::|
>    reqwest|tokio|fn main -> NONE (no I/O). Confirmed verify_artifact's
>    transitive callees (decode_artifact, verify_hash, verify_signature,
>    verify_attestation_set, KeyDirectory::lookup, VerifyingKey::from_bytes) are
>    the RNG/IO-free side; ed25519 verify is deterministic. ke-artifact/Cargo.toml
>    has NO ke-cli dependency (stays backend-free).
>
> 5. verify_surface.rs named cases present (cargo test --list):
>    verified_published_golden, rejected_bad_sig, rejected_missing_attestation,
>    rejected_when_revoked (Revoked->Rejected(NotPublished{Revoked}) with valid
>    crypto), stale_event_head, plus provenance_canonical_json_is_stable. All 6
>    pass.
>
> 6. EXPORT-PROVENANCE:
>    ke-cli/tests/export_provenance.rs::export_provenance_tracks_published_then_revoked
>    drives compile->ml_check->attest->publish->revoke; asserts
>    registry_state==Revoked, hash_hex(registry_event_head_hash)==
>    head_event(...).chain_hash() off the log (independent of the command),
>    is_test_key==true, attestations.len()==3. Sidecar test writes provenance.json
>    byte-equal to canonical JSON. Both pass.
>
> 7. INDEPENDENT RECOMPUTE (python blake3, not the Rust surface):
>    fixtures/artifacts/rule_reserve_assets/artifact.kew, envelope_len=862;
>    located the 32-byte claimed hash slot at offset 1 in the prefix, re-zeroed
>    it, blake3(prefix).hex() =
>    bcebbd1f89619efbab253e9fb463fa089b0d487a28064006ec6fd7a43a0ccb87 ==
>    GOLDEN.md == manifest claim. MATCH: True. The verify_surface
>    verified_published_golden test wraps this exact .kew with
>    RegistryStatus::Published and asserts Verdict::Verified -> agrees with the
>    recompute and with ke-cli.
>
> 8. DOCS: docs/adr/0016-phase4-consumer-agnostic-verification.md -> "**Status:**
>    Accepted (sign-off by Hossain, 2026-06-13)", spec refs §6/§14/§16, splits
>    4a/4b. docs/adr/README.md line 59 indexes 0016 as Accepted under Gate 4.
>    dev/briefs/gate-4-artifact-registry-attestation.md §5 (lines 235-260)
>    "Phase 4 -- consumer-agnostic verification + provenance export ... (RESCOPED
>    by ADR 0016)" with "Phase 4a (delivered)" and "Phase 4b (next)".

**Test counts** (same report): `cargo test --workspace` = **141 passed, 0 failed,
0 ignored**. New 4a suites: `ke-artifact/tests/verify_surface.rs` **6 passed**
(`verified_published_golden`, `rejected_bad_sig`, `rejected_missing_attestation`,
`rejected_when_revoked`, `stale_event_head`, `provenance_canonical_json_is_stable`);
`ke-cli/tests/export_provenance.rs` **2 passed**
(`export_provenance_tracks_published_then_revoked`,
`export_provenance_write_root_writes_sidecar`). clippy default + `test-keys`
clean; fmt clean; `cargo build -p ke-artifact` (no features) clean.

### Implemented vs deferred (honest boundary)

- **Implemented (4a, this repo):** the pure `verify_artifact` surface +
  `VerificationOutcome`/`Verdict`/`RejectionReason`; `RegistryStatus`/
  `RegistryEvidence`; `ArtifactProvenance` + `AttestationSummary` canonical-JSON
  export with registry state + event-head hash + `is_test_key`; the `ke-cli
  export-provenance` registry-reading producer (stdout + optional sidecar); the
  COMPASS revoked/stale fix proven by `rejected_when_revoked` + `stale_event_head`.
- **Deferred to 4b:** PyO3 `ke-artifact-py` wheel + maturin + the S3 PEP-503
  simple index; the `ke-wasm` wasm-bindgen verifier + the `@platform/atlas-artifact`
  npm package (verifier WASM + TS types + provenance reader); the 3-language
  `contract-test.sh` (Rust ≡ Python ≡ WASM — same `.kew` → identical verdict +
  provenance).
- **Deferred / follow-up (credentialed or downstream):** actual publishing of the
  wheel/npm package (credentialed, Hossain-driven); the **COMPASS Desk-MVP rewire**
  ("surfaced, not re-verified" → in-browser verified + revoked-pack flagging),
  sequenced **after** 4b ships the npm package; `ke-cli serve` (REST/WS) stays
  **Gate 5**.

Phase 4a is ready for Hossain review/commit on `migration/gate-4-artifact`.
