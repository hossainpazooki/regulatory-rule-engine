# docs/ index

What is authoritative now, what is a point-in-time record, and what is legacy.
(Added 2026-07-15; the tie-together map of the decisions lives in
[`adr/README.md`](adr/README.md#how-the-adrs-tie-together).)

## Authoritative, current-state (must track the code)

| Doc | What it binds |
|---|---|
| [`SYSTEM.md`](SYSTEM.md) | The three-repo system (ATLAS · tic · COMPASS): planes, the two binding contracts, the live-verified payment loop. |
| [`STATUS.md`](STATUS.md) | Gates, workstreams, deployment, CI — the dated state of record (moved out of the README 2026-07-15). |
| [`DEVELOPMENT.md`](DEVELOPMENT.md) | Build/test commands, harnesses, fixtures provenance, repo layout. |
| [`adr/`](adr/README.md) | All 23 decisions + index + the tie-together map. |
| [`canonical-encoding.md`](canonical-encoding.md) | The encoding profile — triplet `0.5.0` / `postcard-1` / `ke-canon-5` (ADR-0021). |
| [`attestation-schema.md`](attestation-schema.md) | Typed attestations; § 6B/§ 7 kind-selected co-attestation (ADR-0022). |
| [`consumer-serve-contract.md`](consumer-serve-contract.md) | The consumer surface: fail-closed rules + `ArtifactProvenance` shape (all three ADR-0019 consumers); HTTP endpoint table (COMPASS path). |
| [`spec/ke-workbench-rust-migration-spec-v3.1.md`](spec/ke-workbench-rust-migration-spec-v3.1.md) | The migration plan of record, with dated amendment banners where ADRs superseded it (§ 5, § 8.1/8.2, § 10, § 14). Where spec and ADR disagree, the Accepted ADR wins. |

## Point-in-time records (historical; do not "fix" retroactively)

- Gate briefs / logs: [`gate-1-canonical-ir.md`](gate-1-canonical-ir.md),
  `gate-1..5-implementation-log.md`, [`gate-4-acceptance.md`](gate-4-acceptance.md)
- [`dsl-gap-review-gate-2.md`](dsl-gap-review-gate-2.md) — Gate-2 regime coverage walk
- [`publish-atlas-artifact.md`](publish-atlas-artifact.md) — WASM package publish procedure

Three platform-era legacy imports (EKS deployment strategy, local Kubernetes
development, production enhancements — `legal-compliance-applied-ai` docs
referencing a FastAPI/Temporal stack, not `crates/ke-*`) were **deleted
2026-07-15**; they live in git history if ever needed.
