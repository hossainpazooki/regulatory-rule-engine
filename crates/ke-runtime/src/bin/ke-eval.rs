//! `ke-eval` — evaluate corpus rules against a fact set and print the normalized
//! result. Developer/debug tool (behind the `tools` feature).
//!
//! Usage:
//!   ke-eval <rule.yaml> <facts.json>
//!
//! Lowers every rule in `<rule.yaml>` via `ke-compiler`, evaluates each against
//! the single facts object in `<facts.json>`, and prints a JSON array of
//! normalized evaluations (the same shape the equivalence harness compares).

use std::fs;
use std::process::ExitCode;

use anyhow::{Context, Result};
use ke_runtime::{evaluate, facts_from_json};
use serde_json::{json, Value};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ke-eval: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        anyhow::bail!("usage: ke-eval <rule.yaml> <facts.json>");
    }
    let yaml = fs::read_to_string(&args[0]).with_context(|| format!("reading {}", args[0]))?;
    let facts_src = fs::read_to_string(&args[1]).with_context(|| format!("reading {}", args[1]))?;

    let rules = ke_compiler::compile_rules(&yaml).map_err(|e| anyhow::anyhow!("compile: {e}"))?;
    let facts_json: Value = serde_json::from_str(&facts_src).context("parsing facts JSON")?;
    let facts = facts_from_json(&facts_json).map_err(|e| anyhow::anyhow!("facts: {e}"))?;

    let out: Vec<Value> = rules
        .iter()
        .map(|r| {
            let e = evaluate(r, &facts);
            let mut o = e.normalized_json();
            o.as_object_mut()
                .unwrap()
                .insert("rule_id".into(), json!(e.rule_id));
            o
        })
        .collect();
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}
