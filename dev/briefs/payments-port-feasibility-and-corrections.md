# Payments port — feasibility pressure-test + engineering corrections

> **What this is.** A feasibility assessment of the §6 "payments portability"
> claim in `money-decisioning-loop-architecture-brief.md`, pressure-tested against
> the *actual* ATLAS IR (not the prose). Verdict: the "content swap, not an
> architecture change" claim is **partially true** — and the part that is false is
> exactly the load-bearing AML logic. Includes Hossain's authoritative domain
> corrections to the ACH pack. Source brief authored 2026-06-21; this assessment
> 2026-06-24. Nothing here is built.

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
| SAR — threshold arm ($5,000) | threshold + *suspicion* | ⚠️ threshold native; suspicion → pre-computed fact |
| CTR — aggregation arm (same-person, same-day total > $10,000) | **time-window aggregation** | ❌ not expressible |
| AML structuring / velocity | **multi-event aggregation** | ❌ not expressible |
| OFAC / sanctions (counterparty ∈ SDN list, fuzzy) | **external-list membership** | ❌ not expressible |

\* subject to the ACH corrections below.

**Verified thresholds (primary sources):** CTR **$10,000**, and it explicitly
requires same-business-day aggregation of multiple transactions by/on behalf of
the same person ([FinCEN CTR FAQ](https://www.fincen.gov/resources/frequently-asked-questions-regarding-fincen-currency-transaction-report-ctr)).
Travel rule **$3,000** — name/address/amount/date must travel
([31 CFR 1010.410](https://www.ecfr.gov/current/title-31/subtitle-B/chapter-X/part-1010/subpart-D/section-1010.410)),
with a **proposed** lowering of the *cross-border* threshold to **$250** still in
rulemaking ([Federal Register, 2020](https://www.federalregister.gov/documents/2020/10/27/2020-23756/threshold-for-the-requirement-to-collect-retain-and-transmit-information-on-funds-transfers-and)).

## Two honest corrections to "identical tier stack"

1. **The DSL gap is the AML core.** Sanctions screening (external-list
   membership) and velocity/structuring (time-window aggregation) — the most
   examinable parts of payments compliance — are precisely what the
   per-transaction engine can't express. The corpus's documented workaround
   (`docs/dsl-gap-review-gate-2.md`) is to push them into **pre-computed facts**
   (`ofac_hit`, `aggregate_24h_amount`). That preserves the "content swap" — but
   those computations then live **outside** the signed, T0–T4-verified artifact.
   The brief's "the rule is verified" guarantee then covers the *decision* but not
   the *OFAC match* or the *velocity sum*: the auditor gets "rule fired because
   `ofac_hit=true`" with the screening algorithm outside the replayable trace. A
   real trust-boundary narrowing, not a detail.

2. **T3 has no oracle for payments.** The equivalence tier (Rust↔Python on
   fixtures) uses the platform-api Python `RuleRuntime` as its reference — and
   platform-api is decoupled (ADR-0017) and implements no payments rules. A
   payments artifact can clear T0/T1/T2/T4 + attestation but **cannot clear T3 as
   wired** without a new Python reference oracle for the payments domain. "Identical
   tier stack" isn't quite true until that's resolved.

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
capability. ACH return *timeliness* (`return_date − settlement_date ≤ window`) is
natively expressible once that lookup table is correct.

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
2. **Authoring path + T3 oracle.** `fixtures/rules/` is read-only/synced from the
   decoupled platform. A new payments corpus needs a home + provenance model, and a
   decision on the T3 oracle (no Python reference exists for payments). ADR-worthy.

## Recommended next step (hybrid)

Author the **travel-rule pack** end-to-end (cleanest native fit — `Exists`-based
data-completeness, no gaps, $3,000 threshold) to prove the swap through the real
T0–T2/T4 + attestation + registry + sign + verify pipeline; draft the
**IR-extension ADR** for OFAC list-membership + windowed aggregation; and fold the
two ACH corrections into the ACH pack (R29 in its own corporate short-window class;
`2020-04-01` semantic effective date). Settle the authoring-path/T3-oracle ADR
before the first payments artifact is published.
