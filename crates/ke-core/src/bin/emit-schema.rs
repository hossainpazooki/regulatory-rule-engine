//! Regenerate the committed JSON Schema at
//! `crates/ke-core/schema/ir.schema.json`.
//!
//! Idempotent: running it twice produces byte-identical output. CI regenerates
//! and fails on any `git diff` (brief § 5.9).

use std::fs;
use std::path::Path;

fn main() -> std::io::Result<()> {
    let contents = ke_core::schema::emit_schema_string();
    let out = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("schema")
        .join("ir.schema.json");
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&out, contents)?;
    eprintln!("wrote {}", out.display());
    Ok(())
}
