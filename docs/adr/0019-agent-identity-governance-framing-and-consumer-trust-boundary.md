# 0019. Agent-identity governance framing for the artifact trust chain, and the COMPASS federated-consumer trust boundary

**Status:** Proposed
**Date:** 2026-06-15
**Spec references:** § 5/§ 10/§ 13 (authority boundaries), § 9 (lifecycle state machine), § 14/§ 16 (verification + multi-surface access), § 20 (threat model)
**Relates to:** ADR 0009 (expert key authority / key directory / revocation), ADR 0013 (revocation policy + rollback eligibility), ADR 0016 (consumer-agnostic verification), ADR 0017 (platform-api decoupled; COMPASS is the consumer), ADR 0018 (`ke serve` non-authoritative)
**Prompting source:** an external practitioner article on enterprise *agent-identity* governance (durable agent identity vs. runtime instance; lifecycle as control points; credential ≠ authorization; federated trust boundaries). This ADR adapts that vocabulary; it does **not** import its agent-registry apparatus wholesale (see Decision 3).

## Context

A governance model written for **AI agent identity** (registering the actor, giving it a lifecycle, issuing it short-lived credentials, and authorizing each task) turns out to be a near-isomorphic lens over what ATLAS already built for **artifacts**. Two facts make the mapping worth recording, because both are load-bearing for how we *document and audit* the trust chain — and one of them is a decision we have not yet written down:

1. **ATLAS governs the artifact, not the actor — and excludes the AI actor from authority by design.** The article assumes AI agents are production actors taking authoritative actions, so they must be given a governed identity. ATLAS's authority boundary (CLAUDE.md § "Authority boundaries"; spec § 5/§ 10/§ 13) answers the same risk by *exclusion*: the compiler is structural only, the AI may propose but **never** attest/publish/revoke, only a domain-expert key signs typed attestations, and only the registry transitions lifecycle state. The article's machinery — durable identity separate from the running instance, lifecycle states that act as control points, credentials that prove the caller but not the authority — is already present, but expressed over *artifacts and keys* rather than *agents*. Recording the correspondence makes the existing design legible to an auditor in the register they expect.

2. **The consumer-side federation boundary is not yet in any ADR.** ADR 0009 fixes the *producer/registry* side (key directory, registry root, revocation, dual-time re-verification). ADR 0016/0017/0018 fix the verification surface and that COMPASS — not platform-api — is the consumer. But the rule that COMPASS, as a **federated consumer crossing a trust boundary**, must *re-derive* trust rather than *import* it lives only in a local brief (`dev/briefs/compass-consumer-state-and-gate5-rewire.md`). The article states the principle crisply ("an outside agent should not be treated as known simply because it presents a token from another system… federation requires explicit issuer trust, tenant mapping, registration, credential validation, and limits on what external claims can mean"), and it is exactly the boundary the post-Gate-5 COMPASS rewire must enforce. It deserves a normative home.

This ADR re-decides nothing in 0009/0013/0016/0017/0018. It (a) adopts a governance vocabulary as the documented audit lens over those decisions, and (b) records the consumer federation boundary that is currently only briefed.

## Decision

### 1. Adopt three framing principles as the documented governance register for the trust chain

These restate existing controls in agent-identity vocabulary. They are non-normative *framing* — each maps to an already-accepted mechanism — and are the language to use when documenting the trust chain for audit, security review, or regulatory/legal discovery.

| Principle (article vocabulary) | ATLAS realization (already decided) |
|---|---|
| **Identity is separate from the running instance; identity/audit outlasts the instance.** | The content hash is the durable artifact identity; the `tags/<env>/<tag>` pointer is the "deployed instance." Resolve-by-tag selects the instance; the hash and the append-only event log outlast it (Gate-4 C4 prior-distinct-hash rollback; spec § 9, § 15). |
| **Lifecycle state is a control point, not a label** — suspended/deprecated/**revoked** must *drive* controls, not annotate a row. | Registry lifecycle (`published / deprecated / revoked`, spec § 9) is enforced: only registry authority transitions it (CLAUDE.md); rollback may only target a `published` hash (ADR 0013 D3); a revoked key triggers registry-time recompute **and** runtime re-check (ADR 0009 § 5). |
| **Credential ≠ authority; do not collapse identity + authorization + enforcement into one token.** | Signature/hash verification proves *provenance*; registry lifecycle state authorizes *use*; policy/attestation checks gate *the specific execution*. A valid ed25519 signature over a matching hash does **not** by itself authorize use — current registry state must say `published` (spec § 14/§ 15; ADR 0009). |

### 2. Normative: the COMPASS federated-consumer trust boundary

COMPASS (`cross-border-compliance-navigator`) is a federated consumer crossing the ATLAS → consumer trust boundary. When it verifies in-browser (post-Gate-5, behind `NEXT_PUBLIC_USE_WASM_VERIFY`), it **re-derives** trust and never imports ATLAS's trust decisions. Concretely it MUST, per execution/surface:

- **Verify the issuer.** Trust an attestation only via the registry-signed **key directory** + **registry root** chain (ADR 0009 § 1–§ 3) — key status, expiry, and `authorized_attestation_types`. A signature alone is not issuer trust.
- **Map the tenant.** Bind the verdict to the declared regime (`regime_id`, e.g. `mica_2023`); a pack for one regime may not be surfaced as authority for another.
- **Confirm registration (lifecycle state).** Read the **canonical registry view** (via `ke serve`, ADR 0018), not a vendored snapshot, and treat any non-`published` state — `deprecated`, `revoked`, or **`unknown`** — as **blocked even when the signature and hash verify** (lifecycle-as-control-point, Decision 1). Fail **closed** on `unknown`.
- **Check revocation.** Honor the signed directory `status`/`revoked_at` (ADR 0009 § 2) and the reason-sensitive behavior map (ADR 0009 § 4); a revoked key/pack reads as blocked regardless of crypto validity.
- **Never assert validity it cannot re-derive.** A vendored snapshot proves **origin, not current validity** — it MUST be marked `registry_state: unknown` and surfaced as provenance, never as a live verdict (the current stopgap; spec § 6/§ 16 "surfaced, never silently published").

This is the consumer half of ADR 0009's dual-time verification and the headline acceptance of the COMPASS rewire (revoked-pack flagging).

### 3. Do *not* adopt an agent-identity registry for the artifact path

The article's central remedy — register each AI actor, give it credentials and per-task grants — is intentionally **not** adopted for the artifact/runtime path, because ATLAS removes the AI actor's authority entirely rather than governing it (Decision 1, point 1; spec § 5/§ 10/§ 13). Building an agent registry to govern an actor that has no authority would weaken a deliberately stronger guarantee. The article's delegation/lineage concerns (parent identity ≠ child credential; audit the delegation chain) apply only to ATLAS's **dev-time multi-agent orchestration** (how the system is built and reviewed), which is out of scope for this ADR and not part of the artifact trust chain.

## Consequences

- **Desirable.** Auditors and security reviewers get the trust chain in the vocabulary they expect, mapped 1:1 to accepted mechanisms, with no new code. The COMPASS federation boundary gains a normative home instead of living only in a local brief, so the post-Gate-5 rewire has an ADR to satisfy. The "valid crypto ≠ current authority" and "fail-closed on unknown" rules are stated once, canonically.
- **Cost / undesirable.** One more ADR to keep consistent with 0009/0013/0017/0018 if those evolve. The framing table is documentation that must not drift from the code it describes (mitigated: it points at the accepted ADRs rather than restating their mechanics).
- **Follow-ups.** (a) CLAUDE.md's open-decisions table still lists "Expert key authority → Gate 4" as open — stale since ADR 0009; correct it in a separate docs change. (b) The normative Decision 2 should be cited by the COMPASS rewire acceptance (`dev/briefs/compass-consumer-state-and-gate5-rewire.md` § 3) and by 5c/5d frontend-cutover acceptance.

## Alternatives considered

- **Amend ADR 0009 in place with the framing.** Rejected: 0009 is **Accepted** and records a decision at a point in time; bolting a later framing lens onto it muddies that record. A new **Proposed** ADR that references it is cleaner and reviewable on its own.
- **Build a full agent-identity registry for the AI actor.** Rejected: solves a problem ATLAS's authority boundary already removes by exclusion (Decision 3); it would add governance surface for an actor with no authority and risk implying the AI sits in the signing path.
- **Leave the consumer federation boundary in the local brief only.** Rejected: it is a real cross-trust-boundary decision (what external claims may mean inside the consumer, fail-closed posture, snapshot-is-origin-not-validity) that gates the COMPASS rewire; a local, unpushed brief is not a durable normative home.
- **Treat the article as inspiration and write nothing.** Rejected: the federation boundary is currently unrecorded and the "lifecycle-as-control-point / credential ≠ authority" framing is exactly the register the COMPASS revoked-pack acceptance needs; capturing it now prevents the rewire session from re-deriving it.
