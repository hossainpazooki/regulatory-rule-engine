//! Regenerate `fixtures/graph/expected_edges.json` — the committed pin for
//! the graph edge set recomputed over the golden artifacts (ADR-0023 D4).
//!
//! This binary is the **only** sanctioned writer of that file (the `fixtures/`
//! rule: never hand-edit; updates flow through generation tooling). Run from
//! the workspace root:
//!
//! ```text
//! cargo run -p ke-cli --bin gen-graph-fixture
//! ```
//!
//! Deterministic: the output is a pure function of the committed golden
//! `.kew` bytes, so a clean regeneration on an unchanged tree is a no-op diff.

use ke_cli::graph::golden_edges_json;

fn main() -> anyhow::Result<()> {
    let fixtures_root = std::env::args().nth(1).unwrap_or_else(|| {
        // Default: workspace-root fixtures/, resolved relative to this crate.
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures").to_string()
    });

    let value = golden_edges_json(&fixtures_root)?;
    let rendered = serde_json::to_string_pretty(&value)?;

    let out_dir = std::path::Path::new(&fixtures_root).join("graph");
    std::fs::create_dir_all(&out_dir)?;
    let out_path = out_dir.join("expected_edges.json");
    std::fs::write(&out_path, rendered + "\n")?;

    let goldens = value["goldens"].as_object().map(|g| g.len()).unwrap_or(0);
    println!("wrote {} ({goldens} goldens)", out_path.display());
    Ok(())
}
