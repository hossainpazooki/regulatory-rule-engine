//! Verification passes (spec §11, §12): T0 (structural), T1 (source coverage +
//! interpretation notes), T4 (cross-rule conflicts), T5 (lint-beyond-compiler).
//! T0/T1 are per-rule and blocking; T4 is cross-rule and severity-dependent
//! (ADR 0005); T5 (Gate 5) is per-rule and advisory by default (see [`t5`]).

pub mod conflict;
pub mod t0;
pub mod t1;
pub mod t4;
pub mod t5;

pub use conflict::{Conflict, ConflictClass, Severity};

use ke_core::ir::rule::RuleIR;

/// Which tier produced a finding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    T0,
    T1,
    /// T5 — lint-beyond-compiler (Gate 5). Advisory by default; see [`t5`].
    T5,
}

/// A per-rule verification finding (T0/T1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    pub tier: Tier,
    pub rule_id: String,
    pub code: &'static str,
    pub message: String,
    pub blocking: bool,
}

/// The full result of verifying a rule set.
#[derive(Clone, Debug, Default)]
pub struct VerificationReport {
    pub findings: Vec<Finding>,
    pub conflicts: Vec<Conflict>,
}

impl VerificationReport {
    /// True if anything would block publication (a blocking T0/T1 finding or a
    /// `Blocking`-severity T4 conflict).
    pub fn has_blocking(&self) -> bool {
        self.findings.iter().any(|f| f.blocking)
            || self
                .conflicts
                .iter()
                .any(|c| c.severity == Severity::Blocking)
    }
}

/// Run T0 + T1 on each rule and T4 across the whole set.
pub fn verify(rules: &[RuleIR]) -> VerificationReport {
    let mut findings = Vec::new();
    for rule in rules {
        findings.extend(t0::check(rule));
        findings.extend(t1::check(rule));
        findings.extend(t5::check(rule));
    }
    VerificationReport {
        findings,
        conflicts: t4::detect_conflicts(rules),
    }
}
