//! `CompiledCheck` — a **placeholder** for the flattened T0/T1 check artifact.
//!
//! Gate 1 defines the carrier shape only. The semantics — jump targets,
//! pre-computed value sets, the AST→IR flattening that produces these — are
//! Gate 2's compiler work (brief § 1 non-goals). It is intentionally *not* a
//! field of [`super::rule::RuleIR`] in Gate 1; the un-lowered tree is the
//! authoring representation.

use super::condition::{Operator, ScalarValue};
use serde::{Deserialize, Serialize};

/// A single flattened condition check. Placeholder shape; Gate 2 fills in
/// control-flow (`on_true` / `on_false`) and `value_set` acceleration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledCheck {
    /// Position in the check sequence.
    pub index: u32,
    pub field: String,
    pub operator: Operator,
    pub value: ScalarValue,
}
