# Gate 4 — Platform consumption brief (OUTLINE)

**Status:** OUTLINE only. This is the section list + per-section "must specify"
for the **separate platform-repo brief**. The brief itself is authored and
reviewed in `institutional-defi-platform-api` (it changes that repo's code), not
here. This file exists so the §23 "platform-repo brief authored and reviewed"
checklist item has a concrete, reviewable contract to instantiate against.

**Authority:** consumer-only. The platform **verifies and executes** artifacts;
it never compiles, signs, attests, publishes, or authors rules (CLAUDE.md
authority boundaries; spec §16 "thin adapter, not a new source of truth"). No
LLM/AI code in any verification/execution path.

**Depends on (workbench Gate-4 Phase-0, all must be Accepted first):** ADRs
0009–0014 + the finalized `docs/attestation-schema.md`. Pinned to the platform
SHA recorded in `fixtures/rules/SOURCE.md`.

---

## Sections the platform-repo brief must contain

**§0 — Status / scope / authority.** Separate-repo deliverable; consumer-only;
no authoring/signing/registry code on the platform side; explicit dependency on
workbench ADRs 0009–0014; pinned to the recorded `SOURCE.md` SHA.

**§1 — Context & parity targets.** Names `src/production/{executor.py,schemas.py,
trace.py}` as the runtime that consumes artifacts; states that today's Python
`RuleRuntime` is the parity oracle the artifact must match (Gate-3 equivalence
harness is the baseline).

**§2 — `ke-artifact-py` install path.** MUST specify installing the wheel through
the **S3-backed PEP 503 simple index** (ADR 0012) with **exact version + sha256**
pinning and `pip --require-hashes` (fail-closed on drift) — not a local wheel
path. This is a Gate-4 acceptance requirement.

**§3 — Verification middleware (the core).** MUST enumerate every pre-execution
check, each yielding a **specific policy error** (Gate-4 acceptance criterion 3):
(1) BLAKE3 content hash matches the resolved hash; (2) canonical encoding decodes
(postcard-1); (3) ed25519 compiler signature valid; (4) registry lifecycle state
is `published` (or explicitly requested) for the environment; (5) runtime policy
mode compatible (`PolicyBundle.verification_policy`); (6) required attestation
types + minimum counts present (§11); (7) each attestation's key valid /
non-expired / non-revoked / authorized-for-type against the signed key directory
(ADR 0009), at **runtime**, not just pin time; (8) effective window `[from,to)`
with `jurisdiction_time_zone=None` honored exactly (ADR 0007/0008); (9) IR
schema + codec versions supported. Plus the §10 rejection rules R1–R8 from
`docs/attestation-schema.md` (incl. mock-TSA-under-non-local → reject).

**§4 — Temporal artifact pinning (DESIGN at Gate 4; IMPLEMENT at Gate 6).** MUST
cover, per §15: pin the content hash at workflow start; resolve the selector
(tag / regime+effective-date / env) in a **startup activity outside deterministic
workflow logic**; record the resolved hash in workflow history; downstream
activities load **by pinned hash only**; no implicit mid-run re-resolution
(explicit versioned re-resolution activity + audit event only). Gate 4 reviews
the design; Gate 6 implements + tests it.

**§5 — Pydantic-from-schema generation.** MUST generate platform models from the
**emitted JSON Schema** (deterministic generation; §14 schema-drift prevention),
not hand-written models; regenerate when the canonical version triplet bumps
(e.g. ADR 0013's 0.4.0/ke-canon-4).

**§6 — Cross-language contract test workflow.** MUST run `scripts/contract-test.sh`
(workbench side) round-tripping golden artifacts Rust↔Python and asserting
canonical BLAKE3 hashes match across languages; SHA-gated to the recorded
`SOURCE.md` commit; wired as a platform CI gate.

**§7 — Audit-event emission (§18, Gate-6-owned; design now).** MUST specify
emission of the **dynamic** §18 fields (workflow_id, execution timestamp,
registry state at resolution time, realized trace values) assembled with the
**static** AuditFields frozen into the artifact (ADR 0014), and the
reconstruction-path test (Gate-6 acceptance).

**§8 — End-to-end parity demo (Gate-4 acceptance).** MUST demonstrate: install
`ke-artifact-py` via the S3 index → load a signed artifact → verify (all §3
checks) → execute → output matches the current Python pipeline for a known
scenario; and that missing/stale/revoked/invalid attestations are **rejected with
a specific policy error**; and that a tag rollback resolves to the previous
signed content hash.

**§9 — Out of scope / commit boundary.** No rule authoring/signing/registry
mutation on the platform side; platform changes are a **separate PR** in
`institutional-defi-platform-api`; Python KE-module removal is Gate 6, not here.
