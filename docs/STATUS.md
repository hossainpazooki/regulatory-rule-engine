# Status — gates, workstreams, deployment

State as of 2026-07-19. Live CI: see the badges on the [README](../README.md).

## Gates and workstreams

One table, one row per gate (the former README "Status" and "Migration
roadmap" tables, merged). Every state keeps its evidence link; the long
histories live in the per-gate logs.

| Gate / workstream | Scope | State | Evidence |
|---|---|---|---|
| **0 — Repo synthesis** | Rename to `ke-workbench`, Rust workspace scaffold, frontend to `frontend/`, CI/CD, fixtures snapshot | **Merged** (PR #3) | — |
| **1 — Canonical IR** | `ke-core` IR types, canonical (postcard) encoding + strict decoder, deterministic JSON Schema, golden fixtures | **Merged**; ADRs 0001–0003 | [log](gate-1-implementation-log.md) · [brief](gate-1-canonical-ir.md) |
| **2 — Parser, compiler, T0/T1/T4** | `marked-yaml` → AST → `RuleIR`, semantic normal form, conflict taxonomy | **Accepted 2026-05-30, merged** (PR #5); live Rust↔Python differential PASS over all 7 corpus files; ADRs 0004–0006 | [log](gate-2-implementation-log.md) · [brief](../dev/briefs/gate-2-parser-compiler-verification.md) |
| **3 — Preview runtime + equivalence** | `ke-runtime` tree-walk executor mirroring the Python `RuleRuntime`, scenario generator, property tests | **Merged 2026-05-31** (PR #6); live equivalence PASS over 1,326 generated scenarios; ADRs 0007–0008 | [log](gate-3-implementation-log.md) |
| **4 — Artifact, registry, attestation, verify** | BLAKE3 content addressing + ed25519 signatures, typed attestations R1–R8, pure `verify_artifact` + `ArtifactProvenance`, registry lifecycle, PyO3 + WASM verifiers, 3-language contract test | **Complete (in-repo)**; ADRs 0009–0016; C3 + C4 met, C1 verifier + C2 foundation met in-repo — platform-api decoupled per [ADR-0017](adr/0017-gate5-sequencing-atlas-surfaces-independent.md), so consumer integration belongs to COMPASS | [acceptance](gate-4-acceptance.md) · [log](gate-4-implementation-log.md) |
| **5 — Surface rollout + frontend rewire** | serve, WASM preview, export/import, SQL views, lint, frontend rewire | 5a `ke serve` ✅ ([ADR-0018](adr/0018-serve-transport-sse-and-non-authoritative-scope.md)) · 5b-preview WASM ✅ · 5b-data `.kew` export/import ✅ (G5-4) · SQL views ✅ (G5-3 green on CI) · 5c lint ✅; **5d/5e frontend rewire + review UI DEFERRED** per [ADR-0020](adr/0020-gate5-frontend-rewire-honest-acceptance.md) — COMPASS is the consumer; ATLAS's frontend is producer-side tooling | [log](gate-5-implementation-log.md) |
| **6 — Revocation runtime-decision (reconciled)** | Pure `revocation_decision` (reason-class → `HardStop`/`FinishPinned`/`AuditOnly`), `revoke --reason-class` (floor never lowerable), `serve /resolve?regime=&effective=`, revocation block on `/resolve`+`/verify`; spec's platform Temporal cutover **deferred** post-ADR-0017 | **Merged 2026-07-19** (PR #17; [ADR-0024](adr/0024-gate6-scope-reconciliation.md) + ADR-0015 Accepted by the merge — Gate 6 closed); workspace 207/0 w/ `test-keys`, lifecycle-smoke PASS (byte-identical legacy path), live serve E2E 14/0; verify stays fail-closed | [log](gate-6-implementation-log.md) · [brief](../dev/briefs/gate-6-plan-and-next-session-seed.md) |
| **Treasury / IntentSpec** (no gate number) | Polymorphic `ArtifactPayload` (`Rules \| IntentSpec`), fifth kind `IntentSpec`, **canon bump to `0.5.0`/`ke-canon-5`**, goldens regenerated, `artifact_kind` on provenance, kind-selected R7 co-attestation | **Merged 2026-07** (PRs #12/#13/#14); consumed live by the treasury payment loop — see [SYSTEM.md](SYSTEM.md) | [ADR-0021](adr/0021-intentspec-artifact-kind-polymorphic-payload.md) · [ADR-0022](adr/0022-intentspec-r7-coattestation.md) |
| **Derived views — graph export** | `ke graph export` + `ke graph oracle-*`: verify-gated, read-only Neo4j view; `CONFLICTS_WITH` by deterministic recompute, pinned fixture; non-gating differential harness | **Merged 2026-07-15** (PR #16, ADR-0023 Accepted); harness GREEN 11/0 with both negative controls detected | [ADR-0023](adr/0023-graph-export-derived-view.md) |

Gate discipline: each gate lands on a `migration/gate-N-*` branch → PR →
operator merges; no gate begins until the prior gate's acceptance criteria
(spec § 19) are green.

## Regulatory frameworks

| Framework | Jurisdiction | Status |
|-----------|--------------|--------|
| **MiCA** | EU | Enacted (2023/1114) |
| **FCA Crypto** | UK | Enacted (COBS 4.12A) |
| **GENIUS Act** | US | Enacted (July 2025) |
| **FINMA DLT** | Switzerland | Enacted (DLT Act 2021) |
| **MAS PSA** | Singapore | Enacted (PSA 2019) |
| **RWA Authorization** | Multi-jurisdictional | Demo regime |

Source YAML lives in `fixtures/rules/`; the authoritative copy is in
`institutional-defi-platform-api/src/rules/data/` (provenance:
`fixtures/rules/SOURCE.md`).

## Deployment

| Component | Platform |
|-----------|----------|
| **Frontend** | AWS EKS (Kustomize overlays under `kube/`) |
| **Backend API** | `institutional-defi-platform-api` — **decoupled** (ADR-0017); not in the artifact path |
| **Registry (Gate 4+)** | S3-backed, content-addressed; PEP 503 simple index for `ke-artifact-py` |

## CI/CD

| Workflow | Trigger | Purpose |
|----------|---------|---------|
| `rust-ci.yml` | Push, PR | `cargo fmt` / `clippy` / `check` / `test` on the workspace |
| `frontend-ci.yml` | Push, PR | npm lint, typecheck, test, build, docker build |
| `wasm-build.yml` | Push, PR | real `wasm-bindgen` preview build (Gate 5b); cli↔crate wasm-bindgen pin asserted |
| `contract-tests.yml` | Push, PR | **3-language contract test** (Gate 4): `ke-artifact-py` wheel + WASM package, `scripts/contract-test.sh` (Rust ≡ Python ≡ WASM over golden `.kew`), SHA-gated to `SOURCE.md` |
| `cd-staging.yml` | Push to `main` | Build + push image, deploy to EKS staging |
| `cd-production.yml` | Manual | Approval-gated production deploy with rollback |
