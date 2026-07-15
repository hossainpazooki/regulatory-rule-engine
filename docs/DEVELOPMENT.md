# Development

Build, test, and harness reference. (Moved from the README 2026-07-15;
commands unchanged.)

## Rust workspace

Toolchain is pinned to Rust 1.85.0 (`rust-toolchain.toml`). Install via
[rustup](https://rustup.rs/); `rustup` puts `cargo` under `~/.cargo/bin`,
which a fresh shell (e.g. MINGW64 / Git Bash) may not have on `PATH`:

```bash
source "$HOME/.cargo/env"        # or: export PATH="$HOME/.cargo/bin:$PATH"
```

```bash
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings

# regenerate the committed JSON Schema / golden fixtures (must be byte-stable)
cargo run -p ke-core --bin emit-schema
cargo run -p ke-core --bin gen-fixtures

# compile a rule file and emit its semantic normal form
cargo run -p ke-compiler --bin ke-compile -- compile fixtures/rules/mica_stablecoin.yaml
```

## Frontend

```bash
cd frontend
npm ci
npm run dev          # http://localhost:5173
```

Set `VITE_API_URL` to point at a running backend instance, or use the
default `/api` proxy. The local Rust surfaces (`ke serve`, WASM preview) sit
behind default-off `VITE_USE_*` flags (ADR-0020).

## Harnesses

| Harness | Command | Notes |
|---|---|---|
| Gate-2 differential (Rust↔Python parity) | `./scripts/differential-test.sh` | SHA-gated: requires the platform repo checked out at the SHA recorded in [`fixtures/rules/SOURCE.md`](../fixtures/rules/SOURCE.md); fails fast otherwise |
| Gate-3 equivalence (1,326 scenarios) | `./scripts/equivalence-harness.sh` | same SHA gate |
| Gate-4 3-language contract test | `scripts/contract-test.sh` | CI-canonical (`contract-tests.yml`); Rust ≡ Python ≡ WASM over golden `.kew` |
| ADR-0023 graph differential | `bash scripts/graph-differential.sh` | non-gating; needs docker (Neo4j 5) + cargo; ~2–5 min; self-cleaning |

The Gate-2/3 harnesses need the platform repo as a sibling:

```bash
git -C ../institutional-defi-platform-api checkout <recorded-SOURCE.md-SHA>
```

## Platform fixtures

Rules consumed by Gate 1+ live in `fixtures/rules/`, snapshotted from
`institutional-defi-platform-api/src/rules/data/` via:

```bash
./scripts/bootstrap.sh
```

The script expects `institutional-defi-platform-api` as a sibling of
`ke-workbench`, or `PLATFORM_REPO` set explicitly. Provenance is recorded in
[`fixtures/rules/SOURCE.md`](../fixtures/rules/SOURCE.md). See spec § 4.5.
`fixtures/` is read-only inside ordinary sessions — updates flow only through
the documented sync/generation scripts ([CLAUDE.md](../CLAUDE.md)).

## Repo layout

```text
ke-workbench/
├── Cargo.toml                   # Rust workspace root
├── rust-toolchain.toml          # pinned stable toolchain
├── Dockerfile                   # frontend image (build context = repo root)
├── nginx.conf                   # frontend reverse proxy
├── CLAUDE.md                    # session discipline + hard invariants
├── crates/
│   ├── ke-core/                 # IR, AST, canonicalization        (Gate 1)
│   ├── ke-compiler/             # YAML → IR + T0/T1/T4              (Gate 2)
│   ├── ke-runtime/               # preview executor (NOT prod)        (Gate 3)
│   ├── ke-artifact/              # canonical encoding + signatures + PyO3 binding (Gate 4)
│   ├── ke-cli/                  # ke compile/verify/attest/lint/export/import/sql/graph; serve   (Gate 4; serve 5a, 5b-data, 5c; graph ADR-0023)
│   └── ke-wasm/                  # browser verify (G4) + preview (G5b) (Gate 4/5)
├── crates-deferred/             # placeholder README only — absorbed or dropped
├── frontend/                    # React 18 + TypeScript + Vite + D3.js
├── fixtures/
│   ├── rules/                   # YAML corpus snapshot + SOURCE.md
│   ├── traces/                  # Python runtime traces (Gate 3+)
│   ├── artifacts/               # golden artifact bytes (Gate 1+; canon-5, incl. intentspec_payment)
│   └── graph/                   # generated expected-edges pin (ADR-0023; gen-graph-fixture only)
├── dev/briefs/                  # per-gate Claude Code session briefs (Gate 2+)
├── docs/                        # see docs/README.md for the index
├── scripts/
│   ├── bootstrap.sh             # snapshot platform rules → fixtures/rules/
│   ├── generate-golden-fixtures.sh # Gate 1 golden fixtures (synthetic mode)
│   ├── differential-test.sh     # Gate 2: Rust↔Python parity (SHA-gated)
│   ├── equivalence-harness.sh   # Gate 3
│   ├── contract-test.sh         # Gate 4: 3-language contract test
│   └── graph-differential.sh    # ADR-0023: Neo4j↔Rust differential (non-gating)
├── kube/                        # Kubernetes manifests (frontend)
└── .github/workflows/           # rust-ci, frontend-ci, wasm-build, contract-tests, cd-*
```
