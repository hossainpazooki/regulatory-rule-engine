//! The `ConsistencyBlock` — T2/T3 verification evidence (spec § 11), moved
//! here from `artifact.rs` in Phase 2 (the original `crate::artifact` path
//! stays valid via re-export).
//!
//! Phase 2 adds a plain builder with basic field validation only. T2/T3
//! evidence is produced by the **platform-owned** sidecar (ADR 0011), and the
//! `VerificationReport -> ConsistencyBlock` adapter belongs in `ke-cli`
//! (Phase 3, where both crates are visible) — this module deliberately has
//! **no ke-compiler dependency**. Committed golden artifacts keep
//! `consistency_block = None` until that evidence path exists.

use ke_core::manifest::T2T3Mode;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// T2/T3 verification evidence (spec § 11). Lives inside the hashed+signed
/// envelope, so a populated block is bound by the content address.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsistencyBlock {
    /// Tier result (T2/T3 outcome).
    pub tier_result: String,
    /// Publication-policy mode the evidence was produced under.
    pub policy_mode: T2T3Mode,
    /// Model name for T2/T3.
    pub model_name: String,
    /// Model version for T2/T3.
    pub model_version: String,
    /// Prompt or scoring profile version, where applicable.
    pub scoring_profile_version: Option<String>,
    /// Evidence references.
    pub evidence_references: Vec<String>,
    /// Reviewer overrides.
    pub reviewer_overrides: Vec<String>,
    /// Reviewer rationale.
    pub reviewer_rationale: Option<String>,
    /// Timestamp (representation pending the trusted-timestamp ADR 0010).
    pub timestamp: String,
    /// Execution environment the verification ran in.
    pub execution_environment: String,
}

/// A required field of a [`ConsistencyBlock`] failed basic validation.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum ConsistencyError {
    /// A required string field is empty.
    #[error("consistency block field `{0}` must be non-empty")]
    EmptyField(&'static str),
    /// An evidence reference is empty.
    #[error("consistency block evidence reference at index {0} is empty")]
    EmptyEvidenceReference(usize),
}

/// Builder for [`ConsistencyBlock`] with basic field validation at
/// [`build`](ConsistencyBlockBuilder::build). Required fields are
/// constructor arguments; optional fields are chained.
#[derive(Clone, Debug)]
pub struct ConsistencyBlockBuilder {
    block: ConsistencyBlock,
}

impl ConsistencyBlockBuilder {
    /// Start a builder from the required fields.
    pub fn new(
        tier_result: impl Into<String>,
        policy_mode: T2T3Mode,
        model_name: impl Into<String>,
        model_version: impl Into<String>,
        timestamp: impl Into<String>,
        execution_environment: impl Into<String>,
    ) -> Self {
        Self {
            block: ConsistencyBlock {
                tier_result: tier_result.into(),
                policy_mode,
                model_name: model_name.into(),
                model_version: model_version.into(),
                scoring_profile_version: None,
                evidence_references: Vec::new(),
                reviewer_overrides: Vec::new(),
                reviewer_rationale: None,
                timestamp: timestamp.into(),
                execution_environment: execution_environment.into(),
            },
        }
    }

    /// Set the prompt / scoring-profile version.
    pub fn scoring_profile_version(mut self, version: impl Into<String>) -> Self {
        self.block.scoring_profile_version = Some(version.into());
        self
    }

    /// Append one evidence reference.
    pub fn evidence_reference(mut self, reference: impl Into<String>) -> Self {
        self.block.evidence_references.push(reference.into());
        self
    }

    /// Append one reviewer override.
    pub fn reviewer_override(mut self, override_entry: impl Into<String>) -> Self {
        self.block.reviewer_overrides.push(override_entry.into());
        self
    }

    /// Set the reviewer rationale.
    pub fn reviewer_rationale(mut self, rationale: impl Into<String>) -> Self {
        self.block.reviewer_rationale = Some(rationale.into());
        self
    }

    /// Validate and produce the block. Basic structural validation only —
    /// non-empty required strings and evidence references; *semantic*
    /// validity of T2/T3 evidence is the platform sidecar's job (ADR 0011).
    pub fn build(self) -> Result<ConsistencyBlock, ConsistencyError> {
        let block = self.block;
        for (name, value) in [
            ("tier_result", &block.tier_result),
            ("model_name", &block.model_name),
            ("model_version", &block.model_version),
            ("timestamp", &block.timestamp),
            ("execution_environment", &block.execution_environment),
        ] {
            if value.is_empty() {
                return Err(ConsistencyError::EmptyField(name));
            }
        }
        if let Some(index) = block.evidence_references.iter().position(String::is_empty) {
            return Err(ConsistencyError::EmptyEvidenceReference(index));
        }
        Ok(block)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn builder() -> ConsistencyBlockBuilder {
        ConsistencyBlockBuilder::new(
            "t2_pass",
            T2T3Mode::Advisory,
            "model-x",
            "1.0.0",
            "2026-06-12T00:00:00Z",
            "ci",
        )
    }

    #[test]
    fn builder_happy_path() {
        let block = builder()
            .scoring_profile_version("profile-1")
            .evidence_reference("evidence://run/1")
            .reviewer_override("override-1")
            .reviewer_rationale("looks consistent")
            .build()
            .expect("valid block");
        assert_eq!(block.tier_result, "t2_pass");
        assert_eq!(block.scoring_profile_version.as_deref(), Some("profile-1"));
        assert_eq!(block.evidence_references, vec!["evidence://run/1"]);
    }

    #[test]
    fn empty_required_field_rejected() {
        let err = ConsistencyBlockBuilder::new(
            "",
            T2T3Mode::Strict,
            "model-x",
            "1.0.0",
            "2026-06-12T00:00:00Z",
            "ci",
        )
        .build()
        .expect_err("empty tier_result must fail");
        assert_eq!(err, ConsistencyError::EmptyField("tier_result"));
    }

    #[test]
    fn empty_evidence_reference_rejected() {
        let err = builder()
            .evidence_reference("")
            .build()
            .expect_err("empty evidence reference must fail");
        assert_eq!(err, ConsistencyError::EmptyEvidenceReference(0));
    }
}
