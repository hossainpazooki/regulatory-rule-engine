//! T1 — source-span coverage + required interpretation notes (blocking).
//!
//! Coverage is **legal-provenance-based, by inheritance** (ADR 0004, C2): a
//! decision node / obligation is covered if it carries its own legal
//! `SourceSpan` **or** inherits the mandatory rule-level `source:`. It is never
//! keyed off YAML line positions. Because rule-level `source.document_id` is
//! mandatory and non-empty (T0), a well-formed rule passes coverage; a finding
//! fires only if the rule itself has no source.
//!
//! Interpretation notes are required where source text does not mechanically
//! imply the encoded condition — for Gate 2, when a rule carries a numeric
//! threshold (`>`,`<`,`>=`,`<=` on a decimal) or a `not_in` exception (spec §17).

use super::{Finding, Tier};
use ke_core::ir::condition::{
    Condition, ConditionGroupSpec, ConditionOrGroup, Operator, ScalarValue,
};
use ke_core::ir::decision::DecisionEntry;
use ke_core::ir::rule::RuleIR;

pub fn check(rule: &RuleIR) -> Vec<Finding> {
    let mut out = Vec::new();

    // Coverage by inheritance from the rule-level source.
    let rule_has_source = !rule.source.document_id.trim().is_empty();
    if !rule_has_source && !fully_self_covered(&rule.decision_tree) {
        out.push(Finding {
            tier: Tier::T1,
            rule_id: rule.rule_id.clone(),
            code: "T1_source_coverage",
            message: "decision nodes lack legal source coverage and the rule has no `source:`"
                .to_string(),
            blocking: true,
        });
    }

    // Required interpretation notes for thresholds / exceptions.
    if rule.interpretation_notes.is_none() && rule_has_threshold_or_exception(rule) {
        out.push(Finding {
            tier: Tier::T1,
            rule_id: rule.rule_id.clone(),
            code: "T1_missing_interpretation_notes",
            message: "rule uses a numeric threshold or `not_in` exception but has no \
                      interpretation_notes"
                .to_string(),
            blocking: true,
        });
    }

    out
}

/// True if every leaf/node carries its own legal `SourceSpan` (so the rule would
/// be covered even without a rule-level source). Used only when the rule has no
/// `source:`.
fn fully_self_covered(entry: &DecisionEntry) -> bool {
    match entry {
        DecisionEntry::Leaf(leaf) => leaf.source_span.is_some(),
        DecisionEntry::Node(node) => {
            node.source_span.is_some()
                && fully_self_covered(&node.true_branch)
                && fully_self_covered(&node.false_branch)
        }
    }
}

fn rule_has_threshold_or_exception(rule: &RuleIR) -> bool {
    rule.applies_if
        .as_ref()
        .map(group_has_threshold)
        .unwrap_or(false)
        || tree_has_threshold(&rule.decision_tree)
}

fn group_has_threshold(group: &ConditionGroupSpec) -> bool {
    group
        .all
        .iter()
        .chain(group.any.iter())
        .flatten()
        .any(|it| match it {
            ConditionOrGroup::Condition(c) => is_threshold(c),
            ConditionOrGroup::Group(g) => group_has_threshold(g),
        })
}

fn tree_has_threshold(entry: &DecisionEntry) -> bool {
    match entry {
        DecisionEntry::Leaf(_) => false,
        DecisionEntry::Node(node) => {
            is_threshold(&node.condition)
                || tree_has_threshold(&node.true_branch)
                || tree_has_threshold(&node.false_branch)
        }
    }
}

fn is_threshold(c: &Condition) -> bool {
    match c.operator {
        Operator::Gt | Operator::Lt | Operator::Gte | Operator::Lte => {
            matches!(c.value, ScalarValue::Decimal { .. })
        }
        Operator::NotIn => true,
        _ => false,
    }
}
