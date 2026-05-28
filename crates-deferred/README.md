# crates-deferred

Placeholder for crate splits that the spec defers until interfaces stabilize.
See `docs/spec/ke-workbench-rust-migration-spec-v3.1.md` § 6.

Planned splits:

- `ke-search` — corpus indexing + T4 acceleration. Split when conflict-detection
  latency over the full corpus exceeds budget.
- `ke-registry` — once registry persistence and policy APIs stabilize. Initial
  registry logic lives inside `ke-artifact` and `ke-cli`.
- `ke-lint` — once lint classes stabilize beyond compiler verification.
- `ke-artifact-py` — may live alongside `ke-artifact` via a `pyo3` feature and
  split when packaging requires (see spec § 14).

Nothing here is compiled — Cargo workspace `members` only lists the six
active crates under `crates/`.
