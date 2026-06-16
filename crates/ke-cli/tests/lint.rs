//! Gate 5 integration test for `ke lint` (the T5 lint-beyond-compiler tier).
//!
//! Runs `lint::run` on a corpus YAML and asserts:
//!   (a) it succeeds,
//!   (b) it returns only `Tier::T5` findings,
//!   (c) on the current clean fixture `blocking_count == 0` — proving
//!       advisory-by-default does not break the compile gate.
//!
//! `ke lint` is non-authoritative: it reads no registry, opens no backend, and
//! signs nothing, so this test needs no `test-keys` feature and no tempdir.

use ke_cli::commands::lint::{self, LintArgs};
use ke_compiler::verify::Tier;
use std::path::PathBuf;

fn fixture_path(name: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/rules")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn lint_clean_corpus_yaml_is_advisory_only() {
    let yaml = fixture_path("mica_stablecoin.yaml");
    let outcome = lint::run(&LintArgs {
        yaml_path: &yaml,
        deny: false,
    })
    .expect("lint run succeeds on a corpus YAML");

    // (b) only T5 findings.
    assert!(
        outcome.findings.iter().all(|f| f.tier == Tier::T5),
        "ke lint returns only T5 findings, got: {:?}",
        outcome
            .findings
            .iter()
            .map(|f| (f.tier, f.code))
            .collect::<Vec<_>>()
    );

    // (c) advisory-by-default: a clean corpus rule produces no blocking T5
    // finding, so the compile gate is unaffected.
    assert_eq!(
        outcome.blocking_count, 0,
        "clean corpus YAML must produce zero blocking T5 findings"
    );
    assert!(
        outcome.findings.iter().all(|f| !f.blocking),
        "no finding is blocking on the clean corpus"
    );
}

#[test]
fn lint_runs_across_multiple_corpus_files_with_no_blocking() {
    // Sweep several corpus files to keep the advisory-by-default guarantee
    // honest beyond a single fixture.
    for name in [
        "mica_stablecoin.yaml",
        "fca_crypto.yaml",
        "mas_psa.yaml",
        "finma_dlt.yaml",
    ] {
        let yaml = fixture_path(name);
        let outcome = lint::run(&LintArgs {
            yaml_path: &yaml,
            deny: true,
        })
        .unwrap_or_else(|e| panic!("lint {name}: {e}"));
        assert!(
            outcome.findings.iter().all(|f| f.tier == Tier::T5),
            "{name}: only T5 findings"
        );
        assert_eq!(
            outcome.blocking_count, 0,
            "{name}: no blocking T5 findings on the corpus"
        );
    }
}
