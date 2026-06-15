//! T0 (structural) and T1 (coverage + interpretation notes) findings.

use ke_compiler::compile_rules;
use ke_compiler::verify::{verify, Tier};

/// Codes of the T0/T1 findings only. T5 (lint-beyond-compiler, Gate 5) shares
/// the same `findings` vector but is advisory and orthogonal to the T0/T1
/// structural/coverage checks this file exercises, so it is filtered out here.
fn findings_codes(src: &str) -> Vec<String> {
    let rules = compile_rules(src).expect("compile");
    verify(&rules)
        .findings
        .into_iter()
        .filter(|f| f.tier != Tier::T5)
        .map(|f| f.code.to_string())
        .collect()
}

#[test]
fn t0_flags_empty_rule_id() {
    let src = r#"
rule_id: ""
decision_tree:
  result: ok
source:
  document_id: doc1
"#;
    assert!(findings_codes(src).contains(&"T0_empty_rule_id".to_string()));
}

#[test]
fn t1_flags_threshold_without_interpretation_notes() {
    // A `>=` decimal threshold and no interpretation_notes → blocking T1 finding.
    let src = r#"
rule_id: thr
applies_if:
  all:
    - field: amount
      operator: ">="
      value: 1000
decision_tree:
  result: ok
source:
  document_id: doc1
"#;
    assert!(findings_codes(src).contains(&"T1_missing_interpretation_notes".to_string()));
}

#[test]
fn t1_satisfied_when_notes_present() {
    let src = r#"
rule_id: thr
applies_if:
  all:
    - field: amount
      operator: ">="
      value: 1000
decision_tree:
  result: ok
source:
  document_id: doc1
interpretation_notes: "1000 is the EUR threshold from Article X."
"#;
    let codes = findings_codes(src);
    assert!(!codes.contains(&"T1_missing_interpretation_notes".to_string()));
}

#[test]
fn clean_rule_has_no_findings() {
    let src = r#"
rule_id: clean
applies_if:
  all:
    - field: jurisdiction
      operator: "=="
      value: EU
decision_tree:
  node_id: n
  condition:
    field: licensed
    operator: "=="
    value: true
  true_branch:
    result: compliant
  false_branch:
    result: non_compliant
source:
  document_id: doc1
"#;
    assert!(findings_codes(src).is_empty(), "expected no T0/T1 findings");
}
