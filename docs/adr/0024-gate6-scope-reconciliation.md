# 0024. Gate-6 scope reconciliation: revocation runtime-decision + registry surface completion; platform cutover deferred

**Status:** Proposed (acceptance = PR merge, per the ADR-0023 precedent)
**Date:** 2026-07-19
**Spec references:** § 19 (Gate 6 acceptance), § 15 (revocation behavior, pinning), § 14 (cross-repo integration), § 18 (audit), § 21 (open decisions: revocation behavior)
**Relates to:** ADR-0009 § 4 (reason-class → policy table), ADR-0013 (revocation-policy reconciliation), ADR-0015 (Temporal orchestration ownership — Accepted with this ADR), ADR-0017 (platform-api decoupled), ADR-0019 (fail-closed consumer trust boundary), ADR-0012 (event shape frozen; sidecars live beside the log)
**Amends:** the spec § 19 Gate-6 acceptance criteria, which predate the ADR-0017 decoupling (the ADR-0020 "amend via ADR, don't rewrite spec prose" precedent).

## Context

Spec Gate 6 is "production cutover": the platform's Temporal worker resolves
rules exclusively through the registry via a startup pinning activity, and the
Python KE module is removed from `institutional-defi-platform-api`. Every one
of its acceptance criteria is phrased against that platform consumer.

**That consumer no longer exists.** ADR-0017 (Accepted 2026-06-15) decoupled
`institutional-defi-platform-api` from the ATLAS artifact path. The consumers
that do exist are ADR-0019-disciplined and verify-only: COMPASS (browser WASM),
the treasury intent resolver (`ke-artifact-py` fold), and the graph exporter
(ADR-0023). None is a Temporal orchestrator. So Gate 6's spec acceptance
criteria **cannot be met as written**, and per repo discipline (CLAUDE.md:
"If a gate's acceptance criteria can't be met, stop — don't lower the bar")
this reconciliation ADR records what closes, what ships, and what defers.

Two recon passes established the ground truth:

- **Already delivered** (Gates 4/5): `Selector::{ByHash,ByTag,ByRegime}` +
  effective-date `resolve()`, CLI `query --regime --effective`, rollback to
  Published-only targets, and `verify_artifact` rejecting any non-Published
  state including Revoked (fail-closed).
- **Out of scope** (ADR-0015/0017 + spec § 2 non-goals): the platform Temporal
  pinning activity (no consumer), Python KE removal (platform-repo concern),
  any Rust Temporal worker (never enters this repo).
- **Genuinely missing and ATLAS-buildable:** (1) the revocation
  **runtime-decision** — the registry records policy/reason/severity but the
  ADR-0009 § 4 reason-class → policy mapping was operationalized nowhere;
  (2) `serve /resolve` did not accept `?regime=&effective=` although the CLI
  did; (3) nothing surfaced the revocation record at the resolve/verify
  boundary for a consumer to act on.

## Decision

Gate 6 closes **as far as ATLAS legitimately can**, with the platform cutover
explicitly deferred (re-openable when a real orchestrator consumer exists):

1. **ADR-0015 Accepted** (same change): the three-channel ownership model,
   with channel 3 (platform pinning activity) deferred for lack of a consumer.

2. **Revocation runtime-decision, pure and consumer-agnostic**
   (`crates/ke-core/src/revocation.rs`):
   - `RevocationReasonClass { KeyCompromise, LegalInvalidity,
     RoutineSupersession, Advisory }` (ADR-0009 § 4);
   - `revocation_floor`: KeyCompromise/LegalInvalidity → HardStop,
     RoutineSupersession → FinishPinned, Advisory → AuditOnly;
   - strictness rank HardStop > FinishPinned > AuditOnly;
   - `revocation_decision(reason, configured) = stricter-of(floor,
     configured)` — a configured policy may only raise strictness above the
     floor, never lower it.

3. **`ke revoke --reason-class`** (`commands/revoke.rs`): records the class +
   the decided policy in the `revocations/<hash>.json` sidecar; a `--policy`
   below the class floor is rejected outright, before any state transition.
   The legacy `--policy`-only path is unchanged and its sidecar JSON is
   shape-identical (no `reason_class` key). The sidecar is **outside the
   canonical envelope** (verified before build: nothing hashes
   `RevocationRecord`; the ADR-0012 event shape stays frozen) — no
   canonicalization bump.

4. **Resolve surface completed** (`serve/handlers.rs`):
   `GET /resolve?regime=&effective=YYYY-MM-DD[&env=]` → `Selector::ByRegime`,
   reusing the CLI's date grammar. Pure read, non-authoritative.

5. **Revocation surfaced at the boundary**: when the resolved/verified state
   is `Revoked`, `ResolutionRecord` and `VerifyResponse` carry the sidecar as
   an optional `revocation` block — the inputs a consumer feeds to
   `revocation_decision`. Absent otherwise, so existing consumers see
   byte-identical responses for non-revoked artifacts.

## Invariant (non-negotiable)

`verify` stays **fail-closed**: it refuses any non-Published artifact
(including Revoked) even with valid crypto. The decision layer *informs* a
consumer how to wind down; it never loosens verify, and nothing here signs,
attests, publishes, or transitions lifecycle state from a new surface. No
Temporal code enters the repo.

## Honesty boundary

The revocation-decision function is **groundwork shipped ahead of its
enforcer**: no live orchestrator consumer runs HardStop/FinishPinned/AuditOnly
today (COMPASS is verify-only; the resolver and graph exporter fold verify per
hash). It operationalizes *already-accepted* policy (ADR-0009 § 4, ADR-0013)
and is fully tested, but "delivered" here means the decision + its surfacing —
not runtime enforcement, which has no home until an orchestrator consumer
exists.

## Re-scoped ATLAS Gate-6 acceptance

Gate 6 (ATLAS) closes when:

1. ADR-0015 Accepted; this ADR Accepted (= its PR merged).
2. `revocation_decision` implemented + unit-tested (reason-class matrix,
   floor-only-raises, strictness ordering).
3. `revoke --reason-class` records the class; `--policy` cannot lower below
   the floor; the legacy path is intact (`scripts/lifecycle-smoke.sh` green).
4. A revoked artifact's `reason_class` + policy are surfaced at
   `serve /resolve` and `/verify`; `/resolve` accepts
   `?regime=&effective=&env=` (all tested).
5. `verify` remains fail-closed — `cargo test --workspace --features
   test-keys` green; a live `serve-published-registry.sh` check still rejects
   the non-Published hash.
6. The CLAUDE.md § 21 revocation open-decision row is updated.

**Deferred + recorded (not blockers):** platform Temporal pinning activity,
Python KE module removal, Rust Temporal worker. Each re-opens only with a real
orchestrator consumer, via a new ADR citing this one.

Evidence for the criteria lives in `docs/gate-6-implementation-log.md`.

## Alternatives considered

- **Paper close** (accept ADR-0015, mark Gate 6 "closed as spec-obsolete",
  build nothing). Rejected: the reason-class decision and the resolve-surface
  gap are consumer-agnostic, small, and operationalize policy that is already
  Accepted — leaving them unbuilt keeps ADR-0009 § 4 a table nobody executes.
- **Wait for an orchestrator consumer, then build decision + enforcement
  together.** Rejected: couples an ATLAS-local, testable deliverable to a
  consumer that may never exist in this form; the decision function is the
  stable half regardless of who enforces it.
- **Enforce revocation policy inside `verify`.** Rejected outright: `verify`
  is fail-closed on any non-Published state already; making it
  policy-sensitive would *loosen* it for AuditOnly (execution allowed) —
  exactly the authority-boundary violation ADR-0019 forbids.
