# Gate 4 — acceptance record

Maps each of the four spec §19 / brief §7 Gate-4 acceptance criteria (C1–C4) to
the **ATLAS-side evidence** that backs it and an honest status. This is the
gate-review artifact for `migration/gate-4-artifact`.

**Read this first — the honest boundary.** The four acceptance criteria were
originally written from a **platform consumer's** point of view ("when loaded by
*the platform*", "when *the platform* executes"). **ADR-0017 (2026-06-15)
decoupled `institutional-defi-platform-api`** from the ATLAS artifact path: it is
**not** the consumer. The real consumer is **COMPASS** (verify-only, in-browser),
whose integration is gated **after Gate 5**. C1/C2 are therefore redefined below
(verifier + equivalence foundation MET in-repo; consumer integration deferred to
COMPASS; the platform execute-parity is obsolete). This repo does not fabricate
any consumer execution.

So the split is:

- **C3 is MET in this repo** — rejection semantics, proven by named, re-runnable
  tests (the R1–R8 matrix + the surface tests) plus the cross-language contract
  test exercising a rejected verdict identically across Rust/Python/WASM.
- **C4 is MET in this repo** — rollback *eligibility* (only `Published` is a valid
  target; else typed `RollbackIneligible`), the tag pointer-move + `tag_moved`
  event, **and** the literal §19 criterion are all proven, with ADR-0013 governing
  the rule. The dedicated test publishes two *distinct* content hashes under one
  tag, rolls the tag back, and asserts resolve-by-tag returns the **previous
  distinct** hash: `crates/ke-cli/tests/lifecycle.rs`
  `rollback_reresolves_to_previous_distinct_hash`.
- **C1 and C2 cannot be closed from this repo** — they require the platform to
  *load* and *execute*. What ATLAS delivers for them is the **verifier** (the
  pure surface + both bindings, proven 3-language-consistent) and the
  **runtime-parity foundation** (the Gate-3 equivalence harness). The
  consumer-side **integration** is **COMPASS's**, deferred to the post-Gate-5
  COMPASS rewire (ADR-0017: platform-api is decoupled, not the consumer). The
  ATLAS deliverables for C1/C2 are MET in-repo; the cross-repo execute-parity that
  C2 originally named is **obsolete** (no production Python target).

`✅ MET (in-repo)` = closed by ATLAS-side tests now. The C1 consumer integration
is deferred to COMPASS (post-Gate-5); the C2 platform execute-parity is obsolete
per ADR-0017.

---

## Criteria → evidence → status

| # | Criterion (spec §19 / brief §7) | ATLAS-side evidence (file:line / test / command) | Status |
|---|---------------------------------|---------------------------------------------------|--------|
| **C1** | Signed artifact loaded by **a consumer** → consumer verifies hash, canonical encoding, compiler signature, required attestations, key validity, **and registry state** before use. *(Consumer redefined — see ADR-0017: COMPASS via the WASM verifier, not platform-api.)* | **Verifier delivered + 3-language-consistent (ATLAS): MET.** `crates/ke-artifact/src/verify.rs` `verify_artifact()` folds decode → `verify_hash` (re-zero-slot, not naive whole-file) → `verify_signature` → `verify_attestation_set` → registry-state check; same verdict + canonical provenance across **Rust ≡ Python ≡ WASM** (`bash scripts/contract-test.sh`, PASS over both goldens; verbatim line in the Phase-4b log). Verified-path test: `crates/ke-artifact/tests/verify_surface.rs:139` `verified_published_golden`. The **consumer-side integration** (in-browser verify + revoked-pack flagging) is **COMPASS's, deferred to the post-Gate-5 COMPASS rewire** (ADR-0017). | ✅ VERIFIER MET (in-repo); consumer integration deferred to COMPASS (post-Gate-5) |
| **C2** | Known scenario, **a consumer** executes a Rust-compiled artifact → output matches the current Python pipeline. *(Redefined — see ADR-0017: no platform-api consumer; no production Python target.)* | **Runtime-parity foundation (ATLAS Gate-3): MET.** `scripts/equivalence-harness.sh` proves Rust runtime ≡ Python `RuleRuntime`. With platform-api **decoupled** (ADR-0017), the "a consumer executes and matches the **production** Python pipeline" half is **obsolete** — there is no such consumer or production Python target; COMPASS is consumer-only (verify, not execute). ATLAS runtime equivalence stands as an internal correctness property. | ✅ EQUIVALENCE FOUNDATION MET (in-repo); platform execute-parity obsolete (ADR-0017) |
| **C3** | Missing / stale / revoked / invalid attestations → execution **rejected with a specific policy error**. | **MET in-repo.** The typed R1–R8 rejection matrix, one named test per variant asserting the exact variant: `crates/ke-artifact/tests/attestation.rs` — `r1a_unknown_key_rejected:151`, `r1b_expired_key_rejected:163`, `r1c_revoked_key_rejected:184`, `r1d_unauthorized_key_rejected:196`, `r2_unbound_artifact_hash_rejected:226`, `r3_unsupported_policy_version_rejected:249`, `r4_expired_attestation_rejected:266`, `r5_legal_source_hash_change_rejected:284`, `r6_required_type_missing_rejected:306`, `r7_publication_approval_without_coattestations_rejected:345`, `r8_mock_tsa_non_local_rejected_local_accepted:399`. Surface-level rejections: `crates/ke-artifact/tests/verify_surface.rs` `rejected_bad_sig:178`, `rejected_missing_attestation:212`, **stale** head `stale_event_head:282`, **revoked** `rejected_when_revoked:251` (`RegistryStatus::Revoked` + valid crypto ⇒ `Rejected(NotPublished{Revoked})`). The contract test proves a **cross-language identical rejection reason set** for `rule_significant_thresholds` (`Attestations([LegalSourceHashChanged x3, RequiredTypeMissing{SourceFidelity, ScenarioCoverage, PublicationApproval}])`, identical in Rust/Python/WASM). | ✅ MET (in-repo) |
| **C4** | Registry rollback → a new workflow resolving by tag resolves to the **previous signed content hash**. | **MET in-repo.** The literal §19 criterion: `crates/ke-cli/tests/lifecycle.rs` `rollback_reresolves_to_previous_distinct_hash` publishes two **distinct** content hashes (A=`mica_stablecoin`, B=`mica_authorization`) under one `staging/current` tag, lets B's publish move the pointer, rolls the tag back to A, and asserts resolve-by-tag returns the **previous distinct** hash A (chain still validates; `tag_moved` appended). Eligibility + the rejection path: `rollback_to_published_ok_and_to_revoked_ineligible` (rollback to `Published` moves the pointer; `Revoked`/`Deprecated` → typed `RollbackIneligible{state}`). Event-shape regression pins: `published_and_revoked_event_heads_are_pinned`. ADR-0013 eligibility predicate `is_rollback_eligible` = `Published` only. | ✅ MET (in-repo) |

---

## How to independently verify

A reviewer reproduces every in-repo claim above with the following (each is
load-bearing — re-run them, don't trust the summary):

```bash
# 1. Whole workspace green (C3/C4 tests + everything else; pyo3 stays out of the
#    default graph). Expect: 142 passed, 0 failed.
cargo test --workspace

# 2. C3 — the typed rejection matrix (one named test per R1–R8 variant) +
#    the revoked/stale surface rejections, run with the signing feature.
cargo test -p ke-artifact --features test-keys --test attestation
cargo test -p ke-artifact --features test-keys --test verify_surface

# 3. C4 — rollback eligibility + tag re-resolution + the event-head pins.
cargo test -p ke-cli --features test-keys --test lifecycle

# 4. C1 verifier readiness — the same verdict + canonical provenance in
#    Rust ≡ Python ≡ WASM over both goldens. Requires the sibling platform
#    checkout at the SOURCE.md SHA (SHA-gated); builds the wheel + WASM leg.
bash scripts/contract-test.sh
```

Named tests to confirm by name in the output:

- C3: `r1a_unknown_key_rejected … r8_mock_tsa_non_local_rejected_local_accepted`
  (12 matrix tests in `attestation.rs`), `rejected_when_revoked`,
  `stale_event_head`, `rejected_bad_sig`, `rejected_missing_attestation`.
- C4: `rollback_reresolves_to_previous_distinct_hash` (the literal prior-distinct-hash
  criterion), `rollback_to_published_ok_and_to_revoked_ineligible`,
  `published_and_revoked_event_heads_are_pinned`.
- C1 verifier: `verified_published_golden`; `contract-test.sh` prints
  `PASS: every present leg agrees on verdict + canonical provenance over all
  goldens`.

The contract test recomputes the golden content hashes from raw `.kew` bytes
with an **independent pure-Python BLAKE3** (re-zeroing the 32-byte slot at
offset 1) and matches `fixtures/artifacts/GOLDEN.md`
(`rule_reserve_assets` = `bcebbd1f…ccb87`, `rule_significant_thresholds` =
`a0a06ee4…f66bf`).

---

## Consumer integration (cross-repo) — COMPASS, post-Gate-5

ADR-0017 decoupled `institutional-defi-platform-api` from the ATLAS artifact path.
The **real consumer is COMPASS** (`cross-border-compliance-navigator`), and it is
**consumer-only** (in-browser verify, not execute). The ATLAS deliverables for
C1/C2 are MET in-repo (above); what remains is the COMPASS-side integration,
gated **after Gate 5 + the Hossain npm publish** of `@platform/atlas-artifact`.

- **C1 (consumer verifies before use).** ATLAS-side (MET): `verify_artifact` in
  `verify.rs`, proven 3-language-consistent (`scripts/contract-test.sh`), exposed
  to the browser as the verify-only `ke-wasm` / `@platform/atlas-artifact` package.
  COMPASS-side (deferred, post-Gate-5): call `verify_artifact` in-browser on the
  fetched artifact + registry evidence, and flag a non-`Published` pack as blocked
  even with valid crypto. See `dev/briefs/compass-consumer-state-and-gate5-rewire.md`.
- **C2 (execution parity).** ATLAS-side (MET): the Gate-3 equivalence harness
  (`scripts/equivalence-harness.sh`) proves the Rust runtime ≡ Python `RuleRuntime`.
  The platform execute-parity this criterion originally named is **obsolete** —
  with platform-api decoupled there is no production Python pipeline to diff
  against, and COMPASS verifies rather than executes.

The retired platform-consumption brief (`dev/briefs/gate-4-platform-consumption.md`)
is **stale** and is not a live deliverable (ADR-0017).

---

## Known residue (carried forward, honest)

Open items that do **not** block the in-repo C3/C4 closure or the verifier
delivery, recorded so review is complete:

- **§8.1-vs-§9 `consistency_block` placement** — a follow-up ADR is flagged (not
  yet written). 3b's adopted resolution: the in-envelope slot is reserved for
  compile-time T0/T1/T4 evidence and left `None`; T2/T3 evidence lives in a
  registry sidecar (`consistency/<hash>.json`). See the Phase-3b log entry,
  "Two placement decisions, flagged for follow-up (i)".
- **Real S3 registry backend** — the `RegistryBackend` trait seam is ready; only
  the `LocalFsBackend` (objects flagged `NON_AUTHORITATIVE`, ADR 0012 §6) ships.
  Under a future S3-WORM backend the `attest` in-place `.kew` re-write must
  become separate objects (3b flag (ii)).
- **HSM custody + signed key-directory object + root-key rotation** — ADR 0009
  infra; not built. Every committed signature uses loud fixed-seed `test-*`
  keys (`test-fixed-seed-1`, `test-expert-fixed-seed-1`, `test-mock-tsa-1`,
  `test-registry-fixed-seed-1`), asserted `test-`-prefixed so they can never be
  mistaken for production keys.
- **Runtime revocation enforcement = Gate 6.** The registry only **records**
  revocation state + policy + severity (`revocations/<hash>.json` sidecar);
  fail/block/audit-emit at execution time is platform/Gate 6. The verifier
  *does* reject non-`Published` packs at verify time (`rejected_when_revoked`),
  which closes the COMPASS "revoked pack shown as authoritative" bug — distinct
  from runtime enforcement.
- **Publish (credentialed, downstream):** actual publish of the wheel to the
  S3-backed PEP 503 index and the npm `@platform/atlas-artifact` package, and the
  COMPASS Desk-MVP rewire to the published WASM verifier — Hossain follow-ups,
  sequenced after the package ships.
- **RFC 3161 TSA** — `TsaUnsupported` is the honest stand-in until vendor
  onboarding (ADR 0010); dev/test uses the deterministic `MockTsa`.

---

## Bottom line

- **C3: met in this repo**, by named re-runnable tests + the cross-language
  contract test.
- **C4: met in this repo** — eligibility + pointer-move + the rollback mechanism,
  and the literal "resolve-by-tag → previous *distinct* hash" criterion, are all
  proven by named re-runnable tests (`rollback_reresolves_to_previous_distinct_hash`).
- **C1: verifier met in-repo** (delivered + 3-language-consistent). The consumer
  is **COMPASS**, not platform-api (ADR-0017); its in-browser verify + revoked-pack
  integration is **deferred to the post-Gate-5 COMPASS rewire**.
- **C2: equivalence foundation met in-repo** (Gate-3 harness, Rust ≡ Python
  `RuleRuntime`). Platform-api is **decoupled** (ADR-0017), so the "consumer
  executes vs the production Python pipeline" half is **obsolete** — no such
  consumer/target exists; COMPASS is verify-only.

With platform-api decoupled (ADR-0017), **Gate 4 closes on ATLAS evidence: C1
(verifier) + C2 (equivalence foundation) + C3 + C4 are all MET in-repo.** The
producer→consumer loop is demonstrated end-to-end later, when COMPASS rewires onto
the published WASM verifier (post-Gate-5).
