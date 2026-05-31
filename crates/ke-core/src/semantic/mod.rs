//! Semantic diff helpers (spec §6): a representation-independent normal form for
//! rules and a field-by-field diff over it. Used by the Gate 2 differential
//! harness (Rust vs Python parity) and by T4 `duplicate_rule` detection.

pub mod diff;
pub mod form;

pub use diff::{semantic_diff, Difference};
pub use form::{
    SemBranch, SemCond, SemOutcome, SemPath, SemPredicate, SemValue, SemWindow, SemanticRule,
};
