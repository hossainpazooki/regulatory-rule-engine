//! The Python-import path reduces to the same semantic normal form as the Rust
//! lowering for an equivalent rule — the core of the Gate 2 differential, run
//! here without a live platform (a Python-`model_dump`-shaped JSON is inlined).

use ke_compiler::{compile_rules, python_import};
use ke_core::semantic::SemanticRule;

const YAML: &str = r#"
rule_id: t1
version: "1.0"
applies_if:
  all:
    - field: jurisdiction
      operator: "=="
      value: EU
    - field: amount
      operator: ">="
      value: 0.90
decision_tree:
  node_id: n1
  condition:
    field: licensed
    operator: "=="
    value: true
  true_branch:
    result: compliant
    obligations:
      - id: ob1
        description: do the thing
  false_branch:
    result: non_compliant
source:
  document_id: doc1
  article: "5"
"#;

// Python `Rule.model_dump(mode="json")` for the same rule, deliberately differing
// in incidental ways: `all` order swapped, decimal 0.9 vs 0.90, a different
// node_id, and different obligation prose — all of which the semantic form
// abstracts away.
const PY_JSON: &str = r#"
[{"rule_id":"t1","version":"1.0","description":null,"effective_from":null,
"effective_to":null,"tags":[],"jurisdiction":"EU","regime_id":"mica_2023",
"cross_border_relevant":false,
"applies_if":{"all":[
  {"field":"amount","operator":">=","value":0.9,"description":null},
  {"field":"jurisdiction","operator":"==","value":"EU","description":null}],"any":null},
"decision_tree":{"node_id":"OTHER","condition":{"field":"licensed","operator":"==","value":true,"description":null},
"true_branch":{"result":"compliant","obligations":[{"id":"ob1","description":"different prose","deadline":null,"source_ref":null}],"notes":null},
"false_branch":{"result":"non_compliant","obligations":[],"notes":null}},
"source":{"document_id":"doc1","article":"5","section":null,"paragraphs":[],"pages":[],"url":null},
"interpretation_notes":null,"consistency":null}]
"#;

#[test]
fn rust_and_python_reduce_to_the_same_semantic_form() {
    let rust = SemanticRule::from_rule(&compile_rules(YAML).expect("rust compile")[0]);

    let value: serde_json::Value = serde_json::from_str(PY_JSON).expect("parse python json");
    let imported = python_import::import_rules(&value).expect("python import");
    let python = SemanticRule::from_rule(&imported[0]);

    assert_eq!(rust, python);
}
