//! `ke-compile` — Gate 2 developer tool (the real CLI is `ke-cli`, Gate 4).
//!
//! Phase 1: `ke-compile compile <file.yaml>` parses + lowers and reports the
//! rules (or the first compile error). Phase 2 adds `--emit semantic-json` and
//! a `diff <yaml> <python-json>` subcommand for the differential harness.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("compile") => match args.get(2) {
            Some(path) => compile(path),
            None => {
                eprintln!("usage: ke-compile compile <file.yaml>");
                ExitCode::from(64)
            }
        },
        _ => {
            eprintln!("usage: ke-compile compile <file.yaml>");
            ExitCode::from(64)
        }
    }
}

fn compile(path: &str) -> ExitCode {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read {path}: {e}");
            return ExitCode::FAILURE;
        }
    };
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
