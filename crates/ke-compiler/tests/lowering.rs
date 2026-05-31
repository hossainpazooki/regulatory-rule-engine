//! Lowering the real corpus: every rule lowers to a `RuleIR` that encodes
//! canonically and round-trips, and the semantic normal form is stable. This
//! exercises the effective_window-optional path (C1) on `fca_crypto.yaml`.

use ke_compiler::compile_rules;
use ke_core::canonical::{decode_rule, encode_rule};
use ke_core::semantic::SemanticRule;
use std::fs;
use std::path::{Path, PathBuf};

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repo root")
        .join("fixtures")
        .join("rules")
}

fn corpus_files() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(corpus_dir())
        .expect("read corpus")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map(|x| x == "yaml").unwrap_or(false))
        .filter(|p| p.file_name().map(|n| n != "schema.yaml").unwrap_or(false))
        .collect();
    files.sort();
    files
}

#[test]
fn whole_corpus_lowers_and_round_trips() {
    let mut total = 0usize;
    let mut without_window = 0usize;
    for path in corpus_files() {
        let src = fs::read_to_string(&path).expect("read");
        let rules =
            compile_rules(&src).unwrap_or_else(|e| panic!("compile {}: {e}", path.display()));
        for r in &rules {
            total += 1;
            if r.effective_window.is_none() {
                without_window += 1;
            }
            let bytes = encode_rule(r).expect("encode");
            let back = decode_rule(&bytes).expect("decode");
            assert_eq!(
                bytes,
                encode_rule(&back).expect("re-encode"),
                "stable bytes"
            );
        }
    }
    assert_eq!(total, 34, "expected 34 corpus rules");
    // fca_crypto's 5 rules have no effective window (C1 / ADR 0006).
    assert_eq!(without_window, 5, "expected 5 always-effective rules");
}

#[test]
fn semantic_form_is_deterministic() {
    let src = fs::read_to_string(corpus_dir().join("mica_stablecoin.yaml")).unwrap();
    let rules = compile_rules(&src).unwrap();
    for r in &rules {
        let a = SemanticRule::from_rule(r);
        let b = SemanticRule::from_rule(r);
        assert_eq!(a, b);
    }
}
