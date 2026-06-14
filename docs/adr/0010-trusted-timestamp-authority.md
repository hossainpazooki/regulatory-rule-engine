# 0010. Trusted timestamp authority for typed attestations

**Status:** Accepted (sign-off by Hossain, 2026-06-11)
**Date:** 2026-06-11
**Spec references:** §10 (typed attestation model — bound `timestamp from a trusted timestamp authority`; "Timestamp authority" subsection), §21.5 (open decision — trusted timestamp authority), §9 (lifecycle — registry events as the time anchor), §20 (risks — key compromise / stale attestation)
**Brief references:** `dev/briefs/gate-4-artifact-registry-attestation.md` §2 (decision 2), §8 ("Expert key compromise / stale attestation")
**Gate:** 4 (prerequisite — must be resolved before Phase 1 per §23)

> Accepted 2026-06-11 (sign-off by Hossain). Final provider selection remains
> a **procurement/legal** open item (see Consequences → "Open operational
> items"); production publish stays blocked until a real TSA is onboarded.
> No LLM/AI code appears anywhere in the timestamping, signing, attestation, or
> registry path ([[project-llm-authority-boundary]]).

## Context

Spec §10 makes a **timestamp from a trusted timestamp authority** a *required
bound field* of every typed attestation. The attestation is a signed claim by a
domain expert key, bound to a specific `artifact_hash`; the timestamp records
*when* that claim was made. §10's "Timestamp authority" subsection (spec line
~476) sets the v1 default as **RFC 3161-compatible timestamping**, permits a
**deterministic mock TSA for local development only**, and states that
"artifacts signed with the mock authority are rejected by non-local runtime
policy." §21.5 leaves the concrete authority **open** — RFC 3161 provider,
internal TSA, or other approved authority — and §23 lists "RFC 3161-compatible
TSA or approved alternative selected" as a **Before-Gate-4 checklist item** that
must be green before Phase 1 begins. This ADR resolves the *design* of that
selection; it does not (and cannot) finalize the *vendor*.

Why this is load-bearing rather than a formality:

1. **The timestamp is a security boundary, not metadata.** Several spec §10
   platform rejection rules are time-relative: "attestation has expired",
   "legal source hash changed *after* attestation", and key validity ("key is
   expired, revoked"). Every one of these reduces to a comparison against the
   attestation timestamp. If the timestamp can be chosen by the attester, all
   three checks are defeatable. Concretely — and this is the audit's primary
   concern (a) — a holder of a **compromised or about-to-be-revoked expert
   key**, working with a **permissive or compromised TSA**, can mint an
   attestation **backdated to before the revocation instant**, making a revoked
   key appear to have signed while still valid. A trustworthy, independent time
   source is what closes that window. The spec's threat model (§20) names "key
   compromise / stale attestation" but under-stresses the TSA itself as the
   trust dependency that the mitigation rests on.

2. **The spec's mock-vs-real distinction, as written, is self-declared.** §10
   says mock-stamped artifacts "are rejected by non-local runtime policy," which
   reads as: the runtime *inspects a field* and decides. A self-declared field
   is forgeable — an attacker who can assemble an attestation can claim the
   "real TSA" value. The audit's concern is that the mock/real marker must be
   **bound into the signed payload and re-derivable from the timestamp token
   itself**, so that forging it breaks signature verification rather than merely
   tripping a policy check. This ADR specifies that binding.

3. **There is no interim production path until a vendor is chosen** (audit
   concern (b)). Selecting an external RFC 3161 provider is a procurement and
   legal action (contract, SLA, trust-anchor onboarding, jurisdiction/retention
   review) that the workbench team cannot complete unilaterally. Gate 4 code can
   and should be built against the *interface*, but **no production publish may
   occur** until a real TSA is onboarded. This ADR states that blocker
   explicitly and defines how dev/staging proceed in the meantime without ever
   letting a mock-stamped artifact reach production.

Carried-forward constraint (Gate 3, ADR 0008 / brief §3.3,
[[toolchain-windows-gnu-getrandom-dlltool]]): all signing/keygen in tests is
**deterministic** with fixed keys; CI must not pull `getrandom`/OS randomness.
The mock TSA therefore must be deterministic (fixed key, fixed or
caller-supplied clock) so golden artifacts and contract-test fixtures stay
reproducible byte-for-byte.

## Decision

**1. v1 production authority = an RFC 3161-compatible *external* timestamp
provider (accepted 2026-06-11; concrete vendor selection still open).** Production attestations are stamped by
a third-party RFC 3161 TSA that returns a signed timestamp token (TSTInfo) over
the hash of the attestation's signed content. The provider is independent of the
party holding the expert signing keys, so the signer cannot unilaterally choose
or backdate the time. The specific vendor is **deferred to org sign-off** (see
Alternatives and Consequences); Gate 4 implements against the RFC 3161
*interface*, not a hard-coded vendor.

**2. Dev/test authority = a deterministic mock TSA, structurally non-production.**
A `MockTsa` produces a timestamp token of a **distinct, signed TSA-class** (see
point 3) using a fixed dev key and a caller-supplied clock. It is reproducible
and offline (no network, no `getrandom`). It exists only so Phases 1–5 and the
cross-language contract test can run without a live provider.

**3. The TSA-class marker is bound into the SIGNED attestation payload — not a
self-declared field.** This is the core correction to the spec's wording. We
define a typed `TimestampAuthorityClass` discriminant
(`Rfc3161External { tsa_identity }` | `Rfc3161Internal { tsa_identity }` |
`Mock`) and a `TimestampToken { class, tsa_token_bytes, claimed_time }`. The
attestation's **signed bytes include `class` and a hash of `tsa_token_bytes`**,
so the expert key signs over the TSA class. At verification:
   - the class is **re-derived from `tsa_token_bytes`** (parse the RFC 3161
     token, validate its TSA signature against the registered TSA trust anchor,
     read the real authority identity) — never read from a plaintext claim;
   - the re-derived class must **equal** the signed `class`, or the attestation
     is **rejected** (tamper-evident: a mock token cannot be relabelled "real"
     without breaking the expert signature, and a real token cannot be forged
     without the TSA's key);
   - a `Mock` class is **rejected by any non-local runtime policy** as a typed
     error (not a soft warning), and is the *only* class accepted under the
     `disabled`/local policy mode (spec §11). This makes "non-local rejects
     mock" a property of the *type system + signature*, not a policy that could
     be misconfigured to accept mock.

**4. Anti-backdating: monotonic ordering against the registry event log.** The
attestation timestamp alone is necessary but not sufficient against backdating.
At the point an attestation is recorded (`expert_attested` transition, spec §9),
the registry **must reject** an attestation whose `claimed_time` is **earlier
than the most recent relevant signed registry event** for that artifact and
signer key (e.g. earlier than the key's registration, or not strictly after the
prior lifecycle event timestamp), and **must reject** any attestation whose
`claimed_time` post-dates a recorded revocation of its signing key. The registry
append-only event log (§9) is the monotonic anchor; the external TSA time and
the registry event time must be **consistent within a stated skew bound**
(skew bound is a sign-off parameter, see Open items). This is verified **at
attestation time** (registry) **and re-verified at runtime** (platform), per the
brief §8 "registry-time and runtime-time verification" mitigation — neither
check alone is trusted.

**5. Production publish is BLOCKED until a real TSA is onboarded; dev/staging
proceed on the mock under a non-production policy.** Until org sign-off selects
and onboards a provider:
   - **dev/test** run on `MockTsa` with `disabled`/local policy (spec §11);
   - **staging** may run on `MockTsa` **only** under an explicit
     `staging`/`advisory` policy whose published artifacts are **structurally
     incapable of being promoted to production** — promotion to a production
     environment requires a non-`Mock` `TimestampAuthorityClass`, enforced by the
     publish path as a typed precondition (a `Mock`-classed artifact returns a
     specific policy error on a production publish attempt);
   - **production** publish is **refused** while no real TSA trust anchor is
     registered. This is the explicit Gate-4 blocker: Phase 3 publish can be
     built and exercised against staging, but the production publish path is
     dark until sign-off + onboarding complete.

## Consequences

**Desirable**
- The required §10 timestamp field gets a concrete, RFC 3161-standard meaning
  with an independent time source, directly mitigating the §20 "key compromise /
  stale attestation" risk and the audit's backdating concern.
- The mock-vs-real boundary becomes **tamper-evident** (bound into the signed
  payload + re-derived from the token), upgrading "non-local policy rejects mock"
  from a configurable check to a signature/type invariant.
- Backdating is constrained from two independent sides: an external TSA the
  signer cannot control, *and* monotonic ordering against the append-only
  registry log, checked at both attestation time and runtime.
- Dev/test stay deterministic and offline (fixed mock key, supplied clock; no
  `getrandom`), so golden vectors and the cross-language contract test remain
  reproducible — consistent with brief §3.3 and the windows-gnu toolchain note.
- Gate-4 code can proceed against the RFC 3161 interface now; only the
  production publish path waits on procurement.

**Undesirable / costs**
- **External provider = runtime/availability + legal dependency.** A live TSA
  is a network dependency at attestation time and a trust anchor that must be
  rotated, monitored, and contractually covered (SLA, jurisdiction, retention).
  TSA outage blocks new production attestations (existing published artifacts are
  unaffected).
- **No interim production publish.** Until onboarding completes, production is
  blocked by design. This is intended (better than a forgeable interim path) but
  is a real schedule dependency on Hossain/org, not on engineering.
- **Skew handling adds a parameter.** Tolerated skew between TSA time and
  registry-event time is a sign-off parameter; too loose reopens a backdating
  window, too tight causes spurious rejections.
- **Mock must never leak.** The whole design rests on the type/signature
  enforcement that mock-classed artifacts cannot be promoted to production;
  this invariant needs an explicit test in Phase 3 (a production publish of a
  mock-stamped artifact must fail with a specific policy error).

**Open operational items (Accepted 2026-06-11; the items below remain open)**
- **Hossain + org security:** select the concrete RFC 3161 provider; approve the
  tolerated TSA/registry skew bound and the TSA key-rotation/trust-anchor policy.
  (External-vs-internal for v1 is decided: external.)
- **Org legal/procurement:** complete TSA contract, jurisdiction, and retention
  review; this is the gating item for the production publish path.
- **Domain reviewer:** confirm the monotonic-ordering rule (attestation
  `claimed_time` strictly after prior lifecycle/key-registration events, and
  rejected if after key revocation) matches the intended attestation semantics.

## Alternatives considered

- **Internal org-run TSA (option B) as v1.** Rejected for v1 (revisit if
  procurement stalls): an internal TSA places the time source under the same org
  that holds the expert signing keys, so a sufficiently privileged insider could
  both sign an attestation and backdate its timestamp — collapsing exactly the
  separation the timestamp is meant to provide against backdating-before-
  revocation. Acceptable as a fallback **only** if operated under
  separated-duty controls and an independent trust anchor, which is a heavier
  lift than onboarding an external provider; kept as a documented fallback if
  external procurement is infeasible.

- **Transparency-log / append-only-ledger or blockchain anchor (option C).**
  Rejected for v1, retained as future: a transparency log (RFC 9162-style) or
  ledger anchor gives strong non-backdating and public verifiability, but adds
  infrastructure and operational surface disproportionate to Gate 4, and is not
  the spec's named default. Revisit if the org adopts transparency-log
  infrastructure org-wide; the `TimestampAuthorityClass` enum is designed to
  accommodate a new variant without changing the signed-binding mechanism.

- **Self-declared `is_mock` boolean checked only by runtime policy (the literal
  reading of spec §10 line ~476).** Rejected: a plaintext, signer-supplied field
  is forgeable and makes "non-local rejects mock" depend on every runtime being
  correctly configured. Binding the TSA class into the signed payload and
  re-deriving it from the token makes the distinction a signature/type invariant
  instead — strictly stronger, and the spec's intent ("rejected by non-local
  runtime policy") is preserved as the policy layer *on top of* the cryptographic
  binding. This ADR amends the *mechanism*, not the spec's stated outcome.

- **No timestamp / signing-key clock only.** Rejected: violates the §10 required
  bound field and provides no independent, non-backdatable time, leaving the
  expiry / "legal source hash changed after attestation" / key-validity checks
  trivially defeatable.

- **Skip monotonic registry-log ordering, rely on the TSA alone.** Rejected: a
  single compromised/permissive TSA would then fully determine attestation time.
  Cross-checking the TSA time against the append-only registry event log (§9) at
  both attestation time and runtime gives a second, independent constraint, per
  the brief §8 "registry-time and runtime-time verification" mitigation.

## Open items (parameters for sign-off, tracked into Phase 0/3)

- Concrete RFC 3161 provider selection + trust-anchor onboarding (legal/
  procurement — the production-publish blocker).
- Tolerated TSA-vs-registry-event skew bound (security sign-off).
- TSA trust-anchor rotation and revocation policy; where the registered TSA
  trust anchor lives in the S3 registry model (coordinate with ADR 0012 layout).
- Exact `TimestampToken` / `TimestampAuthorityClass` encoding within the
  canonical postcard payload (Phase 2, with the attestation schema in
  `docs/attestation-schema.md`); any shape addition bumps the canonical version
  triplet and regenerates golden vectors atomically (brief §4).