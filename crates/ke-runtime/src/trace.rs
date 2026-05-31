//! The normalized public trace contract (spec §20, ADR 0008).
//!
//! Both runtimes reduce to the same shape: an ordered **evaluated-applicability
//! prefix** plus the **taken decision path**, each step a `{field, operator,
//! result}` triple. Operators are normalized to the YAML surface token set
//! below. Everything representation-dependent (timestamps, node ids, prose,
//! the concrete expected/actual values) is intentionally dropped — it is outside
//! the equivalence boundary.
//!
//! The Rust executor records its taken path directly (see `exec`). The Python
//! driver reconstructs the same shape from the matched decision-table entry's
//! `condition_mask` (the gate-3 equivalence harness, Phase 3).

use ke_core::ir::Operator;
use serde::{Deserialize, Serialize};

/// One normalized decision/applicability step.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormStep {
    pub field: String,
    /// The canonical YAML operator token (see [`op_token`]).
    pub operator: String,
    /// Whether this condition evaluated true.
    pub result: bool,
}

impl NormStep {
    pub fn new(field: impl Into<String>, op: Operator, result: bool) -> Self {
        NormStep {
            field: field.into(),
            operator: op_token(op).to_string(),
            result,
        }
    }
}

/// The canonical operator token. The Rust `Operator` already renders to these
/// via serde; the Python compiled tokens (`eq`/`ne`/`gt`/`lt`/`gte`/`lte`/`in`/
/// `not_in`/`exists`) are mapped to the same set by the Python driver. Keeping
/// the mapping explicit here makes the canonical set the single source of truth.
pub fn op_token(op: Operator) -> &'static str {
    match op {
        Operator::Eq => "==",
        Operator::NotEq => "!=",
        Operator::In => "in",
        Operator::NotIn => "not_in",
        Operator::Gt => ">",
        Operator::Lt => "<",
        Operator::Gte => ">=",
        Operator::Lte => "<=",
        Operator::Exists => "exists",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_the_yaml_surface_set() {
        assert_eq!(op_token(Operator::Eq), "==");
        assert_eq!(op_token(Operator::NotEq), "!=");
        assert_eq!(op_token(Operator::In), "in");
        assert_eq!(op_token(Operator::NotIn), "not_in");
        assert_eq!(op_token(Operator::Gte), ">=");
        assert_eq!(op_token(Operator::Exists), "exists");
    }
}
