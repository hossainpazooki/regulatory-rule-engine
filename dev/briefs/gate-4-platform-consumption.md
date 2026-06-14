# Gate 4 — Platform consumption brief (`institutional-defi-platform-api`)

> **Where this brief is authored vs. executed.** This document is **authored and
> reviewed *here*** (in the ATLAS workbench repo, `dev/briefs/`) so it has a
> single reviewable contract pinned to the workbench Gate-4 artifacts. It is the
> **§23 "Platform-repo brief authored and reviewed"** checklist item — the
> *authoring* half. **Review is Hossain's.** The brief is then **instantiated and
> executed as its own PR in the separate `institutional-defi-platform-api`
> repo**; it changes *that* repo's code, never this one. No commit in this
> workbench repo implements any §-section below. (Spec §14 "Platform-side
> changes … require a separate brief executed in the platform repo"; spec §22
> §874 / §1101 "platform consumption … via the separate platform-repo brief".)

**Status:** OUTLINE instantiated → **review-ready draft**. The ATLAS side it
consumes is built and gate-green on `migration/gate-4-artifact` (verify surface,
rejection matrix, lifecycle, PyO3 + WASM verify-only bindings, the 3-language
contract test). The platform-side acceptance (C1/C2 end-to-end) is **pending
this PR landing in the platform repo** — see §0 honesty boundary.

**Authority — consumer-only (CLAUDE.md authority boundaries; spec §16 "thin
adapter, not a new source of truth").** The platform **verifies and executes**
artifacts. It **never** compiles, signs, attests, publishes, transitions
registry state, or authors rules. **No LLM/AI code in any verification or
execution path** (the LLM is out-of-band authoring assistance only — it may emit
`EditProposal` objects per spec §13/§17 but may not sign/attest/publish; see
[[project-llm-authority-boundary]]).

**Depends on (workbench Gate-4 Phase-0, all Accepted before this PR lands):**
ADRs **0009–0016** + the finalized `docs/attestation-schema.md`. **Pinned to the
platform SHA recorded in `fixtures/rules/SOURCE.md`:**
`f73b9403c88a7ab5d741b351dce085b6988b6ba7` (`f73b940`). The corpus and the golden
artifacts are content-addressed off that revision; a SHA drift is a *different*
contract.

**Authoritative spec sections:** §14 (cross-repo integration — the Python
surface, schema-drift prevention, packaging), §15 (runtime selection, pinning,
rollback, revocation), §18 (observability + audit contract), §10/§11
(attestation + verification model), §19 (Gate 4 acceptance), §23 (Before-Gate-4
checklist). **Predecessor (ATLAS):** Gate 3 equivalence harness (the C2 parity
foundation). **Sibling contract:** `dev/briefs/gate-4-artifact-registry-attestation.md`
(the ATLAS producer-side brief; this is its consumer mirror).

---

## §0 — Status / scope / authority

**Separate-repo deliverable.** Every change below is made in
`institutional-defi-platform-api`, on its own branch, as its own PR, reviewed by
Hossain. CLAUDE.md forbids this workbench session from touching the sibling repo;
this brief therefore *specifies* the platform work, it does not perform it.

**Consumer-only — the hard line.** The platform side links **no** signing,
keygen, registry-mutation, or rule-authoring code. It consumes the ATLAS verifier
(the `ke-artifact-py` wheel, and — for COMPASS — the WASM package) and the
emitted JSON Schema. Concretely, the surface it imports
(`crates/ke-artifact/src/python.rs`) **exposes no `SigningKey` and no publish
path**: only `from_bytes`, `canonical_hash`, `verify_compiler_signature`,
`verify_attestations`, `verify_artifact`, `provenance`, `iter_rules`,
`consistency_block`, `attestations`, `source_span_index`
(`python.rs:64-353`). If a reviewer finds an import that signs or publishes on
the platform side, that is a defect against this brief.

**Honest implemented-vs-pending boundary (do not blur):**

| Acceptance (spec §19) | Who owns it | State as of this brief |
|---|---|---|
| **C1** platform verifies hash, encoding, compiler sig, required attestations, key validity, registry state **before execution** | **Platform** (this PR) | **Pending.** ATLAS *provides* the verifier + the checks; the platform *integration* lands here. |
| **C2** platform executes a Rust-compiled artifact → output matches the current Python pipeline | **Platform** (this PR) | **Pending.** Foundation exists: the Gate-3 equivalence harness proved Rust runtime ≡ Python `RuleRuntime` over 1326 scenarios. |
| **C3** missing/stale/revoked/invalid attestations → rejected with a **specific** policy error | **ATLAS verifier** (built) + platform wiring | **ATLAS side built** (the R1–R8 rejection matrix; the revoked/stale verdicts). Platform must surface each as a deny. |
| **C4** registry rollback → resolve-by-tag yields the **previous** signed content hash | **ATLAS registry** (built) + platform resolution | **ATLAS side built** (`ke-cli` rollback + `is_rollback_eligible`). Platform must resolve by hash, not re-resolve mid-run. |

**Dependency on workbench ADRs 0009–0016** (all must be Accepted before this PR
merges): 0009 key authority + revocation, 0010 TSA (RFC 3161 + deterministic
mock), 0011 T2/T3 policy + sidecar, 0012 S3 registry + PEP 503 index layout, 0013
revocation-policy / canon-4 bump (`0.4.0` / `ke-canon-4`), 0014 §18 audit-field
ownership, 0015/0016 the consumer-agnostic verify surface rescope. The platform
must not pin a wheel built under a *different* canon triplet (see §5).

---

## §1 — Context & parity targets

**The runtime that consumes artifacts.** `src/production/executor.py`
(`RuleRuntime.infer` — applicability + decision-table lookup),
`src/production/schemas.py` (the typed I/O the runtime accepts/returns),
`src/production/trace.py` (`ExecutionTrace` / `DecisionResult`). The verification
middleware (§3) sits **in front of** `RuleRuntime` and gates whether a
`CompiledRule` reaches it. The Python KE module that loads YAML today
(`src/rules/service.py` `RuleLoader`) is **not removed in Gate 4** — its removal
is Gate 6 (spec §15 "before removing the Python KE module"); through Gate 4 both
paths coexist.

**The parity oracle (C2).** Today's Python `RuleRuntime` *is* the oracle a
Rust-compiled artifact must match. The Gate-3 equivalence harness
(`scripts/equivalence-harness.sh`, ATLAS repo) already proved the **Rust preview
runtime ≡ the Python runtime** over the full corpus at the recorded SHA — that is
the *foundation*, not the platform demo. C2 closes when the platform executes the
**artifact-delivered** `CompiledRule` through `RuleRuntime` and matches the
current pipeline for a known scenario (§8). Equivalence boundary = observable
semantics per ATLAS ADR 0008 (identical final outcomes, obligation id-sets,
normalized decision path, error classes; representation differences are outside
the boundary).

**Effective-window migration (real behavior change, needs domain-reviewer
awareness).** Per ATLAS ADR 0007/0008, the authoritative window semantics are
**`[from, to)`** (half-open). The platform's `RuleLoader.get_applicable_rules`
date pre-filter is legacy **`[from, to]`** (closed-closed). This PR migrates the
pre-filter to `[from, to)`; closed-closed may survive only as a *temporary
platform-loader compatibility mode*, never as the artifact contract. Boundary
dates flip behavior — flag this for the domain reviewer explicitly.
`jurisdiction_time_zone = None` is a **first-class publishable value**
(zone-independent civil-date semantics, never coerced to UTC); the platform must
honor `None` exactly and must not normalize it.

---

## §2 — `ke-artifact-py` install path (S3 PEP 503 index, `--require-hashes`)

**Acceptance requirement (spec §14 packaging; ADR 0012):** the platform installs
the wheel through the **internal S3-backed PEP 503 simple index**, with an
**exact version + sha256 pin** and `pip --require-hashes` (fail-closed on any
drift) — **not** a local wheel path. A local path is permitted for ad-hoc dev
only; the Gate-4 platform PR must demonstrate the index install.

```toml
# pyproject.toml / requirements — exact pin, fail-closed
# (index URL is the S3-backed PEP 503 simple index from ADR 0012)
[[tool.pip.index]]
url = "https://<atlas-pep503-index-host>/simple/"
```

```
# requirements.lock — hash-pinned, --require-hashes enforced in CI + deploy
ke-artifact-py==0.4.0 \
    --hash=sha256:<exact-wheel-sha256-from-the-index>
```

```bash
pip install \
  --require-hashes \
  --index-url https://<atlas-pep503-index-host>/simple/ \
  -r requirements.lock
```

Rules:

- **`--require-hashes` is mandatory** in CI and in the deploy image build. A hash
  mismatch fails the install closed — this is the supply-chain guard for the
  verifier itself.
- **Version is pinned to the canon triplet** the ATLAS release emitted (`0.4.0` /
  `ke-canon-4` / `postcard-1` at this writing, per ADR 0013). The pinned wheel
  version and the Pydantic models (§5) must be regenerated **together** when the
  triplet bumps.
- **windows-gnu caveat (honest, dev only):** the abi3 + `extension-module` wheel
  built and loaded locally on the workbench's `x86_64-pc-windows-gnu` toolchain
  this session, but **Linux CI is the authoritative consumer build**. The
  platform deploy target is Linux; the windows-gnu floor is `cargo check
  --features pyo3`. Do not pin a windows-gnu-built wheel for production.
- The wheel is **verify-only** (§0). Installing it grants the platform no signing
  capability by construction.

---

## §3 — Verification middleware (the core; C3)

**Where it lives:** new code in `src/production/` (e.g.
`src/production/artifact_verification.py`), invoked by every artifact-load path
**before** the `CompiledRule` reaches `RuleRuntime.infer`. It is a thin adapter
over the ATLAS verifier — it adds **no crypto and no policy** of its own; it wires
the binding's verdict to a platform deny and an audit event.

**The one folded call.** The binding exposes `verify_artifact(kew_bytes,
keydir_json, ctx_json, policy_json, registry_json, exported_at_unix)` returning
`{verdict, registry_state, content_hash, provenance}`
(`python.rs:254-307`). It folds the four pure verifiers
(`decode_artifact → verify_hash → verify_signature → verify_attestation_set`) and
the registry-state checks, short-circuiting to the **first** failing reason
(`crates/ke-artifact/src/verify.rs:241-337`). The middleware calls this, and on
`verdict != "verified"` **refuses execution** and maps the reason to a specific
policy error. The middleware never re-implements a check.

**Every §14 pre-execution check, each yielding a specific policy error.** The
order below is the verifier's own short-circuit order (decode → hash → compiler
signature → attestations → registry state → staleness), with the per-attestation
sub-order from `docs/attestation-schema.md` §7 folded into check (6)/(7):

| # | §14 check | How it is enforced (ATLAS surface) | Specific policy error on failure |
|---|---|---|---|
| 1 | **Canonical decode** (postcard-1) | `decode_artifact` rejects non-canonical / truncated / trailing-byte input (`verify.rs:252-261`) | `ArtifactDecodeError` ← `RejectionReason::Decode(msg)` |
| 2 | **Content hash** matches `manifest.artifact_hash` | `verify_hash` — **re-zero recompute** over the envelope prefix `[0, envelope_len)`, never `blake3(raw .kew)` (`verify.rs:288-291`; `canonical_hash` re-zero proof in `python.rs:143-146`) | `ContentHashMismatch` ← `RejectionReason::HashMismatch` |
| 3 | **Compiler ed25519 signature** valid | verifying key resolved from the **caller-supplied** `KeyDirectory` by `compiler_signature.key_id`; unknown/malformed key ⇒ failure (`verify.rs:297-311`) | `CompilerSignatureInvalid` ← `RejectionReason::CompilerSignatureInvalid` |
| 4 | **Required attestation types + minimum counts** present (§11) | `verify_attestation_set` checks `required_attestation_types` / `minimum_attestation_count_per_type` from the env's `VerificationPolicy` — R6 `RequiredTypeMissing`, R7 `CoAttestationAbsent` (`attestation.rs`; matrix `tests/attestation.rs:305-394`) | `RequiredAttestationMissing{type, required, found}` / `CoAttestationAbsent{missing}` ← `RejectionReason::Attestations([...])` |
| 5 | **Each attestation: key valid / non-expired / non-revoked / authorized-for-type** against the signed key directory (ADR 0009), **at runtime** not just pin time | per-attestation R1: `KeyUnknown` / `KeyExpired` (status **and** validity-window lapse) / `KeyRevoked` / `KeyUnauthorizedForType` (`tests/attestation.rs:148-221`) | `AttestationKeyUnknown` / `…KeyExpired` / `…KeyRevoked` / `…KeyUnauthorizedForType` ← the `AttestationRejection` list |
| 6 | **Attestation bound to *this* artifact hash** | R2 `NotBoundToArtifact{expected, got}` (`tests/attestation.rs:225-244`) | `AttestationNotBound{expected, got}` |
| 7 | **Schema + codec + canon versions supported** | R3 `PolicyVersionUnsupported` for `attestation_policy_version`; ir-schema drift folds to R3 (`ir-schema-X.Y.Z`), regime/compiler-version mismatch folds to R2 (schema §7); the binding also surfaces `ir_schema_version` / `codec_version` / `canonicalization_version` (`python.rs:72-88`) for a platform allow-list | `UnsupportedPolicyVersion{version}` / `UnsupportedSchemaVersion` |
| 8 | **Effective window `[from, to)`** with `jurisdiction_time_zone=None` honored exactly (ADR 0007/0008) | R4 `Expired` half-open from 00:00 UTC of the expiration date (`tests/attestation.rs:264-279`, schema §7); window migration in §1 | `AttestationExpired` / `OutsideEffectiveWindow` |
| 9 | **Legal-source-hash unchanged** since attestation | R5 `LegalSourceHashChanged` — recomputed source hash vs bound `legal_source_hash` (`tests/attestation.rs:283-301`) | `LegalSourceChanged` |
| 10 | **Timestamp authority** — mock-TSA-under-non-local rejected | R8 `MockTsaNonLocal`; class relabel ⇒ `TimestampClassMismatch`; unverifiable RFC 3161 ⇒ `TsaUnsupported` (`tests/attestation.rs:396-495`, schema §7) | `MockTimestampInProduction` / `TimestampClassMismatch` / `TimestampAuthorityUnsupported` |
| 11 | **Registry lifecycle state** is `Published` for the env | `verify_artifact` rejects non-`Published` (deprecated/revoked/unknown) **even with valid crypto** — the COMPASS correctness fix (`verify.rs:318-324`; `tests/verify_surface.rs:251-277` `rejected_when_revoked`) | `ArtifactNotPublished{status}` ← `RejectionReason::NotPublished` |
| 12 | **Registry freshness** — embedded event-head vs a freshly-fetched live head | `StaleEventHead{embedded, live}` when `live_event_head` differs (`verify.rs:326-334`; `tests/verify_surface.rs:281-321` `stale_event_head`) | `StaleRegistryView{embedded, live}` ← `RejectionReason::StaleEventHead` |
| 13 | **Runtime-policy mode compatibility** | the env's `PolicyBundle.verification_policy.t2_t3_mode` (strict / review_override / advisory per ADR 0011) is the `policy_json` the middleware passes; an env mismatch is a config error caught before the call | `PolicyModeIncompatible` |

**Inputs the middleware supplies** (all as the shared JSON the three contract
languages share — `scripts/contract-inputs/{keydir,context,policy,registry}.json`
in ATLAS are the canonical shapes):

- `keydir_json` — the **signed key directory** (ADR 0009): per-key
  `public_key`, `signer_roles`, `authorized_attestation_types`,
  `valid_from/to_unix`, `status`, `revoked_at_unix`, `revocation_reason`,
  `revocation_event_hash` (`tests/attestation.rs:56-69`). Fetched fresh at
  runtime so revocation is enforced **at execution time**, not only at pin time.
- `context_json` — `PolicyContext { environment, now_unix,
  supported_policy_versions, current_legal_source_hash }`
  (`tests/attestation.rs:99-106`). `environment` drives R8 (mock TSA only
  accepted under `local`).
- `policy_json` — the env's `VerificationPolicy` (the strict publish gate is
  required types + min-count-per-type; `tests/verify_surface.rs:113-125`).
- `registry_json` — `RegistryEvidence { status, event_head_hash,
  live_event_head }` (`verify.rs:66-75`), obtained from the registry by content
  hash (the platform reads the registry; it does not mutate it).

**Fail-closed defaults.** Any check that cannot be *positively* satisfied (key
not found, registry status `Unknown`, decode error) is a **deny**, never a
pass-through. `is_test_key` (any `test-`-prefixed signer) is surfaced in the
provenance (`verify.rs:175-177`) and must be **refused in production
environments** — a `test-*` compiler or attestation key is not authoritative.

---

## §4 — Temporal artifact pinning (DESIGN at Gate 4; IMPLEMENT at Gate 6)

**Scope discipline:** Gate 4 **reviews the design** below; Gate 6 **implements +
tests** it in the platform repo and only then removes the Python KE module (spec
§15 §679). Nothing in §4 ships in the Gate-4 platform PR beyond the written
design + the resolve-by-hash plumbing the middleware already needs.

Per spec §15 "Temporal pinning mechanism":

1. **Pin at workflow start.** A workflow pins one artifact **content hash** at
   start. If the caller already knows the hash, it is passed as a workflow input.
2. **Resolve outside deterministic workflow logic.** If the caller supplies a
   selector (semver tag / regime-id+effective-date / environment), the workflow
   invokes a **startup activity** to resolve it to a content hash. Resolution
   **must not** run in deterministic workflow code (it touches the registry —
   non-deterministic I/O). The ATLAS `resolve(Selector::{ByHash,ByTag,ByRegime})`
   returns a hash + a `ResolutionRecord` (`ke-cli` registry; lifecycle test
   `crates/ke-cli/tests/lifecycle.rs:185-200` shows `resolve` by tag → published
   hash + `registry_state_at_resolution`).
3. **Record the resolved hash in workflow history.** The activity result (the
   content hash) is recorded in Temporal history and passed **explicitly** to all
   downstream activities.
4. **Downstream loads by pinned hash only.** Every downstream activity loads the
   artifact **by the pinned hash**, never by selector. A running workflow must not
   observe a new artifact mid-run.
5. **No implicit mid-run re-resolution.** Re-resolution requires an **explicit
   versioned activity + an audit event**; it must never happen implicitly. This is
   data-version pinning, not Temporal code-versioning.

**Why this satisfies C4 at the platform.** Rollback in ATLAS moves a tag/policy
pointer to a previous content hash without mutating bytes
(`lifecycle.rs:286-323` `rollback_to_published_ok…`; eligibility = only
`Published`, `is_rollback_eligible` / `RollbackIneligible(Revoked)`
`lifecycle.rs:346-368`). Because a **new** workflow resolves by tag at start (step
2) and pins the result (step 3), a post-rollback new workflow resolves to the
**previous signed content hash** — exactly C4. Already-pinned in-flight workflows
keep their pinned hash (step 4), which is the intended immutability.

---

## §5 — Pydantic-from-schema generation (the Phase-5 platform half)

**Acceptance requirement (spec §14 schema-drift prevention):** platform models
are **generated from the JSON Schema `ke-artifact` emits**, never hand-written.

- ATLAS emits JSON Schema on every release (`crates/ke-core/schema/ir.schema.json`
  is the IR schema; the artifact/attestation shapes are frozen in
  `ke-core::manifest`). The platform PR adds a generator step
  (`datamodel-code-generator` or equivalent) that produces
  `src/production/ke_models.py` (Pydantic) **from the emitted schema**.
- **Regenerate on every canon-triplet bump.** When the version triplet bumps
  (e.g. ADR 0013's `0.3.0/ke-canon-3 → 0.4.0/ke-canon-4`), regenerate the models
  **and** re-pin the wheel (§2) in the **same** PR. The generated models carry the
  triplet so a mismatch between the wheel's `canonicalization_version`
  (`python.rs:84-88`) and the models is caught in CI.
- **Generation is deterministic** — the generator is run in CI and its output is
  committed; a diff between committed and regenerated models **fails** the build
  (this is the §14 schema-drift guard, point 2). Models are typed I/O only; they
  carry no verification logic (that is the wheel's).

This is the **platform half** of the original ATLAS Phase 5: ATLAS emits the
schema + the contract fixtures; the platform generates the models. It is **not**
an ATLAS-repo deliverable.

---

## §6 — Cross-language contract test workflow (CI gate)

**Run `scripts/contract-test.sh`** (ATLAS side) as a **platform CI gate**. The
platform PR wires a CI job that checks out ATLAS at the recorded SHA, builds the
wheel + the WASM nodejs package, and runs the script.

- The script round-trips every committed golden under `fixtures/artifacts/*/`
  through the verify surface **three ways** — Rust (`contract-verify` example),
  Python (`import ke_artifact_py`), WASM (`@platform/atlas-artifact` via node) —
  over **one shared set of verifier inputs** (`scripts/contract-inputs/*.json`),
  and asserts byte-identical `{verdict, registry_state, content_hash,
  provenance}` (`scripts/contract-test.sh:257-319`).
- **SHA-gated to `fixtures/rules/SOURCE.md`** (spec §4.5): the script fails fast
  if the platform checkout is present but at the wrong commit
  (`contract-test.sh:62-76`); a present leg with an absent toolchain is **skipped
  with a loud message**, never a silent pass (`:121-168`). The script passes iff
  every present leg agrees on every golden and at least the Rust leg ran.
- The platform CI must run the **Python leg present** (the wheel installed via the
  §2 index in CI), so the platform gate proves Rust ≡ Python on the platform's own
  interpreter — the C3-foundation evidence that the verdict + provenance the
  platform sees is byte-identical to the canonical Rust surface. WASM is COMPASS's
  concern, optional here.

---

## §7 — Audit-event emission (§18; Gate-6-owned, design now)

**Scope:** designed here, **implemented + the reconstruction-path test owned by
Gate 6** (spec §23 "Audit reconstruction path tested" is a Before-Gate-6 item).
The Gate-4 platform PR records the design and the field plumbing the middleware
already produces; it does not have to ship the full emitter.

Every production decision must be reconstructable (spec §18). The audit event is
assembled from two halves:

- **Static fields frozen into the artifact (ADR 0014):** `artifact_hash`, rule
  IDs, source spans, effective date, compiler version, IR schema version,
  T0/T1/T4 status, attestation IDs + `attestation_policy_version`, scenario/test
  corpus version. These come from the binding's read accessors —
  `iter_rules` / `source_span_index` / `attestations` / `consistency_block`
  (`python.rs:103-138`) and the provenance projection (`verify.rs:131-216`,
  including `registry_event_head_hash`).
- **Dynamic fields the platform assembles at execution time (§18):**
  `workflow_id`, **execution timestamp**, **registry state at resolution time**
  (from the `ResolutionRecord` of §4 step 2 — `registry_state_at_resolution`,
  `lifecycle.rs:195-199`), the realized **decision trace** + obligation set from
  `RuleRuntime` / `ExecutionTrace` (`src/production/trace.py`), jurisdiction
  resolver version, runtime version, and the **verification evidence** (the
  `verdict` + `provenance` the middleware obtained in §3).

**Reconstruction path (the Gate-6 acceptance test):**
`workflow_id → artifact_hash → artifact bytes → rule trace → source spans →
attestations → verification evidence → final decision` (spec §18 §770-781). The
design ensures every link is captured: the workflow history pins the hash (§4),
the registry yields the bytes by hash, and the binding yields the trace inputs +
attestations + verification evidence. Gate 6 asserts a recorded decision can be
replayed end-to-end.

---

## §8 — End-to-end parity demo (Gate-4 acceptance; C1 + C2)

The platform PR demonstrates, as a runnable test/script, the full chain:

1. **Install** `ke-artifact-py` via the **S3 PEP 503 index** with
   `--require-hashes` (§2) — not a local wheel path.
2. **Load** a signed golden artifact (`fixtures/artifacts/<id>/artifact.kew`)
   resolved **by content hash** from the registry.
3. **Verify** — run the middleware (§3): all checks pass for a valid Published
   artifact → `verdict == "verified"`, `registry_state == Published`
   (`tests/verify_surface.rs:138-173` `verified_published_golden`).
4. **Execute** the delivered `CompiledRule` through `RuleRuntime.infer` and assert
   the output **matches the current Python pipeline** for a known scenario (C2;
   built on the Gate-3 equivalence boundary, ADR 0008).
5. **Negative cases (C3) — each a *specific* policy error, not a generic deny:**
   - **missing** required attestation type → `RequiredAttestationMissing` /
     `Attestations([RequiredTypeMissing{…}])` (`tests/verify_surface.rs:211-246`);
   - **revoked** registry state with valid crypto → `ArtifactNotPublished(Revoked)`
     (`tests/verify_surface.rs:251-277`);
   - **stale** registry view → `StaleRegistryView` /
     `StaleEventHead{embedded, live}` (`tests/verify_surface.rs:281-321`);
   - **invalid** compiler signature → `CompilerSignatureInvalid`
     (`tests/verify_surface.rs:178-207`);
   - and the per-attestation R1–R8 family (unknown/expired/revoked/unauthorized
     key, unbound hash, unsupported policy, expired, legal-source change,
     mock-TSA-in-production) each surfacing its named reason
     (`tests/attestation.rs:148-495`).
6. **Rollback (C4):** publish two artifacts under a tag, roll the tag back to the
   prior hash, and show a **new** workflow resolving by tag pins the **previous
   signed content hash** (§4; `lifecycle.rs:286-323`).

This demo **is** the platform-side C1 + C2 acceptance evidence. Until it runs
green in the platform repo, C1 and C2 are **pending** — this brief does not mark
them met (see §0).

---

## §9 — Out of scope / commit boundary

**Must NOT do on the platform side:**

- No rule **authoring, compiling, signing, attesting, publishing, or registry
  mutation** — consumer-only (§0; spec §16). No `SigningKey`/keygen/publish import.
- **No LLM/AI code** in any verification or execution path
  ([[project-llm-authority-boundary]]).
- **Python KE-module removal is Gate 6**, not this PR (spec §15 §679); through
  Gate 4 the Python loader and the artifact path coexist.
- **Temporal pinning *implementation* is Gate 6** (§4 is design-only here).
- **Audit-event *emitter* + reconstruction test are Gate 6** (§7 is design + field
  plumbing here).
- No REST/WebSocket/DuckDB/flat-file surfaces, no frontend rewire (all **Gate 5**,
  ATLAS side).

**Commit boundary.** Platform changes are a **separate PR in
`institutional-defi-platform-api`**, on its own branch, reviewed and merged by
Hossain. This workbench session makes **no commits or pushes** and does not touch
the sibling repo. The wheel is consumed via the index (§2); the platform never
hand-builds an authoritative artifact. The contract test (§6) is SHA-gated to the
recorded `SOURCE.md` commit — a wrong checkout fails the gate.
