//! Property tests over the corpus, using the deterministic generator (no
//! `proptest`; see ADR 0008). These are pure-Rust invariants — cross-language
//! parity is the shell equivalence harness, not these tests.

use ke_runtime::scenario::{generate_for_rule, Catalog};
use ke_runtime::{evaluate, op_token};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

/// Load every corpus rule (skipping the schema doc), lowering YAML via the
/// compiler.
fn corpus_rules() -> Vec<ke_core::ir::RuleIR> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rules");
    let mut rules = Vec::new();
    for entry in fs::read_dir(&dir).expect("read fixtures/rules") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
            continue;
        }
        if path.file_name().and_then(|n| n.to_str()) == Some("schema.yaml") {
            continue;
        }
        let yaml = fs::read_to_string(&path).unwrap();
        rules.extend(ke_compiler::compile_rules(&yaml).expect("compile corpus rule"));
    }
    assert!(!rules.is_empty(), "corpus must be non-empty");
    rules
}

const TOKENS: &[&str] = &["==", "!=", "in", "not_in", ">", "<", ">=", "<=", "exists"];

#[test]
fn evaluation_is_deterministic_and_total() {
    let rules = corpus_rules();
    let mut checked = 0usize;
    for rule in &rules {
        for sc in generate_for_rule(rule, 7, 30) {
            let facts = sc.facts_map();
            let a = evaluate(rule, &facts);
            let b = evaluate(rule, &facts);
            assert_eq!(a, b, "evaluation not deterministic for {}", sc.label);
            // Complete trees always reach a leaf when applicable.
            if a.applicable {
                assert!(
                    a.decision.is_some(),
                    "applicable but no decision: {} / {}",
                    rule.rule_id,
                    sc.label
                );
            }
            // Trace operators are always canonical tokens.
            for step in a.applicability_steps.iter().chain(a.decision_path.iter()) {
                assert!(
                    TOKENS.contains(&step.operator.as_str()),
                    "bad op token {step:?}"
                );
            }
            checked += 1;
        }
    }
    assert!(
        checked > 300,
        "expected a substantial scenario set, got {checked}"
    );
}

#[test]
fn generator_invariant_no_number_on_in_fields() {
    // ADR 0008: a field used with in/not_in is never given a numeric fact (the
    // only realistic str(float) cross-language hazard).
    let rules = corpus_rules();
    for rule in &rules {
        let in_fields = Catalog::from_rule(rule).in_fields;
        if in_fields.is_empty() {
            continue;
        }
        for sc in generate_for_rule(rule, 99, 50) {
            for f in &in_fields {
                if let Some(v) = sc.facts.get(f) {
                    assert!(
                        !matches!(v, Value::Number(_)),
                        "number fact on in/not_in field {f} in {} / {}",
                        rule.rule_id,
                        sc.label
                    );
                }
            }
        }
    }
}

#[test]
fn op_token_round_trips_all_operators() {
    use ke_core::ir::Operator::*;
    for op in [Eq, NotEq, In, NotIn, Gt, Lt, Gte, Lte, Exists] {
        assert!(TOKENS.contains(&op_token(op)));
    }
}
