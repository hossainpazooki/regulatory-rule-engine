//! T0 — structural / required-field checks (blocking). Most malformed input is
//! already rejected at parse/lower time; T0 formalizes the structural invariants
//! that a *lowered* `RuleIR` could still violate (e.g. empty required strings,
//! empty condition groups).

use super::{Finding, Tier};
use ke_core::ir::condition::ConditionGroupSpec;
use ke_core::ir::decision::DecisionEntry;
use ke_core::ir::rule::RuleIR;

pub fn check(rule: &RuleIR) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut push = |code: &'static str, message: &str| {
        out.push(Finding {
            tier: Tier::T0,
            rule_id: rule.rule_id.clone(),
            code,
            message: message.to_string(),
            blocking: true,
        });
    };

    if rule.rule_id.trim().is_empty() {
        push("T0_empty_rule_id", "rule_id is empty");
    }
    if rule.rule_version.trim().is_empty() {
        push("T0_empty_version", "rule_version is empty");
    }
    if rule.source.document_id.trim().is_empty() {
        push("T0_empty_source_document", "source.document_id is empty");
    }
    if let Some(group) = &rule.applies_if {
        if group_is_empty(group) {
            push(
                "T0_empty_applies_if",
                "applies_if has an empty all/any group",
            );
        }
    }
    check_leaves(&rule.decision_tree, &mut push);

    out
}

fn group_is_empty(group: &ConditionGroupSpec) -> bool {
    let all_empty = group.all.as_ref().map(|v| v.is_empty()).unwrap_or(true);
    let any_empty = group.any.as_ref().map(|v| v.is_empty()).unwrap_or(true);
    all_empty && any_empty
}

fn check_leaves(entry: &DecisionEntry, push: &mut impl FnMut(&'static str, &str)) {
    match entry {
        DecisionEntry::Leaf(leaf) => {
            if leaf.result.trim().is_empty() {
                push("T0_empty_result", "a decision leaf has an empty result");
            }
        }
        DecisionEntry::Node(node) => {
            check_leaves(&node.true_branch, push);
            check_leaves(&node.false_branch, push);
        }
    }
}
