//! Parser produces `YamlSpan`s on the nodes diagnostics care about, and
//! reports positioned errors for malformed input (brief Phase 4).

use ke_compiler::ast::{AstDecision, AstGroupItem};
use ke_compiler::parser::parse_rules;

const RULE: &str = r#"
rule_id: r1
version: "1.0"
applies_if:
  all:
    - field: jurisdiction
      operator: "=="
      value: EU
decision_tree:
  node_id: n1
  condition:
    field: licensed
    operator: "=="
    value: true
  true_branch:
    result: compliant
  false_branch:
    result: non_compliant
    obligations:
      - id: ob1
        description: do the thing
source:
  document_id: doc1
  article: "5"
"#;

#[test]
fn spans_present_for_key_nodes() {
    let rules = parse_rules(RULE).expect("parse");
    let r = &rules[0];

    assert!(r.rule_id.span.is_known(), "rule_id span");
    let applies = r.applies_if.as_ref().expect("applies_if");
    assert!(applies.span.is_known(), "applies_if span");

    // First applicability condition's field carries a span.
    match &applies.value.items[0].value {
        AstGroupItem::Condition(c) => assert!(c.field.span.is_known(), "condition field span"),
        _ => panic!("expected a condition"),
    }

    // Decision node condition + the obligation on the false branch carry spans.
    let AstDecision::Node(node) = &r.decision_tree.value else {
        panic!("expected a decision node");
    };
    let cond = node.condition.as_ref().expect("node condition");
    assert!(cond.span.is_known(), "decision condition span");

    let false_branch = node.false_branch.as_ref().expect("false_branch");
    let AstDecision::Leaf(leaf) = &false_branch.value else {
        panic!("expected a leaf");
    };
    assert!(leaf.result.span.is_known(), "leaf result span");
    assert!(
        leaf.obligations[0].value.id.span.is_known(),
        "obligation id span"
    );
}

#[test]
fn missing_required_field_is_reported() {
    // No rule_id.
    let src = "version: \"1.0\"\nsource:\n  document_id: d\ndecision_tree:\n  result: ok\n";
    let err = parse_rules(src).expect_err("should fail");
    assert!(err.message.contains("rule_id"), "message: {}", err.message);
}

#[test]
fn malformed_yaml_is_reported() {
    let err = parse_rules("rule_id: x\n  : : :\n").expect_err("should fail");
    assert!(
        err.message.to_lowercase().contains("yaml") || err.message.contains("parse"),
        "message: {}",
        err.message
    );
}
