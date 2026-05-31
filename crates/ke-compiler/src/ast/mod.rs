//! The spanned AST produced by [`crate::parser`]. It mirrors the YAML rule
//! shape and carries `YamlSpan`s on the nodes that diagnostics and tests care
//! about (rule id, applicability, conditions, decision nodes/leaves,
//! obligations). Lowering ([`crate::lower`]) consumes it into
//! `ke_core::ir::RuleIR`, dropping the spans.
//!
//! Condition values are already typed (`ke_core::ir::ScalarValue`) at parse time
//! via [`crate::value`]; everything else is kept close to the YAML surface.

pub mod span;

pub use span::{Position, YamlSpan};

use ke_core::ir::ScalarValue;

/// A value paired with its YAML source span.
#[derive(Clone, Debug)]
pub struct Spanned<T> {
    pub value: T,
    pub span: YamlSpan,
}

impl<T> Spanned<T> {
    pub fn new(value: T, span: YamlSpan) -> Self {
        Self { value, span }
    }
}

/// A whole rule, spanned.
#[derive(Clone, Debug)]
pub struct AstRule {
    pub span: YamlSpan,
    pub rule_id: Spanned<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    /// ISO `YYYY-MM-DD` strings as written; parsed to dates in lowering.
    pub effective_from: Option<String>,
    pub effective_to: Option<String>,
    /// Rule-level jurisdiction code if present (e.g. `UK`, `CH`).
    pub jurisdiction: Option<String>,
    pub applies_if: Option<Spanned<AstGroup>>,
    pub decision_tree: Spanned<AstDecision>,
    /// Top-level obligations (rare; most live on decision leaves).
    pub obligations: Vec<Spanned<AstObligation>>,
    pub source: Spanned<AstSource>,
    pub interpretation_notes: Option<String>,
}

/// `all` (AND) or `any` (OR).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupKind {
    All,
    Any,
}

#[derive(Clone, Debug)]
pub struct AstGroup {
    pub kind: GroupKind,
    pub items: Vec<Spanned<AstGroupItem>>,
}

#[derive(Clone, Debug)]
pub enum AstGroupItem {
    Condition(AstCondition),
    Group(AstGroup),
}

#[derive(Clone, Debug)]
pub struct AstCondition {
    pub field: Spanned<String>,
    /// Raw YAML operator string (e.g. `"=="`, `"in"`); validated in lowering.
    pub operator: Spanned<String>,
    pub value: ScalarValue,
    pub description: Option<String>,
}

#[derive(Clone, Debug)]
pub enum AstDecision {
    Node(Box<AstNode>),
    Leaf(AstLeaf),
}

#[derive(Clone, Debug)]
pub struct AstNode {
    pub node_id: Option<String>,
    pub condition: Option<Spanned<AstCondition>>,
    pub true_branch: Option<Spanned<AstDecision>>,
    pub false_branch: Option<Spanned<AstDecision>>,
}

#[derive(Clone, Debug)]
pub struct AstLeaf {
    pub result: Spanned<String>,
    pub obligations: Vec<Spanned<AstObligation>>,
    pub notes: Option<String>,
}

#[derive(Clone, Debug)]
pub struct AstObligation {
    pub id: Spanned<String>,
    pub description: Option<String>,
    pub deadline: Option<String>,
}

#[derive(Clone, Debug)]
pub struct AstSource {
    pub document_id: String,
    pub article: Option<String>,
    pub section: Option<String>,
    pub paragraphs: Vec<String>,
    pub pages: Vec<u32>,
}
