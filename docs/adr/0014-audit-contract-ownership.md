# 0014. Audit/observability contract (§18) ownership and pre-freeze field model

**Status:** Proposed
**Date:** 2026-06-11
**Spec references:** § 18 (audit event fields + reconstruction path), § 15 (revocation), § 17 (interpretation notes), § 19 (Gate 4 + Gate 6 acceptance), § 8 (artifact contract), § 10 (attestations), § 11 (ConsistencyBlock)

## Context

Spec § 18 defines the observability/audit contract: ~16 runtime audit-event fields and a reconstruction path (`workflow_id -> artifact_hash -> bytes -> rule trace -> source spans -> attestations -> verification evidence -> final decision`). "Audit reconstruction returns complete evidence" is a **Gate 6 acceptance criterion** (§ 19), but **no gate currently owns authoring the contract itself**. The Gate-4 brief does not list § 18 in scope.

This is a freeze-ordering hazard. Most § 18 fields are *static artifact-resolution facts* (versions, rule IDs, attestation IDs, source spans, T-tier status) that the artifact and its attestations already carry or must carry. Gate 4 freezes the artifact + attestation + ConsistencyBlock schema and signs golden vectors. If the § 18 field shape is first invented at Gate 6 — after signatures exist — any static field that must be bound into the artifact or covered by an attestation is retrofitted **post-freeze**, which bumps the canonical version triplet and **forces re-attestation of every artifact**. The brief's own rule ("any Gate-4 shape change bumps the version triplet + regenerates golden vectors atomically") makes a late addition maximally expensive.

A clean structurally-verified artifact is currently unreachable because the T4 `contradictory_outcome` detector raises 52 Blocking conflicts on the clean 34-rule corpus (`paths_consistent()` treats paths sharing zero conditions as a shared scenario — vacuous truth). That blocker is tracked separately; this ADR settles the audit-contract *shape* so it is frozen-ready once the artifact path opens, and can be drafted in parallel.

## Decision

Split § 18 ownership by field provenance:

- **Gate 4 owns and freezes the static `AuditFields` shape** into the artifact / attestation / ConsistencyBlock schema at Phase 1, alongside the rest of the canonical contract.
- **Gate 6 owns the dynamic per-execution audit-event assembly, emission, and the reconstruction-path test** (its existing acceptance criterion). The platform-repo brief carries the emission contract.

Minimal pre-freeze partition of the § 18 field list:

**Static — bound into / derivable from the artifact; MUST be in the frozen Gate-4 schema:**
- artifact hash (`Manifest.artifact_hash` slot)
- compiler version, runtime version, IR schema version, codec version (surface the canonical version triplet `0.3.0 / ke-canon-3 / postcard-1` as named audit fields)
- rule IDs evaluated, decision-trace *shape*, source spans (from `compiled_ir` + `source_span_index`)
- T0/T1/T4 status and the T2/T3 evidence slots (from `ConsistencyBlock`)
- attestation IDs, attestation policy version (from `attestations` + `VerificationPolicy`)
- jurisdiction resolver version, scenario/test corpus version where applicable (**new named version slots** — trivial to add now, expensive to retrofit after signing)

**Dynamic — per-execution; NOT in the artifact; Gate 6 / platform owns:**
- registry state at resolution time, workflow ID, execution timestamp, realized decision-trace values

Cut line: every field a signature or attestation must *cover* goes static-and-frozen; every field that is a fact *about a particular run* goes dynamic-and-platform.

## Consequences

- The Gate-4 schema freeze includes the static audit fields, so Gate 6 can assemble the full § 18 event and pass its reconstruction-path acceptance without re-attesting any artifact.
- Two Gate-1-frozen shapes must be reconciled at the Phase-0/Phase-1 boundary (before any signing), because they intersect the audit contract:
  - **`RevocationPolicy` mismatch** (enum reconciliation owned by **ADR 0013**, not re-decided here). `crates/ke-core/src/manifest.rs:84-88` declares `HaltImmediately / FinishPinnedThenHalt / FinishPinnedNoNew`; § 15 names `hard-stop / finish-pinned / audit-only`. The mapping is 3-for-3 off and **audit-only is dropped**. This ADR's only stake in it: audit-only's behavior is "allow execution; emit a high-severity audit event," so `AuditOnly` (restored by ADR 0013) must be **defined in terms of the frozen `AuditFields`** settled here. ADR 0013 owns the rename and the canon bump; this is recorded as a pre-freeze dependency so the two land together.
  - **`interpretation_notes` granularity.** `crates/ke-core/src/ir/rule.rs:41` is a single rule-level `Option<String>`, but § 17 requires per-branch notes. The source-span audit field and the SourceFidelity/Interpretation attestations bind below rule granularity, so the promote-to-per-branch decision must also be taken before freeze or attestations bind an under-specified field. Flagged here as a dependency for a follow-up ADR.
- Adds two named version slots (jurisdiction resolver, scenario/test corpus) to the schema now, costing a small surface increase to avoid a guaranteed re-attestation later.

## Alternatives considered

- **Gate 6 owns the whole § 18 contract (do-nothing).** Rejected: retrofits static fields post-freeze, forcing re-attestation, and contradicts the atomic-version-bump rule.
- **Gate 4 owns the entire § 18 contract, including dynamic emission.** Rejected: `workflow_id`, `execution_timestamp`, registry-state-at-resolution, and realized trace values are runtime facts the artifact cannot carry; this over-scopes Gate 4 into platform-runtime territory.