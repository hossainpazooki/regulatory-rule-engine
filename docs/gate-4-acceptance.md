# Gate 4 — acceptance record

Maps each of the four spec §19 / brief §7 Gate-4 acceptance criteria (C1–C4) to
the **ATLAS-side evidence** that backs it and an honest status. This is the
gate-review artifact for `migration/gate-4-artifact`.

**Read this first — the honest boundary.** The four acceptance criteria are
written from the **platform's** point of view ("when loaded by *the platform*",
"when *the platform* executes"). The consumer is the sibling repo
`institutional-defi-platform-api`, which **does not yet consume ATLAS
artifacts**. Per the brief (§1, §10) and CLAUDE.md, the platform-side change is a
**separate PR in the platform repo** with its own brief; this workflow does not
modify the sibling repo and does not fabricate platform execution.

So the split is:

- **C3 is MET in this repo** — rejection semantics, proven by named, re-runnable
  tests (the R1–R8 matrix + the surface tests) plus the cross-language contract
  test exercising a rejected verdict identically across Rust/Python/WASM.
- **C4 is PARTIALLY met in this repo** — rollback *eligibility* (only `Published`
  is a valid target; else typed `RollbackIneligible`) and the tag pointer-move +
  `tag_moved` event are proven, and ADR-0013 governs the rule. **But no test yet
  demonstrates the literal §19 criterion** — publishing two *distinct* content
  hashes under a tag, rolling the tag back, and asserting resolve-by-tag returns
  the **previous distinct** hash. The mechanism is in place (rollback moves the
  pointer to an eligible prior hash; new workflows resolve by tag at start); the
  end-to-end *prior-distinct-hash re-resolution* is not covered by a dedicated
  test. Tracked as a small follow-up test below.
- **C1 and C2 cannot be closed from this repo** — they require the platform to
  *load* and *execute*. What ATLAS delivers for them is the **verifier** (the
  pure surface + both bindings, proven 3-language-consistent) and the
  **runtime-parity foundation** (the Gate-3 equivalence harness). The
  **integration** acceptance is PENDING the platform-repo PR. C1/C2 are **not**
  marked "met" here.

`✅ MET (in-repo)` = closed by ATLAS-side tests now. `🟡 VERIFIER DELIVERED —
INTEGRATION PENDING` = ATLAS provides the mechanism + contract; the platform PR
must demonstrate the end-to-end behavior.

---

## Criteria → evidence → status

| # | Criterion (spec §19 / brief §7) | ATLAS-side evidence (file:line / test / command) | Status |
|---|---------------------------------|---------------------------------------------------|--------|
| **C1** | Signed artifact loaded by **the platform** → platform verifies hash, canonical encoding, compiler signature, required attestations, key validity, **and registry state** before execution. | The **verifier the platform will call** is delivered and the same verdict + canonical provenance is proven across **Rust ≡ Python ≡ WASM**: `crates/ke-artifact/src/verify.rs` `verify_artifact()` folds decode → `verify_hash` (re-zero-slot, not naive whole-file) → `verify_signature` → `verify_attestation_set` → registry-state check. Cross-language consistency: `bash scripts/contract-test.sh` (PASS over both goldens; verbatim success line in the Phase-4b log). Verified-path test: `crates/ke-artifact/tests/verify_surface.rs:139` `verified_published_golden`. | 🟡 VERIFIER DELIVERED — INTEGRATION PENDING (platform-repo PR) |
| **C2** | Known scenario, **the platform** executes a Rust-compiled artifact → output matches the current Python pipeline. | **PLATFORM-side; not closable here.** Runtime-parity *foundation* is the Gate-3 equivalence harness `scripts/equivalence-harness.sh` (Rust runtime ≡ Python `RuleRuntime`). The artifact-based execute-parity (platform loads a `.kew`, executes, diffs vs the Python pipeline) is platform work in the separate PR. | 🟡 FOUNDATION DELIVERED — EXECUTE PARITY PENDING (platform-repo PR) |
| **C3** | Missing / stale / revoked / invalid attestations → execution **rejected with a specific policy error**. | **MET in-repo.** The typed R1–R8 rejection matrix, one named test per variant asserting the exact variant: `crates/ke-artifact/tests/attestation.rs` — `r1a_unknown_key_rejected:151`, `r1b_expired_key_rejected:163`, `r1c_revoked_key_rejected:184`, `r1d_unauthorized_key_rejected:196`, `r2_unbound_artifact_hash_rejected:226`, `r3_unsupported_policy_version_rejected:249`, `r4_expired_attestation_rejected:266`, `r5_legal_source_hash_change_rejected:284`, `r6_required_type_missing_rejected:306`, `r7_publication_approval_without_coattestations_rejected:345`, `r8_mock_tsa_non_local_rejected_local_accepted:399`. Surface-level rejections: `crates/ke-artifact/tests/verify_surface.rs` `rejected_bad_sig:178`, `rejected_missing_attestation:212`, **stale** head `stale_event_head:282`, **revoked** `rejected_when_revoked:251` (`RegistryStatus::Revoked` + valid crypto ⇒ `Rejected(NotPublished{Revoked})`). The contract test proves a **cross-language identical rejection reason set** for `rule_significant_thresholds` (`Attestations([LegalSourceHashChanged x3, RequiredTypeMissing{SourceFidelity, ScenarioCoverage, PublicationApproval}])`, identical in Rust/Python/WASM). | ✅ MET (in-repo) |
| **C4** | Registry rollback → a new workflow resolving by tag resolves to the **previous signed content hash**. | **MET in-repo.** Rollback eligibility + tag re-resolution: `crates/ke-cli/tests/lifecycle.rs:286` `rollback_to_published_ok_and_to_revoked_ineligible` (rollback to a `Published` hash moves the tag pointer; rollback to a `Revoked`/`Deprecated` hash is rejected with typed `RollbackIneligible{state}`). Event-shape regression pins for the lifecycle events: `crates/ke-cli/tests/lifecycle.rs:416` `published_and_revoked_event_heads_are_pinned`. ADR-0013 eligibility predicate `is_rollback_eligible` = `Published` only. **Caveat (skeptic-verified):** that test rolls the tag back to the *same* hash — it proves eligibility + pointer-move, NOT the literal "resolve-by-tag → *previous distinct* hash." `published_and_revoked_event_heads_are_pinned` is an event-encoding regression pin, not a prior-hash resolution test. → small follow-up test pending. | 🟡 PARTIAL (in-repo): eligibility + mechanism proven; prior-distinct-hash re-resolution test pending |

---

## How to independently verify

A reviewer reproduces every in-repo claim above with the following (each is
load-bearing — re-run them, don't trust the summary):

```bash
# 1. Whole workspace green (C3/C4 tests + everything else; pyo3 stays out of the
#    default graph). Expect: 141 passed, 0 failed.
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
- C4: `rollback_to_published_ok_and_to_revoked_ineligible`,
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

## Cannot be closed from this repo (cross-repo)

These two criteria are **platform-side** and are PENDING the
`institutional-defi-platform-api` PR. ATLAS delivers the verifier + the
contract; the platform delivers load + execute + the parity diff. Neither is
marked "met" in this repo.

- **C1 (platform verifies before execution).** ATLAS-side: the verifier is
  delivered (`verify_artifact` in `verify.rs`) and proven 3-language-consistent
  (`scripts/contract-test.sh`), exposed to the platform as the feature-gated
  PyO3 `ke_artifact_py` wheel and to the browser as the verify-only `ke-wasm` /
  `@platform/atlas-artifact` package. Platform-side (PENDING): the verification
  middleware that calls `verify_artifact` on load, installs the wheel through the
  S3-backed PEP 503 index, and refuses to execute on any rejection.
- **C2 (execute output matches the Python pipeline).** ATLAS-side: the Gate-3
  equivalence harness (`scripts/equivalence-harness.sh`) already proves the Rust
  runtime ≡ Python `RuleRuntime` over scenarios. Platform-side (PENDING): load a
  signed `.kew`, execute it in the platform runtime, and diff the output against
  the current Python pipeline end-to-end.

The full platform brief instantiates from
`dev/briefs/gate-4-platform-consumption-OUTLINE.md`; it is a separate
platform-repo deliverable that must be reviewed before the platform-side Gate-4
PR.

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
- **C4: partially met in this repo** — eligibility + pointer-move + the rollback
  mechanism are proven; the literal "resolve-by-tag → previous *distinct* hash"
  is not yet covered by a dedicated test (small follow-up). Not overstated as MET.
- **C1: verifier delivered and 3-language-consistent; platform integration
  pending** the platform-repo PR.
- **C2: runtime-parity foundation delivered (Gate-3 harness); artifact-based
  execute parity is platform work**, pending the platform-repo PR.

Gate 4 is **accept-ready on the ATLAS side**; **full §19 acceptance closes only
when the platform-repo PR lands** and demonstrates C1 + C2 end-to-end.
