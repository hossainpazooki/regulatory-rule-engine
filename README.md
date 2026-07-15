# ATLAS — Automated Transjurisdictional Legal Rule Assurance System

Compiles contradictory multi-jurisdiction regulation into machine-verified,
executable, auditable rule artifacts.

[![rust-ci](https://github.com/hossainpazooki/regulatory-rule-engine/actions/workflows/rust-ci.yml/badge.svg)](https://github.com/hossainpazooki/regulatory-rule-engine/actions/workflows/rust-ci.yml)
[![contract-tests](https://github.com/hossainpazooki/regulatory-rule-engine/actions/workflows/contract-tests.yml/badge.svg)](https://github.com/hossainpazooki/regulatory-rule-engine/actions/workflows/contract-tests.yml)
[![wasm-build](https://github.com/hossainpazooki/regulatory-rule-engine/actions/workflows/wasm-build.yml/badge.svg)](https://github.com/hossainpazooki/regulatory-rule-engine/actions/workflows/wasm-build.yml)

## 0. The system

ATLAS is the **policy plane** of a three-repo system: it compiles, signs, and
publishes the verified artifacts — rule packs and treasury IntentSpecs — that
the other planes consume. `treasury-intent-controller` authorizes payment
intents against ATLAS-signed IntentSpecs; COMPASS decides and settles from
the gate's durable feed.

```mermaid
flowchart LR
  A["ATLAS (this repo)<br/>policy plane — Rust<br/>compile · sign · publish"] -->|"signed artifacts:<br/>rule packs · IntentSpecs"| T["treasury-intent-controller<br/>authorization plane — Go gate + Python scorer"]
  A -->|"signed rule artifacts<br/>(verified in-browser)"| C["COMPASS<br/>decision + settlement plane — TypeScript"]
  T -->|"durable ACHIEVED feed"| C
```

Full story and live-loop evidence: [docs/SYSTEM.md](docs/SYSTEM.md).

## What it is

Not a retrieval application — retrieval finds relevant passages; ATLAS turns
legal text into verified, executable rule artifacts. AI may **interpret**
source documents (advisory only); verification gates and typed expert
attestations **decide** what becomes trusted; a deterministic engine
**executes** only verified, signed, content-addressed artifacts.

## The trust pipeline

**ATLAS never executes an unverified candidate rule.**

```mermaid
flowchart TB
  Src["Legal source documents<br/>contradictory · multi-jurisdiction"]:::ai --> AI["AI interpretation<br/>ADVISORY only"]:::ai
  AI --> Gates{"Verification gates DECIDE<br/>schema · semantic · source-span · conflict · expert attestation"}:::gate
  Gates -->|pass| Art["Signed artifact<br/>content-addressed · ke-canon-5"]:::trust
  Gates -->|fail| Rej["Rejected / review-required<br/>never executed"]:::reject
  Art --> Reg["Registry lifecycle<br/>Published · Deprecated · Revoked"]:::trust
  Reg --> Engine["Deterministic engine<br/>executes ONLY verified artifacts"]:::trust
  Reg --> C1["COMPASS<br/>WASM verify, fail-closed"]:::trust
  Reg --> C2["tic resolver<br/>PyO3 folded verify, fail-closed"]:::trust
  Reg --> C3["graph exporter<br/>read-only Neo4j view"]:::trust
  classDef ai fill:#fef3c7,stroke:#b45309,color:#1f2937;
  classDef gate fill:#dbeafe,stroke:#1d4ed8,color:#1f2937;
  classDef trust fill:#dcfce7,stroke:#15803d,color:#1f2937;
  classDef reject fill:#fee2e2,stroke:#b91c1c,color:#1f2937;
```

## What makes it different

- **Byte-deterministic canon.** Every artifact is a pure function of its
  input (`ke-canon-5`); a 3-language contract test proves Rust ≡ Python ≡
  WASM byte-identically in CI ([encoding profile](docs/canonical-encoding.md)).
- **Cryptography is not legal truth.** Only typed, kind-aware expert
  attestations bound to an artifact hash carry legal authority
  ([attestation schema](docs/attestation-schema.md)).
- **Proof by differential, with negative controls.** Rust↔Python equivalence
  over 1,326 generated scenarios; Cypher↔Rust graph oracles where a mutated
  edge must break the harness ([STATUS](docs/STATUS.md)).
- **Consumers re-derive trust and fail closed.** Three of them — COMPASS
  (WASM), the treasury resolver (PyO3), the graph exporter — all reject
  non-`Published` artifacts even with valid crypto
  ([consumer contract](docs/consumer-serve-contract.md)).
- **One signed envelope, polymorphic payloads.** Rules and treasury
  IntentSpecs ship through the same content-addressed artifact
  ([ADR-0021](docs/adr/0021-intentspec-artifact-kind-polymorphic-payload.md)/[0022](docs/adr/0022-intentspec-r7-coattestation.md)).

## Verification tiers

| Tier | Check | Authority |
|------|-------|-----------|
| **T0** | Schema and structural validity | Compiler (Rust, deterministic) |
| **T1** | Semantic well-formedness (type, domain, span integrity) | Compiler |
| **T2** | Scenario coverage / property tests | Compiler + curated suites |
| **T3** | Rust↔Python equivalence on fixtures | Differential harness |
| **T4** | Cross-jurisdictional conflict taxonomy | Compiler (structural) + AI rationale (advisory only) |
| **Expert** | Typed attestation bound to artifact hash | Domain expert (signed) |
| **Registry** | Lifecycle transition: candidate → published → revoked | Registry (verifies all of the above) |

Compiler tiers are structural — they never assert legal truth. Spec § 5, § 10, § 13.

## Where things live

- [docs/SYSTEM.md](docs/SYSTEM.md) — the three-repo system and the live payment loop
- [docs/STATUS.md](docs/STATUS.md) — gates, workstreams, deployment, CI
- [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) — build, test, harnesses, repo layout
- [docs/adr/](docs/adr/README.md) — 23 decision records + the tie-together map
- [Migration spec v3.1](docs/spec/ke-workbench-rust-migration-spec-v3.1.md) — plan of record (amendment banners mark superseded sections)
- [Canonical encoding](docs/canonical-encoding.md) · [Attestation schema](docs/attestation-schema.md) · [Consumer contract](docs/consumer-serve-contract.md)
- [CLAUDE.md](CLAUDE.md) — session discipline and hard invariants

Research project — not legal advice; encoded rules are interpretive models.
License: proprietary.
