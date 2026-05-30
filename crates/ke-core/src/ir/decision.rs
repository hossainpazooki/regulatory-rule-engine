//! The decision tree. Ported from the platform's `DecisionNode` / `DecisionLeaf`
//! (`src/rules/service.py`). This is the *tree* shape; the flattening into a
//! jump-table (`CompiledCheck` / decision table) is Gate 2's lowering, not
//! Gate 1.

use super::condition::Condition;
use super::obligation::ObligationSpec;
use super::source_span::SourceSpan;
use serde::{Deserialize, Serialize};

/// Either an internal branch node or a terminal leaf. Externally tagged for
/// the same postcard reason as [`super::condition::ConditionOrGroup`]. Both
/// variants are boxed to keep the enum small (the variants carry large inline
/// `SourceSpan` payloads); boxing does not affect serde/postcard encoding.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecisionEntry {
    Node(Box<DecisionNode>),
    Leaf(Box<DecisionLeaf>),
}

/// A branch node: evaluate `condition`, descend into `true_branch` or
/// `false_branch`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionNode {
    pub node_id: String,
    pub condition: Condition,
    pub true_branch: DecisionEntry,
    pub false_branch: DecisionEntry,
    pub source_span: Option<SourceSpan>,
}

/// A terminal leaf: a decision result plus any obligations it imposes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionLeaf {
    pub result: String,
    pub obligations: Option<Vec<ObligationSpec>>,
    pub notes: Option<String>,
    pub source_span: Option<SourceSpan>,
}
