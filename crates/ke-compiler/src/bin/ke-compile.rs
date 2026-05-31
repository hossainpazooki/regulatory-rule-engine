//! `ke-compile` — Gate 2 developer tool (the real CLI is `ke-cli`, Gate 4).
//!
//! Subcommands:
//! - `compile <file.yaml> [--emit semantic-json]` — parse + lower; optionally
//!   emit the semantic normal forms as JSON.
//! - `diff <file.yaml> <python-rules.json>` — compile the Rust side, import the
//!   Python `Rule` JSON, and compare both at the semantic-normal-form level.
//!   Exits non-zero on any divergence (used by `differential-test.sh`).

use ke_core::semantic::{semantic_diff, SemanticRule};
use std::collections::BTreeMap;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("compile") => cmd_compile(&args),
        Some("diff") => cmd_diff(&args),
        _ => usage(),
    }
}

fn usage() -> ExitCode {
    eprintln!(
        "usage:\n  ke-compile compile <file.yaml> [--emit semantic-json]\n  ke-compile diff <file.yaml> <python-rules.json>"
    );
    ExitCode::from(64)
}

fn cmd_compile(args: &[String]) -> ExitCode {
    let Some(path) = args.get(2) else {
        return usage();
    };
    let emit_semantic = args.iter().any(|a| a == "semantic-json")
        || args
            .windows(2)
            .any(|w| w[0] == "--emit" && w[1] == "semantic-json");

    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read {path}: {e}");
            return ExitCode::FAILURE;
        }
    };

    if emit_semantic {
        match ke_compiler::compile_to_semantic(&source) {
            Ok(forms) => {
                println!("{}", serde_json::to_string_pretty(&forms).unwrap());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("compile error in {path}: {e}");
                ExitCode::FAILURE
            }
        }
    } else {
        match ke_compiler::compile_rules(&source) {
            Ok(rules) => {
                println!("compiled {} rule(s) from {path}:", rules.len());
                for r in &rules {
                    println!("  - {} (v{})", r.rule_id, r.rule_version);
                }
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("compile error in {path}: {e}");
                ExitCode::FAILURE
            }
        }
    }
}

fn cmd_diff(args: &[String]) -> ExitCode {
    let (Some(yaml_path), Some(json_path)) = (args.get(2), args.get(3)) else {
        return usage();
    };

    let yaml = match std::fs::read_to_string(yaml_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read {yaml_path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let rust = match ke_compiler::compile_rules(&yaml) {
        Ok(rules) => index(rules.iter().map(SemanticRule::from_rule)),
        Err(e) => {
            eprintln!("rust compile error in {yaml_path}: {e}");
            return ExitCode::FAILURE;
        }
    };

    let json_text = match std::fs::read_to_string(json_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read {json_path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let json: serde_json::Value = match serde_json::from_str(&json_text) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("invalid python JSON in {json_path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let python = match ke_compiler::python_import::import_rules(&json) {
        Ok(rules) => index(rules.iter().map(SemanticRule::from_rule)),
        Err(e) => {
            eprintln!("python import error in {json_path}: {e}");
            return ExitCode::FAILURE;
        }
    };

    let mut divergences = 0usize;

    // Rule-id set mismatch.
    for id in rust.keys() {
        if !python.contains_key(id) {
            eprintln!("  - rule `{id}` present in Rust but not Python");
            divergences += 1;
        }
    }
    for id in python.keys() {
        if !rust.contains_key(id) {
            eprintln!("  - rule `{id}` present in Python but not Rust");
            divergences += 1;
        }
    }

    // Per-rule semantic diff.
    for (id, r) in &rust {
        if let Some(p) = python.get(id) {
            let diffs = semantic_diff(r, p);
            if !diffs.is_empty() {
                divergences += 1;
                eprintln!("  - rule `{id}` diverges:");
                for d in diffs {
                    eprintln!("      {}: {}", d.field, d.detail);
                }
            }
        }
    }

    if divergences == 0 {
        println!(
            "OK: {} rule(s) semantically equivalent ({})",
            rust.len(),
            yaml_path
        );
        ExitCode::SUCCESS
    } else {
        eprintln!("DIVERGENCE: {divergences} issue(s) in {yaml_path}");
        ExitCode::FAILURE
    }
}

fn index(forms: impl Iterator<Item = SemanticRule>) -> BTreeMap<String, SemanticRule> {
    forms.map(|f| (f.rule_id.clone(), f)).collect()
}
