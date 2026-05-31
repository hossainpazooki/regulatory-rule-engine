//! The semantic normal form of a rule: a representation-independent reduction of
//! `RuleIR` to its *meaning*, used to compare a Rust-lowered rule against a
//! Python-loaded one (differential testing, Gate 2) and to detect
//! `duplicate_rule` conflicts (T4).
//!
//! What it abstracts away (incidental, not semantic — spec §20):
//! - `all`/`any` member order (AND/OR are commutative → sorted),
//! - decision-node ids and tree shape (reduced to the **set of root-to-leaf
//!   decision paths**),
//! - decimal representation (`0.90` ≡ `0.9` → trailing zeros stripped),
//! - `in`/`not_in` operand order (set membership → sorted),
//! - free-form prose (descriptions, notes) and obligation descriptions
//!   (obligations compared by **id**),
//! - the jurisdiction time zone (a Gate-2 placeholder, ADR 0006).
//!
//! What it keeps (the semantic contract): rule id, applicability predicate,
//! decision paths + outcomes (result + obligation ids), legal source **document**
//! (provenance via `DocumentRef`, never a YAML span — C2), and the effective
//! window, **which stays optional** (Rust `None` ≡ Python `None` — C1).

use crate::ir::condition::{
    Condition, ConditionGroupSpec, ConditionOrGroup, Operator, ScalarValue,
};
use crate::ir::decision::DecisionEntry;
use crate::ir::rule::RuleIR;
use serde::{Deserialize, Serialize};

/// Representation-independent meaning of a rule.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SemanticRule {
    pub rule_id: String,
    pub applicability: Option<SemPredicate>,
    /// Set of root-to-leaf decision paths, in canonical order.
    pub decision_paths: Vec<SemPath>,
    /// Legal source document id (provenance), not a YAML span (C2).
    pub source_document: String,
    /// Effective window — optional (C1). Dates only; the placeholder tz is dropped.
    pub effective: Option<SemWindow>,
    /// Canonically sorted tag set.
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SemPredicate {
    All(Vec<SemPredicate>),
    Any(Vec<SemPredicate>),
    Cond(SemCond),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SemCond {
    pub field: String,
    pub operator: String,
    pub value: SemValue,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SemValue {
    Str(String),
    Bool(bool),
    /// Canonical decimal: no trailing zeros, non-negative scale.
    Num {
        mantissa: i128,
        scale: i8,
    },
    /// Operand list, canonically sorted (set semantics).
    List(Vec<SemValue>),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SemPath {
    pub branches: Vec<SemBranch>,
    pub outcome: SemOutcome,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SemBranch {
    pub cond: SemCond,
    pub taken: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SemOutcome {
    pub result: String,
    /// Obligation ids only (descriptions are prose, not semantic), sorted.
    pub obligations: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SemWindow {
    pub from: (i16, u8, u8),
    pub to: Option<(i16, u8, u8)>,
}

impl SemanticRule {
    /// Reduce a `RuleIR` to its semantic normal form.
    pub fn from_rule(rule: &RuleIR) -> Self {
        let mut tags = rule.tags.clone().unwrap_or_default();
        tags.sort();
        tags.dedup();

        let mut decision_paths = Vec::new();
        collect_paths(&rule.decision_tree, &mut Vec::new(), &mut decision_paths);
        decision_paths.sort();

        SemanticRule {
            rule_id: rule.rule_id.clone(),
            applicability: rule.applies_if.as_ref().map(sem_predicate),
            decision_paths,
            source_document: rule.source.document_id.clone(),
            effective: rule.effective_window.as_ref().map(|w| SemWindow {
                from: (
                    w.effective_from.year,
                    w.effective_from.month,
                    w.effective_from.day,
                ),
                to: w.effective_to.map(|d| (d.year, d.month, d.day)),
            }),
            tags,
        }
    }
}

fn sem_predicate(group: &ConditionGroupSpec) -> SemPredicate {
    if let Some(items) = &group.all {
        let mut v: Vec<SemPredicate> = items.iter().map(sem_item).collect();
        v.sort();
        SemPredicate::All(v)
    } else if let Some(items) = &group.any {
        let mut v: Vec<SemPredicate> = items.iter().map(sem_item).collect();
        v.sort();
        SemPredicate::Any(v)
    } else {
        SemPredicate::All(Vec::new())
    }
}

fn sem_item(item: &ConditionOrGroup) -> SemPredicate {
    match item {
        ConditionOrGroup::Condition(c) => SemPredicate::Cond(sem_cond(c)),
        ConditionOrGroup::Group(g) => sem_predicate(g),
    }
}

fn sem_cond(c: &Condition) -> SemCond {
    SemCond {
        field: c.field.clone(),
        operator: op_token(c.operator).to_string(),
        value: sem_value(&c.value),
    }
}

fn sem_value(v: &ScalarValue) -> SemValue {
    match v {
        ScalarValue::Str(s) => SemValue::Str(s.clone()),
        ScalarValue::Bool(b) => SemValue::Bool(*b),
        ScalarValue::Decimal { mantissa, scale } => {
            let (m, s) = norm_decimal(*mantissa, *scale);
            SemValue::Num {
                mantissa: m,
                scale: s,
            }
        }
        ScalarValue::List(items) => {
            let mut out: Vec<SemValue> = items.iter().map(sem_value).collect();
            out.sort();
            SemValue::List(out)
        }
    }
}

fn collect_paths(entry: &DecisionEntry, acc: &mut Vec<SemBranch>, out: &mut Vec<SemPath>) {
    match entry {
        DecisionEntry::Leaf(leaf) => {
            let mut obligations: Vec<String> = leaf
                .obligations
                .as_ref()
                .map(|v| v.iter().map(|o| o.id.clone()).collect())
                .unwrap_or_default();
            obligations.sort();
            obligations.dedup();
            out.push(SemPath {
                branches: acc.clone(),
                outcome: SemOutcome {
                    result: leaf.result.clone(),
                    obligations,
                },
            });
        }
        DecisionEntry::Node(node) => {
            let cond = sem_cond(&node.condition);
            acc.push(SemBranch {
                cond: cond.clone(),
                taken: true,
            });
            collect_paths(&node.true_branch, acc, out);
            acc.pop();
            acc.push(SemBranch { cond, taken: false });
            collect_paths(&node.false_branch, acc, out);
            acc.pop();
        }
    }
}

/// Stable token for an operator (its YAML surface form).
fn op_token(op: Operator) -> &'static str {
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

/// Canonical decimal form: fold negative scale into the mantissa, strip trailing
/// zeros, and map `0` to scale `0`. Mirrors the canonical-encoding rule so the
/// semantic form agrees with the wire form.
fn norm_decimal(mantissa: i128, scale: i8) -> (i128, i8) {
    let mut m = mantissa;
    let mut s = scale;
    while s < 0 {
        m = m.saturating_mul(10);
        s += 1;
    }
    while s > 0 && m != 0 && m % 10 == 0 {
        m /= 10;
        s -= 1;
    }
    if m == 0 {
        s = 0;
    }
    (m, s)
}
