//! Publish-time [`VerificationPolicy`] resolution for `ke publish` (Phase 3b).
//!
//! Two sources, in priority order:
//!
//! 1. `--policy <PolicyBundle.json>` — a serde-deserialized
//!    [`ke_core::manifest::PolicyBundle`]; its `verification_policy` is used.
//! 2. The **built-in default** ([`default_verification_policy`]): require
//!    `SourceFidelity` + `ScenarioCoverage` + `PublicationApproval`, count ≥ 1
//!    each, `t2_t3_mode = Strict`. This mirrors the golden three-type
//!    attestation set the Phase-2 fixtures carry, so a freshly-attested local
//!    artifact publishes under the default with no policy file.
//!
//! The default is **intentionally strict**: omitting any required type is the
//! policy gate `ke publish` enforces via `verify_attestation_set` (rejection
//! rule R6 / R7). Loading a `PolicyBundle` lets an operator widen or narrow that
//! per environment without code changes.

use crate::registry::RegistryError;
use ke_core::manifest::{
    AttestationCount, AttestationType, PolicyBundle, T2T3Mode, VerificationPolicy,
};

/// The built-in default verification policy `ke publish` applies when no
/// `--policy` file is given: the three types a strict publication requires, each
/// at minimum count 1, under `T2T3Mode::Strict`.
pub fn default_verification_policy() -> VerificationPolicy {
    let required = [
        AttestationType::SourceFidelity,
        AttestationType::ScenarioCoverage,
        AttestationType::PublicationApproval,
    ];
    VerificationPolicy {
        t2_t3_mode: T2T3Mode::Strict,
        required_attestation_types: required.to_vec(),
        minimum_attestation_count_per_type: required
            .iter()
            .map(|attestation_type| AttestationCount {
                attestation_type: *attestation_type,
                count: 1,
            })
            .collect(),
    }
}

/// Resolve the publish-time policy: load `path`'s [`PolicyBundle`] and return
/// its `verification_policy`, or the [`default_verification_policy`] when
/// `path` is `None`. A read/parse failure is a typed
/// [`RegistryError::PolicyLoad`].
pub fn resolve_policy(path: Option<&str>) -> Result<VerificationPolicy, RegistryError> {
    match path {
        None => Ok(default_verification_policy()),
        Some(path) => {
            let bytes = std::fs::read(path)
                .map_err(|e| RegistryError::PolicyLoad(format!("read {path}: {e}")))?;
            let bundle: PolicyBundle = serde_json::from_slice(&bytes)
                .map_err(|e| RegistryError::PolicyLoad(format!("parse {path}: {e}")))?;
            Ok(bundle.verification_policy)
        }
    }
}
