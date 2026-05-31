//! ke-runtime: preview executor + scenario/trace harness.
//!
//! **Preview-only.** Production rule execution is the Python `RuleRuntime` in
//! `institutional-defi-platform-api` (spec §2 non-goals, §17, §20). This crate
//! exists for fast CLI/browser dry-run, scenario tracing, and as the differential
//! oracle that keeps the Rust and Python engines from drifting.
//!
//! The executor walks the **authoring tree** (`ke_core::ir::RuleIR`) and is built
//! to be **observationally equivalent** to the Python runtime's
//! flattened-decision-table execution — identical outcomes, obligation id-sets,
//! normalized traces, and error classes (spec §20; the boundary is pinned in
//! ADR 0008). It is deliberately date-agnostic in that boundary (Python's runtime
//! never evaluates effective windows; ADR 0007).
//!
//! The executor lib depends only on `ke-core`, so it stays wasm-clean for the
//! Gate 5 browser dry-run. Native developer tooling (the `ke-eval` /
//! `gen-scenarios` binaries, YAML lowering, scenario generation) sits behind the
//! default `tools` feature.

#![deny(unsafe_code)]

pub mod compare;
pub mod effective;
pub mod exec;
pub mod scenario;
pub mod trace;
pub mod value;

pub use effective::effective_at;
pub use exec::{evaluate, Evaluation, Mode, Obligation};
pub use scenario::{generate_for_rule, Scenario};
pub use trace::{op_token, NormStep};
pub use value::{facts_from_json, lookup, FactValue, Facts};
