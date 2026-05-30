//! JSON Schema determinism (brief § 5.9, spec § 19 Gate 1).
//!
//! Emission is a pure function, and the committed
//! `crates/ke-core/schema/ir.schema.json` matches a fresh emit (the CI
//! `git diff --exit-code` gate, asserted here too).

use ke_core::schema::emit_schema_string;
use std::fs;
use std::path::Path;

#[test]
fn emit_is_deterministic() {
    assert_eq!(emit_schema_string(), emit_schema_string());
}

#[test]
fn committed_schema_matches_fresh_emit() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("schema")
        .join("ir.schema.json");
    let on_disk = fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "missing {} ({e}); run `cargo run -p ke-core --bin emit-schema`",
            path.display()
        )
    });
    assert_eq!(
        on_disk,
        emit_schema_string(),
        "committed schema is stale; regenerate with `cargo run -p ke-core --bin emit-schema`"
    );
}

#[test]
fn top_level_key_order_is_fixed() {
    // Brief § 5.1: $schema, $id, title, description, type, properties, required,
    // additionalProperties, $defs.
    let s = emit_schema_string();
    let order = [
        "\"$schema\"",
        "\"$id\"",
        "\"title\"",
        "\"description\"",
        "\"type\"",
        "\"properties\"",
        "\"required\"",
        "\"additionalProperties\"",
        "\"$defs\"",
    ];
    let mut last = 0usize;
    for key in order {
        let at = s
            .find(key)
            .unwrap_or_else(|| panic!("missing top-level key {key}"));
        assert!(at >= last, "top-level key {key} out of order");
        last = at;
    }
}
