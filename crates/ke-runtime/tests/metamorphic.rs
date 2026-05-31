//! Metamorphic tests: transformations that must preserve (or predictably flip)
//! the outcome. Pure Rust over both constructed rules and the corpus.

use ke_core::ir::{
    Condition, ConditionGroupSpec, ConditionOrGroup, DecisionEntry, DecisionLeaf, DecisionNode,
    DocumentRef, Operator, ProvenanceMarker, RuleIR, ScalarValue,
};
use ke_runtime::scenario::generate_for_rule;
use ke_runtime::{evaluate, facts_from_json, Facts};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

// --- builders --------------------------------------------------------------

fn cond(field: &str, op: Operator, v: ScalarValue) -> Condition {
    Condition {
        field: field.into(),
        operator: op,
        value: v,
        description: None,
    }
}

fn leaf(result: &str) -> DecisionEntry {
    DecisionEntry::Leaf(Box::new(DecisionLeaf {
        result: result.into(),
        obligations: None,
        notes: None,
        source_span: None,
    }))
}

fn node(c: Condition, t: DecisionEntry, f: DecisionEntry) -> DecisionEntry {
    DecisionEntry::Node(Box::new(DecisionNode {
        node_id: "n".into(),
        condition: c,
        true_branch: t,
        false_branch: f,
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

fn facts(v: Value) -> Facts {
    facts_from_json(&v).unwrap()
}

// --- outcome-flipping ------------------------------------------------------

#[test]
fn threshold_negation_flips_decision() {
    // x >= 10 ? compliant : non_compliant
    let r = rule(
        None,
        node(
            cond("x", Operator::Gte, ScalarValue::int(10)),
            leaf("compliant"),
            leaf("non_compliant"),
        ),
    );
    assert_eq!(
        evaluate(&r, &facts(json!({"x": 10}))).decision.as_deref(),
        Some("compliant")
    );
    assert_eq!(
        evaluate(&r, &facts(json!({"x": 9}))).decision.as_deref(),
        Some("non_compliant")
    );
}

#[test]
fn boolean_negation_flips_decision() {
    let r = rule(
        None,
        node(
            cond("authorized", Operator::Eq, ScalarValue::Bool(true)),
            leaf("yes"),
            leaf("no"),
        ),
    );
    assert_eq!(
        evaluate(&r, &facts(json!({"authorized": true})))
            .decision
            .as_deref(),
        Some("yes")
    );
    assert_eq!(
        evaluate(&r, &facts(json!({"authorized": false})))
            .decision
            .as_deref(),
        Some("no")
    );
}

#[test]
fn removing_a_required_all_fact_flips_applicability() {
    let g = ConditionGroupSpec {
        all: Some(vec![
            ConditionOrGroup::Condition(cond("a", Operator::Eq, ScalarValue::Str("x".into()))),
            ConditionOrGroup::Condition(cond("b", Operator::Eq, ScalarValue::Str("y".into()))),
        ]),
        any: None,
    };
    let r = rule(Some(g), leaf("ok"));
    assert!(evaluate(&r, &facts(json!({"a":"x","b":"y"}))).applicable);
    assert!(!evaluate(&r, &facts(json!({"a":"x"}))).applicable); // b missing → not applicable
}

// --- outcome-preserving ----------------------------------------------------

#[test]
fn reordering_all_group_preserves_outcome() {
    let mk = |order: [&str; 2]| {
        let pair = |f: &str| match f {
            "a" => {
                ConditionOrGroup::Condition(cond("a", Operator::Eq, ScalarValue::Str("x".into())))
            }
            _ => ConditionOrGroup::Condition(cond("b", Operator::Eq, ScalarValue::Str("y".into()))),
        };
        rule(
            Some(ConditionGroupSpec {
                all: Some(vec![pair(order[0]), pair(order[1])]),
                any: None,
            }),
            leaf("ok"),
        )
    };
    let forward = mk(["a", "b"]);
    let reversed = mk(["b", "a"]);
    for f in [
        json!({"a":"x","b":"y"}),
        json!({"a":"no","b":"y"}),
        json!({"a":"x","b":"no"}),
    ] {
        let a = evaluate(&forward, &facts(f.clone()));
        let b = evaluate(&reversed, &facts(f.clone()));
        // Outcome + obligations are stable under reorder; applicability_steps may
        // differ (short-circuit picks a different deciding check) — not compared.
        assert_eq!(a.applicable, b.applicable, "applicability changed for {f}");
        assert_eq!(a.decision, b.decision);
        assert_eq!(a.obligation_ids(), b.obligation_ids());
    }
}

#[test]
fn irrelevant_facts_preserve_outcome_over_corpus() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rules");
    let mut tested = 0usize;
    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("yaml")
            || path.file_name().and_then(|n| n.to_str()) == Some("schema.yaml")
        {
            continue;
        }
        let yaml = fs::read_to_string(&path).unwrap();
        for rule in ke_compiler::compile_rules(&yaml).unwrap() {
            for sc in generate_for_rule(&rule, 3, 10) {
                let base = evaluate(&rule, &sc.facts_map());
                // Augment with fields no condition references.
                let mut aug = sc.facts.as_object().unwrap().clone();
                aug.insert("__noise_x".into(), json!("zzz"));
                aug.insert("__noise_y".into(), json!(123));
                let with_noise = evaluate(&rule, &facts(Value::Object(aug)));
                assert_eq!(base.applicable, with_noise.applicable);
                assert_eq!(base.decision, with_noise.decision);
                assert_eq!(base.obligation_ids(), with_noise.obligation_ids());
                assert_eq!(base.decision_path, with_noise.decision_path);
                assert_eq!(base.applicability_steps, with_noise.applicability_steps);
                tested += 1;
            }
        }
    }
    assert!(tested > 100, "expected many corpus scenarios, got {tested}");
}
