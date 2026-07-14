# fixtures

Test fixtures consumed by Rust crates and cross-language equivalence checks.

- `rules/` — YAML rule corpus snapshotted from
  `institutional-defi-platform-api/src/rules/data/` via `scripts/bootstrap.sh`.
  Records the source platform commit SHA in `rules/SOURCE.md`.
- `traces/` — Python `RuleRuntime` trace outputs used by the equivalence harness
  in Gate 3.
- `artifacts/` — golden artifact bytes used to verify canonical encoding and
  cross-language hash stability (Gate 1+).
- `graph/` — the ADR-0023 graph-edge pin: `expected_edges.json` records the
  full edge set (extraction + recomputed T4 conflicts) over the goldens in
  `artifacts/`. Generated **only** by
  `cargo run -p ke-cli --bin gen-graph-fixture` (the sanctioned generator for
  this subdirectory — a cargo bin, mirroring `gen-golden-artifacts`); pinned
  by the ke-cli test `golden_edges_match_the_committed_fixture`. Regeneration
  on an unchanged tree is a byte-identical no-op.

This directory is **read-only inside ordinary Claude Code implementation
sessions**. Updates happen exclusively through the documented generation
tooling — the sync/generation scripts under `scripts/` and the generator
binaries named per-subdirectory above — which regenerate dependent fixtures
atomically. See spec § 4.4 and § 8.3.
