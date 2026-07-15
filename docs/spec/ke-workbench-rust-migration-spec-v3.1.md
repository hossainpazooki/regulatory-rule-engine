# ke-workbench Rust Migration — Technical Spec v3.1

**Status:** Draft v3.1 — Claude Code IDE reference
**Owner:** Hossain
**Scope:** Migration of `applied-ai-regulatory-workbench` (currently a React/Vite frontend) into `ke-workbench` — a unified Rust-first knowledge engineering product that compiles regulatory rule artifacts and ships a review-first authoring UI in the same repo. The institutional DeFi platform continues to execute artifacts in Python.

**Reading order for Claude Code IDE:** §3 (product synthesis) → §4 (repo restructure) → §4.5 (platform-repo access) → §22 (per-gate session briefs). Everything between is the contract the briefs reference.

---

## 1. Summary

`ke-workbench` is **one product**, not two. It compiles regulatory rules in Rust into signed, content-addressed artifacts, and presents a review-first authoring UI in React/D3 over the same Rust engine. The existing `applied-ai-regulatory-workbench` repo is the seed — its frontend, fixtures, and UX vocabulary are preserved; its dependence on an external Python backend for compilation is replaced by a local Rust engine, WASM preview, and a thin axum REST surface.

The architectural thesis carries forward from v2: separate **structural correctness** (Rust-enforced, deterministic, continuous) from **semantic correctness** (domain-expert attested, typed, cryptographically bound). The system must not present compiler success, embedding similarity, NLI agreement, or AI-generated rationale as legal truth.

The artifact is the contract. The platform does not link against the compiler. The compiler does not decide production behavior. The workbench emits canonical, signed, content-addressed artifacts; the platform verifies policy, signatures, schema, attestation state, and revocation status before execution.

---

## 2. Goals and Non-goals

### Goals

- Migrate `applied-ai-regulatory-workbench` into `ke-workbench` as a unified product (one repo, one product narrative, multi-language surfaces).
- Compile YAML rules in Rust with deterministic serialization, content addressing, and verifiable provenance.
- Preserve every existing frontend page and visualization (Rule Browser, Navigator, Similarity Search, Analytics, Graph Visualizer, Production Monitor) by rewiring them to the new Rust-backed surfaces.
- Enforce T0/T1/T4 verification before attestation and publication.
- Run T2/T3 as explicit publication-policy inputs, not passive diagnostics.
- Preserve Python `RuleRuntime` as the production execution layer; prove equivalence against Rust preview execution.
- Expose one canonical artifact through Rust SDK, Python SDK, REST, WebSocket feed, SQL view, flat-file export, and WASM preview.
- Support typed expert attestations for source fidelity, scenario coverage, interpretation, and publication approval.
- Make every production decision reconstructable from artifact hash, rule trace, source spans, compiler version, runtime version, verification evidence, and attestation IDs.

### Non-goals

- Replacing the Python `RuleRuntime` in production. Rust `ke-runtime` exists only for preview, differential testing, and scenario tracing.
- Porting search, jurisdiction resolution, RAG, decoder, analytics, credit pipeline, or Temporal orchestration to Rust.
- Treating T2/T3 ML checks as legal interpretation.
- Allowing AI-generated edits to bypass human review or expert attestation.
- Building Verdikt or extracting a shared `regulatory-ir-core` crate.
- Replacing the platform's Temporal worker with a Rust worker.
- Greenfield rewriting the frontend. The existing React/D3 codebase is the seed.

---

## 3. Product Synthesis

This migration combines two existing concerns into one product:

| Source                                | What it contributes                                                                                       | What it loses                                                            |
| ------------------------------------- | --------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------ |
| `applied-ai-regulatory-workbench`     | React/Vite/TS frontend, 8 pages, D3 visualizations, React Query hooks, Axios clients, existing UX patterns | Dependence on external `VITE_API_URL` for compilation; passive UI posture |
| Planned Rust rule engine              | Compiler, IR, canonicalization, runtime preview, artifact, registry, WASM, attestation model               | Standalone framing                                                       |
| **Synthesis (ke-workbench)**          | Unified review-first KE product where the same repo owns compilation, preview, authoring, and the surfaces that connect them | — |

The unified product narrative: **ke-workbench is the place where regulatory rules are authored, compiled, verified, attested, and published as signed artifacts.** The institutional DeFi platform is the consumer that executes them. There is no third repo and no shared library.

Three things this synthesis fixes structurally:

1. **One repo for the KE loop.** Authoring (React), compilation (Rust), preview (WASM in React), review (React reading Rust outputs), and publication (Rust CLI emitting to registry) are no longer split across repos with unclear ownership.
2. **WASM eliminates the round-trip.** Today the frontend round-trips to the Python backend for compilation. Post-migration, in-browser dry-run runs locally via WASM. The REST surface still exists for canonical compile and for corpus queries, but iteration is local.
3. **One product story.** No more "the workbench is the React frontend and also the FastAPI backend, depending on who you ask." The workbench is one product with multiple surfaces over one canonical artifact.

---

## 4. Repo Restructure

### 4.1 Renaming and migration approach

The migration is performed in-place on the existing `applied-ai-regulatory-workbench` repo. The repo is renamed to `ke-workbench` at Gate 0. Git history is preserved.

### 4.2 Before/after directory layout

**Before** (current `applied-ai-regulatory-workbench`):

```text
applied-ai-regulatory-workbench/
├── .github/workflows/
│   ├── ci.yml
│   ├── cd-staging.yml
│   └── cd-production.yml
├── kube/
│   ├── deployment.yaml
│   ├── service.yaml
│   └── hpa.yaml
├── Dockerfile
├── nginx.conf
├── package.json
├── package-lock.json
├── vite.config.ts
├── tsconfig.json
├── src/
│   ├── pages/
│   ├── components/
│   ├── hooks/
│   ├── api/
│   └── ...
├── public/
└── README.md
```

**After** (target `ke-workbench`):

```text
ke-workbench/
├── Cargo.toml                          # workspace root
├── rust-toolchain.toml                 # pinned toolchain
├── CLAUDE.md                           # session discipline + repo invariants
├── README.md                           # product README
├── Dockerfile                          # preserved; paths updated for frontend/ build output
├── nginx.conf                          # preserved; subpath-safe config retained
├── kube/                               # preserved; manifests audited after path move
│   ├── deployment.yaml
│   ├── service.yaml
│   └── hpa.yaml
├── crates/
│   ├── ke-core/                        # IR, AST, canonicalization
│   ├── ke-compiler/                    # parser + compiler + T0/T1/T4
│   ├── ke-runtime/                     # preview executor + scenario harness
│   ├── ke-artifact/                    # canonical encoding, signing, attestations
│   ├── ke-cli/                         # compile/verify/attest/publish/serve binary
│   └── ke-wasm/                        # browser bindings
├── crates-deferred/                    # placeholder; ke-search, ke-registry, ke-lint, ke-artifact-py land here
├── frontend/                           # React/Vite/TS (formerly repo root contents)
│   ├── package.json
│   ├── package-lock.json               # retained in Gate 0; package-manager migration is deferred
│   ├── vite.config.ts
│   ├── tsconfig.json
│   ├── src/
│   │   ├── pages/                      # preserved
│   │   ├── components/                 # preserved
│   │   ├── hooks/                      # rewired in Gate 5
│   │   ├── api/                        # rewired in Gate 5
│   │   ├── wasm/                       # new; bindings to ke-wasm
│   │   └── ...
│   └── public/
├── fixtures/
│   ├── rules/                          # YAML rule corpus (copied from platform `src/rules/data/`)
│   ├── traces/                         # trace fixtures for equivalence harness
│   └── artifacts/                      # golden artifact bytes for canonicalization tests
├── docs/
│   ├── spec/                           # this spec lives here
│   ├── canonical-encoding.md
│   ├── attestation-schema.md
│   └── adr/                            # architecture decision records
├── scripts/
│   ├── bootstrap.sh
│   ├── differential-test.sh
│   └── equivalence-harness.sh
├── .github/
│   └── workflows/
│       ├── ci.yml                      # retired or replaced explicitly in Gate 0
│       ├── cd-staging.yml              # preserved with frontend/ path updates or retired explicitly
│       ├── cd-production.yml           # preserved with frontend/ path updates or retired explicitly
│       ├── rust-ci.yml
│       ├── frontend-ci.yml
│       ├── contract-tests.yml
│       └── wasm-build.yml
└── .gitignore
```

### 4.3 What moves, what stays, what's new

- `package.json`, `package-lock.json`, `vite.config.ts`, `tsconfig.json`, `src/`, `public/` → `frontend/` (preserve git history via `git mv`).
- README.md → preserved, but rewritten in Gate 0 to reflect the product synthesis.
- `Dockerfile` and `nginx.conf` → preserved at repo root in Gate 0; paths updated to build and serve `frontend/` output while retaining the EKS subpath behavior from commit `5ec457d`.
- `kube/` → preserved at repo root in Gate 0; frontend image, path, env, probe, and HPA references audited after the `frontend/` move.
- Existing `.github/workflows/{ci.yml,cd-staging.yml,cd-production.yml}` → either replaced by the new split workflows or updated to the new `frontend/` paths in Gate 0. No legacy workflow may remain ambiguous after Gate 0.
- Existing `pages/`, `components/`, D3 visualizations → preserved unchanged through Gate 4. Rewired in Gate 5.
- `Cargo.toml`, `crates/`, `rust-toolchain.toml`, `fixtures/`, `docs/spec/`, `.github/workflows/rust-ci.yml`, `.github/workflows/contract-tests.yml`, `.github/workflows/wasm-build.yml` are new.
- `fixtures/rules/` is populated by copying `institutional-defi-platform-api/src/rules/data/` (one-time snapshot; updates flow back through the platform repo until Gate 5).
- Gate 0 retains npm and `package-lock.json`. A pnpm migration is explicitly deferred unless it is separately justified by a frontend tooling ADR.

### 4.4 CLAUDE.md invariants for this repo

Drafted at Gate 0, lives at repo root. Required content:

- Sequential file operations rule.
- No git commit/push from Claude Code — Hossain owns history. Claude Code may stage-preserving file moves with `git mv`, but it must never create commits or push branches.
- Run `cargo test --workspace` and `cd frontend && npm test` after every batch once those commands exist.
- Plan Mode required for any change touching ≥ 2 files.
- Frontend changes preserve existing routes and page-level public APIs through Gate 4.
- The `fixtures/` directory is read-only inside ordinary Claude Code implementation sessions. Fixture updates are allowed only through documented sync/generation scripts that regenerate dependent fixtures atomically.
- Gate boundaries are commit boundaries; no gate may begin until the prior gate's acceptance criteria are green.
- Gate work happens on per-gate migration branches named `migration/gate-N-*`. Hossain merges each gate after reviewing the diff and committing manually.

### 4.5 Platform-repo access model

Several gates depend on `institutional-defi-platform-api` for source rules, Python compiler behavior, Python runtime traces, and platform-side contract tests. The access mechanism is standardized so every brief can reference it rather than inventing its own path.

- The workbench repo never vendors the platform repo.
- Local development and Claude Code actor sessions expect a sibling checkout by default:

```text
parent/
├── ke-workbench/
└── institutional-defi-platform-api/
```

- `PLATFORM_REPO` may override the sibling path. All scripts must resolve the platform path through `${PLATFORM_REPO:-../institutional-defi-platform-api}`.
- `scripts/bootstrap.sh` copies `src/rules/data/` into `fixtures/rules/` from the resolved platform path and records the platform git commit SHA in `fixtures/rules/SOURCE.md`.
- `scripts/differential-test.sh` invokes the Python compiler from the resolved platform path. It must fail fast if the platform checkout is missing, dirty in relevant files, or at an unrecorded commit.
- `scripts/equivalence-harness.sh` invokes the Python `RuleRuntime` from the resolved platform path. It must record the platform commit SHA in its test output.
- Gate 4 platform-side contract tests run in the platform repo through a separate PR/brief. The workbench side produces artifacts and package outputs; the platform side consumes them.

### 4.6 Branching and in-flight frontend work

Gate 0 must not trample active frontend work such as the credit module or `DocumentIngestion` page.

- Before Gate 0 begins, Hossain either commits current frontend work, stashes it, or creates a named preservation branch.
- Gate 0 starts from a clean working tree.
- Active feature work after Gate 0 targets either the current gate branch or a short-lived feature branch rebased onto the latest completed gate.
- The phrase "frontend preserved unchanged through Gate 4" means page-level behavior, routes, public component contracts, and API assumptions are preserved. It does not mean the frontend directory is frozen against unrelated user-approved feature work.

---

## 5. Architecture

> **Amended 2026-07-15.** The consumer side of this diagram is superseded:
> per **ADR-0017** `institutional-defi-platform-api` is decoupled and is not
> in the artifact path. The consumers are now the three
> **ADR-0019-disciplined** surfaces — COMPASS (in-browser WASM verify), the
> treasury intent resolver (`ke-artifact-py` fold,
> `treasury-intent-controller/scorer`; ADR-0021/0022), and the graph exporter
> (`ke graph export`, read-only derived view; ADR-0023). The producer side
> (left box) and the authority boundaries below are unchanged. The original
> diagram is retained as the migration-era plan of record.

Two repos, one artifact contract.

```text
┌──────────────────────────────────┐   signed artifact   ┌──────────────────────────────────┐
│ ke-workbench                     │   content hash      │ institutional-defi-platform-api  │
│                                  │ ──────────────────> │                                  │
│ • Rust compiler + verifier       │                     │ • Python RuleRuntime executes    │
│ • React/D3 authoring UI          │                     │ • Temporal workflows orchestrate │
│ • WASM preview/dry-run           │                     │ • Jurisdiction resolver remains  │
│ • Registry + policy gates        │                     │ • Platform verifies artifacts    │
│ • REST + WS + SQL surfaces       │                     │                                  │
└──────────────────────────────────┘                     └──────────────────────────────────┘
```

The four authority boundaries from v2 are unchanged:

- **Compiler authority:** May produce structurally valid artifacts and compiler signatures.
- **AI assistant authority:** May propose edits, generate rationales, produce candidate scenarios, and identify possible inconsistencies. May not attest, publish, revoke, or silently modify committed rules.
- **Domain expert authority:** May sign typed attestations over specific legal and review claims.
- **Registry authority:** May transition artifact lifecycle state after verifying policy, signatures, keys, revocation status, and required checks.

---

## 6. Crate Layout

The initial Cargo workspace starts with the smallest stable boundaries and splits further as interfaces harden.

| Crate          | Purpose                                                            | wasm32  | native | Phase  |
| -------------- | ------------------------------------------------------------------ | :-----: | :----: | ------ |
| `ke-core`      | IR types, AST, semantic diff helpers, canonicalization              |    ✓    |   ✓    | Gate 1 |
| `ke-compiler`  | YAML parsing, AST to IR, optimizer, T0/T1/T4 passes                 | partial |   ✓    | Gate 2 |
| `ke-runtime`   | Preview executor and trace/scenario harness                         |    ✓    |   ✓    | Gate 3 |
| `ke-artifact`  | Canonical encoding, BLAKE3 content addressing, ed25519 signatures, attestations | partial | ✓ | Gate 4 |
| `ke-cli`       | `compile`, `verify`, `attest`, `publish`, `query`, `serve`          |    ✗    |   ✓    | Gate 4 |
| `ke-wasm`      | Browser wrappers for preview compile and dry-run                    |    ✓    |   ✗    | Gate 5 |

### Deferred crate splits (`crates-deferred/`)

- `ke-search` for corpus indexing and T4 acceleration. Split when conflict-detection latency over the full corpus exceeds budget.
- `ke-registry` once registry persistence and policy APIs stabilize. Initial registry logic lives inside `ke-artifact` and `ke-cli`.
- `ke-lint` once lint classes stabilize beyond compiler verification.
- `ke-artifact-py` may live beside `ke-artifact` (via a `pyo3` feature) and split when packaging requires.

### WASM discipline

The WASM target depends only on a `wasm-safe` subset of dependencies. Browser execution is preview-only. Browser code must not sign, attest, publish, or otherwise produce authoritative artifacts.

### Workspace `Cargo.toml` skeleton

```toml
[workspace]
resolver = "2"
members = [
    "crates/ke-core",
    "crates/ke-compiler",
    "crates/ke-runtime",
    "crates/ke-artifact",
    "crates/ke-cli",
    "crates/ke-wasm",
]

[workspace.package]
edition = "2021"
# Gate 0 selects current stable and fills this with a concrete version.
# rust-version = "..."
license = "Proprietary"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
postcard = { version = "1", features = ["use-std"] }
blake3 = "1"
ed25519-dalek = "2"
thiserror = "1"
tracing = "0.1"
anyhow = "1"
```

`rust-toolchain.toml` pins the current stable toolchain selected at Gate 0. If the workspace later needs a lower MSRV, that lower MSRV must be justified in an ADR and reflected in `workspace.package.rust-version`.

---

## 7. Frontend Layout

The frontend is preserved structurally through Gate 4 and rewired in Gate 5.

### 7.1 Inherited surface (unchanged through Gate 4)

Pages, routes, components, and the existing D3 visualizations remain functional against the platform's existing API for the duration of Gates 1–4. This guarantees the frontend ships continuously and the migration is gated by the cross-repo artifact contract rather than by frontend availability.

### 7.2 New additions

- `frontend/src/wasm/` — TypeScript wrappers around the `ke-wasm` package. Loaded asynchronously; provides `compilePreview(yaml)` and `dryRun(ir, scenario)` functions.
- `frontend/src/api/local.ts` — adapter that resolves between WASM-local execution and remote REST execution depending on operation kind (preview → WASM; canonical compile/publish → REST).
- `frontend/src/components/review/` — new review-first surfaces (side-by-side source/encoded view, attestation UI, conflict viewer, scenario coverage). Added incrementally in Gate 5.

### 7.3 Frontend tooling decisions

- Package manager in Gate 0: npm, preserving the existing `package-lock.json` and current frontend commands.
- Package-manager migration: deferred. A future switch to pnpm requires an ADR covering why the migration is load-bearing, deletion of `package-lock.json`, generation of `pnpm-lock.yaml`, CI updates, local developer command updates, and validation of every existing npm script.
- Frontend CI in Gate 0: `npm ci`, `npm run typecheck` if present, `npm test`, `npm run build`. WASM artifact integration is added later; the Gate 0 `wasm-build.yml` is a no-op or stub workflow that proves path structure only.
- Visual regression: Playwright + percy or Chromatic. Spec deferred to a separate frontend-review brief.

### 7.4 Feature flag mechanism

Gate 5 rewires surfaces behind explicit flags. The initial mechanism is:

- Build-time Vite environment variables for coarse routing, including `VITE_USE_LOCAL_KE_API`, `VITE_USE_WASM_PREVIEW`, and `VITE_USE_REVIEW_UI`.
- Runtime user/session overrides may be added later, but are not required for Gate 5.
- The previous external `VITE_API_URL` path remains available until each page passes parity tests against the local Rust surface.

---

## 8. Artifact Contract

Compiled artifacts are deterministic bundles serialized through a canonical encoding profile, content-addressed by BLAKE3, and signed with ed25519. The canonical encoding profile defines field ordering, map ordering, optional-field representation, numeric representation, string normalization, schema versioning, and codec versioning.

The canonical encoding profile also covers JSON Schema emission. Field ordering, enum representation, `$defs` ordering, reference naming, and schema metadata must be deterministic across repeated local runs and CI runs. Schema determinism is tested separately from artifact-byte determinism.

### 8.1 Artifact structure

```text
Artifact {
  manifest: Manifest {
    artifact_kind,            // RegimePack | EquivalenceMatrix | TestCorpus | PolicyBundle | IntentSpec (ADR-0021)
    artifact_hash,            // BLAKE3 of canonical bytes (self-referential; computed last)
    regime_id,
    effective_from,
    effective_to,
    compiler_version,
    compiler_build_hash,
    ir_schema_version,
    codec_version,
    canonicalization_version,
    corpus_root_hash,
    source_corpus_hash,
    attestation_policy_version,
  },
  payload,                    // ArtifactPayload: Rules(Vec<RuleIR>) | IntentSpec(IntentSpecIR)
                              //   (was `compiled_ir` pre-canon-5; amended per ADR-0021 —
                              //    kind↔payload agreement enforced at decode)
  source_span_index,
  consistency_block,
  compiler_signature,
  attestations: [Attestation],
  registry_state_metadata,
}
```

### 8.2 Artifact kinds

> **Amended 2026-07-15 (ADR-0021):** a fifth kind, **IntentSpec**, was added
> — appended LAST for discriminant-value stability. It is the first non-rule
> kind with an envelope payload representation (`IntentSpecIR`); the middle
> three kinds still await payload variants.

1. **Regime pack:** Rules scoped to a single `regime_id`.
2. **Equivalence matrix:** Relational claims linking rule pairs across regimes.
3. **Test corpus:** Scenario fixtures, expected outcomes, coverage metadata, expert attestations.
4. **Policy bundle:** Publication and runtime-enforcement policy for a named environment.
5. **Intent spec:** Authorization criteria for an action class (treasury payments first) — the artifact the intent-gated action loop's resolver verifies and scores against (ADR-0021; kind-selected attestation policy per ADR-0022).

### 8.3 Golden test vectors

Required at Gate 1 and continuously enforced thereafter:

- Canonical serialization (input AST → bytes).
- Artifact hash computation.
- Compiler signature verification.
- Expert attestation verification.
- Cross-language Rust/Python artifact decoding.
- Rejection of non-canonical encodings (must produce identifiable errors).

Stored in `fixtures/artifacts/`. Updated only via a documented procedure that requires regenerating all dependent fixtures atomically.

### 8.4 Effective dates and jurisdiction time

Regulatory effective windows are legal-date concepts, not implicit UTC-midnight instants.

- `effective_from` and `effective_to` are stored as jurisdiction-local dates plus `jurisdiction_time_zone`.
- Runtime applicability uses a closed-open interval: `[effective_from, effective_to)`.
- A scenario timestamp is converted into the rule's `jurisdiction_time_zone` before date-window evaluation.
- If a regime has a non-standard legal-effective-time convention, the artifact must carry an explicit `effective_time_policy`.
- Gate 1 IR shape must include these fields before fixtures are frozen.

---

## 9. Artifact Lifecycle State Machine

The registry treats artifact state as an explicit state machine. Transitions are append-only registry events.

```text
draft
  → structurally_verified
  → ml_checked
  → expert_attested
  → published
  → deprecated
  → revoked
```

### State definitions

- **`draft`:** Produced locally or in CI. Previewable. Must not be consumed by production runtime.
- **`structurally_verified`:** T0, T1, and blocking T4 checks passed. Compiler signature valid.
- **`ml_checked`:** T2/T3 completed under a named policy. Results may pass, warn, fail, or require expert override.
- **`expert_attested`:** Required typed attestations present, valid, non-expired, bound to artifact hash.
- **`published`:** Registry policy approved for a named environment.
- **`deprecated`:** Executable for pinned historical workflows; not selected for new workflows.
- **`revoked`:** Not selected for new workflows. Existing workflow behavior depends on platform revocation policy.

### Transition rules

- Only compiler/CI authority moves `draft` → `structurally_verified`.
- Only the ML verification sidecar attaches T2/T3 evidence and moves to `ml_checked`.
- Only domain expert keys add typed attestations.
- Only registry policy moves `expert_attested` → `published`.
- Deprecation and revocation require signed registry events.
- State transitions never mutate artifact bytes. They append registry metadata and signed events.

---

## 10. Typed Attestation Model

> **Amended 2026-07-15 (ADR-0022):** the required attestation set and the R7
> approval co-attestation rule are **kind-selected** — rule-shaped kinds
> require the `{scenario_coverage, source_fidelity}` pair alongside
> `publication_approval`; `IntentSpec` requires `source_fidelity` only. The
> normative table lives in `docs/attestation-schema.md` § 6B / § 7.

Expert signatures attest to specific claims, not vague semantic correctness.

### Attestation types

- **`source_fidelity`:** Encoded rule logic faithfully reflects cited legal source spans for the stated regime and effective period.
- **`interpretation`:** Interpretation notes for vague terms, thresholds, exceptions, and regime-specific judgments are acceptable.
- **`scenario_coverage`:** The signed test corpus is sufficient for the expert's review scope.
- **`equivalence_claim`:** A cross-regime equivalence or non-equivalence claim is valid under stated conditions.
- **`publication_approval`:** The artifact may be published to a named environment under a named policy.

### Bound fields (all required)

- artifact hash
- rule IDs or artifact scope
- attestation type
- signer identity, key ID, signer role
- regime ID
- effective date range
- legal source hash
- IR schema version
- compiler version
- attestation policy version
- timestamp from a trusted timestamp authority
- optional expiration
- optional reviewer comments

### Platform rejection rules

The platform must reject attestations if:

- key is unknown, expired, revoked, or unauthorized for the attestation type
- attestation is not bound to the artifact hash being executed
- attestation policy version is unsupported
- attestation has expired
- legal source hash changed after attestation
- required attestation types are missing

### Timestamp authority

Gate 4 must select a concrete timestamp authority before implementation. The default v1 choice is RFC 3161-compatible timestamping. Local development may use a deterministic mock timestamp authority, but artifacts signed with the mock authority are rejected by non-local runtime policy.

---

## 11. Verification Model

| Tier | What                                                                  | Where it runs                                | Blocks publication?  |
| ---- | --------------------------------------------------------------------- | -------------------------------------------- | -------------------- |
| T0   | Schema validation and required fields                                  | `ke-compiler`                                | Yes                  |
| T1   | Lexical/source-span checks, required interpretation notes, decision-node source-span coverage | `ke-compiler`                            | Yes                  |
| T2   | Embedding consistency against source spans and related rules           | Python sidecar v1                            | Policy-dependent     |
| T3   | NLI consistency against source spans and claims                        | Python sidecar v1                            | Policy-dependent     |
| T4   | Cross-rule conflict detection                                          | `ke-compiler` plus indexed corpus            | Severity-dependent   |

T2/T3 do not decide legal meaning. They produce evidence. Publication policy decides whether evidence blocks publish, warns, or requires explicit expert override.

For Gate 2, "source coverage" means every decision node, obligation, threshold, exception, and discretionary term has at least one source-span reference. Whole-document coverage, such as measuring ignored or uncovered legal text, requires source-text storage and is deferred until the legal source storage decision is resolved.

### Policy modes

- **`strict`:** T2/T3 failures block publication.
- **`review_override`:** T2/T3 failures require typed expert override with reason.
- **`advisory`:** T2/T3 failures recorded but do not block publication.
- **`disabled`:** Allowed only in local development.

### `ConsistencyBlock` fields

- tier result
- policy mode
- model name and version for T2/T3
- prompt or scoring profile version where applicable
- evidence references
- reviewer overrides
- reviewer rationale
- timestamp
- execution environment

### T2/T3 sidecar ownership

The v1 T2/T3 sidecar is platform-owned and lives in `institutional-defi-platform-api` until a separate extraction decision is made. Gate 6 removes the old Python KE compiler/runtime module, but it does not automatically remove the T2/T3 verification sidecar. If the sidecar remains Python, it should be isolated under a verification-specific package or service boundary rather than coupled to the deprecated KE module.

Gate 4 publication gating may consume T2/T3 evidence from either:

- a platform-side verification job that writes evidence back to the registry, or
- a workbench-triggered command that calls the platform-owned sidecar through the standardized platform-repo access model in §4.5.

The exact deployment shape must be selected before Gate 4.

---

## 12. T4 Conflict Taxonomy

T4 classifies conflicts explicitly. Generic "cross-rule conflict" is insufficient.

### Conflict classes

- **`contradictory_outcome`:** Same scenario produces incompatible decisions.
- **`overlapping_scope`:** Multiple rules claim authority over the same scenario without precedence.
- **`temporal_overlap`:** Effective date windows overlap unexpectedly.
- **`source_span_divergence`:** Rules cite equivalent or overlapping source text but encode materially different logic.
- **`equivalence_matrix_conflict`:** Equivalence claims disagree with compiled behavior.
- **`obligation_collision`:** Two applicable rules impose incompatible obligations.
- **`missing_precedence`:** A specific/general rule relationship exists but precedence is not encoded.
- **`duplicate_rule`:** Rules appear semantically duplicate but differ in metadata or source attribution.

### Severity levels

- **Blocking:** Must be fixed or explicitly overridden before publication.
- **Review-required:** Requires expert review; may be published with typed override.
- **Advisory:** Recorded as quality signal.

### Required finding fields

- rule IDs
- conflict class
- severity
- minimal counterexample scenario (if available)
- source spans
- trace comparison
- suggested resolution class

---

## 13. AI Edit Provenance

AI-generated contributions are proposals, not committed truth.

### `EditProposal` shape

- proposal ID
- generating model and version
- prompt template or task profile version
- source spans used
- affected rule IDs
- proposed YAML diff
- proposed scenario changes
- rationale
- uncertainty markers
- verification results before and after proposal
- reviewer disposition: accepted, modified, rejected, superseded

### Rules

- AI may create proposals, rationales, source-span mappings, scenario candidates, conflict explanations.
- AI may not sign attestations.
- AI may not publish artifacts.
- AI may not silently modify committed rules.
- Accepted AI proposals become ordinary commits but retain provenance back to the proposal.
- Rejected proposals remain auditable if they influenced later human edits.

### UI distinction requirements

Minimum Gate 5 UI scope is visual distinction and provenance inspection, not full expert-review workflow completion. The UI must visually distinguish:

- human-authored rule text
- AI-proposed changes
- compiler-generated diagnostics
- ML consistency evidence
- expert attestations

Full source coverage visualization, counterexample exploration, and semantic diff review may ship as staged Gate 5 follow-ups if the minimum distinction/provenance UI is present.

---

## 14. Cross-Repo Integration

> **Amended 2026-07-15.** This section predates ADR-0016/0017 and describes
> an architecture no longer in effect: `institutional-defi-platform-api` is
> **decoupled** (ADR-0017) and does not consume artifacts. The `ke-artifact-py`
> binding survived its original consumer and is now consumed by the
> **treasury intent resolver** (`treasury-intent-controller/scorer` — folded
> `verify_artifact` per hash, fail-closed; ADR-0021/0022 govern its
> IntentSpec artifacts). The other consumers are COMPASS (WASM, ADR-0019/0020)
> and the graph exporter (ADR-0023). The Python surface below is retained as
> the binding's API sketch; the authoritative surface is
> `crates/ke-artifact/src/python.rs`.

The platform consumes artifacts through `ke-artifact-py`, a PyO3 binding over `ke-artifact`.

### Python surface

```python
class Artifact:
    @classmethod
    def from_bytes(cls, b: bytes) -> "Artifact": ...
    def canonical_hash(self) -> str: ...
    def verify_compiler_signature(self, public_key: bytes) -> bool: ...
    def verify_attestations(self, policy: RuntimePolicy) -> VerificationResult: ...
    def iter_rules(self) -> Iterator[CompiledRule]: ...
    def consistency_block(self) -> ConsistencyBlock: ...
    def attestations(self) -> list[Attestation]: ...
    def source_span_index(self) -> SourceSpanIndex: ...
```

The platform's `RuleRuntime` accepts a `CompiledRule` and executes it without changing production business logic. Before execution, the platform verifies:

- artifact hash
- canonical encoding
- compiler signature
- registry state
- runtime policy compatibility
- required attestation types
- key validity and revocation status
- effective date window
- schema and codec versions

### Schema drift prevention

1. `ke-artifact` emits JSON Schema on every release.
2. Platform Pydantic models are generated from that schema.
3. CI in both repos round-trips golden artifact fixtures.
4. Contract tests verify canonical hashes across Rust and Python.
5. Runtime trace fixtures verify behavioral equivalence.

### Platform-side changes (in `institutional-defi-platform-api`)

These are out of scope for the workbench Claude Code sessions but are documented here for completeness — they are Gate 4 dependencies and require a separate brief executed in the platform repo:

- Add `ke-artifact-py` dependency.
- Implement artifact verification middleware in `src/production/` consuming the binding.
- Update Temporal worker activity wrappers to resolve artifacts by content hash from the registry.
- Generate Pydantic models from the JSON Schema emitted by `ke-artifact`.
- Add contract test workflow consuming `fixtures/artifacts/`.

### `ke-artifact-py` packaging

Gate 4 publishes `ke-artifact-py` to an internal PEP 503-compatible simple package index backed by S3. The platform pins an exact wheel version and hash from that index. Test/dev may consume a locally built wheel path, but the platform-repo Gate 4 PR must demonstrate installation through the same index mechanism intended for staging.

---

## 15. Runtime Selection, Pinning, Rollback, Revocation

Production workflows execute deterministic artifacts.

### Rules

- A workflow pins an artifact content hash at workflow start.
- A running workflow must not observe a new artifact mid-run unless it explicitly opts into re-resolution.
- New workflows may resolve by semver tag, regime ID, effective date, and environment, but resolution returns a content hash.
- Tags move only through signed registry events.
- Published artifacts are immutable.
- Rollback moves a tag or policy pointer to a previous content hash; it does not mutate artifact bytes.
- Revocation is an append-only registry event.

### Temporal pinning mechanism

Temporal workflow code must stay deterministic. Artifact resolution therefore happens outside deterministic workflow logic:

- If the caller already knows the artifact hash, it passes the hash as a workflow input.
- If the caller provides a tag/regime/effective-date selector, the workflow invokes a startup activity to resolve that selector to a content hash.
- The resolved hash is recorded in Temporal workflow history as the activity result and passed explicitly to all downstream activities.
- Downstream activities load artifacts by the pinned hash only.
- Re-resolution requires an explicit versioned activity and an audit event; it must not happen implicitly in the middle of a workflow.

This is data-version pinning, not Temporal code-versioning. Gate 6 must implement and test this behavior in the platform repo before removing the Python KE module.

### Platform behavior by state

- `published`: eligible for new workflows.
- `deprecated`: ineligible for new workflows; valid for pinned historical workflows.
- `revoked`: ineligible for new workflows; pinned workflow behavior depends on severity and policy.

### Revocation policy options

- **hard stop:** fail any workflow attempting to execute.
- **finish pinned:** allow already-started workflows to finish; block new starts.
- **audit-only:** allow execution; emit high-severity audit event.

---

## 16. Multi-Surface Access

One canonical artifact, many adapters:

1. **Rust SDK** — Direct artifact, registry, and verification APIs for future Rust services.
2. **Python SDK** — `ke-artifact-py` for the institutional DeFi platform.
3. **REST** — `ke-cli serve` (axum). Used by the frontend for canonical compile, registry queries, and operations that browser WASM cannot perform.
4. **WebSocket change feed** — Events: `artifact.published`, `artifact.deprecated`, `artifact.revoked`, `conflict.detected`, `attestation.recorded`, `proposal.created`.
5. **SQL** — DuckDB views over artifact metadata, rule metadata, source spans, conflicts, attestations.
6. **Flat-file export** — Signed `.kew` bundles in versioned S3 for disaster recovery and offline audit.
7. **WASM** — Browser preview: `compile_preview(yaml)` and `dry_run(ir, scenario)`.

Discipline: a new consumer gets a thin adapter over the artifact contract. It does not get a new compiler, a new source of truth, or a bypass around registry policy.

---

## 17. Authoring and Review Workflow

The workbench is optimized for review-first knowledge engineering.

1. An engineer or AI assistant proposes a YAML rule change.
2. Each decision node references legal source spans.
3. AI-generated changes are stored as `EditProposal` objects until accepted by a human.
4. On save, `ke-compiler` emits draft IR and runs T0/T1/T4.
5. The UI renders decision tree, source spans, interpretation notes, conflicts, scenario traces.
6. WASM preview runs local dry-runs for fast iteration — clearly labeled non-authoritative.
7. The author submits a review bundle.
8. The domain expert reviews:
   - source text beside encoded logic
   - semantic diff from prior attested version
   - source coverage
   - ignored or uncovered legal text
   - scenario coverage
   - generated counterexamples
   - T2/T3 evidence
   - T4 conflicts
   - interpretation notes
9. The expert signs typed attestations.
10. Registry policy verifies required checks and transitions the artifact to `published`.
11. The platform picks up the artifact by content hash on the next workflow resolution.

`interpretation_notes` are required for:

- numeric thresholds
- vague legal terms
- exceptions
- cross-regime mappings
- discretionary standards
- any branch where source text does not mechanically imply the encoded condition

---

## 18. Observability and Audit Contract

Every production decision must be reconstructable.

### Runtime audit event fields

- artifact hash
- registry state at resolution time
- rule IDs evaluated
- decision trace
- source spans
- effective date
- jurisdiction resolver version
- runtime version
- compiler version
- IR schema version
- T0/T1/T2/T3/T4 status
- attestation IDs
- attestation policy version
- scenario/test corpus version where applicable
- workflow ID
- execution timestamp

### Audit reconstruction path

```text
workflow_id
  → artifact_hash
  → artifact bytes
  → rule trace
  → source spans
  → attestations
  → verification evidence
  → final decision
```

---

## 19. Migration Sequence

Six gates. Each gate produces a commit boundary. No gate begins until the prior gate's acceptance criteria are green.

### Gate 0 — Repo synthesis (pre-Rust)

**Scope:**

- Rename `applied-ai-regulatory-workbench` → `ke-workbench`.
- `git mv` frontend assets into `frontend/`.
- Preserve and path-update `Dockerfile`, `nginx.conf`, and `kube/` for the new `frontend/` layout.
- Preserve npm/package-lock in Gate 0; do not introduce pnpm.
- Create top-level scaffolding: `Cargo.toml`, `rust-toolchain.toml`, `crates/`, `fixtures/`, `docs/spec/`, `scripts/`, `.github/workflows/`.
- Write `CLAUDE.md` with repo invariants (§4.4).
- Copy `institutional-defi-platform-api/src/rules/data/` → `fixtures/rules/` through `scripts/bootstrap.sh` using the platform-repo access model in §4.5.
- Copy this spec to `docs/spec/ke-workbench-rust-migration-spec-v3.1.md`.
- Rewrite top-level `README.md` reflecting unified product narrative.
- Wire `.github/workflows/rust-ci.yml` (cargo fmt/clippy/test skeleton, no-op crates pass).
- Wire `.github/workflows/frontend-ci.yml` (existing npm pipeline, paths updated).
- Wire `.github/workflows/wasm-build.yml` as a no-op/stub workflow that validates future path structure only.
- Retire or path-update existing `.github/workflows/{ci.yml,cd-staging.yml,cd-production.yml}` explicitly.

**Acceptance:**

- Given a fresh clone, when `npm ci && npm run dev` runs in `frontend/`, then the existing UI loads against the platform's existing API (no regression).
- Given a fresh clone, when `cargo check --workspace` runs, then it succeeds against an empty workspace.
- Given the repo, when read top-to-bottom, then the README and `CLAUDE.md` explain the unified product without referring to the old repo name.
- Given the repo, when CI workflow files are inspected, then no workflow still assumes frontend files live at repo root.

**Non-goals for this gate:**

- Any Rust logic. All crates are empty stubs.
- Any frontend rewiring. The frontend is functionally unchanged from the seed repo.

### Gate 1 — Canonical IR and artifact foundations

**Scope:**

- `ke-core` IR types ported from `institutional-defi-platform-api/src/production/` (`RuleIR`, `CompiledCheck`, `DecisionEntry`, `ObligationSpec`, `ConditionGroupSpec`, `DecisionNode`, `DecisionLeaf`).
- Canonicalization rules defined in `docs/canonical-encoding.md`.
- JSON Schema emitted by `ke-core` build script.
- Golden fixtures generated from existing Python artifacts through the documented fixture-generation script and stored in `fixtures/artifacts/`.

**Acceptance:**

- Given Python-emitted artifacts, when converted into canonical Rust representation, then normalized semantic content matches expected fixtures.
- Given golden fixture bytes, when decoded and re-encoded by Rust, then artifact hash is stable.
- Given the JSON Schema, when consumed by platform model generation, then generation is deterministic.

### Gate 2 — Parser, compiler, structural verification

**Scope:**

- YAML → AST with source spans.
- AST → IR.
- T0/T1/T4 verification with conflict taxonomy initial classes.

**Acceptance:**

- Given every YAML rule in `fixtures/rules/`, when compiled by Rust and Python, then normalized IRs are semantically equivalent.
- Given boundary-value fixtures, when executed through both runtimes, then normalized traces and outcomes match.
- Given known conflict fixtures, when compiled, then T4 emits expected conflict class and severity.

### Gate 3 — Preview runtime and equivalence harness

**Scope:**

- Rust preview executor.
- Scenario scaffolding.
- Property-based and metamorphic tests.
- Fuzzed equivalence harness across Rust and Python runtimes.

**Acceptance:**

- Given existing trace fixtures, when executed by Rust preview runtime, then normalized public trace events match Python output.
- Given generated scenarios, when evaluated by both runtimes, then outputs, obligation sets, error classes, and normalized traces are equivalent.
- Given metamorphic transformations, when semantics should be unchanged, then outputs remain stable.

**Coverage targets:** operator, boundary-value, obligation, date-window, jurisdiction/scope, historical regression.

### Gate 4 — Artifact, registry, attestation, platform unblock

**Scope:**

- `ke-artifact` canonical encoding, content addressing, compiler signature.
- Typed expert attestations.
- Registry state machine backed by the v1 S3 registry model.
- `ke-artifact-py` PyO3 binding.
- `ke-artifact-py` wheel published to the v1 S3-backed PEP 503 package index.
- Platform consumption by content hash (executed via the separate platform-repo brief documented in §14).

**Acceptance:**

- Given a signed artifact, when loaded by the platform, then the platform verifies hash, canonical encoding, compiler signature, required attestations, key validity, and registry state before execution.
- Given a known scenario, when the platform executes a Rust-compiled artifact, then output matches the current Python pipeline.
- Given missing, stale, revoked, or invalid attestations, when the platform attempts execution, then execution is rejected with a specific policy error.
- Given registry rollback, when a new workflow resolves by tag, then it resolves to the previous signed content hash.

**Hard stop:** everything past Gate 4 is incremental and parallelizable.

### Gate 5 — Surface rollout and frontend rewire

**Scope:**

- `ke-cli serve` REST + WebSocket server.
- `ke-wasm` and `frontend/src/wasm/` bindings.
- DuckDB SQL views.
- Flat-file export.
- Lint integration.
- Frontend rewire: existing pages migrated from external `VITE_API_URL` to local REST + WASM.
- Review-first UI components incrementally added.
- Minimum AI-provenance UI from §13 implemented behind `VITE_USE_REVIEW_UI`.

**Acceptance:**

- Given each surface enabled behind a feature flag, when it reads artifact data, then it reads from the canonical artifact or registry view.
- Given browser WASM preview output, when compared to authoritative compile output, then differences are explicitly surfaced and never silently published.
- Given SQL queries over metadata, when compared to artifact contents, then results match canonical artifact data.
- Given flat-file export, when imported offline, then signatures and hashes verify.
- Given the frontend, when every previously-working page is loaded post-rewire, then it functions against local Rust surfaces.

### Gate 6 — Production cutover

**Scope:**

- Platform repo cut over to artifact-based consumption.
- Temporal startup artifact pinning implemented via workflow input or startup activity result recorded in workflow history.
- Python KE module in `institutional-defi-platform-api` deprecated and removed.
- Workbench becomes single source of truth for rule authoring.

**Acceptance:**

- Given the platform's Temporal worker, when it resolves rules, then it resolves exclusively through the registry.
- Given the platform repo, when the Python KE module is removed, then no test or workflow regresses.
- Given any production decision, when audit reconstruction is invoked, then the path from §18 returns complete evidence.

---

## 20. Risks and Mitigations

### Semantic laundering through cryptographic provenance

Cryptographic signatures may make weak legal encoding appear authoritative.

*Mitigation:* Typed attestations, review-first UI, source coverage, scenario coverage, explicit expert overrides, visible distinction between compiler validity, ML evidence, AI suggestions, and legal attestation.

### T2/T3 publish gap

Publishing before ML consistency checks complete may allow suspicious artifacts into production.

*Mitigation:* Make T2/T3 explicit publication-policy inputs. Require strict pass or typed expert override for production environments.

### Duplicate runtime drift

Python and Rust runtimes diverge.

*Mitigation:* Trace fixtures, property-based tests, metamorphic tests, operator coverage, CI equivalence harnesses in both repos.

The equivalence boundary is observable semantics, not incidental internal execution order. Gate 3 requires:

- identical final outcomes
- identical obligation sets
- identical rule/branch decisions after trace normalization
- identical error classes for invalid scenarios

Step-by-step trace parity applies only to normalized trace events that are part of the public audit contract. Rust may use internal optimizations that reorder private evaluation steps if the normalized trace and final result remain equivalent.

### DSL ontology mismatch

The DSL cannot faithfully encode discretionary or standards-based regulation.

*Mitigation:* Before Gate 2, walk MiCA, FSMA UK, and the GENIUS Act with a domain expert. Identify rules that resist conditional encoding. Extend the DSL before hardening the compiler.

### Expert key compromise or stale attestation

Compromised, replayed, or stale attestations bypass semantic gates.

*Mitigation:* Hardware-backed keys or IdP-backed signing, trusted timestamping, key revocation, attestation expiration, source-hash binding, registry-time plus runtime-time verification.

### Schema and canonicalization drift

Rust and Python agree on nominal schema but compute different hashes or semantics.

*Mitigation:* Canonical encoding spec, golden test vectors, JSON Schema generation, cross-language hash verification, contract tests.

### WASM authority confusion

Browser preview output is mistaken for authoritative compiled output.

*Mitigation:* Label WASM as preview-only, exclude signing and publication from browser code, require registry-side authoritative compilation for publish.

### Frontend regression during synthesis

The frontend breaks during Gate 0 restructure or Gate 5 rewire.

*Mitigation:* Gate 0 preserves the frontend's existing dependency on `VITE_API_URL` so the seed UI continues to ship while Rust crates land. Gate 5 rewires page-by-page behind feature flags with the prior code path retained until each page's local-surface variant passes parity tests.

### Crate over-fragmentation

Ten crates too early create coordination overhead.

*Mitigation:* Start with six crates and split only when dependency boundaries become stable. Deferred splits land in `crates-deferred/` when justified.

---

## 21. Open Decisions

1. **Expert key authority:** Self-managed hardware keys, org-issued PKI, IdP-backed signing, or managed HSM.
2. **T2/T3 production policy:** Strict block, expert override, or advisory for initial launch.
3. **T2/T3 sidecar deployment:** Platform-owned package, platform-owned service, or extracted verification service. V1 ownership remains platform-side, but deployment shape must be selected before Gate 4.
4. **Legal source text storage:** Hash-only, encrypted object storage, or indexed source text with copyright controls. Whole-document source coverage is blocked until this is resolved.
5. **Trusted timestamp authority:** RFC 3161 provider, internal TSA, or other approved authority. Mock TSA is local-dev only.
6. **Revocation behavior:** Hard stop vs finish-pinned vs audit-only for already-running workflows.
7. **Review UI follow-up scope:** Source coverage visualization, counterexample generation, and semantic diff are staged follow-ups unless explicitly promoted into Gate 5.
8. **Frontend visual regression tooling:** Playwright + Percy vs Chromatic + Storybook.
9. **Package-manager migration:** Whether npm remains long-term or a later ADR justifies pnpm.

### Resolved decisions in v3.1

- **Registry persistence v1:** S3-backed registry with content-hash objects, append-only lifecycle event objects, and S3-hosted manifest/tag objects. DynamoDB/Redis indexes are deferred until S3 manifest operations become a measured bottleneck.
- **`ke-artifact-py` package index v1:** S3-backed PEP 503 simple package index with exact version and hash pinning in the platform repo.

---

## 22. Claude Code Session Briefs

Each gate is executed by one or more Claude Code actor sessions, each scoped tightly with explicit deliverables, file paths, and acceptance commands. Briefs are stored in `dev/briefs/` as separate documents and referenced here as outlines.

### Brief structure (mandatory for every gate)

Every brief must include:

1. **Context block:** Pointers to this spec's relevant sections; pointers to relevant Python source paths.
2. **Phase 1 deliverables:** Files to create or modify, with paths.
3. **Phase 2 verification commands:** Exact shell commands the evaluator session runs.
4. **Acceptance criteria:** Given/when/then statements from §19.
5. **Known risks for this gate:** From §20.
6. **Out-of-scope clarifications:** What the actor must NOT do.
7. **Commit boundary:** Where Hossain commits manually; no auto-commit.
8. **Platform access:** Whether the brief requires `PLATFORM_REPO`; if yes, exact scripts and expected platform commit recording.

### Gate 0 brief outline — `dev/briefs/gate-0-repo-synthesis.md`

- **Context:** §3 (product synthesis), §4 (repo restructure), §4.4 (CLAUDE.md invariants), §4.5 (platform access), §4.6 (branching and in-flight work).
- **Phase 1 deliverables:**
  - Rename repo (manual; documented step for Hossain).
  - `git mv` frontend files (Phase 1 actor task; explicit file list).
  - Preserve and update `Dockerfile`, `nginx.conf`, and `kube/` paths for `frontend/`.
  - Preserve npm and `package-lock.json`; do not migrate to pnpm.
  - Create `Cargo.toml`, `rust-toolchain.toml`, six empty crate scaffolds.
  - Write `CLAUDE.md`.
  - Copy fixtures (script in `scripts/bootstrap.sh`).
  - Author top-level `README.md`.
  - Create `.github/workflows/{rust-ci,frontend-ci,wasm-build,contract-tests}.yml`, with `wasm-build.yml` and `contract-tests.yml` allowed to be no-op/stub workflows in Gate 0.
  - Retire or path-update existing `.github/workflows/{ci,cd-staging,cd-production}.yml`.
- **Phase 2 verification:**
  - `cargo check --workspace` succeeds.
  - `cd frontend && npm ci && npm run build` succeeds.
  - `npm run dev` loads against existing `VITE_API_URL`.
  - All CI workflows pass on a draft PR.
  - Existing deployment manifests and Docker/nginx paths no longer reference the old root-level frontend layout.
- **Out of scope:** Any Rust logic, any frontend changes beyond moves.

### Gate 1 brief outline — `dev/briefs/gate-1-canonical-ir.md`

- **Context:** §5 (architecture), §6 (crate layout), §8 (artifact contract), §11 (verification model).
- **Phase 1 deliverables:**
  - Port IR types from `institutional-defi-platform-api/src/production/{compiler.py,schemas.py}` into `crates/ke-core/src/ir/`.
  - Write canonicalization spec in `docs/canonical-encoding.md`.
  - Implement canonical serialization in `crates/ke-core/src/canonical.rs`.
  - Build script emits JSON Schema to `crates/ke-core/schema/ir.schema.json`.
  - Generate golden fixtures via the documented fixture-generation script; commit bytes to `fixtures/artifacts/`.
- **Phase 2 verification:**
  - `cargo test -p ke-core` passes.
  - Round-trip test: every golden fixture decodes, re-encodes, hashes identically.
  - JSON Schema generation is deterministic across runs, including field ordering, enum representation, `$defs` ordering, and reference names.
- **Out of scope:** Compiler, runtime, signing.

### Gate 2 brief outline — `dev/briefs/gate-2-parser-compiler-verification.md`

- **Context:** §11 (verification model), §12 (T4 taxonomy).
- **Phase 1 deliverables:**
  - `crates/ke-compiler/src/parser.rs` — YAML to AST with source spans.
  - `crates/ke-compiler/src/lower.rs` — AST to IR.
  - T0/T1 passes in `crates/ke-compiler/src/verify/`.
  - T4 implementation with initial conflict classes (`contradictory_outcome`, `overlapping_scope`, `temporal_overlap`, `duplicate_rule`).
- **Phase 2 verification:**
  - `cargo test -p ke-compiler` passes.
  - Differential test (`scripts/differential-test.sh`) green against every YAML in `fixtures/rules/`, invoking the Python compiler through §4.5 platform access.
  - T4 fixture tests emit correct conflict class and severity.
- **Out of scope:** Optimizer (deferred), source-text-divergence T4 class (requires `ke-search`), preview execution.

### Gate 3 brief outline — `dev/briefs/gate-3-preview-runtime.md`

- **Context:** §17 (authoring workflow), §20 (duplicate runtime drift).
- **Phase 1 deliverables:**
  - `crates/ke-runtime/src/exec.rs` — IR interpreter mirroring Python's `RuleRuntime`.
  - Scenario scaffolding in `crates/ke-runtime/src/scenario.rs`.
  - Property-based tests via `proptest`.
  - Fuzzed equivalence harness in `scripts/equivalence-harness.sh`.
- **Phase 2 verification:**
  - `cargo test -p ke-runtime` passes.
  - Equivalence harness green over N=1000 generated scenarios.
  - Normalized trace-fixture parity against Python output through §4.5 platform access.
- **Out of scope:** Production use; `ke-runtime` remains preview-only.

### Gate 4 brief outline — `dev/briefs/gate-4-artifact-registry-attestation.md`

- **Context:** §8 (artifact contract), §9 (lifecycle), §10 (attestation), §14 (cross-repo integration).
- **Phase 1 deliverables:**
  - `crates/ke-artifact/` complete (canonical encoding, BLAKE3, ed25519, attestations).
  - Initial registry implemented inside `ke-cli` against S3, with local filesystem allowed only for dev/test.
  - `crates/ke-artifact/src/python.rs` — PyO3 binding behind `pyo3` feature.
  - Packaged `ke-artifact-py` wheel published to the S3-backed PEP 503 simple index.
  - RFC 3161-compatible timestamp authority selected; local mock TSA restricted to dev/test.
  - T2/T3 sidecar deployment path selected and wired into publication evidence flow.
  - Coordination with platform-repo brief for consumer side (separate session).
- **Phase 2 verification:**
  - `cargo test -p ke-artifact` passes (including signature, attestation, rejection tests).
  - Cross-language contract test (`scripts/contract-test.sh`) green.
  - Platform-side test in the corresponding `institutional-defi-platform-api` PR shows end-to-end artifact load + execute matches Python pipeline.
- **Out of scope:** REST/WS surfaces, WASM, frontend rewire.

### Gate 5 brief outline — `dev/briefs/gate-5-surfaces-and-frontend.md`

- **Context:** §7 (frontend layout), §16 (multi-surface access), §17 (review workflow).
- **Phase 1 deliverables (parallelizable sub-briefs):**
  - 5a — `ke-cli serve`: axum REST + WebSocket feed.
  - 5b — `ke-wasm`: wasm-bindgen wrappers + frontend integration in `frontend/src/wasm/`.
  - 5c — DuckDB SQL view binary subcommand.
  - 5d — `.kew` flat-file export.
  - 5e — Lint integration into `ke-cli compile`.
  - 5f — Frontend rewire: page-by-page migration from external API to local surfaces.
  - 5g — Minimum AI-provenance UI: visual distinction and proposal inspection from §13.
- **Phase 2 verification:**
  - Each sub-brief has its own verification suite.
  - End-to-end smoke: `npm run dev` against `ke-cli serve` + WASM passes all existing page-level integration tests.
- **Out of scope:** Production cutover (Gate 6); review-first UI components beyond the minimum required for rewire parity.

### Gate 6 brief outline — `dev/briefs/gate-6-production-cutover.md`

- **Context:** §14 (cross-repo integration), §15 (runtime selection), §18 (audit).
- **Phase 1 deliverables:**
  - Platform-repo PR removing the Python KE module.
  - Temporal artifact pinning implemented as workflow input or startup activity result recorded in workflow history.
  - Workbench registry promoted to authoritative for all environments.
  - Audit reconstruction path tested end-to-end.
- **Phase 2 verification:**
  - Platform CI green with KE module removed.
  - No regression in any existing Temporal workflow.
  - Sample audit reconstruction returns full evidence trail.
- **Out of scope:** Anything not directly required for cutover.

---

## 23. Implementation Readiness Checklist

### Before Gate 0

- [ ] Confirm repo rename plan and GitHub coordination.
- [ ] Commit, stash, or branch current frontend work, including credit module and `DocumentIngestion`.
- [ ] Confirm per-gate branch naming and merge discipline.
- [ ] Confirm npm/Node version pinning; pnpm is deferred unless separately approved.
- [ ] Confirm Rust current-stable toolchain version (`rust-toolchain.toml`).
- [ ] Confirm `Dockerfile`, `nginx.conf`, `kube/`, and existing CI/CD workflow disposition.
- [ ] Confirm local platform checkout path or `PLATFORM_REPO` override.

### Before Gate 1

- [ ] Canonical encoding profile drafted in `docs/canonical-encoding.md`.
- [ ] JSON Schema deterministic-emission rules included in the canonical encoding profile.
- [ ] Artifact manifest fields finalized (§8.1).
- [ ] Golden fixture generation script tested in Python.
- [ ] Runtime policy shape drafted.

### Before Gate 2

- [ ] T4 conflict classes and severities accepted by domain reviewer.
- [ ] DSL gap review completed with at least three regimes.
- [ ] Source-span requirements finalized.
- [ ] Legal effective-date/time-zone representation finalized in IR.

### Before Gate 4

- [ ] Typed attestation schema finalized (§10).
- [ ] Key authority and revocation design selected (Open Decision §21.1).
- [ ] RFC 3161-compatible TSA or approved alternative selected (Open Decision §21.5).
- [ ] T2/T3 sidecar deployment path selected (Open Decision §21.3).
- [ ] T2/T3 publication policy selected (Open Decision §21.2).
- [ ] Platform rejection rules specified (§10).
- [ ] Rollback and revocation policies specified (§15).
- [ ] Temporal artifact pinning design reviewed with the platform-repo brief (§15).
- [ ] S3 registry bucket/key layout and S3 PEP 503 package-index layout documented.
- [ ] Platform-repo brief authored and reviewed.

### Before Gate 6

- [ ] Audit reconstruction path tested.
- [ ] Cross-language hash tests passing.
- [ ] Runtime trace equivalence passing.
- [ ] Missing/stale/revoked attestation rejection tests passing.
- [ ] Temporal workflow pinning integration passing in platform CI.

---

## 24. References

- `institutional-defi-platform-api/src/production/` — current Python compiler/runtime; source of truth for existing semantics.
- `institutional-defi-platform-api/src/rules/data/` — YAML corpus and equivalence fixtures.
- `applied-ai-regulatory-workbench` (seed repo) — React/Vite/D3 frontend; preserved through Gate 4, rewired in Gate 5.
- Existing five-tier consistency framework — retained as T0–T4 with stronger publication-policy semantics.
- Multi-surface access model — retained as adapter discipline over one canonical artifact.
- `ke-workbench-rust-migration-spec-v2.md` — predecessor spec; v3 superseded it.
- `ke-workbench-rust-migration-spec-v3.md` — predecessor spec; this v3.1 patches Gate 0, platform-repo, packaging, timestamping, and runtime-pinning ambiguities.
