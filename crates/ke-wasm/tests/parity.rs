//! G5-2 surfaced-difference proof, run as a NATIVE test (not wasm32).
//!
//! Parity holds **by construction, not by re-derivation**: [`ke_wasm::compile_preview`]
//! and [`ke_wasm::dry_run`] call the SAME pure functions
//! (`ke_compiler::compile_rules`, `ke_compiler::verify::verify`,
//! `ke_runtime::facts_from_json`, `ke_runtime::evaluate`,
//! `Evaluation::normalized_json`) the native `ke-cli serve` handlers call, and
//! emit the SAME JSON shapes via the SAME projection. There is no second
//! algorithm to drift.
//!
//! This test is therefore an EQUALITY ASSERTION over identical inputs run two
//! ways:
//! - (A) through the wasm fn's pure compute body
//!   ([`ke_wasm::compile_preview_impl`] / [`ke_wasm::dry_run_impl`] — the exact
//!   bodies the `#[wasm_bindgen] compile_preview`/`dry_run` wrappers delegate to;
//!   the `JsError`-returning wrappers themselves cannot run natively because
//!   `JsValue` operations abort off-wasm), and
//! - (B) directly through the shared pure fns + the canonical native projection
//!   (recomputed here, mirroring `ke-cli` `serve::handlers`),
//!
//! asserting the two `serde_json::Value`s are byte-equal after canonicalization.
//! If they ever differ the difference is SURFACED (a failing test), never
//! silently published. No fixtures — the YAML/facts are inline literals.

use serde_json::{json, Value};

/// A clean two-rule document (no blocking findings) plus a rule that trips a
/// T1 finding, so the report's findings/conflicts projection is exercised, not
/// just an empty report.
const YAML: &str = r#"
- rule_id: clean
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
- rule_id: thr
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

const FACTS: &str = r#"{"jurisdiction": "EU", "licensed": true, "amount": 1500}"#;

/// Canonicalize a `Value` so field-ordering differences never mask or fabricate
/// a mismatch: re-serialize through `BTreeMap`-backed JSON. We compare the
/// canonical *strings*.
fn canonical(v: &Value) -> String {
    // serde_json::Value keys are a BTreeMap, so to_string is already key-sorted;
    // round-tripping normalizes any residual representation differences.
    serde_json::to_string(&serde_json::from_str::<Value>(&v.to_string()).unwrap()).unwrap()
}

// --- (B) the canonical native compute path, recomputed here ------------------
// Mirrors `ke-cli` serve::handlers::{project_report, project_finding,
// project_conflict} byte-for-byte. If the wasm crate's duplicated copy ever
// drifts from these arms, the equality assertions below fail.

fn native_compile_preview(source: &str) -> Value {
    let rules = ke_compiler::compile_rules(source).expect("YAML compiles");
    let report = ke_compiler::verify::verify(&rules);

    let findings: Vec<Value> = report
        .findings
        .iter()
        .map(|f| {
            use ke_compiler::verify::Tier;
            let tier = match f.tier {
                Tier::T0 => "T0",
                Tier::T1 => "T1",
            };
            json!({
                "tier": tier,
                "rule_id": f.rule_id,
                "code": f.code,
                "message": f.message,
                "blocking": f.blocking,
            })
        })
        .collect();
    let conflicts: Vec<Value> = report
        .conflicts
        .iter()
        .map(|c| {
            json!({
                "class": format!("{:?}", c.class),
                "severity": format!("{:?}", c.severity),
                "message": c.detail,
            })
        })
        .collect();

    json!({
        "rules": rules,
        "report": {
            "has_blocking": report.has_blocking(),
            "findings": findings,
            "conflicts": conflicts,
        }
    })
}

fn native_dry_run(source: &str, facts_json: &str) -> Value {
    let rules = ke_compiler::compile_rules(source).expect("YAML compiles");
    let facts_value: Value = serde_json::from_str(facts_json).expect("facts JSON parses");
    let facts = ke_runtime::facts_from_json(&facts_value).expect("facts build");
    let evaluations: Vec<Value> = rules
        .iter()
        .map(|rule| ke_runtime::evaluate(rule, &facts).normalized_json())
        .collect();
    json!({ "evaluations": evaluations })
}

#[test]
fn compile_preview_matches_native_canonical_compute() {
    let wasm_json: Value =
        serde_json::from_str(&ke_wasm::compile_preview_impl(YAML).expect("compile_preview ok"))
            .unwrap();
    let native_json = native_compile_preview(YAML);
    assert_eq!(
        canonical(&wasm_json),
        canonical(&native_json),
        "WASM compile_preview must be byte-identical to the canonical native compute"
    );
}

#[test]
fn dry_run_matches_native_canonical_compute() {
    let wasm_json: Value =
        serde_json::from_str(&ke_wasm::dry_run_impl(YAML, FACTS).expect("dry_run ok")).unwrap();
    let native_json = native_dry_run(YAML, FACTS);
    assert_eq!(
        canonical(&wasm_json),
        canonical(&native_json),
        "WASM dry_run must be byte-identical to the canonical native compute"
    );
}

#[test]
fn compile_preview_compile_error_carries_native_error_body() {
    // Malformed YAML → the SAME {error:"compile_error", detail:"<{e:?}>"} body
    // the native 422 returns (and the wrapper throws as a JsError message).
    let body: Value =
        serde_json::from_str(&ke_wasm::compile_preview_impl("- 12345").unwrap_err()).unwrap();
    assert_eq!(body["error"], "compile_error");
    assert!(
        body["detail"].is_string(),
        "compile_error body must carry a string detail; got: {body}"
    );
}

#[test]
fn dry_run_facts_error_carries_native_error_body() {
    // Valid YAML, facts that are not an object → facts_error from
    // facts_from_json (and the wrapper throws as a JsError message).
    let body: Value =
        serde_json::from_str(&ke_wasm::dry_run_impl(YAML, "[1, 2, 3]").unwrap_err()).unwrap();
    assert_eq!(body["error"], "facts_error");
    assert!(
        body["detail"].is_string(),
        "facts_error body must carry a string detail; got: {body}"
    );
}
