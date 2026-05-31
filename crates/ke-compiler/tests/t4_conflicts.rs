//! T4 conflict fixtures: each fixture demonstrates one class and asserts the
//! detected conflict class + (provisional, ADR 0005) severity.

use ke_compiler::compile_rules;
use ke_compiler::verify::t4::detect_conflicts;
use ke_compiler::verify::{Conflict, ConflictClass, Severity};
use std::fs;
use std::path::Path;

fn load(name: &str) -> Vec<Conflict> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("conflicts")
        .join(name);
    let src = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let rules = compile_rules(&src).expect("compile fixture");
    detect_conflicts(&rules)
}

fn has(conflicts: &[Conflict], class: ConflictClass, severity: Severity) -> bool {
    conflicts
        .iter()
        .any(|c| c.class == class && c.severity == severity)
}

#[test]
fn duplicate_rule_advisory() {
    let c = load("duplicate.yaml");
    assert!(
        has(&c, ConflictClass::DuplicateRule, Severity::Advisory),
        "{c:?}"
    );
}

#[test]
fn contradictory_outcome_blocking() {
    let c = load("contradictory.yaml");
    assert!(
        has(&c, ConflictClass::ContradictoryOutcome, Severity::Blocking),
        "{c:?}"
    );
}

#[test]
fn overlapping_scope_review_required() {
    let c = load("overlapping_scope.yaml");
    assert!(
        has(
            &c,
            ConflictClass::OverlappingScope,
            Severity::ReviewRequired
        ),
        "{c:?}"
    );
    // No effective windows → no temporal_overlap on this fixture.
    assert!(
        !c.iter().any(|x| x.class == ConflictClass::TemporalOverlap),
        "unexpected temporal_overlap: {c:?}"
    );
}

#[test]
fn temporal_overlap_review_required() {
    let c = load("temporal_overlap.yaml");
    assert!(
        has(&c, ConflictClass::TemporalOverlap, Severity::ReviewRequired),
        "{c:?}"
    );
}
