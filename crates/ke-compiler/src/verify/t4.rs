//! T4 — cross-rule conflict detection (the four ADR-0005 classes).
//!
//! Detection is structural/bounded (no SAT, no runtime):
//! - **scope overlap** via top-level `all` equality/`in` premises (mirrors the
//!   platform `_extract_premise_keys` idea): two scopes overlap iff they share a
//!   constrained field and no shared field has disjoint value sets;
//! - **contradiction** via a consistent pair of decision paths (no condition
//!   assigned both true and false) with different results;
//! - **duplicate** via semantic-form equality of the logic (ignoring ids);
//! - **temporal overlap** via overlapping `[from, to)` windows + scope overlap.

use super::conflict::{Conflict, ConflictClass};
use ke_core::ir::condition::{ConditionOrGroup, Operator, ScalarValue};
use ke_core::ir::rule::RuleIR;
use ke_core::semantic::{SemCond, SemPath, SemWindow, SemanticRule};
use std::collections::{BTreeMap, BTreeSet};

/// Detect conflicts across a set of rules.
pub fn detect_conflicts(rules: &[RuleIR]) -> Vec<Conflict> {
    let forms: Vec<SemanticRule> = rules.iter().map(SemanticRule::from_rule).collect();
    let premises: Vec<Premises> = rules.iter().map(extract_premises).collect();

    let mut conflicts = Vec::new();
    for i in 0..rules.len() {
        for j in (i + 1)..rules.len() {
            let (a, b) = (&forms[i], &forms[j]);

            // Duplicate logic short-circuits the rest for this pair.
            if a.applicability == b.applicability && a.decision_paths == b.decision_paths {
                conflicts.push(Conflict::between(
                    ConflictClass::DuplicateRule,
                    a,
                    b,
                    "rules are semantically identical but differ in id/metadata",
                ));
                continue;
            }

            let overlap = scopes_overlap(&premises[i], &premises[j]);
            if overlap {
                if let Some((ra, rb)) = contradiction(a, b) {
                    conflicts.push(Conflict::between(
                        ConflictClass::ContradictoryOutcome,
                        a,
                        b,
                        format!("a shared scenario yields `{ra}` vs `{rb}`"),
                    ));
                } else {
                    conflicts.push(Conflict::between(
                        ConflictClass::OverlappingScope,
                        a,
                        b,
                        "applicability scopes overlap with no encoded precedence",
                    ));
                }

                if windows_overlap(a.effective, b.effective) {
                    conflicts.push(Conflict::between(
                        ConflictClass::TemporalOverlap,
                        a,
                        b,
                        "overlapping effective windows over an overlapping scope",
                    ));
                }
            }
        }
    }
    conflicts
}

// --- scope premises -------------------------------------------------------

struct Premises {
    /// field → allowed value set (from top-level `all` `==`/`in` conditions).
    fields: BTreeMap<String, BTreeSet<String>>,
}

fn extract_premises(rule: &RuleIR) -> Premises {
    let mut fields: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    if let Some(group) = &rule.applies_if {
        if let Some(items) = &group.all {
            for item in items {
                if let ConditionOrGroup::Condition(c) = item {
                    match c.operator {
                        Operator::Eq => {
                            fields
                                .entry(c.field.clone())
                                .or_default()
                                .insert(stringify(&c.value));
                        }
                        Operator::In => {
                            if let ScalarValue::List(vs) = &c.value {
                                let entry = fields.entry(c.field.clone()).or_default();
                                for v in vs {
                                    entry.insert(stringify(v));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    Premises { fields }
}

fn scopes_overlap(a: &Premises, b: &Premises) -> bool {
    let mut shared = false;
    for (field, av) in &a.fields {
        if let Some(bv) = b.fields.get(field) {
            shared = true;
            if av.is_disjoint(bv) {
                return false; // a field forces disjoint values → cannot co-apply
            }
        }
    }
    shared
}

fn stringify(v: &ScalarValue) -> String {
    match v {
        ScalarValue::Str(s) => s.clone(),
        ScalarValue::Bool(b) => b.to_string(),
        ScalarValue::Decimal { mantissa, scale } => format!("{mantissa}:{scale}"),
        ScalarValue::List(items) => {
            let parts: Vec<String> = items.iter().map(stringify).collect();
            format!("[{}]", parts.join(","))
        }
    }
}

// --- contradiction --------------------------------------------------------

/// If some path of `a` and some path of `b` are mutually consistent (no shared
/// condition assigned both ways) but reach different results, return that pair.
fn contradiction(a: &SemanticRule, b: &SemanticRule) -> Option<(String, String)> {
    for pa in &a.decision_paths {
        for pb in &b.decision_paths {
            if paths_consistent(pa, pb) && pa.outcome.result != pb.outcome.result {
                return Some((pa.outcome.result.clone(), pb.outcome.result.clone()));
            }
        }
    }
    None
}

fn paths_consistent(pa: &SemPath, pb: &SemPath) -> bool {
    let mut assign: BTreeMap<&SemCond, bool> = BTreeMap::new();
    for br in &pa.branches {
        assign.insert(&br.cond, br.taken);
    }
    for br in &pb.branches {
        if let Some(&taken) = assign.get(&br.cond) {
            if taken != br.taken {
                return false;
            }
        }
    }
    true
}

// --- temporal -------------------------------------------------------------

/// Temporal overlap requires **both** rules to declare an effective window: an
/// always-effective rule (no window) carries no temporal dimension to conflict
/// on, so that case is left to `overlapping_scope`. Open `to` means "to ∞".
fn windows_overlap(a: Option<SemWindow>, b: Option<SemWindow>) -> bool {
    let (Some(a), Some(b)) = (a, b) else {
        return false;
    };
    let open_end = (i16::MAX, 12u8, 31u8);
    let a_end = a.to.unwrap_or(open_end);
    let b_end = b.to.unwrap_or(open_end);
    a.from < b_end && b.from < a_end
}
