# 0025. Production key authority: custody split by signer role, compromise scope — closing ADR-0009's open items

**Status:** Proposed (sign-off: Hossain)
**Date:** 2026-07-21 (re-scoped 2026-09-18 — see "What this ADR no longer claims")
**Spec references:** § 20 (risks: key custody gap), § 21.1 (expert key authority), § 21.6 (revocation behavior), § 10 (typed attestations), § 9 (registry events)
**Related ADRs:** 0009 (key authority architecture — Accepted; this ADR closes its two post-acceptance open items), 0010 (trusted timestamp authority — named dependency, not solved here), 0012 (S3 registry / signed directory head distribution), 0013 (revocation policy enum), 0019 (consumer trust boundary), 0023 (graph exporter — exposure enumeration reused), 0024 (Gate-6 revocation decision — composition target)
**Amends:** nothing. ADR-0009's architecture stands unchanged; this ADR resolves the two items its acceptance note explicitly left open — (§ 1) IdP-backed vs self-managed custody, (§ 5) retroactive-vs-prospective compromise scope — and hardens the test-key policy for production contexts.
**Scope:** this registry's own signers — registry root, compiler, expert attester. Nothing here decides key custody for any downstream system that consumes these artifacts.

## Context

Every signature this platform produces today comes from fixed-seed test keys,
and every provenance record honestly says so (`is_test_key: true`). That was
the correct Phase-1 posture: ADR-0009 deliberately designed the on-artifact
shape (ed25519 signature + `key_id` + `signer_role`, resolved through a
registry-signed key directory with mandatory expiry and directory-carried
revocation) so that the custody decision could be deferred without blocking
any code.

ADR-0009 left exactly two things open, and both are decided here. Everything
else below is operationalization of what 0009 already accepted.

Why record the closure now rather than at the moment a production consumer
appears: these two items are the last *design-level* gaps between the registry
as built and any use of its artifacts outside `local` policy, and the Gate-6
revocation machinery this composes with (ADR-0024) is freshly landed and
fully in view. Deciding them while that context is loaded costs nothing and
removes the excuse of a design question from whatever consumer arrives first.
Nothing here asserts a production date, a deployment, or a buyer.

### What this ADR no longer claims

The 2026-07-21 draft grounded this work in a corporate repositioning, and
named as its forcing signer a tenant's own officer — a CFO or controller —
attesting a delegation-of-authority `IntentSpec` from this registry. **That
framing is withdrawn**, on two independent grounds:

1. The repositioning it cited was recorded, by explicit instruction in the
   same session, as *discussion substance and not a decision*. An ADR resting
   its necessity on an unratified strategy was overstating its own trigger.
2. The signer it described has moved. The authorization plane that consumes
   this registry has since made its own signed payload the single spec it
   executes, attested at its own control seat, and it reaches this registry as
   a declarant rather than as a reader of specs published here. Key custody
   for *that* plane's attesters is that project's decision, not this one's.

What survives is what was always about this registry's own signers: root and
compiler custody, expert-attester custody, compromise scope, and the test-key
policy. The decisions below are unchanged from the draft; only their
justification and the naming of the third signer role have been corrected.

## Decision

### 1. Custody is decided per signer role, not globally

The IdP-vs-self-managed question dissolves once it is asked per role — the
three roles have different holders, volumes, and blast radii:

| Role | Holder | Custody (decided) | Rationale |
|---|---|---|---|
| **Registry root** | platform operator | Managed KMS/HSM, never on disk (as ADR-0009 § 3 already decided); break-glass rotation via the signed `root-rotation` chain | Single trust anchor; one key justifies HSM-grade custody |
| **Compiler key** | platform operator | Same custody class as the root (KMS/HSM or non-extractable hardware token) | Low signing volume, root-adjacent blast radius (a forged compiler signature launders unverified IR) |
| **Expert attester keys** (the domain expert of record, per ADR-0009 § 1) | the attesting organization | **IdP-backed signing service — the default path.** Enrollment binds a short-lived ed25519 signing identity to the organization's IdP claims (identity + role); short mandatory `valid_to`; step-up authentication at attest time. **Self-managed hardware tokens remain the offered high-assurance alternative** for organizations whose own policy demands non-extractable custody. | Expert attesters already live in an IdP; hardware-token enrollment toil falls on exactly the person least able to absorb it, and an unusable custody path produces unsigned artifacts, not safer ones; short-lived keys make rotation continuous and shrink the compromise window. Both paths reduce to the same on-artifact shape (ADR-0009 § 1), so this choice is purely operational — no code change. |

**Signature scheme is pinned; custody adapts to it, never the reverse.**
ed25519 (RFC 8032, deterministic) is load-bearing in the canonical contract
and in CI determinism. Managed-KMS support for ed25519 signing is uneven
across providers as of this writing — that is a *selection criterion for the
KMS/HSM*, verified per provider during procurement, not a reason to migrate
the scheme. A signature-scheme migration is rejected here outright; if ever
genuinely forced, it is its own ADR with canonical-contract implications.

Multi-organization note: v1 keeps the single registry-signed directory; expert
attesters are ordinary entries whose `authorized_attestation_types` (and, for
kinds that require a co-attestation, the kind-selected rule of ADR-0022) scope
what they may sign. Per-organization directory partitioning is future work,
not blocked by anything here.

### 2. Compromise scope: retroactive for trust, prospective for history — with exposure made enumerable

Closing ADR-0009 § 5's open item:

- **Selection/verification time — retroactive, decided.** A key revoked for
  compromise renders every attestation it produced suspect **from the key's
  `valid_from`**. All future resolve/verify/publish paths fail closed on
  those attestations, exactly as 0009 § 5 sketched. Routine expiry/rotation
  stays prospective: attestations valid at signing time (per the trusted
  timestamp) remain valid.
- **Already-executed historical decisions — prospective by default,
  re-adjudication is the artifact owner's call, made on evidence we generate.**
  The platform does not auto-invalidate executed history: whether a past
  decision must be unwound is a legal/domain judgment the engine is forbidden
  to make (the compiler-authority boundary, applied to time). What the
  platform owes instead is **enumerable exposure**: a compromise event
  triggers an exposure report — every artifact whose attestation set involves
  the revoked key, and transitively everything selected or settled against
  those artifacts. The ADR-0023 graph exporter's oracle-exposure query
  (citation-closure-then-one-hop) is the existing machinery for exactly this
  shape and is reused, not rebuilt.

### 3. Production policy contexts reject test keys outright

Today `is_test_key: true` is surfaced and the consumer must display it. That
is right for `local`. In any non-local policy environment, a test-key
signature becomes a **rejection**, not a warning — the same fail-closed
pattern R8 already applies to mock-TSA timestamps under non-local policy.
This is the one code deliverable of this ADR (verify-layer rule + tests);
everything else is operational.

### 4. Composition with Gate 6 (shipped machinery, no new code)

A key revoked for compromise cascades artifact revocations recorded via
`revocation_decision(KeyCompromise, configured)` — HardStop floor, a
configured policy may only raise (ADR-0024, live since PR #17). The
key-directory `revocation_reason` and the artifact sidecar `reason_class`
speak the same ADR-0009 § 4 vocabulary end to end.

## Named dependency, not solved here

**Trusted timestamps (ADR-0010).** Prospective validity for rotated keys
rests on "valid at signing time per trusted timestamp." Production
attestation therefore needs the ADR-0010 TSA decision operationalized;
today's MockTsa verifies only under `local` policy. This ADR sequences
before, and does not substitute for, that work.

## Invariant

No signing authority moves: the authoring plane still cannot sign at all;
experts/attesters sign only attestations; only the registry root signs
lifecycle, directory, and tag events; `verify` stays fail-closed and gains
one more reason to reject (test key under non-local policy). Nothing here
touches the canonical encoding — no canon bump.

## Acceptance criteria

1. Registry root + compiler keys held in the selected KMS/HSM (ed25519
   signing verified live against it); break-glass root-rotation runbook
   written and rehearsed once.
2. Expert-attester enrollment path specified end-to-end (IdP claim binding,
   `valid_to` policy, step-up at attest) and exercised against one real IdP;
   hardware-token alternative documented.
3. Non-local policy rejects `is_test_key` signatures — rule shipped, tested
   (positive + negative), `cargo test --workspace --features test-keys`
   green.
4. Compromise runbook: revoke key (KeyCompromise) → directory event →
   cascaded artifact revocations (0024 floor) → exposure report generated
   from the graph exporter — rehearsed once on a seeded registry.
5. ADR-0009's acceptance note updated to point here for its two closed
   items; spec § 21.1 row and CLAUDE.md open-decisions table updated.

## Alternatives considered

- **One custody model for all roles.** Rejected: forces either HSM toil onto
  expert attesters (an unusable path yields unsigned artifacts, not safer
  ones) or IdP dependence onto the trust anchor (root custody weaker than
  0009 § 3 already requires). The role split is the whole answer.
- **Retroactive invalidation of executed history on compromise.** Rejected
  as an automatic behavior: the engine would be asserting legal consequence.
  Kept as the artifact owner's decision made on the exposure report —
  evidence generation is our job, adjudication is not.
- **Signature-scheme migration to widen KMS choice.** Rejected (above):
  custody adapts to the pinned scheme, not the reverse.
- **Warn-only test-key handling in production.** Rejected: a warning on a
  forged-authority path is a rubber stamp with extra steps; contradicts
  ADR-0019's fail-closed discipline.
- **Waiting for a named production consumer before deciding.** Rejected on
  the narrow ground stated in Context: these are design-level gaps, the
  context for closing them is loaded now, and a decision recorded as Proposed
  binds nobody until it is signed off.
