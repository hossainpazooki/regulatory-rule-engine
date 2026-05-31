//! T4 conflict finding types and the **provisional** class→severity policy.
//!
//! Spec §12 finding fields are populated as far as Gate 2 allows: rule ids,
//! class, severity, involved source documents, and a suggested-resolution hint.
//! Counterexample scenario and trace comparison stay `None` until the Gate 3
//! preview runtime can generate them.

use ke_core::semantic::SemanticRule;

/// The four Gate-2 conflict classes (ADR 0005). The other spec §12 classes are
/// deferred (need `ke-search`, the equivalence matrix, or a runtime).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConflictClass {
    ContradictoryOutcome,
    OverlappingScope,
    TemporalOverlap,
    DuplicateRule,
}

/// Spec §12 severity levels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Blocking,
    ReviewRequired,
    Advisory,
}

/// The class→severity policy, **Accepted (ADR 0005, signed off 2026-05-30)**.
/// This is the single place the policy lives; keep it here and easy to adjust.
/// Gate 4 will let a `PolicyBundle` override it per environment.
pub fn default_severity(class: ConflictClass) -> Severity {
    match class {
        ConflictClass::ContradictoryOutcome => Severity::Blocking,
        ConflictClass::OverlappingScope => Severity::ReviewRequired,
        ConflictClass::TemporalOverlap => Severity::ReviewRequired,
        ConflictClass::DuplicateRule => Severity::Advisory,
    }
}

fn suggested_resolution(class: ConflictClass) -> &'static str {
    match class {
        ConflictClass::ContradictoryOutcome => "reconcile outcomes or add precedence",
        ConflictClass::OverlappingScope => "encode precedence between the rules",
        ConflictClass::TemporalOverlap => "narrow the effective windows or add precedence",
        ConflictClass::DuplicateRule => "merge the rules or differentiate their scope",
    }
}

/// A single T4 conflict finding (spec §12 required fields, Gate-2 subset).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conflict {
    pub class: ConflictClass,
    pub severity: Severity,
    /// The rule ids involved (the conflicting pair).
    pub rule_ids: Vec<String>,
    /// The involved legal source documents (provenance, not YAML spans).
    pub source_documents: Vec<String>,
    pub detail: String,
    pub suggested_resolution: &'static str,
    // Gate 3: minimal counterexample scenario + trace comparison.
}

impl Conflict {
    pub(crate) fn between(
        class: ConflictClass,
        a: &SemanticRule,
        b: &SemanticRule,
        detail: impl Into<String>,
    ) -> Self {
        Conflict {
            class,
            severity: default_severity(class),
            rule_ids: vec![a.rule_id.clone(), b.rule_id.clone()],
            source_documents: dedup(vec![a.source_document.clone(), b.source_document.clone()]),
            detail: detail.into(),
            suggested_resolution: suggested_resolution(class),
        }
    }
}

fn dedup(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v.dedup();
    v
}
