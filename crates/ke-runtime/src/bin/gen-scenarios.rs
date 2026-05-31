//! `gen-scenarios` — emit a deterministic JSONL stream of scenarios with the
//! Rust executor's normalized result embedded, for the equivalence harness.
//! Developer tool (behind the `tools` feature).
//!
//! Usage:
//!   gen-scenarios [--seed N] [--fuzz K] <rule.yaml>...
//!
//! For every rule in every YAML file (skipping `schema.yaml`), generates the
//! structure-aware coverage scenarios plus `K` seeded-fuzz scenarios per rule,
//! evaluates each with the Rust runtime, and prints one JSON object per line:
//!
//!   {"rule_id","label","facts":{...},"rust":{applicable,decision,obligations,
//!    applicability_steps,decision_path}}
//!
//! The Python reference driver (`scripts/py_reference_runtime.py`) reads these
//! lines, recomputes the `python` side, and compares. The seed + fuzz count are
//! printed to stderr so a run is reproducible.

use std::fs;
use std::path::Path;
use std::process::ExitCode;

use anyhow::{Context, Result};
use ke_runtime::{evaluate, generate_for_rule};
use serde_json::json;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("gen-scenarios: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let mut seed: u64 = 0x5EED_1234_ABCD_0001;
    let mut fuzz: usize = 25;
    let mut files: Vec<String> = Vec::new();

    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--seed" => {
                seed = args
                    .next()
                    .context("--seed needs a value")?
                    .parse()
                    .context("--seed must be a u64")?;
            }
            "--fuzz" => {
                fuzz = args
                    .next()
                    .context("--fuzz needs a value")?
                    .parse()
                    .context("--fuzz must be an integer")?;
            }
            _ => files.push(a),
        }
    }
    if files.is_empty() {
        anyhow::bail!("usage: gen-scenarios [--seed N] [--fuzz K] <rule.yaml>...");
    }

    eprintln!(
        "gen-scenarios: seed={seed} fuzz_per_rule={fuzz} files={}",
        files.len()
    );

    let mut total = 0usize;
    for path in &files {
        if Path::new(path).file_name().and_then(|n| n.to_str()) == Some("schema.yaml") {
            continue;
        }
        let yaml = fs::read_to_string(path).with_context(|| format!("reading {path}"))?;
        let rules = ke_compiler::compile_rules(&yaml)
            .map_err(|e| anyhow::anyhow!("compile {path}: {e}"))?;
        for rule in &rules {
            for sc in generate_for_rule(rule, seed, fuzz) {
                let facts = sc.facts_map();
                let line = json!({
                    "rule_id": sc.rule_id,
                    "label": sc.label,
                    "facts": sc.facts,
                    "rust": evaluate(rule, &facts).normalized_json(),
                });
                println!("{line}");
                total += 1;
            }
        }
    }
    eprintln!("gen-scenarios: emitted {total} scenarios");
    Ok(())
}
