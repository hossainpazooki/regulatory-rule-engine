//! T4 over the *clean* production corpus (regression guard for the
//! contradictory_outcome false-positive fixed in the ADR-0005 amendment).
//!
//! Before the fix, `contradiction()` treated any two scope-touching rules with
//! different result strings as a Blocking `ContradictoryOutcome`, even when they
//! answered unrelated legal questions (they shared only a broad applicability
//! premise and branched on disjoint variables). That produced 52 Blocking
//! conflicts on the clean 34-rule corpus, so `verify().has_blocking()` was true
//! and no artifact could reach `draft -> structurally_verified` (spec §9) — the
//! compiler asserting legal incompatibility, which it must never do (CLAUDE.md).
//!
//! This guard asserts the clean corpus is structurally publishable while proving
//! detection was *narrowed*, not disabled: real cross-rule findings
//! (`OverlappingScope`, `TemporalOverlap`) are still emitted, and the hand-built
//! `contradictory.yaml` fixture still fires (see `t4_conflicts.rs`).

use ke_compiler::verify::{self, ConflictClass};
use std::fs;
use std::path::PathBuf;

fn corpus_rules() -> Vec<ke_core::ir::RuleIR> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rules");
    let mut all = Vec::new();
    for entry in fs::read_dir(&dir).expect("read fixtures/rules") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("yaml")
            || path.file_name().and_then(|n| n.to_str()) == Some("schema.yaml")
        {
            continue;
        }
        let yaml = fs::read_to_string(&path).unwrap();
        all.extend(ke_compiler::compile_rules(&yaml).expect("compile corpus"));
    }
    all
}

#[test]
fn clean_corpus_has_no_blocking_contradiction() {
    let rules = corpus_rules();
    assert_eq!(rules.len(), 34, "corpus rule count drifted");

    let report = verify::verify(&rules);

    let contradictions = report
        .conflicts
        .iter()
        .filter(|c| c.class == ConflictClass::ContradictoryOutcome)
        .count();
    assert_eq!(
        contradictions, 0,
        "clean corpus must yield zero contradictory_outcome findings (rules answering \
         different questions are not contradictions); found {contradictions}"
    );

    assert!(
        !report.has_blocking(),
        "clean corpus must be structurally publishable (no blocking T0/T1/T4); \
         draft -> structurally_verified is impossible otherwise (spec §9)"
    );
}

#[test]
fn detection_is_narrowed_not_disabled() {
    let rules = corpus_rules();
    let report = verify::verify(&rules);

    // The same scope overlaps that previously misfired as contradictions must
    // still be reported as review-required findings — proof the narrowing did not
    // simply suppress all cross-rule detection.
    let overlapping = report
        .conflicts
        .iter()
        .filter(|c| c.class == ConflictClass::OverlappingScope)
        .count();
    assert!(
        overlapping > 0,
        "expected OverlappingScope findings over the corpus; detection looks disabled"
    );
}
