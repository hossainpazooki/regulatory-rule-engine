# Gate 6 (reconciled) — brief + next-session implementation seed

> **Originating question (session start):** *"You will be tasked with a plan
> requiring knowledge of ATLAS (this repo) and COMPASS. The next prompt will
> require you to research regulatory requirements for a new domain. First, tell me
> whether key implementations remain before I provide you with the new design
> spec."*
>
> **Answer, after this session's work:** the *blocking* implementations are done —
> the producer→consumer verification loop is closed and proven (Gate 5 merged, PR
> #10; COMPASS live verify wired + verified). The platform is ready for the new
> design spec. **Gate 6 is the remaining gate work** — and post-ADR-0017 it is
> mostly already-built or out-of-scope, leaving the narrow consumer-agnostic
> deliverables this plan covers. The two spec-dependent readiness conditions to
> re-check against the new domain: (1) IR/DSL expressiveness for novel rule
> constructs, (2) where new-domain rule YAML is authored now that platform-api is
> decoupled.

> **Status:** APPROVED plan, not yet implemented. This doc is both the brief and
> the seed for a fresh session to execute Gate 6. Nothing in it is built yet.
>
> **Renumbering note (2026-07-13):** this brief originally called the planned
> gate-6 scope-reconciliation ADR "ADR-0021". That number was since consumed by
> the IntentSpec track (0021 polymorphic payload, 0022 kind-aware R7 — both
> Accepted and merged; 0023 graph export, Proposed). All references below now say
> **ADR-0024** — the next free number at the time of this note; re-check the
> `docs/adr/` index when authoring.

## Next-session seed (read this first)

**Where things stand (2026-06-24):** Gate 5 is done and merged (`origin/main` @
`3c5cbf0`, PR #10); the live verifier loop is closed and verified end-to-end.
Gate 6 is the next gate. Two scope decisions are already locked (don't re-litigate):
- **Q1 = substantive build** — ship the consumer-agnostic pieces, not a paper close.
- **Q2 = accept ADR-0015 + add ADR-0024** (additive reconciliation, repo's
  established ADR pattern).

**Your job:** execute the plan below (Workstreams A–D). Suggested order: confirm
the load-bearing assumption first (the revocation **sidecar is NOT canonical-
hashed** — verify before extending it; if it is, STOP, adding `reason_class` would
illegally bump canonicalization), then **B → C → D → A**, on a gate branch
`migration/gate-6-revocation-decision`. Output git/PR commands for Hossain; don't
commit yourself.

**Reuse map (already located — don't re-derive):** `Selector::ByRegime` + effective-
date `resolve()` `crates/ke-cli/src/registry/mod.rs:329-406`; verify fail-closed
`crates/ke-artifact/src/verify.rs:318-324`; `RevocationPolicy{HardStop,
FinishPinned,AuditOnly}` `crates/ke-core/src/manifest.rs:84-92`;
`severity_for`/`parse_revocation_policy` + `RevocationRecord{policy,reason,
event_ref,severity}` `crates/ke-cli/src/commands/revoke.rs:38-105`; serve resolve
handler (hash/tag only) `crates/ke-cli/src/serve/handlers.rs:64-83`; CLI effective-
date parse `crates/ke-cli/src/cli.rs:291-299`. Regression scripts:
`scripts/lifecycle-smoke.sh`, `scripts/serve-published-registry.sh`.

**Invariant:** `verify` stays fail-closed; the revocation-decision layer only
*informs* a consumer. No Temporal code enters the repo. The decision fn is honest
groundwork (no live orchestrator consumer yet) — ADR-0024 must say so.

---

# Plan — Gate 6 (reconciled): revocation runtime-decision + registry surface completion

## Context

Spec Gate 6 = "platform production cutover": platform Temporal worker resolves
rules through the registry via a startup **pinning activity**, and the Python KE
module is removed from `institutional-defi-platform-api`. **ADR-0017 (Accepted)
decoupled platform-api** — that consumer no longer exists; COMPASS (the real
consumer) is browser **verify-only**, not a Temporal orchestrator. So Gate 6's
spec acceptance criteria **cannot be met as written**, and per repo discipline a
reconciliation ADR is required.

Two recon passes established the ground truth:
- **Already delivered** (Gate 4/5): `Selector::{ByHash,ByTag,ByRegime}` + effective-
  date `resolve()` (`crates/ke-cli/src/registry/mod.rs:329-406`), CLI `query
  --regime --effective` (`cli.rs:75-80,291-299`), rollback to Published-only
  targets (`commands/rollback.rs`), and **`verify_artifact` already rejects any
  non-Published incl. Revoked** (`crates/ke-artifact/src/verify.rs:318-324`).
- **Out of scope** (ADR-0017 + spec non-goal): platform Temporal pinning activity
  (no consumer), Python KE removal (platform-repo), and **any Rust Temporal
  worker** (never enters this repo).
- **Genuinely missing, ATLAS-buildable:** (1) revocation **runtime-decision** —
  the registry *records* policy/reason/severity but the reason-class→policy
  mapping (ADR-0009 §4 / ADR-0013) is operationalized nowhere
  (`commands/revoke.rs:10-13` defers it to "platform/Gate 6"); (2) `serve
  /resolve` doesn't accept `?regime=&effective=` though the CLI does
  (`serve/handlers.rs:64-83`).

**Decisions taken (this planning session):** (Q1) **substantive build** — ship the
consumer-agnostic pieces, not a paper close; (Q2) **accept ADR-0015 + add
ADR-0024** as an additive reconciliation (matches the ADR-0017/0019/0020 pattern).

**Outcome:** Gate 6 closes *as far as ATLAS legitimately can* — accepted scope
ADRs, a pure tested revocation-decision function operationalizing already-accepted
policy, those inputs surfaced at the resolve/verify boundary, and the HTTP resolve
surface completed — while the platform Temporal cutover is explicitly deferred
(re-openable when a real orchestrator consumer exists).

## Invariant (non-negotiable, holds in every step)

`verify` stays **fail-closed**: refuse any non-Published artifact (incl. Revoked)
even with valid crypto (`verify.rs:318-324`). The revocation-decision work is a
**separate orchestration layer** that *informs* a consumer; it never loosens
verify. Nothing here signs/attests/publishes from a new surface, and no Temporal
code enters the repo.

## Workstream A — Governance / ADRs / doc reconciliation

- **Accept ADR-0015** (`docs/adr/0015-temporal-orchestration-ownership.md`): flip
  Status Proposed→Accepted, dated. Its three-channel model + no-Rust-Temporal-
  worker non-goal still hold and govern the delivered Gate-4/5 channels.
- **New `docs/adr/0024-gate6-scope-reconciliation.md`** (additive, cites the
  ADR-0020 "Amends § 19" precedent). Records: spec Gate 6 scope; what's delivered
  (pinning/rollback + verify-layer Revoked rejection); channel-3 platform pinning
  activity **deferred — no consumer (ADR-0017)**; Python KE removal + Rust Temporal
  worker **out of scope**; the substantive ATLAS deliverables below; the verify
  fail-closed invariant; and the **re-scoped ATLAS Gate-6 acceptance** (see
  Verification).
- **Update the open-decisions tables**: `CLAUDE.md` "Revocation behavior | Gate 6"
  row → consumer-agnostic decision delivered, orchestration enforcement deferred to
  a real consumer (ref ADR-0024). Mirror in spec § 21 via the ADR "Amends" line
  (do not rewrite spec prose — amend via ADR, matching ADR-0020).

## Workstream B — Revocation runtime-decision (pure, consumer-agnostic core)

Home: **`ke-core`** (where `RevocationPolicy` lives, `manifest.rs:84-92`). Add a
small `revocation` module reusing the existing `RevocationPolicy`:
- `enum RevocationReasonClass { KeyCompromise, LegalInvalidity, RoutineSupersession,
  Advisory }` (ADR-0009 §4 table).
- `fn revocation_floor(reason) -> RevocationPolicy`: KeyCompromise/LegalInvalidity
  → HardStop; RoutineSupersession → FinishPinned; Advisory → AuditOnly.
- strictness rank `HardStop > FinishPinned > AuditOnly`; `fn revocation_decision(
  reason, configured: Option<RevocationPolicy>) -> RevocationPolicy` =
  **stricter-of(floor, configured)** — implements ADR-0009's "env policy may only
  raise strictness above the floor, never lower." Pure, no deps.

Wire into **`commands/revoke.rs`** (extend, keep back-compat):
- Add optional `--reason-class`. When present, recorded policy =
  `revocation_decision(reason_class, --policy?)`; reject if `--policy` would lower
  below the floor. Legacy `--policy`-only path unchanged (reason_class=None) so
  `scripts/lifecycle-smoke.sh` (`revoke --policy auditonly`) still passes.
- Extend the sidecar `RevocationRecord` (registry struct; reuse
  `severity_for`/`parse_revocation_policy`) with `reason_class:
  Option<RevocationReasonClass>`. The sidecar is **not** canonical-hashed (separate
  from the frozen event, ADR-0012) → **no canonicalization bump**.

## Workstream C — Surface inputs + complete the resolve surface

- **serve `/resolve` ByRegime+effective**: extend `serve/handlers.rs::resolve` to
  parse `?regime=&effective=YYYY-MM-DD&env=` → `Selector::ByRegime`, reusing the
  CLI's effective-date parse (`cli.rs` `query_selector`, ~291-299) and the existing
  `registry::resolve`. Pure read; non-authoritative.
- **Surface revocation at the boundary**: when an artifact's current state is
  Revoked, include an optional `revocation: { reason_class, policy }` block in the
  resolve `ResolutionRecord` and the `/verify` `VerifyResponse`, read via the
  existing `backend` revocation getter. Gives a consumer the inputs to apply
  `revocation_decision` — without changing verify's reject verdict.

## Workstream D — Tests + docs

- **ke-core unit tests**: the reason-class→policy matrix, floor-only-raises (e.g.
  Advisory + configured HardStop ⇒ HardStop; KeyCompromise + configured AuditOnly
  ⇒ HardStop), strictness ordering.
- **revoke tests**: `--reason-class` records reason_class + derived policy;
  `--policy` below floor rejected; legacy `--policy` path intact.
- **serve tests**: `/resolve?regime=&effective=` resolves the expected hash; a
  Revoked artifact's resolve/verify response carries the `revocation` block;
  unknown regime → 404/ambiguous as today.
- **Docs**: update `docs/consumer-serve-contract.md` (new resolve params +
  revocation block); new `docs/gate-6-implementation-log.md` with evidence.

## Files

- New: `docs/adr/0024-gate6-scope-reconciliation.md`,
  `crates/ke-core/src/revocation.rs` (or extend `manifest.rs`),
  `docs/gate-6-implementation-log.md`
- Edit: `docs/adr/0015-temporal-orchestration-ownership.md` (Accept),
  `crates/ke-cli/src/commands/revoke.rs`, the `RevocationRecord` struct +
  `ResolutionRecord` (`crates/ke-cli/src/registry/`), `crates/ke-cli/src/serve/
  {handlers.rs,dto.rs}`, `CLAUDE.md` (open-decisions row),
  `docs/consumer-serve-contract.md`, `crates/ke-core/src/lib.rs` (module export)
- Reuse (no change): `registry::resolve`/`Selector::ByRegime`
  (`registry/mod.rs:329-406`), `verify_artifact` (`verify.rs`),
  `RevocationPolicy` (`manifest.rs:84-92`), `severity_for`/`parse_revocation_policy`
  (`revoke.rs:39-57`), `scripts/lifecycle-smoke.sh`,
  `scripts/serve-published-registry.sh`

## Verification (re-scoped Gate-6 acceptance, recorded in ADR-0024)

Gate 6 (ATLAS) closes when:
1. ADR-0015 Accepted; ADR-0024 Accepted.
2. `revocation_decision` implemented + unit-tested (matrix + floor-only-raises).
3. `revoke --reason-class` records reason_class; `--policy` cannot lower below
   floor; legacy path intact (`lifecycle-smoke.sh` green).
4. Revoked artifact's `reason_class`+policy surfaced at serve `/resolve` and
   `/verify`; `serve /resolve` accepts `?regime=&effective=&env=` (tested).
5. `verify` remains fail-closed — regression: `cargo test --workspace --features
   test-keys` green; a live `serve-published-registry.sh` check still rejects the
   non-Published hash.
6. `CLAUDE.md`/§21 revocation row updated.
Deferred + recorded (not blockers): platform Temporal pinning activity, Python KE
removal, Rust Temporal worker.

End-to-end smoke: seed → publish → revoke `--reason-class key_compromise` →
`/resolve` shows `revocation{reason_class:KeyCompromise, policy:HardStop}`,
`/verify` rejects (NotPublished); `revoke --reason-class advisory` → policy
AuditOnly surfaced, verify still rejects (fail-closed invariant holds).

## Handoff (per repo git discipline)

Gate work on branch `migration/gate-6-revocation-decision`; commit + push + `gh pr
create --base main` + merge commands **output for Hossain**, not run here. One
gate-scope change → one branch → one PR.

## Risks / honesty

- The revocation-decision fn is **groundwork**: no live orchestrator consumer runs
  HardStop/FinishPinned/AuditOnly yet (COMPASS is verify-only). It operationalizes
  *already-accepted* policy and is fully tested, but ships ahead of its enforcer —
  ADR-0024 states this plainly.
- If implementation reveals the sidecar IS hashed somewhere, stop — adding
  `reason_class` would then bump canonicalization (it must not); re-confirm the
  sidecar is outside the canonical envelope before extending it.
