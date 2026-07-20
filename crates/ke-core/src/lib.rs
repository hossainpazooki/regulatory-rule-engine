//! ke-core: IR types, canonical encoding, and JSON Schema emission for
//! ke-workbench.
//!
//! Gate 1 deliverable. This crate *freezes the data shapes* every later gate
//! targets. It deliberately contains **no** YAML parser, AST→IR lowering,
//! verification (T0/T1/T4), runtime, signing, PyO3, or WASM — those are
//! Gates 2–5. See `docs/spec/ke-workbench-rust-migration-spec-v3.1.md` § 6, § 8
//! and the gate brief `docs/gate-1-canonical-ir.md`.
//!
//! Three sub-systems:
//! - [`ir`] — the intermediate-representation types (the un-lowered authoring
//!   tree ported from the platform's `src/rules/service.py`).
//! - [`canonical`] — deterministic byte encoding (postcard + an explicit
//!   ordering/normalization profile) and a strict decoder. See
//!   `docs/canonical-encoding.md`.
//! - [`schema`] — deterministic JSON Schema emission for downstream consumers.
//!
//! The version triplet that pins all three lives in [`version`].

#![deny(unsafe_code)]

pub mod canonical;
pub mod examples;
pub mod ir;
pub mod manifest;
pub mod revocation;
pub mod schema;
pub mod semantic;
pub mod version;

pub use version::{
    CanonicalizationVersion, CodecVersion, SchemaVersion, CANONICALIZATION_VERSION, CODEC_VERSION,
    IR_SCHEMA_VERSION,
};
