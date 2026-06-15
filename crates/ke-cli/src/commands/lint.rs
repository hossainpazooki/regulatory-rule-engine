//! `ke lint <yaml> [--deny]`: lint a YAML rule document with the T5
//! lint-beyond-compiler tier.
//!
//! NON-AUTHORITATIVE. This command compiles the YAML to `RuleIR` and runs
//! **only** [`ke_compiler::verify::t5::check`] per rule, returning the T5
//! findings. It reads no registry, opens no backend, signs nothing, and writes
//! nothing — so unlike `ke compile` it is **not** gated behind the `test-keys`
//! feature (it never touches a signing key).
//!
//! T5 findings are advisory by default. With `--deny`, the dispatcher maps a
//! nonzero `blocking_count` (blocking T5 findings only) to exit code 2.

use anyhow::Result;
use ke_compiler::verify::{t5, Finding, Tier};

/// Arguments for `ke lint`.
pub struct LintArgs<'a> {
    pub yaml_path: &'a str,
    /// If true, a blocking T5 finding (and only T5) makes `run` return a nonzero report.
    pub deny: bool,
}

/// Outcome of a `ke lint` run.
pub struct LintOutcome {
    /// T5 findings only (the lint tier).
    pub findings: Vec<Finding>,
    /// Count of `blocking: true` findings among `findings`.
    pub blocking_count: usize,
}

/// Compiles the YAML to RuleIR, runs ONLY t5::check per rule, returns findings.
/// Reads no registry, signs nothing, writes nothing.
pub fn run(args: &LintArgs<'_>) -> Result<LintOutcome> {
    let source = std::fs::read_to_string(args.yaml_path)
        .map_err(|e| anyhow::anyhow!("read {}: {e}", args.yaml_path))?;
    let rules = ke_compiler::compile_rules(&source)
        .map_err(|e| anyhow::anyhow!("compile {}: {e:?}", args.yaml_path))?;

    let mut findings = Vec::new();
    for rule in &rules {
        findings.extend(t5::check(rule));
    }
    // T5 is the only tier this command runs; assert the invariant so a future
    // wiring mistake (e.g. someone routing T0/T1 in here) is caught loudly.
    debug_assert!(
        findings.iter().all(|f| f.tier == Tier::T5),
        "ke lint must return only T5 findings"
    );

    let blocking_count = findings.iter().filter(|f| f.blocking).count();
    // `deny` does not change which findings are produced; it only governs the
    // exit-code mapping at the CLI edge. Touch it here so the field is part of
    // the contract surface without affecting the (pure) finding set.
    let _ = args.deny;

    Ok(LintOutcome {
        findings,
        blocking_count,
    })
}
