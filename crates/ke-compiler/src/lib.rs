//! ke-compiler: YAML → spanned AST → `ke_core::ir::RuleIR` + T0/T1/T4
//! verification (Gate 2).
//!
//! See `docs/spec/ke-workbench-rust-migration-spec-v3.1.md` §11, §12 and the
//! gate brief `dev/briefs/gate-2-parser-compiler-verification.md`.
//!
//! The compiler lowers to the **un-lowered authoring tree** (`RuleIR`); it does
//! not flatten to a jump-table (out of Gate 2). Equivalence with the platform
//! Python compiler is proven at a semantic-normal-form level (`ke_core::semantic`,
//! added in this gate's Phase 2).

#![deny(unsafe_code)]

pub mod ast;
pub mod error;
pub mod lower;
pub mod parser;
pub mod python_import;
pub mod value;
pub mod verify;

pub use error::CompileError;
use ke_core::ir::RuleIR;
use ke_core::semantic::SemanticRule;

/// Parse and lower a YAML document into one or more canonical rules.
pub fn compile_rules(source: &str) -> Result<Vec<RuleIR>, CompileError> {
    parser::parse_rules(source)?
        .iter()
        .map(lower::lower_rule)
        .collect()
}

/// Compile a YAML document straight to semantic normal forms (one per rule).
pub fn compile_to_semantic(source: &str) -> Result<Vec<SemanticRule>, CompileError> {
    Ok(compile_rules(source)?
        .iter()
        .map(SemanticRule::from_rule)
        .collect())
}
