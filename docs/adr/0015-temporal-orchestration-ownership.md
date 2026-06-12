# 0015. Temporal orchestration ownership: orchestration stays Python, the work moves to Rust

**Status:** Proposed
**Date:** 2026-06-11
**Spec references:** § 2 (non-goals), § 14, § 15, § 19 (Gate 6)

> This ADR **restates existing spec policy** so it is citable from gate briefs
> and platform-repo work; it does not make a new decision. The decision below
> is already fixed by spec § 2's non-goals and elaborated by § 14, § 15, and
> § 19 (Gate 6). It is recorded as an ADR because "who owns Temporal" keeps
> recurring in gate planning and deserves one canonical answer.

## Context

The platform repo (`institutional-defi-platform-api`) runs its production
pipelines as Temporal workflows on a Python Temporal worker. As Gates 4–6 move
artifact verification and consumption into Rust, the question recurs: does any
part of Temporal orchestration migrate to the workbench or to Rust?

The spec already answers this, twice, in § 2 Non-goals:

- line 38: "Porting search, jurisdiction resolution, RAG, decoder, analytics,
  credit pipeline, or **Temporal orchestration** to Rust."
- line 42: "**Replacing the platform's Temporal worker with a Rust worker.**"

What the spec *does* migrate is the work performed inside activities, not the
orchestration around it: § 14 defines `ke-artifact-py` (PyO3 binding) as the
platform's consumption surface and lists "Update Temporal worker activity
wrappers to resolve artifacts by content hash from the registry" as a
platform-side Gate 4 change; § 15 defines the Temporal pinning mechanism
("data-version pinning, not Temporal code-versioning") that Gate 6 must
implement in the platform repo; § 19 Gate 5 ships `ke-cli serve` as a REST +
WebSocket surface over the same Rust engine.

## Decision

**Orchestration stays Python/Temporal in `institutional-defi-platform-api`.
The work inside the workflows moves to Rust through three channels:**

1. **`ke-artifact-py` (Gate 4, § 14).** Activity wrappers load, hash-verify,
   signature-verify, and attestation-check artifacts through the PyO3 binding
   instead of Python re-implementations. The Python `RuleRuntime` remains the
   production execution layer (§ 2 non-goals, line 37).

2. **Gate-5 `ke-cli serve` verification surface (§ 19 Gate 5).** The
   platform's `RuleVerificationWorkflow` calls ke-compiler's native T0/T1/T4
   verification — `verify(rules: &[RuleIR]) -> VerificationReport` in
   `crates/ke-compiler/src/verify/mod.rs` — via the served surface, instead of
   re-implementing those tiers in Python. The saga/orchestration shell of the
   workflow stays in the platform worker.

3. **Gate-6 platform-side pinning startup activity (§ 15).** A startup
   activity resolves a tag/regime/effective-date selector to a content hash;
   the hash is recorded in Temporal workflow history as the activity result
   and passed explicitly to downstream activities, which load artifacts by the
   pinned hash only. Re-resolution requires an explicit versioned activity and
   an audit event.

**Per-workflow partition:**

| Workflow | Ownership outcome |
|----------|-------------------|
| RuleVerification | Either-way: verification tiers re-home to Rust (channel 2); the saga stays in the Python worker |
| ComplianceCheck | Stays platform — depends on jurisdiction resolution, a named non-goal (§ 2 line 38) |
| Counterfactual | Stays platform — same dependency footprint |
| DriftDetection | Stays platform — analytics is a named non-goal (§ 2 line 38) |
| CreditDecision | Stays platform — credit pipeline is a named non-goal (§ 2 line 38) |
| Pinning startup activity | New platform capability (Gate 6, § 15), not a port of anything |

## Consequences

- No Rust Temporal worker, SDK dependency, or workflow definition ever enters
  this repo. Gate planning can cite this ADR instead of re-litigating.
- The workbench's deliverables to the platform are exactly: the
  `ke-artifact-py` wheel (Gate 4), the `ke-cli serve` surface (Gate 5), and
  the registry semantics the pinning activity consumes (Gate 6). Platform-side
  Temporal changes are executed via separate platform-repo briefs (§ 14).
- Tier verification logic exists once, in Rust; Python keeps only the
  orchestration shell. Divergence between a Python re-implementation of
  T0/T1/T4 and the canonical compiler is structurally impossible.
- Trade-off accepted: verification inside `RuleVerificationWorkflow` gains a
  network hop (activity → `ke-cli serve`) or a binding call, and the platform
  inherits an availability dependency on that surface for verification-path
  workflows. Pinned execution paths are unaffected — they consume artifacts
  by content hash through `ke-artifact-py`, not through `serve`.

## Alternatives considered

- **Full Rust Temporal worker.** Rejected. It is an explicit spec non-goal
  (§ 2 line 42), and independently impractical today: the official Rust
  Temporal SDK is pre-1.0 `sdk-core` (the substrate the other SDKs build on)
  with no production-credible high-level Rust API. Even without the non-goal,
  this would re-platform the credit pipeline for zero verification benefit.
- **Workbench-native durable registry event log.** Noted, not rejected — this
  is already the ADR 0012 design: the S3-backed append-only registry event
  log gives the workbench durability for lifecycle state without taking any
  dependency on Temporal. It is complementary to, not a substitute for, the
  platform's orchestration.
- **Re-implement T0/T1/T4 in Python inside the platform.** Rejected: creates
  a second verification implementation that can drift from the canonical
  compiler — exactly the duplicate-logic risk class § 20 exists to prevent.
