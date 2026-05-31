//! Golden trace-fixture parity (spec §19 Gate 3: "given existing trace fixtures,
//! when executed by the Rust preview runtime, then normalized public trace events
//! match Python output").
//!
//! `fixtures/traces/golden.json` is emitted by the Python oracle in
//! `scripts/equivalence-harness.sh` — and only when that fuzzed run is clean, so
//! each committed entry is a verified Rust≡Python agreement. This test re-derives
//! the normalized result with the Rust runtime and asserts it still matches,
//! guarding against runtime regressions. The fixtures are never hand-edited;
//! re-run the harness to refresh them.

use ke_runtime::{evaluate, facts_from_json};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

fn corpus_rules_by_id() -> BTreeMap<String, ke_core::ir::RuleIR> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rules");
    let mut map = BTreeMap::new();
    for entry in fs::read_dir(&dir).expect("read fixtures/rules") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("yaml")
            || path.file_name().and_then(|n| n.to_str()) == Some("schema.yaml")
        {
            continue;
        }
        let yaml = fs::read_to_string(&path).unwrap();
        for rule in ke_compiler::compile_rules(&yaml).expect("compile corpus") {
            map.insert(rule.rule_id.clone(), rule);
        }
    }
    map
}

#[test]
fn rust_runtime_reproduces_golden_traces() {
    let golden_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/traces/golden.json");
    let raw = fs::read_to_string(&golden_path).expect(
        "fixtures/traces/golden.json missing — run scripts/equivalence-harness.sh to generate it",
    );
    let fixtures: Vec<Value> = serde_json::from_str(&raw).expect("golden.json is a JSON array");
    assert!(!fixtures.is_empty(), "golden.json must have entries");

    let rules = corpus_rules_by_id();

    for fx in &fixtures {
        let rule_id = fx["rule_id"].as_str().expect("rule_id");
        let label = fx["label"].as_str().unwrap_or("");
        let rule = rules
            .get(rule_id)
            .unwrap_or_else(|| panic!("golden rule_id {rule_id} not in corpus"));
        let facts = facts_from_json(&fx["facts"]).expect("facts object");

        let got = evaluate(rule, &facts).normalized_json();
        let expected = &fx["normalized"];
        assert_eq!(
            &got, expected,
            "trace mismatch for {rule_id} [{label}]:\n  got      {got}\n  expected {expected}"
        );
    }
}
