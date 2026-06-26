# Payments port — feasibility pressure-test + engineering corrections

> **What this is.** A feasibility assessment of the §6 "payments portability"
> claim in `money-decisioning-loop-architecture-brief.md`, pressure-tested against
> the *actual* ATLAS IR (not the prose). Verdict: the "content swap, not an
> architecture change" claim is **partially true** — and the part that is false is
> exactly the load-bearing AML logic. Includes Hossain's authoritative domain
> corrections to the ACH pack. Source brief authored 2026-06-21; this assessment
> 2026-06-24. Nothing here is built.
>
> **Revised 2026-06-26** after a verification pass run with **`rigor`** — the
> portable verification-discipline plugin (designed, not yet built), exercised here
> by hand as a production battle-test of its `refute` spine. Part of this brief's
> purpose is to be that test: a real, high-stakes document for rigor to break.
> Five corrections folded in: the §6 verdict and the ACH domain corrections
> survived; one mis-citation, one tier mislabel, two regulatory field/threshold
> overstatements, and one undercounted gap were fixed. The pass also surfaced two
> design gaps in rigor itself. Changes are marked inline; see **Verification pass**
> at the end.

## Verdict

The ATLAS engine is a **stateless, per-transaction decision-tree evaluator**:
operators `{Eq,NotEq,In,NotIn,Gt,Lt,Gte,Lte,Exists}`, exact decimals, one flat
fact-set in → one verdict out (`crates/ke-core/src/ir/condition.rs`,
`crates/ke-runtime/src/exec.rs`; evaluation is stateless per-transaction per
ADR-0008). Against that reality the §6 packs split cleanly:

| §6 pack | Core computational shape | Native DSL fit |
|---|---|---|
| Travel rule (`amount ≥ $3,000` ⇒ originator name/addr present) | data-completeness (`Exists`) | ✅ native |
| Money-transmitter licensing (state ∈ license-required set) | jurisdiction matrix → inline `In` | ✅ via inline-list |
| Nacha ACH / card disputes (return/reason code + time limit) | code mapping + per-tx date compare | ✅ mostly native* |
| CTR — single-tx arm (one cash tx > $10,000) | single threshold | ✅ native |
| SAR — threshold arm ($5,000 banks/casinos; **$2,000 MSBs**) | threshold + *suspicion* | ⚠️ threshold native; suspicion → pre-computed fact |
| CTR — aggregation arm (same-person, same-day total > $10,000) | **time-window aggregation** | ❌ not expressible |
| AML structuring / velocity | **multi-event aggregation** | ❌ not expressible |
| OFAC / sanctions (counterparty ∈ SDN list, fuzzy) | **external-list membership** | ❌ not expressible |

\* subject to the ACH corrections below.

**Verified thresholds (primary sources):** CTR **$10,000**, and it explicitly
requires same-business-day aggregation of multiple transactions by/on behalf of
the same person ([FinCEN CTR FAQ](https://www.fincen.gov/resources/frequently-asked-questions-regarding-fincen-currency-transaction-report-ctr)).
Travel rule **$3,000** — and the transmittal-order information that must travel is
**more than name/address/amount/date**: per [31 CFR 1010.410(f)](https://www.ecfr.gov/current/title-31/subtitle-B/chapter-X/part-1010/subpart-D/section-1010.410)
it also includes the transmittor's **account number** (if ordered from an account),
the **recipient's name, address, account number and any other identifier**, and the
**identity of the recipient's (and, to the extent received, the transmittor's) financial
institution**. The data-completeness `Exists` shape still fits — but the field set the
pack must check is the full (f) list, not four fields. The **proposed** lowering of the
*cross-border* threshold to **$250** is still in rulemaking as of 2026-06-26 (verified:
no successor/final rule under docket FINCEN-2020-0002)
([Federal Register, 2020](https://www.federalregister.gov/documents/2020/10/27/2020-23756/threshold-for-the-requirement-to-collect-retain-and-transmit-information-on-funds-transfers-and)).

## Two honest corrections to "identical tier stack"

1. **The DSL gap is the AML core.** Sanctions screening (external-list
   membership) and velocity/structuring (time-window aggregation) — the most
   examinable parts of payments compliance — are precisely what the
   per-transaction engine can't express. The corpus already has a precedent for
   externalizing what the DSL can't derive: `docs/dsl-gap-review-gate-2.md` pushes
   *discretionary/standards-based* provisions into **pre-computed boolean facts**
   (`whitepaper_compliant`, `issuer_authorized`, `fit_and_proper`). One *could*
   extend that same pattern to OFAC/velocity via facts like `ofac_hit` /
   `aggregate_24h_amount` — but note two things that doc does **not** say: it does
   not itself cover sanctions or aggregation, and it explicitly flags the
   velocity/aggregation class ("temporal sequencing / quant over collections") as
   a case that should trigger an **IR-extension ADR**, not a pre-computed-fact
   workaround. So pre-computing them preserves the "content swap" — but those
   computations then live **outside** the signed, T0–T4-verified artifact, against
   the corpus's own stated guidance for that class.
   The brief's "the rule is verified" guarantee then covers the *decision* but not
   the *OFAC match* or the *velocity sum*: the auditor gets "rule fired because
   `ofac_hit=true`" with the screening algorithm outside the replayable trace. A
   real trust-boundary narrowing, not a detail.

2. **The execution-equivalence harness has no oracle for payments.** The Rust↔Python
   runtime-equivalence proof (`scripts/equivalence-harness.sh`, ADR-0008 —
   *distinct from* the T0–T4 verification tiers; spec §11 T3 is NLI source-span
   consistency, not this harness) uses the platform-api Python `RuleRuntime` as its
   reference — and platform-api is decoupled (ADR-0017) and implements no payments
   rules. A payments artifact can clear T0–T4 + attestation but **cannot clear the
   execution-equivalence harness as wired** without a new Python reference oracle
   for the payments domain. "Identical tier stack" isn't quite true until that's
   resolved.

## Domain corrections (Hossain — authoritative; fold into the ACH pack)

These correct a research-layer proposal before it could be encoded — the
"AI proposes, only a domain expert attests" boundary on a live example.

1. **R29 ≠ 60-day/WSUD.** R29 ("Corporate Customer Advises Not Authorized") is the
   **corporate** unauthorized-return code and must **not** be grouped with the
   consumer 60-day / WSUD bucket (R10/R11). Corporate unauthorized returns run on
   the **2-banking-day** deadline, not 60 calendar days — grouping R29 into the
   60-day class produces wrong timeliness verdicts on corporate returns. R29 gets
   its own corporate short-window timeliness class.
2. **R10/R11 semantic split = 2020-04-01, not 2021.** The effective date for the
   *semantic* differentiation (R10 redefined to "customer advises not authorized";
   R11 created for "not in accordance with terms / improper") is **April 1, 2020**.
   **2021 is fee-applicability only** and must not be used as the semantic
   effective date in the engine's effective-date table.

Both corrections are about getting the **window-class mapping** (R29 → 2 days;
R10/R11 → 60 days) and the **effective-date table** right — not about engine
capability. One caveat on "natively expressible": the engine compares a field
against a constant and has **no date-arithmetic operator**, so
`return_date − settlement_date ≤ window` is not literally native — it needs a
pre-computed `days_since_settlement` (or derived `deadline_date`) fact. That is the
same pre-computed-fact dependency flagged for OFAC/velocity above, but a benign one:
the elapsed-days computation is mechanical and auditable, not a screening algorithm
or a cross-event aggregation. Once that fact and a correct window-class lookup table
are in place, ACH timeliness encodes cleanly as a single `<=` comparison.

## Decisions (open — for Hossain)

1. **The two real gaps (OFAC list + velocity aggregation):**
   - (a) **Pre-computed facts** — swap now; document the narrowed guarantee.
   - (b) **IR extension** — add versioned/hashed `ScalarValue::ListReference` + a
     windowed-aggregation operator + `evaluate(rule, facts, context)`, bringing
     screening/aggregation *under* the signed umbrella. Gate-scoped: canonicalization
     bump (→ ke-canon-5) → cross-language parity re-proof → WASM rebuild.
   - (c) **Hybrid (recommended)** — content-swap the natively-expressible packs now
     to prove the port; open an IR-extension ADR for the two gaps, where the real
     payments-AML value *and* the verification guarantee live. Don't fake them with
     pre-computed facts and call it "verified."
2. **Authoring path + equivalence-harness oracle.** `fixtures/rules/` is read-only/synced from the
   decoupled platform. A new payments corpus needs a home + provenance model, and a
   decision on the equivalence-harness oracle (no Python reference exists for
   payments). ADR-worthy.

## Recommended next step (hybrid)

Author the **travel-rule pack** end-to-end (cleanest native fit — `Exists`-based
data-completeness, no gaps, $3,000 threshold) to prove the swap through the real
T0–T2/T4 + attestation + registry + sign + verify pipeline; draft the
**IR-extension ADR** for OFAC list-membership + windowed aggregation; and fold the
two ACH corrections into the ACH pack (R29 in its own corporate short-window class;
`2020-04-01` semantic effective date). Settle the authoring-path/equivalence-harness-oracle
ADR before the first payments artifact is published.

## Verification pass (2026-06-26) — rigor battle-test

This pass had a **dual purpose**: verify the brief, *and* exercise **`rigor`** — the
portable verification-discipline plugin (designed + Phase-1-planned, **not yet
built**) — against a real, high-stakes document instead of a toy fixture. So rigor's
core method was run **by hand** here, as a production dry-run of the spine before it
ships as code; the findings below are therefore evidence about *both* the brief and
rigor's own design.

rigor's `refute` spine is three moves, all applied: **recompute from the primary
source** (FinCEN/eCFR/Federal Register for regulatory figures; Nacha-via-Federal-Reserve
for ACH), **re-execute the real gate** (re-read the actual IR source — `condition.rs`,
`exec.rs`, the dsl-gap doc, the spec §11 tier table — not the brief's prose), and
**dispatch independent skeptics** (three `skeptic-verifier` agents owning disjoint
claim-sets: IR/code, BSA/AML thresholds, Nacha ACH) — plus an independent re-read by
the orchestrating session. The §6 verdict and the ACH domain corrections (R5/R6)
survived intact. Five issues were surfaced and corrected (all marked inline above).

| # | Claim | Verdict | Correction folded in |
|---|---|---|---|
| C1 | Operator set = the 9 listed | ✅ survives | — (AND/OR are a separate `ConditionGroupSpec`, not operators) |
| C2 | Stateless per-tx evaluator | ✅ survives | — (`evaluate(&RuleIR,&Facts)->Evaluation`, total; ADR-0008 *paraphrased*, not quoted) |
| C3 | No aggregation / no external-list operator | ✅ survives | — (`In`/`NotIn` are inline lists; no window/sum/`ListReference` in the IR) |
| C4 | `evaluate(rule, facts)` has no `context` param | ✅ survives | — (3-arg form is future-only) |
| — | Exact decimals, no floats | ✅ survives | — (`Decimal{mantissa:i128,scale:i8}`, ADR-0003) |
| R3 | Proposed $250 cross-border still pending | ✅ survives | strengthened (no successor rule under docket FINCEN-2020-0002 as of today) |
| R5 | R29 = corporate, 2 banking days, not 60-day/WSUD | ✅ survives | — (Fed-restated Nacha) |
| R6 | R10/R11 split 2020-04-01; 2021 = fee-only | ✅ survives | — (Fed + Nacha confirm two-phase) |
| R1 | CTR $10,000 + same-day aggregation | ⚠️ survives w/ precision | trigger is *more than* $10k; aggregation conditioned on institution *knowledge* |
| R2 | Travel-rule fields = name/addr/amount/date | ⚠️ partial | **field list completed** to the full 31 CFR 1010.410(f) set |
| R4 | SAR threshold $5,000 | ⚠️ partial | **qualified**: $5k banks/casinos, **$2k MSBs** |
| C6 | "T3 has no oracle for payments" | ⚠️ partial | **tier relabeled**: it's the execution-equivalence harness (ADR-0008), not T3 (spec §11 T3 = NLI) |
| C5 | dsl-gap doc documents `ofac_hit`/`aggregate_24h_amount` | ❌ refuted | **mis-citation removed**: those names appear only here; the doc's pattern is for discretionary standards and flags aggregation as IR-extension territory |
| — | ACH timeliness "✅ mostly native" | ❌ undercounted | **caveat added**: no date operator → needs a pre-computed `days_since_settlement` fact |

**Sourcing caveats (flagged by the verifiers, not papered over):** the FinCEN CTR
FAQ, the primary Nacha rule pages, and eCFR/federalregister.gov HTML all blocked
automated fetch (timeouts / 403 / redirect-to-unblock). The figures above therefore
rest on stronger or equivalent substitutes — the underlying CFR via govinfo.gov (GPO
XML), the Federal Register JSON API, and the Federal Reserve's verbatim restatement
of the Nacha R10/R11/R29 rules — rather than the originally-cited FAQ/Nacha HTML. The
substantive claims are confirmed in primary law; the *exact wording* of the FinCEN
FAQ and the paywalled Nacha pages remains unverified.

### What this battle-test told us about rigor

How each finding actually surfaced is the useful part — it maps to which spine move
caught it, and exposes two gaps in rigor's current design:

- **The two regulatory overstatements (R2, R4) were caught by *recompute-from-source*.**
  The skeptics pulled 31 CFR 1010.410(f) and 1022.320 directly and saw the field list
  was longer than four items and that MSBs sit at $2,000 — neither is visible if you
  trust the brief's restatement. Working as designed.
- **The worst defect (C5, the fabricated citation) was caught by *re-execute-the-real-gate*.**
  Reading `docs/dsl-gap-review-gate-2.md` and `grep`-ing the repo showed the fact names
  `ofac_hit` / `aggregate_24h_amount` appear **only in this brief** — the cited doc never
  contains them. **This exposed a gap in rigor's spine:** that miss is not a wrong number
  (`refute`'s recompute move) and not an implemented-vs-planned slip (`implemented-vs-planned`),
  so *no current rigor move explicitly owns it.* The fix: rigor needs a **citation-fidelity
  move** — "every named identifier/quote in a claim must `grep`-hit its cited source or it's
  flagged" — cheap and mechanical, a natural extension of the executable `surface-scrub` gate.
- **The tier mislabel (C6) was caught by *re-execute-the-real-gate*** — reading the spec §11
  table showed T3 = NLI, so the Rust↔Python harness was mis-named. A prose-only review misses
  this; reading the authoritative table doesn't.
- **The undercounted ACH gap was caught by the *orchestrator's own re-read*, which all three
  skeptics missed.** Reading `ir/time.rs` showed `EffectiveWindow` is the *rule's* legal window,
  not per-tx date math, so `return_date − settlement_date` has no native operator. **Second
  design lesson:** orchestrator self-re-execution is load-bearing, not optional — a pure-delegation
  harness would have shipped this gap. rigor should make the "re-run ≥1 load-bearing check
  yourself" step explicit, not implicit.
- **Positive signal worth keeping:** the disjoint-fan-out + structured-verdict shape held, and
  each skeptic **self-reported its sourcing degradation** (the Nacha/FinCEN/eCFR fetch blocks)
  unprompted — the honesty discipline fired without being asked. That earns `skeptic-verifier`
  its spine-first slot in rigor v1.

Both design gaps (citation-fidelity move; explicit orchestrator self-re-execution) are recorded
for the rigor build; this brief is their first production datapoint.
