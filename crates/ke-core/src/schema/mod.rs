//! Deterministic JSON Schema emission.
//!
//! The emitted schema is the **authoritative** documentation of the IR wire
//! shape for downstream consumers — platform Pydantic/msgspec model generation
//! and frontend type generation (brief principle 6, § 9.2). Schema determinism
//! is therefore a cross-language correctness invariant, not a cosmetic CI gate:
//! `crates/ke-core/schema/ir.schema.json` is committed and CI fails on any
//! `git diff` after regeneration.
//!
//! Generate the committed file with `cargo run -p ke-core --bin emit-schema`.

pub mod defs;
pub mod emit;

pub use emit::{emit_schema, emit_schema_string};
