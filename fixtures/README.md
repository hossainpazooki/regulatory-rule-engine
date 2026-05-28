# fixtures

Test fixtures consumed by Rust crates and cross-language equivalence checks.

- `rules/` — YAML rule corpus snapshotted from
  `institutional-defi-platform-api/src/rules/data/` via `scripts/bootstrap.sh`.
  Records the source platform commit SHA in `rules/SOURCE.md`.
- `traces/` — Python `RuleRuntime` trace outputs used by the equivalence harness
  in Gate 3.
- `artifacts/` — golden artifact bytes used to verify canonical encoding and
  cross-language hash stability (Gate 1+).

This directory is **read-only inside ordinary Claude Code implementation
sessions**. Updates happen exclusively through the documented sync/generation
scripts under `scripts/`, which regenerate dependent fixtures atomically.
See spec § 4.4 and § 8.3.
