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

/// If some path of `a` and some path of `b` describe a **shared scenario** (ADR
/// 0005) yet reach different results, return that pair.
///
/// "Shared scenario" is the load-bearing notion. The caller has already
/// established that the two rules' *applicability* scopes overlap
/// (`scopes_overlap`). On top of that, a contradiction requires the two decision
/// **paths** to be (a) mutually consistent — no branch condition assigned both
/// ways — *and* (b) genuinely co-extensive: they must pin down the same scenario,
/// not merely answer different questions that happen to share a broad
/// applicability premise. Co-extensive means the paths share at least one
/// decision-branch variable, or both rules are leaf-only (so the shared
/// applicability scope alone fixes the scenario — the `contradictory.yaml` case).
///
/// Without (b), two rules that only coincide on a broad premise like
/// `jurisdiction == UK` but branch on disjoint variables (e.g. one on
/// `risk_warning_present`, the other on `investor_type`) were reported as
/// `ContradictoryOutcome` purely because their result *vocabularies* differ —
/// the compiler asserting legal incompatibility between rules answering unrelated
/// questions. That is a legal judgment the compiler must not make (CLAUDE.md:
/// "structural validity only; never legal truth"), and it contradicts ADR 0005's
/// own definition ("incompatible decision results *for a shared scenario*").
///
/// Bounded-detection trade-off (ADR 0005 §Consequences): requiring a shared
/// branch variable can miss a contradiction between a leaf-only rule and a
/// branched rule over the same scope. Such cases are left to Gate-3 counterexample
/// generation; pairs that lose `ContradictoryOutcome` still surface as
/// `OverlappingScope` (Review-required).
fn contradiction(a: &SemanticRule, b: &SemanticRule) -> Option<(String, String)> {
    for pa in &a.decision_paths {
        for pb in &b.decision_paths {
            if pa.outcome.result != pb.outcome.result
                && paths_consistent(pa, pb)
                && shared_scenario(pa, pb)
            {
                return Some((pa.outcome.result.clone(), pb.outcome.result.clone()));
            }
        }
    }
    None
}

/// Two paths are mutually consistent if no shared branch condition is assigned
/// both `true` and `false` (a necessary, not sufficient, condition for a shared
/// scenario).
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

/// Whether two consistent paths describe the *same* scenario rather than answering
/// orthogonal questions: they must constrain a common decision variable, or both
/// be leaf-only (in which case the caller-verified applicability overlap fixes the
/// scenario). See [`contradiction`] for why vacuous "no shared condition" is not
/// enough.
fn shared_scenario(pa: &SemPath, pb: &SemPath) -> bool {
    let fa: BTreeSet<&str> = pa
        .branches
        .iter()
        .map(|br| br.cond.field.as_str())
        .collect();
    let fb: BTreeSet<&str> = pb
        .branches
        .iter()
        .map(|br| br.cond.field.as_str())
        .collect();
    if !fa.is_disjoint(&fb) {
        return true; // the paths pin a common decision variable
    }
    fa.is_empty() && fb.is_empty() // both leaf-only: shared applicability scope is the scenario
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
