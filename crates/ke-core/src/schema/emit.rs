//! Deterministic top-level schema assembly (brief § 5).
//!
//! Top-level keys are emitted in the fixed order `$schema, $id, title,
//! description, type, properties, required, additionalProperties, $defs`
//! (§ 5.1). No timestamps or environment data appear in `$id` or `description`
//! (§ 5.7); the schema metadata version is exactly the IR schema version
//! (§ 5.8). Determinism relies on `serde_json`'s `preserve_order` feature.

use super::defs;
use crate::version::IR_SCHEMA_VERSION;
use serde_json::{json, Map, Value};

/// Emit the canonical IR JSON Schema as a `serde_json::Value`. Pure function of
/// the pinned schema version and the Rust type declarations — no I/O, no clock,
/// no environment.
pub fn emit_schema() -> Value {
    let version = IR_SCHEMA_VERSION;

    // The root schema body is the RuleIR object; splice its properties/required
    // up to the root so the top-level document *is* a RuleIR.
    let rule = defs::rule_ir();
    let (properties, required) = match &rule {
        Value::Object(m) => (
            m.get("properties").cloned().unwrap_or(Value::Null),
            m.get("required").cloned().unwrap_or(Value::Null),
        ),
        _ => unreachable!("rule_ir() always builds an object"),
    };

    let mut defs_map = Map::new();
    for (name, value) in defs::all_defs() {
        defs_map.insert(name.to_string(), value);
    }

    let mut root = Map::new();
    root.insert(
        "$schema".to_string(),
        json!("https://json-schema.org/draft/2020-12/schema"),
    );
    root.insert(
        "$id".to_string(),
        json!(format!(
            "https://ke-workbench.dev/schema/ir/{version}/ir.schema.json"
        )),
    );
    root.insert("title".to_string(), json!("ke-workbench RuleIR"));
    root.insert(
        "description".to_string(),
        json!(
            "Canonical intermediate-representation schema for ke-workbench rule \
             artifacts. The root document is a RuleIR. Authoritative for downstream \
             model generation (platform Pydantic/msgspec, frontend types). See \
             docs/canonical-encoding.md and the Gate 1 brief."
        ),
    );
    root.insert("type".to_string(), json!("object"));
    root.insert("properties".to_string(), properties);
    root.insert("required".to_string(), required);
    root.insert("additionalProperties".to_string(), json!(false));
    root.insert("$defs".to_string(), Value::Object(defs_map));

    Value::Object(root)
}

/// Emit the schema as pretty-printed JSON with a trailing newline — the exact
/// bytes committed to `crates/ke-core/schema/ir.schema.json`.
pub fn emit_schema_string() -> String {
    let mut s = serde_json::to_string_pretty(&emit_schema()).expect("schema serializes");
    s.push('\n');
    s
}
