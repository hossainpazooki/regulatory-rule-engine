# Gate 4 — Artifact, registry, attestation, platform unblock (brief)

**Status:** in progress — **Phases 0–3 complete** (3a + 3b; see
`docs/gate-4-implementation-log.md`); Phases 4–6 (PyO3 wheel, cross-language
contract test, acceptance) ahead. The §2 prerequisite decisions were resolved as
Gate-4 ADRs 0009–0014 in Phase 0 (Accepted 2026-06-11), unblocking Phase 1.
This brief is the contract; it mirrors the Gate 1–3 briefs and the mandatory §22
brief sections.
**Authoritative spec sections:** §8 (artifact contract), §9 (lifecycle state
machine), §10 (typed attestation model), §11 (verification model /
`ConsistencyBlock`), §14 (cross-repo integration), §15 (runtime selection,
pinning, rollback, revocation), §19 (Gate 4 acceptance), §20 (risks), §21 (open
decisions), §22 (Gate 4 outline), §23 (Before-Gate-4 checklist).
**Predecessor:** Gate 3 (preview runtime + equivalence harness) — complete.
**Successor:** Gate 5 (surfaces + frontend rewire). **Hard stop:** everything past
Gate 4 is incremental and parallelizable (spec §19).

---

## 1. Context

Gates 1–3 produced: the canonical IR + encoding + JSON Schema (`ke-core`), the
YAML→IR compiler + T0/T1/T4 (`ke-compiler`), and a preview runtime proven
equivalent to the Python `RuleRuntime` (`ke-runtime`). **The artifact is the
contract** (spec §1): Gate 4 turns a compiled IR into a *signed, content-addressed,
attestable* artifact, stands up the registry lifecycle, and unblocks the platform
to consume artifacts by content hash.

**What already exists (Gate 1-frozen, in `ke-core`)** — Gate 4 fills in behavior,
not shapes:
- `manifest::Manifest` (§8.1 fields incl. the `artifact_hash: [u8;32]` slot),
  `ArtifactKind`, `SemVer`, version triplet.
- `manifest::{T2T3Mode, AttestationType, RevocationPolicy, VerificationPolicy,
  AttestationCount, PolicyBundle}` — policy/attestation *shapes* (sketched in
  Gate 1 so canonical encoding never needs revisiting).
- `ir::ProvenanceMarker` — the lifecycle states (§9) as a typed enum.
- `canonical::{encode,decode}` + `tests/artifact_hash_offset.rs` — the
  zero-then-patch hash-offset derivation is exercised at the byte level; Gate 4
  wires it for real.

**What Gate 4 builds (new, in `ke-artifact` + `ke-cli`):** the outer `Artifact`
assembly (§8.1: `compiled_ir`, `source_span_index`, `consistency_block`,
`compiler_signature`, `attestations`, `registry_state_metadata`), BLAKE3 content
addressing, ed25519 compiler signature, typed expert attestations + verification +
rejection rules, the registry state machine over an S3 v1 model, the
`ke-artifact-py` PyO3 binding + wheel + PEP 503 index, and the cross-language
contract test.

**Parity targets (Python, in `institutional-defi-platform-api`):**
`src/production/{executor.py,schemas.py}` (runtime that will consume artifacts),
`src/production/trace.py`. The platform *consumer* side (verification middleware,
Temporal artifact pinning, Pydantic-from-schema generation, contract-test
workflow) is **out of scope for this repo** — it lands via a **separate
platform-repo brief** (§14, §11.0, §23). The workbench side produces artifacts +
wheel + contract fixtures; the platform side consumes them.

---

## 2. Prerequisite decisions (BLOCKING — resolve before Phase 1)

Per CLAUDE.md and spec §21/§23, **no Phase-1 work proceeds until these are
resolved**, each in its own Gate-4 ADR (numbers provisional; assigned when
authored). Don't lower the bar — if a decision can't be made, stop and surface it.
Recommendations below are starting points for Hossain + the security/domain
reviewers, **not** decisions I can make.

| # | Decision (spec) | Options | Recommended v1 (needs sign-off) | ADR |
|---|-----------------|---------|---------------------------------|-----|
| 1 | **Expert key authority + revocation** (§21.1, §20 "key compromise") | self-managed HW keys · org PKI · IdP-backed signing · managed HSM | IdP-backed or self-managed ed25519 signer identities + a registry-held key directory with explicit revocation list + key expiry; HSM deferred | 0009 |
| 2 | **Trusted timestamp authority** (§21.5, §10) | RFC 3161 provider · internal TSA · other | RFC 3161-compatible provider; **deterministic mock TSA for dev/test only** — artifacts stamped by the mock are rejected by non-local runtime policy | 0010 |
| 3 | **T2/T3 publication policy** (§21.2, §11 modes) | strict · review_override · advisory (per env) | production = `strict` or `review_override`; lower envs `advisory`; carried per-environment in `PolicyBundle.verification_policy` | 0011 |
| 4 | **T2/T3 sidecar deployment** (§21.3, §11) | platform-owned package · platform-owned service · extracted service | v1 stays **platform-owned** (spec §11); evidence reaches the registry via either a platform job writing back, or a workbench-triggered command calling the sidecar through §4.5 | 0011 |
| 5 | **S3 registry + PEP 503 index layout** (§21 resolved-v1, §14) | — (persistence model resolved: S3) | document bucket/key layout for content-hash objects, append-only lifecycle events, manifest/tag objects; and the S3-backed PEP 503 simple-index layout with exact version+hash pinning | 0012 |
| 6 | **Legal source text storage** (§21.4, §11) | hash-only · encrypted object store · indexed text | **hash-only** for Gate 4 (attestations bind a `legal_source_hash`); whole-document source coverage stays deferred until promoted | (note in 0009/§3) |

The **Before-Gate-4 checklist** (§23) must all be green before Phase 1: typed
attestation schema finalized (§10), key authority + revocation selected (1),
TSA selected (2), T2/T3 sidecar path (4) + policy (3) selected, platform rejection
rules specified (§10), rollback + revocation policies specified (§15), Temporal
pinning design reviewed with the platform brief (§15), S3 + PEP 503 layouts
documented (5), and the **platform-repo brief authored and reviewed**.

---

## 3. Decisions carried forward (already made in Gate 3 — ADRs 0007/0008)

These Gate-3 "Gate-4 readiness decisions" are inputs to this gate, not open:

1. **`jurisdiction_time_zone = None` is a first-class publishable value**
   (zone-independent civil-date semantics, never `UTC`). The registry **must not**
   normalize/mutate it — `artifact_hash`, `compiler_signature`, and attestations
   bind to `None` exactly. Publish validation accepts a date-only effective window
   with zone `None` **or** `Some(..)`, and fails closed for any future
   datetime-precision window with no zone (forward-guard; no such variant in the
   IR today).
2. **`[from, to)` is the authoritative effective-window semantics.** Gate 4 (via
   the platform brief) migrates the platform `RuleLoader.get_applicable_rules`
   pre-filter from legacy `[from, to]` to `[from, to)` — a real boundary-date
   behavior change needing domain-reviewer awareness; closed-closed may survive
   only as a temporary platform-loader compatibility mode, never as the artifact
   contract.
3. **Deterministic test keys / fixed seeds for all signing/keygen tests.** ed25519
   signing is deterministic (RFC 8032); CI must not depend on OS randomness or the
   `getrandom 0.3` raw-dylib path that breaks on this repo's `x86_64-pc-windows-gnu`
   toolchain (see [[toolchain-windows-gnu-getrandom-dlltool]]). Golden artifacts
   are signed with a fixed test key so hashes/signatures stay reproducible;
   production keygen uses secure randomness behind the key-authority boundary (1).

## 4. Locked decisions (resolved in spec v3.1 / earlier ADRs)

- **Registry persistence v1 = S3** (content-hash objects, append-only lifecycle
  events, S3-hosted manifest/tag objects). DynamoDB/Redis indexes deferred until
  S3 manifest ops become a measured bottleneck.
- **`ke-artifact-py` index v1 = S3-backed PEP 503 simple index**, exact version +
  hash pinned in the platform repo.
- **Codec = postcard-1** (ADR 0002); **content hash = BLAKE3**; **signature =
  ed25519** (spec §8). Canonical encoding profile + version triplet is now
  (`0.4.0` / `ke-canon-4` / `postcard-1`) — the ADR 0013 canon-4 landing bumped
  the Gate-1–3 contract (`0.3.0` / `ke-canon-3`) and regenerated the golden
  vectors atomically; any further Gate-4 shape change bumps it again.
- **Initial registry lives inside `ke-cli`** (spec §6 deferred-splits); a
  `ke-registry` crate splits out only when persistence/policy APIs stabilize.

---

## 5. Phase plan + deliverables (files)

Phased to keep the doc-each-phase convention; sequenced so each phase is
independently verifiable.

- **Phase 0 — prerequisite ADRs + attestation schema.** The §2 ADRs (0009–0012)
  **plus ADR 0013** (revocation-policy reconciliation — the only canon-bumping
  prerequisite, sequence first) and **ADR 0014** (§18 audit-contract ownership +
  pre-freeze field model). All six are drafted **Proposed** and need Hossain +
  security/domain sign-off before Phase 1. Finalize the typed attestation schema
  (§10 bound fields) in `docs/attestation-schema.md` (done — Proposed). Author the
  **platform-repo brief** (separate repo) against
  `dev/briefs/gate-4-platform-consumption-OUTLINE.md`. *No code.*
  - **Hard prerequisite, not in the original §23 checklist:** the T4
    `contradictory_outcome` detector must first be fixed so `verify()` over the
    clean corpus yields `has_blocking() == false` (done — Gate-2 remediation, ADR
    0005 amendment + `crates/ke-compiler/tests/t4_corpus.rs`). Until that landed,
    no artifact could reach `draft → structurally_verified` (§9) and Gate 4 had
    nothing to attest.
  - **Sequencing:** ADR 0013's canon bump (`0.3.0/ke-canon-3 → 0.4.0/ke-canon-4`)
    was absorbed in a single Phase-1 "canon-4 landing" (**landed**: enum
    reconciled, triplet bumped, corpus regenerated once); ADRs 0009/0011/schema
    bind to the post-0013 `RevocationPolicy` names.
    ADR 0014's static-field decision must be settled before the attestation schema
    freezes (else a post-freeze §18 retrofit forces re-attestation).
- **Phase 1 — `ke-artifact` core encoding + content addressing + signature.**
  **Delivered 2026-06-12** — see `docs/gate-4-implementation-log.md` (Phase 1)
  for what landed, the byte-range contract, and the verbatim gate evidence
  (89/0 workspace, 16/0 ke-artifact, generator idempotence, key hygiene).
  - `crates/ke-artifact/src/artifact.rs` — the `Artifact` assembly (§8.1):
    `manifest`, `compiled_ir`, `source_span_index`, `consistency_block`,
    `compiler_signature`, `attestations`, `registry_state_metadata`.
  - `crates/ke-artifact/src/hash.rs` — BLAKE3 zero-then-patch content addressing
    over canonical bytes (reuses the `ke-core` hash-offset derivation).
  - `crates/ke-artifact/src/sign.rs` — ed25519 compiler signature (deterministic
    test keys per §3.3).
  - `fixtures/artifacts/` extended with §8.3 golden vectors (signed; via a
    documented generator — never hand-edited).
- **Phase 2 — typed attestations + verification + `ConsistencyBlock`.**
  **Delivered 2026-06-12** — see `docs/gate-4-implementation-log.md` Phase 2
  (117/0 workspace; R1–R8 + signature/class/TSA as typed variants, each pinned
  by a named test; attested golden vectors with the Phase-1 content addresses
  pinned unchanged — the §9 append property is now mechanical).
  - `crates/ke-artifact/src/attestation.rs` — `Attestation` (all §10 bound
    fields), signing, and the platform **rejection rules** (§10): unknown/expired/
    revoked/unauthorized key, not bound to the artifact hash, unsupported policy
    version, expired, legal-source-hash changed, missing required types.
  - `crates/ke-artifact/src/consistency.rs` — `ConsistencyBlock` (§11) carrying
    T0–T4 evidence, policy mode, model/profile versions, overrides, timestamps
    (builder only; evidence path is platform-owned per ADR 0011, adapter in
    ke-cli Phase 3).
  - Also landed: `tsa.rs` (deterministic MockTsa; ADR-0010 class binding),
    `keydir.rs` (ADR-0009 directory shape).
- **Phase 3 — registry state machine (local-FS backend). COMPLETE (3a + 3b); S3 v1 deferred (trait seam ready, see below).**
  **Delivered 2026-06-13** — see `docs/gate-4-implementation-log.md` (Phase 3a +
  Phase 3b) for what landed and the verbatim gate evidence. 3a stood up the
  registry-core library, the hash-chained registry-root-signed event log, the
  `can_transition` table, the `LocalFsBackend`, `resolve`, and `ke compile`/`ke
  query`. **3b** drove the rest of the §9 lifecycle via six CLI commands —
  `ml-check` (dev stand-in writing a `consistency/<hash>.json` sidecar, **not**
  the envelope), `attest` (expert attestations re-written into `.kew`
  post-envelope with `artifact_hash` asserted unchanged), `publish` (the
  `verify_attestation_set` policy gate; typed `AttestationSetRejected` on a
  missing required type), `deprecate`, `revoke` (revocation policy + severity
  **recorded** in a `revocations/<hash>.json` sidecar — runtime enforcement is
  platform/Gate 6), and `rollback` (ADR-0013 eligibility). `LifecycleEvent`
  shape is unchanged, so the 3a canonical-event-head pin still holds; 3b adds
  its own published/revoked event-head pins. Signing stays behind `test-keys`;
  a no-feature build keeps each command a typed "requires `--features
  test-keys`" error. Evidence: `cargo test -p ke-cli --features test-keys` =
  16/0; `cargo test --workspace` = 0 failed; fmt + clippy (`-D warnings`, both
  feature sets) clean; `bash scripts/lifecycle-smoke.sh` PASS with twice-run
  byte-identical determinism across `events/artifacts/tags/consistency/
  revocations`.
  - **Deferred past Phase 3** (recorded honestly): real T2/T3 sidecar evidence
    (platform-owned, ADR 0011 — `ml-check` is a loudly-marked dev stand-in);
    **runtime** revocation **enforcement** (platform/Gate 6 — the registry only
    records state + policy + severity); registry-root HSM custody + signed
    key-directory object + root rotation (ADR 0009, infra); real S3 backend +
    Object-Lock/versioning + attestations-as-separate-objects under WORM (trait
    seam ready; the local-FS `.kew` re-write is the dev path); PyO3 (Phase 4);
    `contract-test.sh` (Phase 5); `ke serve` (Gate 5). The §8.1-vs-§9
    `consistency_block` placement (in-envelope slot reserved for compile-time
    T0/T1/T4 and left `None`; T2/T3 lives in the registry sidecar) is **flagged
    for a possible follow-up ADR**, not resolved here.
  - Registry inside `ke-cli` (`crates/ke-cli/src/registry/`): the §9 state
    machine (`draft → structurally_verified → ml_checked → expert_attested →
    published → deprecated → revoked`) as append-only signed events; transition
    authority rules (§9); rollback = move tag/policy pointer to a prior content
    hash (no byte mutation); revocation = append-only event (§15).
    - **3a (done):** registry-core library — `LifecycleState` derived from a
      hash-chained, registry-root-signed `LifecycleEvent` log (`current_state`),
      the `can_transition` precondition table (the full §9 edge set, but only
      `draft`+`structurally_verified` *executed*), `RegistryBackend` trait +
      `LocalFsBackend` (ADR-0012 paths, `NON_AUTHORITATIVE` marker), `resolve`
      (ByHash/ByTag/ByRegime) + the §18 `ResolutionRecord`,
      `is_rollback_eligible`. Clock-free core (`now_unix` injected).
    - **3b (done):** the `ml-check`/`attest`/`publish`/`deprecate`/`revoke`/
      `rollback` *commands* + revocation-policy **recording** (§15; policy +
      severity in a `revocations/<hash>.json` sidecar, runtime enforcement is
      platform/Gate 6). Anti-backdating skew bound remains deferred (monotonic
      `now_unix` + the hash chain are present; the bound itself is not built).
  - S3 layout per ADR 0012; **local filesystem backend allowed for dev/test
    only** — and is what 3a ships (objects flagged `NON_AUTHORITATIVE`,
    ADR 0012 §6). S3 slots behind the same `RegistryBackend` trait later.
  - `ke-cli` subcommands: `compile` + `query` **(done, 3a)**; `ml-check` /
    `attest` / `publish` / `deprecate` / `revoke` / `rollback` **(done, 3b)** —
    signing behind `test-keys`, no-feature build returns a typed "requires
    `--features test-keys`" error per command (spec §6).
- **Phase 4 — consumer-agnostic verification + provenance export, with both
  bindings (RESCOPED by ADR 0016; supersedes the original "Python-only" Phase 4
  below).** Driver: the platform-api is a hypothetical consumer while **COMPASS
  is a live consumer today** that surfaces ATLAS provenance "surfaced, not
  re-verified," reads a sibling `fixtures/` dir absent on Vercel, and has **no
  revocation channel**. So Phase 4 ships **one** pure verification surface with
  two thin bindings, plus a provenance export carrying registry state.
  - **Phase 4a (delivered): ADR 0016 + the pure Rust core, CI-testable.**
    - `crates/ke-artifact/src/verify.rs` — the consumer surface, RNG-free and
      backend-free (WASM-ready): `verify_artifact(kew, keydir, ctx, registry) ->
      VerificationOutcome` wrapping the existing pure verifiers
      (`decode_artifact` → `verify_hash` → `verify_signature` → `verify_attestation_set`),
      then folding in registry state; `Verdict` / `RejectionReason`
      (`HashMismatch` / `CompilerSignatureInvalid` / `Attestations` /
      `NotPublished` / `StaleEventHead` / `Decode`); `RegistryStatus` /
      `RegistryEvidence` (status + event-head hash, optional live head for
      staleness); `ArtifactProvenance` / `AttestationSummary` (`artifact_provenance(...)`,
      plain serde → one canonical JSON, `is_test_key` surfaces `test-*` keys).
      **`verify_artifact` takes registry state as DATA — no I/O, no RNG.**
    - `crates/ke-cli/src/commands/export_provenance.rs` + `ke export-provenance`
      — the **only** registry-touching part: reads the `.kew` and the event log
      (`current_state` → `RegistryStatus`, `head_event.chain_hash()` →
      event-head hash), builds `RegistryEvidence`, calls `artifact_provenance`,
      prints canonical JSON (and optionally writes `artifacts/<hash>/provenance.json`).
      `--now` / `KE_NOW` for `exported_at`.
  - **Phase 4b (next): bindings + cross-language contract test.** PyO3
    `ke-artifact-py` wheel (the §14 surface) published to the S3-backed PEP 503
    index (ADR 0012); the `ke-wasm` wasm-bindgen verifier + `@platform/atlas-artifact`
    npm package (verify-only, spec §6); `scripts/contract-test.sh` (Rust ≡ Python
    ≡ WASM — same `.kew` → identical verdict + provenance). Actual publishing +
    the COMPASS rewire are separate credentialed/follow-up steps. (Original
    Python-only surface, retained for reference: `crates/ke-artifact/src/python.rs`
    behind a `pyo3` feature — `from_bytes`, `canonical_hash`,
    `verify_compiler_signature`, `verify_attestations`, `iter_rules`,
    `consistency_block`, `attestations`, `source_span_index`.)
- **Phase 5 — cross-language contract test + schema→Pydantic.**
  - `scripts/contract-test.sh` — round-trips golden artifacts Rust↔Python,
    verifies canonical hashes match across languages (§14 schema-drift
    prevention); SHA-gated to the recorded `SOURCE.md` commit (mirrors
    `differential-test.sh`).
  - JSON Schema emission consumed by platform Pydantic-model generation
    (platform side; contract fixtures provided here).
- **Phase 6 — acceptance + platform coordination.** `docs/gate-4-implementation-log.md`;
  confirm the platform-repo PR demonstrates end-to-end load + execute parity.

---

## 6. Phase verification commands

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p ke-artifact     # incl. signature, attestation, and rejection tests
cargo test --workspace
# cross-language (platform at the recorded SOURCE.md SHA):
bash scripts/contract-test.sh
# wheel build + install through the same index mechanism intended for staging
# (exact command set lands with ADR 0012 / Phase 4)
```

Signing/keygen tests use **fixed/deterministic keys** (§3.3) — CI must never pull
`getrandom`/OS randomness.

## 7. Acceptance criteria (spec §19 Gate 4)

- Given a signed artifact, when loaded by the platform, then the platform verifies
  **hash, canonical encoding, compiler signature, required attestations, key
  validity, and registry state** before execution.
- Given a known scenario, when the platform executes a Rust-compiled artifact,
  then output matches the current Python pipeline (builds on the Gate 3
  equivalence harness).
- Given missing, stale, revoked, or invalid attestations, when the platform
  attempts execution, then execution is **rejected with a specific policy error**.
- Given registry rollback, when a new workflow resolves by tag, then it resolves
  to the **previous signed content hash**.

## 8. Known risks (spec §20)

- **Semantic laundering through cryptographic provenance.** A valid signature must
  not make weak legal encoding look authoritative. *Mitigation:* typed
  attestations + source/scenario coverage + explicit expert overrides + the
  review-first UI distinction (Gate 5). Compiler authority is structural only.
- **T2/T3 publish gap.** Publishing before ML checks complete. *Mitigation:* make
  T2/T3 explicit publication-policy inputs (decision 3); production requires strict
  pass or typed override.
- **Expert key compromise / stale attestation.** *Mitigation:* key authority +
  revocation (decision 1), trusted timestamping (decision 2), attestation
  expiration, source-hash binding, registry-time **and** runtime-time verification.
- **Schema / canonicalization drift.** Rust and Python agree on schema but compute
  different hashes/semantics. *Mitigation:* the §8.3 golden vectors, JSON Schema
  generation, the cross-language contract test, and the existing differential +
  equivalence harnesses.
- **Toolchain (`getrandom`/`dlltool`).** `ed25519-dalek`'s keygen pulls
  `getrandom`, which won't build on this `windows-gnu` toolchain. *Mitigation:*
  deterministic test keys (§3.3); keep keygen out of the test path.

## 9. Out-of-scope clarifications (must NOT do)

REST/WebSocket surfaces, WASM, DuckDB/SQL views, flat-file export, and the
frontend rewire (**all Gate 5**); production cutover, Temporal pinning
*implementation*, and Python-KE-module removal (**Gate 6** — only the *design* is
reviewed here, §15); the other T4 conflict classes (`source_span_divergence` needs
`ke-search`); and **any LLM/AI code anywhere in the artifact / signing /
attestation / registry path** — the LLM is out-of-band authoring assistance only,
may produce `EditProposal` objects (§13) but **may not** sign, attest, publish, or
modify committed rules ([[project-llm-authority-boundary]]). Browser/WASM code
remains preview-only and never produces authoritative artifacts.

## 10. Commit boundary

Hossain commits/merges manually on `migration/gate-4-*` after review. Claude Code
makes no commits or pushes. `fixtures/` is never hand-edited; `fixtures/artifacts/`
is regenerated by its (signed) generator. The platform-repo consumer changes are a
**separate PR in `institutional-defi-platform-api`** (its own brief).

## 11. Platform access (spec §4.5)

`contract-test.sh` resolves `${PLATFORM_REPO:-../institutional-defi-platform-api}`,
requires HEAD == the SHA recorded in `fixtures/rules/SOURCE.md`, records that SHA
in its output, and fails fast if the checkout is missing, dirty under
`src/rules/data`, or at any other commit. The platform-side Gate-4 PR must
demonstrate installing `ke-artifact-py` through the **same S3 PEP 503 index
mechanism** intended for staging (not just a local wheel path), and show
end-to-end artifact load + verify + execute matching the Python pipeline.
