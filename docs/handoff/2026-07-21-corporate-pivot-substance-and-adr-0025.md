# Handoff — Gate-6 closure aftermath, ADR-0025 draft, and the corporate-pivot discussions (substance, not decisions)

2026-07-21. Newest commits this brief describes: `origin/main` @ `53501bd`
(PR #18, 0024 acceptance stamp), branch `docs/adr-0024-acceptance-stamp` @
`151036a` (PR #19, OPEN — ADR-0025 draft). Pick-up measures drift from those
two refs.

> **Framing instruction from the operator (2026-07-21, binding on this
> brief):** the strategy material below records the *substance of
> discussions* — layers examined, corporate-context evaluation, development
> paths considered — **without recording strategic decisions**. Nothing in
> § "Discussion substance" is locked; it exists to inform future build
> sessions, not to bind them. Only § "Locked decisions" is locked, and it
> contains repo-record facts only.

## Current state

- **built** — Gate 6 (reconciled): revocation runtime-decision
  (`ke_core::revocation`), `revoke --reason-class` (floor never lowerable),
  serve `/resolve?regime=&effective=`, revocation block on
  `ResolutionRecord`+`VerifyResponse`. Merged PR #17 (`7f2c99a`); ADR-0024 +
  ADR-0015 Accepted by that merge; acceptance stamped on main via PR #18
  (`53501bd`).
  re-verify: `git log --oneline -3 origin/main` shows #18 and #17; `sed -n
  3p docs/adr/0024-gate6-scope-reconciliation.md` reads "Accepted — merged
  to main 2026-07-19 (PR #17…)"; `cargo test --workspace --features
  test-keys` (207/0 at merge; evidence `docs/gate-6-implementation-log.md`).
- **in-progress** — ADR-0025 "Production key authority" drafted as
  **Proposed**, sitting in **OPEN PR #19** with its ADR-index entry. Merging
  #19 lands the *draft*; acceptance is a separate operator sign-off.
  re-verify: `gh pr view 19 --json state,title`; `sed -n 3p
  docs/adr/0025-production-key-authority.md` reads "Proposed".
- **built** — regulation research run (2026-07-20, adversarially verified,
  16 confirmed / 9 refuted claims): survivors and kills summarized in
  § "Discussion substance" below and in `docs/learnings/` (the raw workflow
  output was session-scratchpad and is gone; the durable record is here and
  in the learnings entries).
- **planned / not started** — everything in the pivot discussions: authoring
  plane, production-key implementation, any new rule-pack ingest. No code,
  no repo, no schema change exists for any of it.
- Housekeeping: stale remote branch `migration/gate-6-revocation-decision`
  still exists carrying the stranded stamp commit `1d556b8` (see learnings);
  local `adr/0023-graph-export` branch also still deletable. Operator's
  call.

## Locked decisions (repo record only)

- **ADR-0024 + ADR-0015 Accepted** — acceptance criterion was the PR #17
  merge itself (ADR-0023 precedent); reason: gates close via PR on the
  remote, never a local pointer. Stamped on main by #18.
- **Gate 6 is closed as re-scoped by ADR-0024**; the spec's platform
  Temporal cutover is *deferred with named re-open conditions*, not
  abandoned — reason: the consumer those criteria assumed was decoupled by
  ADR-0017.
- **`verify` stays fail-closed; the revocation-decision layer informs and
  never loosens** — ADR-0024's invariant, load-bearing for everything
  discussed since.
- Explicitly **NOT locked**: every strategy item below — pivot framing,
  plane scoping, regulation choices, ADR-0025's content (Proposed, unsigned).

## Discussion substance (2026-07-20 → 21, recorded to inform, not bind)

**Treasury framings examined.** Bank treasury (own-balance-sheet: liquidity,
ALM, funding, FTP) vs corporate treasury (cash as fuel: DoA matrices,
maker-checker, duplicate prevention) vs this system — which maps
one-to-one onto corporate-treasury *payment controls* (criteria ↔ approval
matrix; emit-and-observe ↔ dual control; idempotency-at-dispatch ↔
duplicate-payment prevention; volatile re-check ↔ pre-release
re-validation). Corollary lens that explained the research results: the
control model is corporate-shaped while most financial regulation binds
*institutions*, so institution-level obligations (Basel capital ratios)
misfit while transaction-level ones (AMLR thresholds) fit.

**Generalization paths considered** (none chosen): (a) payments as first
action class — strong pain fit, agentic-commerce tailwind (AP2/mandate
convergence), but payments *regulation* binds PSPs not corporates; (b)
financial pre-trade/mandate compliance — most IntentSpec-shaped domain
found, but re-verticalizes into finserv against entrenched OMS incumbents;
(c) cross-border/export-controls + sanctions — corporate-general,
rule-shaped, and what COMPASS's name already means; (d) the wedge framing
"delegation of authority for AI agents" with internal policy (DoA/treasury
docs) as the first attested corpus. A working scope narrowing to **intent
plane + authoring plane** (COMPASS settlement out of surface, ATLAS as
substrate) was explored in depth — brief at
`~/dev/briefs/2026-07-21-intent-authoring-planes-scope.md` — including four
load-bearing observations: extraction target narrows RuleIR→IntentSpec;
`Unevaluable`-never-passes gives fail-closed composition (authoring bugs can
only close gates); human judgment can be an approval-checking *evaluator*
(no canon bump); stable|volatile is an authoring output with asymmetric
misclassification risk (default-volatile argued).

**Authoring-plane concept** (proposed, unbuilt): proposer-only plane
(extract → refute-against-source → human attests), coverage and abstention
as first-class outputs, span anchors mandatory, structured internal docs
first / free-form regulation last. A buyer-facing prose rendering of this
correctness model was written in-session (transcript
`7f20dfba…`, "as if I'm a corporate buyer" turn) — liftable for marketing/
sales material.

**Regulation research survivors** (adversarial, 3-vote): AMLR (EU
2024/1624, applies 2027-07-10) has the most gate-shaped obligations — Art 19
CDD thresholds (EUR 10k/1k/1k/3k/2k, all aggregating *linked* transactions —
direct implication for idempotency-key/window design), Art 80 cash cap (EUR
10k, jurisdiction-parameterized downward), Art 79 categorical anonymity
prohibitions (boolean eligibility). IPR (EU 2024/886): the codifiable
obligation is at-least-daily customer sanctions screening (a volatile
criterion with freshness bound); **VoP is NOT a gate** (refuted 0-3 three
ways — see learnings). GENIUS effective date ≤ 2027-01-18 (refresh of
existing corpus, cheap); FATF R.16 lands end-2030 (ruled out for nearness);
Basel SCO60 pre-dates the window AND its deliberately-numberless basis-risk
test is the canonical non-codifiable obligation. Refuted and must not be
reused: GENIUS "$10B state-licensing threshold" (0-3).

**Engineering gaps named against "the engineering is done":** (1)
aggregate/windowed state for velocity & linked-transaction rules (gate reads
params only, by design); (2) explicit non-codifiable/human-judgment routing;
(3) production key authority (ADR-0025 drafts the closure); (4)
IR-expressiveness — mostly dissolved for IntentSpec-only scope, returns if
the rules corpus ingests new domains.

**ADR numbering options discussed** for next work: 0025 = key authority
(drafted); authoring-plane charter (mirror of 0019, could absorb spec § 21
"legal source text storage"); generalization-reframing ADR (or fold into the
charter's context). Numbering caution: don't pre-assign numbers in briefs
(the Gate-6 brief lost "0021" that way).

## Reuse map

- `docs/gate-6-implementation-log.md` — Gate-6 evidence, format precedent.
- `docs/adr/0025-production-key-authority.md` (PR #19) — the key-authority
  closure draft; `docs/adr/0009-…md` line 29 scopes what it may decide.
- `ke graph export` oracle-exposure query (ADR-0023) — reused by 0025's
  compromise exposure report; do not rebuild.
- `../treasury-intent-controller/CONTRACT.md` — the intent-plane invariants
  (tri-state, stable|volatile, idempotency, single-ACHIEVED); the ground
  truth that corrected the asset-tag misreading.
- `docs/SYSTEM.md` — three-plane map; `~/dev/briefs/2026-07-21-…scope.md` —
  the explored two-plane scope.
- `docs/learnings/` (new this session) — four entries with re-verify lines.

## Invariants

- Repo git discipline: no commits/pushes by the assistant; gates and ADRs
  land via PR on the remote; **never commit follow-ups to an
  already-squash-merged branch** (learnings entry; that's how the stamp
  stranded).
- Authority boundaries (CLAUDE.md/spec § 5, § 10, § 13): AI proposes, never
  attests/publishes/revokes; any future authoring plane inherits
  proposer-only.
- `verify` fail-closed incl. Revoked/Unknown; `Unevaluable` never collapses
  to pass (tic invariant 2) — the pair that makes the two-plane composition
  safe.
- Canonical encoding untouched without an ADR + canon bump (ADR-0021
  precedent); ed25519 pinned (0025 draft: custody adapts to scheme, never
  the reverse).
- Status verbs stay honest: 0025 is *Proposed*; merging PR #19 lands a
  draft, not an acceptance.

## Open / next

1. **PR #19** (0025 draft) — operator review/merge; then, separately, the
   0025 sign-off decision itself (its § "Acceptance criteria" lists the
   follow-through edits deliberately left undone: 0009 acceptance-note
   pointer, spec § 21.1 row, CLAUDE.md table).
2. First build candidate *if* 0025 is accepted: the one code deliverable —
   non-local policy rejects `is_test_key` (R8 mock-TSA pattern; verify-layer
   rule + tests).
3. Strategy: no committed next step by design. The recorded paths above are
   inputs to whatever session Hossain opens on the pivot.
4. Housekeeping when convenient: delete stale
   `origin/migration/gate-6-revocation-decision` (carries stranded
   `1d556b8`) and local `adr/0023-graph-export`.
