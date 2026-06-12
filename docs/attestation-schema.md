# Typed attestation schema

**Status:** Finalized draft for Gate 4 (proposes; does not decide). Concrete
values for fields/rules marked _(pending ADR)_ are fixed only when the named
ADR is **Accepted** by Hossain + the security/domain reviewers. This document
is documentation only — it signs, hashes-as-authority, and publishes nothing.

**Spec references:** §10 (typed attestation model — types, bound fields,
rejection rules, timestamp authority), §11 (verification model,
`ConsistencyBlock`, policy modes), §9 (lifecycle states), §15 (revocation),
§20 (semantic-laundering risk).

> **Gate status caveat (read first).** Finalizing this schema satisfies the
> §23 "typed attestation schema finalized" and "platform rejection rules
> specified" checklist items, but it does **not** by itself unblock Gate 4
> Phase 1. The `ke-compiler` T4 `contradictory_outcome` detector currently
> flags 52 Blocking conflicts on the clean 34-rule corpus
> (`verify()` over `fixtures/rules` => `has_blocking() == true`). Per spec §9,
> no artifact can reach `structurally_verified`, so **there is no
> structurally-verified artifact to attest** and no attestation can be
> exercised end-to-end. That T4 false-positive must be fixed (separate
> work-item) before this schema has anything to bind to.

---

## 1. What an attestation is (and is not)

A typed attestation is a domain expert's **ed25519 signature over a canonical
encoding of a fixed set of bound fields**, asserting one specific, named claim
about one specific artifact (identified by content hash). It is the **only**
authority that moves an artifact toward `expert_attested` (spec §9).

An attestation is **not**:

- a statement that the legal encoding is _correct_ — only that the named,
  scoped claim holds for the expert's review scope (spec §10, §20);
- producible by the compiler (compiler authority is **structural validity
  only**, never legal truth — CLAUDE.md, spec §5);
- producible by any AI/LLM path (LLM is out-of-band authoring assistance only;
  it may emit `EditProposal` objects per §13 but **may not** attest, sign,
  publish, or modify committed rules — CLAUDE.md, `project-llm-authority-boundary`).

---

## 2. Attestation types (spec §10)

Each attestation carries exactly one `attestation_type`. The frozen enum is
`ke-core` `AttestationType` (`SourceFidelity`, `Interpretation`,
`ScenarioCoverage`, `EquivalenceClaim`, `PublicationApproval`).

| Type | Asserts (the named claim) | Type-specific scope it binds |
|------|---------------------------|------------------------------|
| `source_fidelity` | Encoded rule logic faithfully reflects the cited legal source spans for the stated regime and effective period. | `rule_ids`/artifact scope, `regime_id`, `effective_range`, `legal_source_hash` are all load-bearing here. |
| `interpretation` | Interpretation notes for vague terms, thresholds, exceptions, and regime-specific judgments are acceptable. | The set of `rule_ids` whose `interpretation_notes` are under review. |
| `scenario_coverage` | The signed test corpus is sufficient for the expert's review scope. | The reviewed `TestCorpus` artifact — see `test_corpus_hash` (§3, **proposed addition**). |
| `equivalence_claim` | A cross-regime equivalence / non-equivalence claim is valid under stated conditions. | The `EquivalenceMatrix` artifact + the regime pair; conditions live in `reviewer_comments`. |
| `publication_approval` | The artifact may be published to a named environment under a named policy. | `environment` name + `attestation_policy_version`; honored only with required co-attestations (§6). |

The expert's **authorization** to sign a given type is keyed by `signer_role`
and verified against the key directory _(pending ADR 0009)_; an
otherwise-valid key signing a type it is not authorized for is rejected (§5,
rejection rule R1).

---

## 3. Bound fields (all required unless marked optional)

Every field below is part of the canonically-encoded, signed payload. "Binds"
states what the signature commits to. Canonical encoding is the Gate-1 profile
(postcard codec, ADR 0002; decimal mantissa/scale, ADR 0003; jurisdiction time,
ADR 0001), so attestation bytes round-trip identically in Rust and Python.

| Field | Type (shape) | Binds | Notes |
|-------|--------------|-------|-------|
| `artifact_hash` | `[u8; 32]` (BLAKE3) | The exact artifact bytes being attested. | Mismatch vs the artifact under execution => rejection R2. |
| `scope` | `rule_ids: Vec<RuleId>` **or** `artifact_scope` marker | Which rules / whole artifact the claim covers. | One of the two must be present. |
| `attestation_type` | `AttestationType` (§2) | The single named claim. | Drives required-type checks (§6) and authorization (R1). |
| `signer_identity` | signer DN / subject | Who signed. | Form _(pending ADR 0009)_. |
| `key_id` | key identifier | Which key signed; resolves to the directory entry. | Verified against directory + revocation list (R1). |
| `signer_role` | role enum | Authorization basis for the type. | Role->allowed-types map _(pending ADR 0009)_. |
| `regime_id` | `String` | The regime the claim is scoped to. | Must match the artifact manifest `regime_id`. |
| `effective_range` | `from: JurisdictionDate`, `to: Option<JurisdictionDate>` | The effective period the claim covers. | `[from, to)` half-open per ADR 0007. |
| `legal_source_hash` | `[u8; 32]` (BLAKE3) | The legal source the encoding was reviewed against. | Hash-only storage for Gate 4 (brief decision 6). Change after attestation => rejection R5. |
| `ir_schema_version` | `SchemaVersion` | The IR schema the artifact was compiled under. | Drift => unsupported (folds into R3 / policy). |
| `compiler_version` | `SemVer` | The compiler that produced the artifact. | Recorded for audit reconstruction (§18). |
| `attestation_policy_version` | `String` | The policy version the attestation was made under. | Unsupported => rejection R3. |
| `timestamp` | trusted-timestamp envelope (§4) | When it was signed, per a trusted authority. | Mock TSA => rejection R8 under non-local policy. |
| `expiration` | `Option<JurisdictionDate>` | Optional validity horizon. | Past => rejection R4. |
| `reviewer_comments` | `Option<String>` | Free-text rationale / stated conditions. | Required-conditions for `equivalence_claim` live here. |
| `test_corpus_hash` _(proposed §10 addition — see §6)_ | `[u8; 32]` (BLAKE3) | The `TestCorpus` artifact the expert actually reviewed. | **Not in the current spec §10 list.** Proposed for `scenario_coverage` / `equivalence_claim` to close a semantic-laundering gap; needs a §10 amendment / ADR sign-off before it becomes binding. |

All §10 fields above the divider are spec-mandated. `test_corpus_hash` is a
**proposed addition** flagged for reviewers, not yet authoritative.

---

## 4. Signature and timestamp envelope

- **Signature scheme:** ed25519 over the canonical encoding of the bound-field
  payload (postcard, ADR 0002). The signed bytes are the canonical encoding —
  there is no separate JSON-then-sign step, to avoid canonicalization drift
  (spec §20 "schema/canonicalization drift").
- **Self-reference:** `artifact_hash` is computed by the Gate-4
  zero-then-patch derivation (brief §3) over the artifact, **before** any
  attestation exists; attestations are appended and never alter artifact bytes
  (spec §9 "state transitions never mutate artifact bytes").
- **Timestamp authority _(pending ADR 0010)_:** v1 recommendation is an
  RFC 3161-compatible TSA. Local development may use a **deterministic mock
  TSA**, but artifacts/attestations stamped by the mock are **rejected by
  non-local runtime policy** (spec §10 "Timestamp authority"; rejection R8).
  The timestamp envelope carries the TSA token plus the authority identifier
  so the platform can distinguish mock from production at verification time.

---

## 5. Key identity and revocation verification _(pending ADR 0009)_

At both **registry time** and **runtime** (spec §20 "registry-time plus
runtime-time verification"), the verifier resolves `key_id` against the key
directory and checks:

1. the key exists and is **authorized for `attestation_type`** given `signer_role`;
2. the key is **not expired** (key-level expiry);
3. the key is **not revoked** (explicit revocation list);
4. the signature verifies against the directory's public key for `key_id`.

The concrete authority model (IdP-backed signing vs self-managed ed25519
identities + registry-held key directory + revocation list; HSM deferred) is
the brief's recommended v1 but is **not decided** — it is ADR 0009, Status:
Proposed, and must be Accepted before Phase 1.

---

## 6. Required types, co-attestation, and the semantic-laundering tie

**Required types** are policy-driven, not hard-coded: the registry enforces
`PolicyBundle.verification_policy.required_attestation_types` and
`minimum_attestation_count_per_type` (`ke-core` `VerificationPolicy`) per named
environment. Missing a required type => rejection R6.

**Semantic-laundering mitigation (spec §20; audit finding).** A valid ed25519
signature must never be sufficient on its own to make a weak encoding look
authoritative. Two mechanically-enforceable bindings are **recommended for v1**
(both reuse existing machinery; neither is new crypto):

- **(A) `test_corpus_hash` bound field** on `scenario_coverage` and
  `equivalence_claim`, content-addressing the reviewed `TestCorpus` artifact
  the same way `legal_source_hash` content-addresses the source. The platform
  verifies the referenced corpus is a registered artifact whose hash is
  unchanged. This proves _which_ corpus was reviewed; it is a **proposed §10
  amendment** (the field is not in the current §10 list) and needs reviewer
  sign-off.
- **(B) Required co-attestation for `publication_approval`:** honored only when
  a valid, non-expired `scenario_coverage` **and** `source_fidelity`
  attestation over the **same `artifact_hash`** already exist. Encoded purely
  as a registry-side required-types check via `required_attestation_types` —
  no new field.

**Explicit limit (do not overstate).** A+B prove the expert reviewed a
specific hash-pinned corpus and that required types are present. They **cannot**
prove the corpus is _adequate_ or the encoding _legally correct_; that judgment
is the human expert's and is outside what any signature can enforce. The
review-first UI distinction (compiler validity vs ML evidence vs AI suggestion
vs legal attestation) that further mitigates laundering is a **Gate-5 surface
concern and enforces nothing at execution time** — it must not be cited as a
Gate-4 enforcement control.

---

## 7. Platform rejection rules (spec §10, complete enumeration)

The platform **must reject** an attestation (and refuse to treat the artifact
as attested/executable under the relevant policy) if any of the following hold.
R1–R6 are the spec §10 list verbatim; R7–R8 are derived from spec §10
timestamp-authority text and the lifecycle/binding rules and are flagged as
**spec-derived, reviewer-confirm**.

| ID | Condition | Source |
|----|-----------|--------|
| R1 | The signing key is **unknown, expired, revoked, or unauthorized** for the `attestation_type`. | §10 (verbatim) + §5 |
| R2 | The attestation is **not bound to the `artifact_hash`** of the artifact being executed. | §10 (verbatim) |
| R3 | The `attestation_policy_version` is **unsupported** by the platform. | §10 (verbatim) |
| R4 | The attestation has **expired** (`expiration` in the past). | §10 (verbatim) |
| R5 | The `legal_source_hash` **changed after** attestation (recomputed source hash != bound hash). | §10 (verbatim) |
| R6 | One or more **required attestation types are missing** (per `required_attestation_types` / `minimum_attestation_count_per_type` for the environment). | §10 (verbatim) + §11 |
| R7 | A **required co-attestation is absent** — e.g. `publication_approval` without a valid `scenario_coverage` + `source_fidelity` over the same `artifact_hash` (if recommendation B is Accepted). | §20 mitigation; **spec-derived, reviewer-confirm** |
| R8 | The attestation is stamped by the **mock TSA under a non-local policy** (mock-stamped artifacts are rejected by non-local runtime policy). | §10 "Timestamp authority"; **spec-derived, reviewer-confirm** |

Additional binding sanity checks (subsumed by the above but called out for the
implementer): `regime_id` / `ir_schema_version` / `compiler_version` mismatch
versus the artifact manifest are treated as binding failures and rejected
(they make the attestation not validly bound to the artifact — folds into R2/R3
depending on field). These are recorded so the Gate-4 rejection-test matrix
(brief §6 acceptance: "missing, stale, revoked, or invalid attestations =>
rejected with a specific policy error") has named cases.

---

## 8. Frozen shapes this binds (Gate 1)

The enum/carrier shapes are already frozen in `ke-core`
(`crates/ke-core/src/manifest.rs`, JSON Schema
`crates/ke-core/schema/ir.schema.json`): `AttestationType`, `T2T3Mode`,
`RevocationPolicy`, `AttestationCount`, `VerificationPolicy`, `PolicyBundle`.
This document binds the **field semantics, signature/timestamp envelope, and
rejection rules** onto those shapes. Two shape gaps noted for the relevant
work-items (out of scope here, do not silently patch):

- `RevocationPolicy` — **resolved (ADR 0013, landed with the canon-4 bump to
  `0.4.0` / `ke-canon-4`):** the enum is now spec §15's named policies in §15
  order (`HardStop` / `FinishPinned` / `AuditOnly`).
- `interpretation_notes` is one rule-level `Option<String>`
  (`crates/ke-core/src/ir/rule.rs`); spec §17 wants per-branch coverage. The
  `interpretation` attestation binds `rule_ids` today; per-branch binding is a
  follow-on once the IR field is widened.

---

## 9. Blocking decisions (must be Accepted before Phase 1)

| Decision | ADR | Affects fields/rules here |
|----------|-----|---------------------------|
| Expert key authority + revocation (§21.1) | 0009 | `signer_identity`, `key_id`, `signer_role`, §5, R1 |
| Trusted timestamp authority (§21.5) | 0010 | `timestamp` envelope (§4), R8 |
| T2/T3 publication policy + sidecar (§21.2/§21.3) | 0011 | required-types policy (§6), `publication_approval` honoring |
| S3 registry + PEP 503 layout (§21 resolved-v1) | 0012 | where attestations/events are persisted |
| `test_corpus_hash` as a §10 bound field | (spec §10 amendment / note in 0009) | §3, §6(A), R7 |

All ADRs are **Status: Proposed** until Hossain + the security/domain
reviewers accept them. This schema proposes; it does not decide.
