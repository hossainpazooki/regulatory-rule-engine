# 0009. Expert key authority, key lifecycle, and revocation behavior

**Status:** Proposed
**Date:** 2026-06-11
**Spec references:** § 21.1, § 21.6, § 20 ("Expert key compromise or stale attestation"), § 10 (typed attestation bound fields: signer identity / key ID / signer role), § 9 (lifecycle state machine), § 15 (revocation events, platform behavior by state), § 18 (audit contract)

## Context

Gate 4 turns a compiled IR into a *signed, content-addressed, attestable* artifact and stands up the registry lifecycle (`draft → structurally_verified → ml_checked → expert_attested → published → deprecated → revoked`, § 9). Two things gate Phase 1 of that work and have no resolved answer:

1. **Whose keys, governed how (§ 21.1).** § 10 requires every attestation to bind a *signer identity, key ID, and signer role*, and the platform must reject an attestation whose key is "unknown, expired, revoked, or unauthorized for the attestation type." None of that is enforceable without a decision on **where expert signing keys come from, how they are authorized per attestation type, how they expire, and how they are revoked** — and without a **distribution mechanism** the platform consumer can re-check at runtime. § 20 names this directly: compromised, replayed, or stale attestations bypass every semantic gate.

2. **What revocation *does* to work in flight (§ 21.6).** § 15 lists three revocation behaviors — **hard stop**, **finish pinned**, **audit-only** — but does not say which applies when, nor what a *compromised key* (as opposed to a routine recall) triggers. A pinned, already-running Temporal workflow (§ 15 pinning) is the hard case: its artifact hash was valid at pin time but a signer key has since been revoked.

A prior adversarial audit surfaced a third, related gap the § 23 "Before Gate 4" checklist omits:

3. **The registry is itself a trusted root, and its own signing key has no documented custody.** § 9 / § 15 require lifecycle transitions, tag moves, and revocations to be *signed registry events*. If the registry-event-signing key is compromised, an attacker can forge `published`/`revoked` transitions and rewrite the key directory — defeating every expert-key control above. The key-authority decision must therefore cover the registry root, not only expert keys.

A fourth, **contract-level** problem blocks the implementation regardless of which option is chosen. The Gate-1-frozen `RevocationPolicy` enum (`crates/ke-core/src/manifest.rs:84-88`) reads `HaltImmediately / FinishPinnedThenHalt / FinishPinnedNoNew`. Spec § 15 names the behaviors `hard stop / finish-pinned / audit-only`. The enum is mismatched 3-for-3 and **drops `audit-only` entirely**, so the canonical shape cannot represent one of the three spec behaviors this ADR selects among. This is recorded here and routed to **ADR 0013** (it touches frozen encoding and must bump the version triplet + regenerate golden vectors atomically).

Constraints that bound the decision:

- **ed25519 + postcard only in the verification path.** ADR 0002 (postcard codec) and spec § 8 fix signatures at ed25519 and the codec at postcard-1. Any option dragging X.509/ASN.1 or CRL/OCSP parsing into the artifact verification path conflicts with that minimalism and couples the Python consumer to a CA stack.
- **Deterministic test keys (brief § 3.3, MEMORY: `toolchain-windows-gnu-getrandom-dlltool`).** ed25519 signing is deterministic (RFC 8032); CI must not pull `getrandom`/OS randomness. Production keygen lives behind the key-authority boundary this ADR defines; tests use fixed keys.
- **Authority boundary (CLAUDE.md § "Authority boundaries").** Only domain-expert keys sign typed attestations; only the registry transitions lifecycle state; the compiler is structural only; **no LLM/AI code may sign, attest, publish, or modify rules** in any path. This ADR proposes; Hossain plus the security and domain reviewers decide. Status stays **Proposed**.

## Decision

> All choices below are **recommended v1 starting points requiring sign-off**, not decisions the author can make. The § 23 checklist boxes for § 21.1 / § 21.6 remain unchecked until Hossain + security + domain reviewers approve.

### 1. Expert key authority (§ 21.1)

Adopt **ed25519 signer identities sourced from either an IdP-backed signing service or self-managed hardware keys** — the operational choice between the two is deferred to security review, because **both reduce to the same on-artifact shape**: an ed25519 signature plus a `key_id` and a `signer_role`. Phase-1 code targets that shape and is therefore unblocked on the IdP-vs-self-managed pick; only the *enrollment/custody* operations differ.

Both are governed by a single **registry-held, registry-signed key directory** — the one authoritative source the platform consumer re-reads at runtime. Each entry:

```
KeyDirectoryEntry {
  key_id:                       String,          // stable identifier, bound into each attestation (§10)
  public_key:                   [u8; 32],        // ed25519
  signer_roles:                 Vec<SignerRole>, // e.g. DomainExpert, PublicationApprover, Registry
  authorized_attestation_types: Vec<AttestationType>, // enforces §10 "unauthorized for the type"
  valid_from:                   Timestamp,
  valid_to:                     Timestamp,       // expiry — non-optional; no perpetual keys
  status:                       Active | Expired | Revoked,
  revoked_at:                   Option<Timestamp>,
  revocation_reason:            Option<RevocationReason>,
  revocation_event_hash:        Option<[u8; 32]>, // back-pointer to the append-only event
}
```

The directory object is itself a **signed registry event** (§ 9), so the directory's integrity rests on the registry root key (item 3 below). Authorization is enforced as `key_id → authorized_attestation_types`: an attestation whose type is not in its signer's set is rejected exactly as § 10 requires.

**HSM (managed) and org PKI are deferred / rejected for v1.** Managed HSM is overkill for v1 expert volume and adds latency/cost; it remains the natural upgrade path if expert volume or a compliance mandate requires non-extractable custody. Org X.509 PKI is rejected for v1 because CRL/OCSP/ASN.1 in the verification path conflicts with the ed25519/postcard contract and couples the Python consumer to a CA chain. We get PKI-grade revocation more cheaply via the registry-held directory + revocation list below.

### 2. Key lifecycle: custody, rotation, expiry, revocation-list distribution

- **Custody.** Self-managed: keys are generated on hardware tokens (non-extractable) and only the public half is enrolled into the directory. IdP-backed: a signing service mints short-lived ed25519 signing identities bound to authenticated IdP claims (identity + role), and enrolls their public keys with a short `valid_to`.
- **Expiry.** `valid_to` is mandatory. Expired keys cannot produce new valid attestations; existing attestations are evaluated against the expiry/timestamp rules in § 5 below.
- **Rotation.** A new key is enrolled (new `key_id`) and the old key is moved to `Expired` (routine) via a signed directory event. Rotation does **not** rewrite past attestations; their validity is judged by the directory state + trusted timestamp (ADR 0010) at verification time.
- **Revocation-list distribution.** The revocation list is **not a separate artifact** — it is the `status`/`revoked_at` fields of the directory entries, carried in the registry-signed directory object. The platform consumer fetches the current signed directory head (S3, per ADR 0012 layout) and verifies its registry-root signature before trusting any `key_id`. Append-only revocation events (§ 15) chain to prior directory state so the history is auditable (§ 18).

### 3. Registry root key custody (the § 20 gap)

The registry signs lifecycle events, tag moves, revocations, and the key directory with its **own ed25519 registry-root key**. This is the single trust anchor for everything above, so it is the **one place HSM-grade custody is justified in v1**: hold the registry root in a managed KMS/HSM, never on disk. Root rotation is a documented break-glass procedure that issues a signed `root-rotation` event chaining the new root to the prior root and re-signs the current directory head, so consumers can follow the chain of trust across a rotation. (This item is arguably its own ADR; it is captured here because the audit found it ownerless and it cannot be left implicit before Phase 3 registry work.)

### 4. Revocation behavior for running pinned workflows (§ 21.6)

Revocation behavior is **a function of (revocation reason class, environment policy)**, not a single global mode. Recommended v1 mapping (needs sign-off):

| Revocation reason class | Recommended behavior | Rationale |
|---|---|---|
| **Key compromise** / cryptographic invalidity | **hard stop** — fail any workflow, pinned or new | A compromised key means the attestation's authority is void; continuing to execute on it launders a forged provenance (§ 20). |
| **Legal invalidity** (rule found wrong / superseded by law) | **hard stop** | The decision itself is now legally unsafe to emit. |
| **Routine supersession / deprecation-class recall** | **finish-pinned** — already-started pinned workflows complete; new starts blocked | Matches § 15 `deprecated` / `finish-pinned`; avoids tearing down in-flight work for a non-safety recall. |
| **Advisory / informational recall** | **audit-only** — execute, emit high-severity audit event (§ 18) | Evidence/visibility without disruption; reserved for non-blocking cases. |
| **Unclassified** (default) | **finish-pinned** | Fail-safer than audit-only, less disruptive than hard-stop; production policy MAY raise the floor to hard-stop. |

Production `PolicyBundle.revocation_policy` may only **raise** strictness above this floor, never lower it.

### 5. Compromised key → attestation invalidation (registry-time AND runtime-time)

A revocation event for a key triggers re-verification on **both** sides, per § 20:

- **Registry-publish time.** The registry recomputes lifecycle state for every artifact whose required typed-attestation set (§ 10, § 11 policy) now fails because an attestation was signed by the revoked key. An artifact in `expert_attested` whose only `source_fidelity` attestation came from the revoked key drops below the required count → it is no longer publishable, and any `published` pointer to it is moved to `revoked` via an append-only event.
- **Runtime time.** The platform re-checks **each attestation's signer `key_id`** against the *current* signed directory (status + expiry + authorized types) at execution time — not just at pin time. A pinned hash that was valid at pin time but whose attestation key is now revoked is handled by the § 4 behavior mapping above.

**Retroactive vs prospective scope** is an explicit sign-off item: for **compromise**, treat all attestations by that key as suspect from the key's `valid_from` (retroactive); for **routine expiry/rotation**, attestations valid at signing time (per trusted timestamp, ADR 0010) remain valid (prospective). Whether retroactive compromise additionally invalidates *already-executed historical decisions* (vs only future selection/execution) is a domain/legal call left open.

### 6. Contract reconciliation (blocking, routed out)

`RevocationPolicy` (`crates/ke-core/src/manifest.rs:84-88`) must be renamed to `{ HardStop, FinishPinned, AuditOnly }` to (a) match § 15 names and (b) restore the missing `audit-only` variant this ADR depends on. Because that enum is part of the **Gate-1-frozen canonical encoding**, the edit bumps the version triplet (`0.3.0 / ke-canon-3 / postcard-1`) and regenerates golden vectors atomically (brief § 4). This ADR does **not** make that edit and does **not** re-decide the enum: it depends on **ADR 0013 (revocation policy reconciliation)**, which owns the enum shape, the variant names, and the canonicalization bump, and must land before any § 15 revocation code in Phase 3. The revocation *behavior* mapping in § 4 above refers to ADR 0013's `HardStop / FinishPinned / AuditOnly` names (not the as-built `…ThenHalt/…NoNew` split).

## Consequences

**Desirable**

- § 10's "unknown / expired / revoked / unauthorized-for-type" rejection rules become mechanically enforceable from one signed source of truth (the key directory), without dragging X.509/CRL into the ed25519/postcard verification path.
- Phase 1 is unblocked on the IdP-vs-self-managed operational pick, since both share the on-artifact signer shape.
- The § 20 "key compromise" risk gets concrete, dual-sided mitigation (registry-time recompute + runtime re-check), and the previously-ownerless registry-root custody gap is closed.
- Revocation behavior is reason-sensitive: a forged/compromised attestation hard-stops, while a routine recall does not needlessly tear down in-flight pinned workflows.

**Undesirable / costs**

- IdP-backed signing adds an online dependency at attest time and a signing service to operate; self-managed hardware keys add manual enrollment/rotation toil. The choice is deferred, so one of those costs is incurred later.
- A managed KMS/HSM for the registry root is now a hard infra dependency for production (acceptable: it is one key, the trust anchor).
- Runtime re-verification of attestation keys against the current directory adds a directory fetch + signature check to the execution hot path (mitigated by caching the signed directory head with its own freshness/expiry).
- The canonical `RevocationPolicy` rename forces a version-triplet bump and golden-vector regeneration before Phase 3 — unavoidable, but it is a coordinated atomic change with cross-language (Python wheel) fallout.
- Retroactive compromise scope is left open; if domain/legal later decide historical decisions must be re-adjudicated, that is significant additional work not scoped here.

## Alternatives considered

- **Org-issued X.509 PKI (CRL/OCSP).** Mature revocation and role binding via cert extensions, but pulls ASN.1/X.509 and online OCSP into the verification path, conflicting with the ed25519/postcard contract (ADR 0002, § 8) and coupling the Python consumer to a CA. Rejected for v1; the registry-held directory + revocation list gives equivalent revocation semantics more cheaply.
- **Managed HSM / KMS for *expert* keys.** Strongest custody but cost/latency overkill at v1 expert volume. Deferred as the upgrade path; retained *only* for the registry root, where a single trust anchor justifies it.
- **Pure self-managed software keys with no directory.** Simplest, but no central revocation or expiry and no way for the consumer to learn a key is compromised — directly defeats the § 20 mitigation. Rejected.
- **Single global revocation mode** (one of hard-stop / finish-pinned / audit-only for all cases). Simpler, but conflates a compromised-key safety event with a routine recall: a global `audit-only` would let forged attestations keep executing, while a global `hard-stop` would needlessly kill in-flight pinned workflows on every routine deprecation. Rejected in favor of the reason×policy mapping.
- **Pin-time-only key verification (no runtime re-check).** Cheaper hot path, but a key compromised *after* pin time would never be caught for already-running workflows — exactly the § 20 stale/replayed-attestation hole. Rejected; § 20 explicitly requires registry-time **plus** runtime-time verification.
- **Keep the existing `RevocationPolicy` enum names.** Avoids a canonical bump, but leaves the shape unable to express `audit-only` and misaligned with § 15 vocabulary, guaranteeing drift between spec and code. Rejected; reconcile via shape-amendment ADR.