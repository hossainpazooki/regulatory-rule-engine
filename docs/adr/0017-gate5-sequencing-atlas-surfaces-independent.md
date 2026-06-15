# 0017. Platform-api decoupled: ATLAS+COMPASS are an independent producer→consumer pair; Gate-4 C1/C2 redefined; Gate-5 proceeds

**Status:** Proposed (pending sign-off by Hossain)
**Date:** 2026-06-15
**Spec references:** § 19 (Gate 4 / Gate 5 acceptance), § 14 (consumer surface), § 16 (multi-surface access), § 6 (WASM/serve discipline)
**Amends:** `CLAUDE.md` Git discipline ("No gate may begin until the prior gate's § 19 acceptance is green") and the spec § 19 Gate-4 acceptance C1/C2, both of which assumed `institutional-defi-platform-api` as the artifact consumer.
**Supersedes the premise of:** `dev/briefs/gate-4-platform-consumption.md` and the 0.3 section of `dev/briefs/phase-0-closeout-seed.md` (both now stale — they target platform-api).

## Context

Spec § 19 Gate-4 acceptance C1/C2 were written assuming the sibling
`institutional-defi-platform-api` is the consumer that **loads, verifies, and
executes** ATLAS artifacts, with C2 asserting parity against that repo's Python
`RuleRuntime`. Phase-0 sequencing treated the platform-api "consumption PR"
(0.3) as the long pole gating Gate 5.

**Decision input (Hossain, 2026-06-15):** *"I'm not sure platform-api matters
here … the intent is to have ATLAS and COMPASS run on their own."* The
platform-api is **decoupled** from the ATLAS artifact path (its own CLAUDE.md
already scopes ke-workbench out as of 2026-05-25 — correct, not a conflict). The
0.3 platform consumption PR is **dropped**, not held.

Consequences for the acceptance criteria:

- The **real consumer is COMPASS** (`cross-border-compliance-navigator`), via the
  Gate-4 **WASM verifier** (`ke-wasm` / `@platform/atlas-artifact`). COMPASS is
  **consumer-only**: it verifies provenance + registry state in-browser. It does
  **not** execute the rule engine against a Python pipeline.
- There is therefore **no production Python pipeline** to execute-match against;
  the Python `RuleRuntime` parity that C2 named lives only in ATLAS's own Gate-3
  equivalence harness, as an internal property — not a consumer integration.

## Decision

1. **ATLAS and COMPASS are an independent producer→consumer pair.** No
   `ke-artifact` / `ke_artifact_py` code lands in platform-api. The
   platform-consumption brief is retired as a live target.

2. **Redefine Gate-4 C1/C2 (consumer-integration is no longer platform-api):**
   - **C1 — verifier delivered + 3-language-consistent (ATLAS): MET.** The
     consumer-side *integration* (in-browser verify + revoked-pack flagging) is
     **COMPASS's**, explicitly **deferred to the post-Gate-5 COMPASS rewire** —
     no longer "pending a platform PR."
   - **C2 — runtime execution-parity foundation (ATLAS Gate-3 equivalence
     harness, Rust ≡ Python `RuleRuntime`): MET.** The "a consumer executes and
     matches the production Python pipeline" half is **obsolete** — there is no
     such consumer or production Python target. ATLAS's runtime equivalence
     stands as an internal correctness property.

3. **Gate 5 proceeds on the ATLAS surfaces** (`ke-cli serve` ✅ 5a done,
   `ke-wasm` preview 5b-preview, DuckDB/export 5b-data, lint 5c). With no platform
   PR, the sequencing question is moot — nothing external gates the start. The
   **frontend rewire (5d) and review UI (5e) remain gated** behind per-page parity
   and default-off flags. The COMPASS rewire stays gated **after Gate 5 + the
   Hossain npm publish** of `@platform/atlas-artifact`.

## Consequences

- **Desirable:** the acceptance story is honest again — Gate-4 closes on ATLAS
  evidence (C1 verifier + C2 equivalence foundation + C3 + C4, all MET in-repo)
  without a cross-repo dependency that no longer exists. Gate-5 work is unblocked.
- **Undesirable / managed:** "consumer verifies a live artifact end-to-end" is now
  demonstrated only when COMPASS rewires (post-Gate-5), so the producer→consumer
  loop is proven later than originally planned. The 3-language
  `scripts/contract-test.sh` (Rust ≡ Python ≡ WASM) remains the drift gate that
  keeps the WASM verifier COMPASS will call byte-identical to canonical.

## Alternatives considered

- **Keep platform-api as the consumer (original spec):** rejected by the decoupling
  decision — it ships a binding for a consumer that does not exist.
- **Strict sequencing (wait for the platform PR):** moot — there is no platform PR.
- **Mark C1/C2 simply "MET" with no qualification:** rejected as overclaiming — the
  consumer-integration half is genuinely deferred (C1) or obsolete (C2), and the
  doc must say which.
