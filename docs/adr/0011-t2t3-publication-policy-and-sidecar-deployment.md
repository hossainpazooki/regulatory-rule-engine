# 0011. T2/T3 publication policy + sidecar deployment

**Status:** Proposed
**Date:** 2026-06-11
**Spec references:** § 11 (verification model / policy modes / `ConsistencyBlock` / T2/T3 sidecar ownership), § 21.2 (T2/T3 production policy), § 21.3 (T2/T3 sidecar deployment), § 20 (T2/T3 publish gap), § 10 (typed attestations), § 9 (lifecycle state machine), § 4.5 (platform-repo access)

## Context

Gate 4 turns a compiled IR into a signed, content-addressed, attestable artifact and stands up the registry lifecycle (§9). Two of the gate's blocking prerequisite decisions (brief § 2 rows 3–4) concern the T2/T3 verification tier:

- **T2** — embedding consistency against source spans and related rules.
- **T3** — NLI consistency against source spans and claims.

Per spec § 11, T2/T3 **do not decide legal meaning**; they produce *evidence*. They run in a Python sidecar (v1), not in `ke-compiler`. Whether that evidence blocks a publish, merely warns, or requires an explicit expert override is a **publication-policy** question, and where the sidecar physically runs is a **deployment** question. Spec § 21 lists both as open and § 23's *Before Gate 4* checklist requires both resolved before Phase 1.

Two decisions are bundled here because they are tightly coupled: the deployment shape (where T2/T3 runs and how its evidence reaches the registry) determines what the policy gate can actually enforce, and the policy mode determines what the deployment must guarantee about evidence freshness and authenticity.

The relevant § 20 risk is the **T2/T3 publish gap**: "Publishing before ML consistency checks complete may allow suspicious artifacts into production." The spec's own mitigation is to "make T2/T3 explicit publication-policy inputs" and "require strict pass or typed expert override for production environments." This ADR operationalizes that.

The Gate-1-frozen shapes that this decision must map onto already exist in `crates/ke-core/src/manifest.rs`:

- `T2T3Mode { Strict, ReviewOverride, Advisory, Disabled }` (the four § 11 policy modes).
- `VerificationPolicy { t2_t3_mode, required_attestation_types, minimum_attestation_count_per_type }`.
- `PolicyBundle { environment, verification_policy, revocation_policy, effective_window }` — i.e. policy is **per named environment**, exactly as § 11 and brief § 2 row 3 require.
- `AttestationType::PublicationApproval` (the typed attestation that authorizes publish to a named environment under a named policy, § 10).

No shape change is needed; this ADR fills in the *semantics* of those shapes for Gate 4.

This ADR does **not** resolve, and is not blocked by, the separate T4 `contradictory_outcome` defect (the detector currently flags 52 Blocking conflicts on the clean 34-rule corpus, making `draft → structurally_verified` impossible). That is a code-fix tracked independently; until it is fixed there is no `structurally_verified` artifact to which any T2/T3 policy or attestation could attach. The two are sequenced: T4 fix first, then this policy applies.

## Decision

This ADR is **Proposed**. The recommendations below are starting points for Hossain + the security and domain reviewers; the AI may propose but not decide. No LLM/AI code sits anywhere in the verification, attestation, or publish path — T2/T3 produces evidence consumed by deterministic registry logic.

### Decision A — T2/T3 publication policy (§ 21.2, § 11 policy modes)

Carry the policy **per environment** in `PolicyBundle.verification_policy.t2_t3_mode`, using the existing `T2T3Mode` enum, with the following recommended v1 mapping (each value is the § 11 mode of the same name):

| Environment | Recommended `t2_t3_mode` | Effect of a T2/T3 evidence failure |
|-------------|--------------------------|-------------------------------------|
| `production` | `Strict` (default) — `ReviewOverride` only if a launch demands a documented escape hatch | `Strict`: publish blocked. `ReviewOverride`: publish requires a typed expert override attestation with reason. |
| `staging` | `ReviewOverride` | Publish requires typed override with reason. |
| `dev` | `Advisory` | Failure recorded in the `ConsistencyBlock`; publish proceeds. |
| local-only | `Disabled` | Allowed **only** in local dev (§ 11); never in any shared environment. |

The recommended production posture is **`Strict` by default**. `ReviewOverride` is permitted in production only as an explicit, signed-off escape hatch, and the override is itself a **typed attestation** (`PublicationApproval`, optionally accompanied by an `interpretation` attestation carrying the reason), bound to the artifact hash per § 10 — never a config flag, environment variable, or ad-hoc registry edit. An override leaves a cryptographic, timestamped audit trail through the same attestation machinery as any other expert sign-off.

This directly closes the § 20 **T2/T3 publish gap**: a production publish is **fail-closed**. It requires *either* a `Strict` T2/T3 pass *or* a typed expert override attestation. There is no path that publishes to production before ML checks have completed and been evaluated against policy.

### Decision B — T2/T3 sidecar deployment (§ 21.3, § 11)

The v1 T2/T3 sidecar **stays platform-owned** (spec § 11): it lives in `institutional-defi-platform-api`, as a platform-owned package or platform-owned service. Extraction into a standalone verification service is explicitly **deferred** until a measured need (model-versioning cadence, multi-consumer demand, or isolation requirements) justifies its own ADR. The workbench never runs the ML models.

T2/T3 evidence reaches the registry by one of the two § 11-sanctioned paths, both of which materialize a **`ConsistencyBlock`** (§ 11 fields: tier result, policy mode, T2/T3 model name + version, scoring-profile version, evidence references, reviewer overrides, reviewer rationale, timestamp, execution environment):

- **B1 (recommended for production):** a platform-side verification job computes T2/T3 and **writes a signed `ConsistencyBlock` back to the registry**.
- **B2 (allowed for dev/CI reproducibility):** a workbench-triggered command (`ke-cli verify --t2t3`, name TBD) calls the platform-owned sidecar **through the § 4.5 access model** — `${PLATFORM_REPO:-../institutional-defi-platform-api}`, SHA-gated to the commit recorded in `fixtures/rules/SOURCE.md`, failing fast if the checkout is missing, dirty, or at another commit — and records the resulting `ConsistencyBlock`.

In both paths the registry's **`draft → ml_checked` lifecycle transition (§ 9)** is the single chokepoint that consumes the evidence and applies Decision A's policy. The transition is gated deterministically on the `ConsistencyBlock` and the environment's `t2_t3_mode`; it never re-runs or second-guesses the ML models.

## Consequences

- **Desirable.** Production is fail-closed against the § 20 publish gap. Policy is per-environment data inside the signed `PolicyBundle`, so the same artifact can be `Strict` in production and `Advisory` in dev without recompilation. Overrides are typed, signed, timestamped attestations — fully auditable, no silent bypass. The workbench stays out of the ML business; authority boundaries (compiler = structural, expert = attestation, registry = lifecycle) are preserved. No new shapes — the Gate-1 enums absorb the decision as-is.
- **Undesirable / cost.** The override path (`ReviewOverride`) is inert until **ADR 0009** (expert key authority) and **ADR 0010** (timestamp authority) land, because an override is a signed, trusted-timestamped attestation; this ADR must be sequenced after those. The `ConsistencyBlock` struct itself is not yet built (brief Phase 2, `crates/ke-artifact/src/consistency.rs`) — this ADR specifies its policy role but the code lands later. B1 introduces a registry-trust question: the platform-written `ConsistencyBlock` must be signed by a key the registry recognizes (ties back to ADR 0009); an unsigned or untrusted write-back must be rejected, not accepted on faith. Keeping the sidecar platform-owned means the platform's ML/runtime data remains the source of truth — acceptable for v1 but a coupling to revisit if/when extraction is justified.
- **Boundary preserved.** No LLM/AI code in the verification or publish path; T2/T3 emits evidence, deterministic registry code applies policy, experts sign overrides. The AI may produce `EditProposal` objects (§ 13) only, never attestations or publishes.

## Alternatives considered

- **Production = `Advisory` for initial launch.** Rejected as the default: it reopens the § 20 publish gap exactly (suspicious artifacts reach production before/regardless of ML checks). `Advisory` is appropriate only for `dev`.
- **A single global policy mode rather than per-environment.** Rejected: contradicts the `PolicyBundle.environment` shape (manifest.rs:109) and § 11's per-environment model, and would force the same strictness on dev and production.
- **Override as a registry config flag / CLI `--force` rather than a typed attestation.** Rejected: it would let a publish bypass T2/T3 without a cryptographic, expert-bound audit trail, defeating the § 20 mitigation and violating the § 10 expert-authority boundary (only experts sign; only signed, hash-bound attestations authorize publish).
- **Extracting the T2/T3 sidecar into its own service now (option 3).** Rejected for v1: spec § 11 says v1 ownership stays platform-side; premature extraction adds a deploy surface and coordination overhead (§ 20 crate/service over-fragmentation) with no measured need.
- **Workbench runs T2/T3 itself (pull the ML models into `ke-artifact`/`ke-cli`).** Rejected: it pulls ML/LLM-adjacent code into the artifact/registry path (forbidden by CLAUDE.md authority boundaries and the LLM-authority memory), duplicates the platform's model stack, and creates a second drift surface. The workbench consumes evidence; it does not produce it.