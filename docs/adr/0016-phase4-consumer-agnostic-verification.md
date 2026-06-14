# 0016. Phase 4 is consumer-agnostic verification + provenance export, with both bindings

**Status:** Accepted (sign-off by Hossain, 2026-06-13)
**Date:** 2026-06-13
**Spec references:** § 6 (WASM discipline — preview/verify-only), § 14 (consumer surface / bindings), § 16 (canonical compile path vs preview)
**Amends:** `dev/briefs/gate-4-artifact-registry-attestation.md` § 5 Phase 4 (supersedes the narrower "Python-only" scope).
**Gate:** 4 (Phase 4). Splits into **4a** (this ADR + the pure Rust core, CI-testable) and **4b** (PyO3 wheel + `ke-wasm` verifier + npm package + 3-language contract test).

## Context

The brief's Phase 4 was scoped as **Python-only**: a `ke-artifact-py` PyO3 wheel
for `institutional-defi-platform-api`, published to the S3-backed PEP 503 index.
Two verified facts make that scope too narrow:

1. **The platform-api is a hypothetical consumer.** It does not consume ATLAS
   artifacts yet (greenfield); a Python-only Phase 4 ships a binding for a
   consumer that does not exist today.
2. **COMPASS is a live consumer right now**, and it is incorrect in three ways
   that a verification surface fixes:
   - it surfaces ATLAS provenance **"surfaced, not re-verified"** — it trusts a
     vendored snapshot rather than checking hash / signature / attestations;
   - it reads a sibling `fixtures/` directory that **does not exist on Vercel**;
   - it has **no revocation channel**, so it can show a **revoked** pack as
     authoritative.

The verification logic this needs already exists in `ke-artifact` as **pure,
RNG-free** functions: `decode_artifact`, `content_hash` / `verify_hash`,
`verify_signature`, `verify_attestation_set`. ed25519 *verify* is deterministic;
only signing / fixed-seed test keys touch RNG, and that is feature-gated. So
**Phase 4 is bindings + provenance export, not new crypto.**

## Decision

Phase 4 delivers a **consumer-agnostic verification API + provenance export +
both bindings (PyO3 and WASM)**, over **one** pure verification surface in
`ke-artifact`.

- **One surface, two thin bindings.** PyO3 and WASM are thin wrappers over the
  same `ke-artifact` verify surface. Building only one binding now and the other
  later would **double the cross-language contract surface** (two independent
  bindings drifting against the Rust core at different times); shipping both over
  one surface keeps a single contract.
- **`ke-wasm` verification moves into Gate 4.** It was nominally a Gate-5 crate
  (currently a stub with no `ke-artifact` dependency). COMPASS needs in-browser
  verify + revocation **now**, and the WASM verifier reuses the Phase 1–2 surface
  **verbatim** — no new logic. WASM stays **preview / verify-only** per spec § 6:
  it never signs, attests, publishes, or otherwise produces an authoritative
  artifact. The canonical compile/publish path remains `ke-cli` against an
  authoritative registry (spec § 16).
- **`ke-cli serve` (REST/WS) stays Gate 5.** Moving WASM verify earlier does not
  pull the server surface forward.
- **The export embeds registry state.** `verify_artifact` takes registry status
  as **data** (it performs no I/O, so it stays pure and WASM-ready); the
  `ArtifactProvenance` export carries the **registry lifecycle state** and the
  **event-head hash** as-of-export. An offline consumer (COMPASS on Vercel)
  therefore **refuses non-`Published` packs** and can **detect staleness** by
  comparing the embedded event-head hash against a freshly-fetched live head.
  This closes the "revoked pack shown as authoritative" correctness bug.

### 4a / 4b split (this gate)

- **4a (this ADR + delivered now):** ADR 0016; the pure consumer surface
  `ke-artifact::verify` (`verify_artifact`, `artifact_provenance`,
  `VerificationOutcome`, `Verdict` / `RejectionReason`, `RegistryStatus` /
  `RegistryEvidence`, `ArtifactProvenance` / `AttestationSummary`); the
  registry-reading `ke export-provenance` producer in `ke-cli`; CI tests. No new
  toolchain, no PyO3/WASM/AWS/async.
- **4b (next):** the PyO3 `ke-artifact-py` wheel; the `ke-wasm` wasm-bindgen
  verifier + `@platform/atlas-artifact` npm package; the 3-language
  `contract-test.sh` (Rust ≡ Python ≡ WASM — same `.kew` → identical verdict +
  provenance). Actual S3 PEP-503 / npm publishing and the COMPASS rewire are
  separate credentialed / follow-up steps Hossain drives.

## Consequences

- **Desirable.** One verification contract, not two; COMPASS gets a real
  in-browser verifier with revocation awareness once 4b ships; the offline export
  is self-describing (registry state + head travel with the provenance). The 4a
  core is pure, RNG-free, backend-free, and needs no new build toolchain, so it
  lands behind ordinary `cargo test`.
- **Undesirable / accepted.** `ke-wasm` work is pulled earlier than the original
  gate sequencing. The `RegistryStatus` enum is a **`ke-artifact`-local mirror**
  of the `ke-cli` `LifecycleState`; the mapping `LifecycleState → RegistryStatus`
  lives at the `ke-cli` boundary (`ke-cli` depends on `ke-artifact`, never the
  reverse), so the mirror must be kept in step by that one mapping site.
- **Authority unchanged.** WASM remains preview/verify-only (spec § 6); nothing
  in 4a or 4b signs, attests, publishes, or transitions lifecycle state. The
  `is_test_key` flag on the provenance surfaces that `test-*` keys are not
  production keys.

## Alternatives considered

- **Python-only (the brief's original Phase 4).** Rejected: it ships a binding
  for a hypothetical consumer (platform-api) while leaving the live consumer
  (COMPASS) unverified and unable to detect revocation.
- **Split the bindings into separate gates (PyO3 now, WASM later, or vice
  versa).** Rejected: it doubles the cross-language contract surface — two
  bindings drifting against the Rust core at different times — for no benefit,
  since both are thin wrappers over the same surface.
