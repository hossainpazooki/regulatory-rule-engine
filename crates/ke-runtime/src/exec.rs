//! The preview executor: walks the authoring tree IR against a fact set, mirroring
//! the Python `RuleRuntime.infer` observable semantics (ADR 0008).
//!
//! Applicability mirrors the Python compiler's **flattening** (`_flatten_conditions`
//! in `src/production/compiler.py`): nested `all`/`any` group members are spliced
//! under the **parent** mode and the nested mode is discarded. This deliberately
//! differs from the Gate-2 semantic form (which recurses); for execution parity
//! the runtime must follow the executor. The decision tree is walked node-by-node
//! to the reached leaf — observationally equal to Python's first-matching
//! decision-table entry (the matched entry's mask constrains exactly the taken
//! path; untaken-branch checks are wildcards).
//!
//! `evaluate` is **total**: it never errors (the Python executor never raises).

use crate::compare;
use crate::trace::NormStep;
use crate::value::{lookup, Facts};
use ke_core::ir::{
    Condition, ConditionGroupSpec, ConditionOrGroup, DecisionEntry, DecisionLeaf, RuleIR,
};
use serde::{Deserialize, Serialize};

/// Applicability combination mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    All,
    Any,
}

/// An obligation imposed by the matched leaf (id/description/deadline mirror the
/// Python `DecisionResult` obligation dict). Equivalence compares the **id set**.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Obligation {
    pub id: String,
    pub description: Option<String>,
    pub deadline: Option<String>,
}

/// The full evaluation result: the decision outcome plus the normalized trace.
/// This single struct is both the `DecisionResult` analogue and the public
/// normalized-trace contract the equivalence harness compares.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evaluation {
    pub rule_id: String,
    pub applicable: bool,
    /// The matched leaf's `result`; `None` when not applicable.
    pub decision: Option<String>,
    /// Obligations from the matched leaf only (not rule-level obligations).
    pub obligations: Vec<Obligation>,
    /// Applicability checks actually evaluated, in order (short-circuit aware).
    pub applicability_steps: Vec<NormStep>,
    /// The root-to-leaf branch decisions actually taken.
    pub decision_path: Vec<NormStep>,
}

impl Evaluation {
    /// The obligation id set, sorted and deduplicated — the comparison key for
    /// equivalence (spec §20 "identical obligation sets").
    pub fn obligation_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.obligations.iter().map(|o| o.id.clone()).collect();
        ids.sort();
        ids.dedup();
        ids
    }

    /// The compact normalized form the equivalence harness compares against the
    /// Python side: obligations as a sorted id set; representation-dependent
    /// fields (descriptions, deadlines) dropped. `rule_id` is carried at the
    /// enclosing line level, not here, so the object is field-comparable to the
    /// Python normalized object.
    pub fn normalized_json(&self) -> serde_json::Value {
        serde_json::json!({
            "applicable": self.applicable,
            "decision": self.decision,
            "obligations": self.obligation_ids(),
            "applicability_steps": self.applicability_steps,
            "decision_path": self.decision_path,
        })
    }
}

/// Evaluate a rule against facts. Total — never panics, never errors.
pub fn evaluate(rule: &RuleIR, facts: &Facts) -> Evaluation {
    let mut applicability_steps = Vec::new();
    let applicable = applicability(&rule.applies_if, facts, &mut applicability_steps);
    if !applicable {
        return Evaluation {
            rule_id: rule.rule_id.clone(),
            applicable: false,
            decision: None,
            obligations: Vec::new(),
            applicability_steps,
            decision_path: Vec::new(),
        };
    }

    let mut decision_path = Vec::new();
    let leaf = walk(&rule.decision_tree, facts, &mut decision_path);
    let obligations = leaf
        .obligations
        .as_ref()
        .map(|os| {
            os.iter()
                .map(|o| Obligation {
                    id: o.id.clone(),
                    description: o.description.clone(),
                    deadline: o.deadline.clone(),
                })
                .collect()
        })
        .unwrap_or_default();

    Evaluation {
        rule_id: rule.rule_id.clone(),
        applicable: true,
        decision: Some(leaf.result.clone()),
        obligations,
        applicability_steps,
        decision_path,
    }
}

/// Evaluate applicability, mirroring `RuleRuntime.check_applicability`.
fn applicability(
    applies_if: &Option<ConditionGroupSpec>,
    facts: &Facts,
    steps: &mut Vec<NormStep>,
) -> bool {
    // No applicability group → applicable (executor: empty checks → True).
    let Some(group) = applies_if else {
        return true;
    };

    let mut checks: Vec<&Condition> = Vec::new();
    flatten(group, &mut checks);

    // `if not ir.applicability_checks: return True` wins over mode selection.
    if checks.is_empty() {
        return true;
    }

    let mode = group_mode(group);
    for c in &checks {
        let result = compare::evaluate(lookup(facts, &c.field), c.operator, &c.value);
        steps.push(NormStep::new(c.field.as_str(), c.operator, result));
        match mode {
            Mode::All if !result => return false,
            Mode::Any if result => return true,
            _ => {}
        }
    }
    // No short-circuit: `all` → every check was true; `any` → none was true.
    mode == Mode::All
}

/// Flatten an applicability group to `(linear checks, mode)` exactly as the
/// executor sees it. Exposed so the scenario generator satisfies/violates
/// applicability the same way the runtime evaluates it.
pub fn flattened(group: &ConditionGroupSpec) -> (Vec<&Condition>, Mode) {
    let mut checks = Vec::new();
    flatten(group, &mut checks);
    (checks, group_mode(group))
}

/// The applicability mode, keyed off `all` truthiness exactly as Python's
/// `mode = "all" if condition_group.all else "any"` (an empty `all` list is
/// falsy → `any`).
fn group_mode(group: &ConditionGroupSpec) -> Mode {
    match &group.all {
        Some(a) if !a.is_empty() => Mode::All,
        _ => Mode::Any,
    }
}

/// Flatten a condition group to a linear check list, mirroring
/// `_flatten_conditions`: take `all or any or []`, splice nested groups in place,
/// and **discard nested modes**.
fn flatten<'a>(group: &'a ConditionGroupSpec, out: &mut Vec<&'a Condition>) {
    let items: &[ConditionOrGroup] = match (&group.all, &group.any) {
        (Some(a), _) if !a.is_empty() => a,
        (_, Some(b)) if !b.is_empty() => b,
        _ => &[],
    };
    for item in items {
        match item {
            ConditionOrGroup::Condition(c) => out.push(c),
            ConditionOrGroup::Group(g) => flatten(g, out),
        }
    }
}

/// Walk the decision tree to the reached leaf, recording each node's taken
/// branch as a normalized step.
fn walk<'a>(tree: &'a DecisionEntry, facts: &Facts, path: &mut Vec<NormStep>) -> &'a DecisionLeaf {
    let mut cur = tree;
    loop {
        match cur {
            DecisionEntry::Leaf(leaf) => return leaf.as_ref(),
            DecisionEntry::Node(node) => {
                let cond = &node.condition;
                let result =
                    compare::evaluate(lookup(facts, &cond.field), cond.operator, &cond.value);
                path.push(NormStep::new(cond.field.as_str(), cond.operator, result));
                cur = if result {
                    &node.true_branch
                } else {
                    &node.false_branch
                };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::FactValue;
    use ke_core::ir::{
        Condition, ConditionGroupSpec, ConditionOrGroup, DecisionEntry, DecisionLeaf, DecisionNode,
        DocumentRef, ObligationSpec, Operator, ProvenanceMarker, RuleIR, ScalarValue,
    };

    fn cond(field: &str, op: Operator, v: ScalarValue) -> Condition {
        Condition {
            field: field.into(),
            operator: op,
            value: v,
            description: None,
        }
    }

    fn leaf(result: &str, obligations: Vec<&str>) -> DecisionEntry {
        DecisionEntry::Leaf(Box::new(DecisionLeaf {
            result: result.into(),
            obligations: if obligations.is_empty() {
                None
            } else {
                Some(
                    obligations
                        .into_iter()
                        .map(|id| ObligationSpec {
                            id: id.into(),
                            description: None,
                            deadline: None,
                            source_span: None,
                        })
                        .collect(),
                )
            },
            notes: None,
            source_span: None,
        }))
    }

    fn rule(applies_if: Option<ConditionGroupSpec>, tree: DecisionEntry) -> RuleIR {
        RuleIR {
            rule_id: "r".into(),
            rule_version: "1.0".into(),
            description: None,
            tags: None,
            applies_if,
            decision_tree: tree,
            obligations: Vec::new(),
            source: DocumentRef {
                document_id: "doc".into(),
                article: None,
                section: None,
                paragraphs: Vec::new(),
                pages: Vec::new(),
                url: None,
            },
            interpretation_notes: None,
            effective_window: None,
            provenance: ProvenanceMarker::Candidate { proposal_id: None },
        }
    }

    fn facts(pairs: &[(&str, FactValue)]) -> Facts {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    fn all(items: Vec<ConditionOrGroup>) -> ConditionGroupSpec {
        ConditionGroupSpec {
            all: Some(items),
            any: None,
        }
    }
    fn any(items: Vec<ConditionOrGroup>) -> ConditionGroupSpec {
        ConditionGroupSpec {
            all: None,
            any: Some(items),
        }
    }

    #[test]
    fn no_applies_if_is_applicable() {
        let r = rule(None, leaf("ok", vec![]));
        let e = evaluate(&r, &facts(&[]));
        assert!(e.applicable);
        assert_eq!(e.decision.as_deref(), Some("ok"));
    }

    #[test]
    fn empty_all_group_is_applicable() {
        // Group present but no checks → executor's empty-checks early-return wins.
        let r = rule(Some(all(vec![])), leaf("ok", vec![]));
        assert!(evaluate(&r, &facts(&[])).applicable);
    }

    #[test]
    fn all_mode_short_circuits_false() {
        let g = all(vec![
            ConditionOrGroup::Condition(cond("a", Operator::Eq, ScalarValue::Str("x".into()))),
            ConditionOrGroup::Condition(cond("b", Operator::Eq, ScalarValue::Str("y".into()))),
        ]);
        let r = rule(Some(g), leaf("ok", vec![]));
        let e = evaluate(&r, &facts(&[("a", FactValue::Str("nope".into()))]));
        assert!(!e.applicable);
        // Only the first (failing) check was evaluated.
        assert_eq!(e.applicability_steps.len(), 1);
        assert_eq!(e.applicability_steps[0].field, "a");
        assert!(!e.applicability_steps[0].result);
    }

    #[test]
    fn any_mode_short_circuits_true() {
        let g = any(vec![
            ConditionOrGroup::Condition(cond("a", Operator::Eq, ScalarValue::Str("x".into()))),
            ConditionOrGroup::Condition(cond("b", Operator::Eq, ScalarValue::Str("y".into()))),
        ]);
        let r = rule(Some(g), leaf("ok", vec![]));
        let e = evaluate(&r, &facts(&[("a", FactValue::Str("x".into()))]));
        assert!(e.applicable);
        assert_eq!(e.applicability_steps.len(), 1);
    }

    #[test]
    fn nested_group_is_flattened_under_parent_mode() {
        // all: [a==x, any:[b==y, c==z]] flattens to a AND b AND c (nested OR lost),
        // mirroring the Python executor (ADR 0008). With a true, b false, c true,
        // the flattened `all` fails on b.
        let nested = ConditionOrGroup::Group(any(vec![
            ConditionOrGroup::Condition(cond("b", Operator::Eq, ScalarValue::Str("y".into()))),
            ConditionOrGroup::Condition(cond("c", Operator::Eq, ScalarValue::Str("z".into()))),
        ]));
        let g = all(vec![
            ConditionOrGroup::Condition(cond("a", Operator::Eq, ScalarValue::Str("x".into()))),
            nested,
        ]);
        let r = rule(Some(g), leaf("ok", vec![]));
        let e = evaluate(
            &r,
            &facts(&[
                ("a", FactValue::Str("x".into())),
                ("b", FactValue::Str("no".into())),
                ("c", FactValue::Str("z".into())),
            ]),
        );
        // Flattened AND: a(T), b(F) → short-circuit false. (A recursive OR would
        // have made the nested group true and the rule applicable — that is the
        // semantic-form behavior we deliberately do NOT use here.)
        assert!(!e.applicable);
    }

    #[test]
    fn tree_walk_reaches_leaf_and_collects_obligations() {
        // has_reserve==true ? (custodian==true ? compliant : non_compliant+oblig) : ...
        let tree = DecisionEntry::Node(Box::new(DecisionNode {
            node_id: "root".into(),
            condition: cond("has_reserve", Operator::Eq, ScalarValue::Bool(true)),
            true_branch: DecisionEntry::Node(Box::new(DecisionNode {
                node_id: "custody".into(),
                condition: cond("custodian", Operator::Eq, ScalarValue::Bool(true)),
                true_branch: leaf("compliant", vec![]),
                false_branch: leaf("non_compliant", vec!["appoint_custodian"]),
                source_span: None,
            })),
            false_branch: leaf(
                "non_compliant",
                vec!["establish_reserve", "appoint_custodian"],
            ),
            source_span: None,
        }));
        let r = rule(None, tree);

        let e = evaluate(
            &r,
            &facts(&[
                ("has_reserve", FactValue::Bool(true)),
                ("custodian", FactValue::Bool(true)),
            ]),
        );
        assert_eq!(e.decision.as_deref(), Some("compliant"));
        assert!(e.obligations.is_empty());
        assert_eq!(e.decision_path.len(), 2);

        let e = evaluate(&r, &facts(&[("has_reserve", FactValue::Bool(false))]));
        assert_eq!(e.decision.as_deref(), Some("non_compliant"));
        assert_eq!(
            e.obligation_ids(),
            vec![
                "appoint_custodian".to_string(),
                "establish_reserve".to_string()
            ]
        );
        assert_eq!(e.decision_path.len(), 1); // only the root was evaluated on the taken path
    }
}
