//! Publish-time [`VerificationPolicy`] resolution for `ke publish` (Phase 3b).
//!
//! Two sources, in priority order:
//!
//! 1. `--policy <PolicyBundle.json>` — a serde-deserialized
//!    [`ke_core::manifest::PolicyBundle`]; its `verification_policy` is used.
//!    An explicit bundle overrides the kind-aware default for **every** kind.
//! 2. The **kind-aware built-in default** ([`policy_for_kind`]), selected by the
//!    artifact's [`ArtifactKind`] when no `--policy` file is given:
//!    - Every rule-shaped kind (`RegimePack` and the other non-`IntentSpec`
//!      kinds) uses [`default_verification_policy`]: `SourceFidelity` +
//!      `ScenarioCoverage` + `PublicationApproval`, count ≥ 1 each,
//!      `t2_t3_mode = Strict`. This mirrors the golden three-type attestation
//!      set the Phase-2 fixtures carry, so a freshly-attested local RegimePack
//!      publishes under the default with no policy file.
//!    - `IntentSpec` uses [`intentspec_verification_policy`]: exactly
//!      `SourceFidelity` + `PublicationApproval` (ADR-0021 §5). `ScenarioCoverage`
//!      is a rule-scenario concept that does not apply to authorization criteria,
//!      so it is not required for an IntentSpec.
//!
//! Each default is **intentionally strict**: omitting any required type is the
//! policy gate `ke publish` enforces via `verify_attestation_set` (rejection
//! rule R6 / R7). Loading a `PolicyBundle` lets an operator widen or narrow that
//! per environment without code changes.

use crate::registry::RegistryError;
use ke_core::manifest::{
    ArtifactKind, AttestationCount, AttestationType, PolicyBundle, T2T3Mode, VerificationPolicy,
};

/// Build a strict (`T2T3Mode::Strict`) policy requiring each listed type at
/// minimum count 1. Shared by the kind-specific defaults below so the two policy
/// shapes differ only in their required-type list, not in count/mode wiring.
fn strict_policy(required: &[AttestationType]) -> VerificationPolicy {
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

/// The built-in default verification policy `ke publish` applies to rule-shaped
/// kinds (`RegimePack` and the other non-`IntentSpec` kinds) when no `--policy`
/// file is given: the three types a strict publication requires, each at minimum
/// count 1, under `T2T3Mode::Strict`.
pub fn default_verification_policy() -> VerificationPolicy {
    strict_policy(&[
        AttestationType::SourceFidelity,
        AttestationType::ScenarioCoverage,
        AttestationType::PublicationApproval,
    ])
}

/// The built-in verification policy `ke publish` applies to an `IntentSpec`
/// artifact when no `--policy` file is given (ADR-0021 §5): **exactly**
/// `SourceFidelity` + `PublicationApproval`, each at minimum count 1, under
/// `T2T3Mode::Strict`. `ScenarioCoverage` — a rule-scenario attestation — is not
/// required for authorization criteria, so it is deliberately omitted.
pub fn intentspec_verification_policy() -> VerificationPolicy {
    strict_policy(&[
        AttestationType::SourceFidelity,
        AttestationType::PublicationApproval,
    ])
}

/// Select the built-in verification policy for an artifact kind. `IntentSpec`
/// gets its own required-attestation set ([`intentspec_verification_policy`]);
/// every other kind keeps the strict three-type [`default_verification_policy`].
/// Callers with a `--policy` override should use [`resolve_policy`], which layers
/// the override on top of this selection.
pub fn policy_for_kind(kind: ArtifactKind) -> VerificationPolicy {
    match kind {
        ArtifactKind::IntentSpec => intentspec_verification_policy(),
        ArtifactKind::RegimePack
        | ArtifactKind::EquivalenceMatrix
        | ArtifactKind::TestCorpus
        | ArtifactKind::PolicyBundle => default_verification_policy(),
    }
}

/// Resolve the publish-time policy: load `path`'s [`PolicyBundle`] and return
/// its `verification_policy`, or the kind-aware built-in default
/// ([`policy_for_kind`]) for `kind` when `path` is `None`. An explicit `--policy`
/// bundle overrides the kind-aware default for every kind. A read/parse failure
/// is a typed [`RegistryError::PolicyLoad`].
pub fn resolve_policy(
    path: Option<&str>,
    kind: ArtifactKind,
) -> Result<VerificationPolicy, RegistryError> {
    match path {
        None => Ok(policy_for_kind(kind)),
        Some(path) => {
            let bytes = std::fs::read(path)
                .map_err(|e| RegistryError::PolicyLoad(format!("read {path}: {e}")))?;
            let bundle: PolicyBundle = serde_json::from_slice(&bytes)
                .map_err(|e| RegistryError::PolicyLoad(format!("parse {path}: {e}")))?;
            Ok(bundle.verification_policy)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Required types as a sorted set, for order-independent comparison.
    fn required_set(policy: &VerificationPolicy) -> Vec<AttestationType> {
        let mut types = policy.required_attestation_types.clone();
        types.sort();
        types
    }

    #[test]
    fn intentspec_requires_exactly_source_fidelity_and_publication_approval() {
        let policy = policy_for_kind(ArtifactKind::IntentSpec);
        assert_eq!(
            required_set(&policy),
            vec![
                AttestationType::SourceFidelity,
                AttestationType::PublicationApproval,
            ],
            "IntentSpec must require exactly {{SourceFidelity, PublicationApproval}} (ADR-0021 §5)"
        );
        assert_eq!(policy.t2_t3_mode, T2T3Mode::Strict);
        // Each required type is demanded at least once.
        for entry in &policy.minimum_attestation_count_per_type {
            assert_eq!(entry.count, 1);
        }
        assert_eq!(policy.minimum_attestation_count_per_type.len(), 2);
    }

    #[test]
    fn intentspec_default_does_not_require_scenario_coverage() {
        let policy = policy_for_kind(ArtifactKind::IntentSpec);
        assert!(
            !policy
                .required_attestation_types
                .contains(&AttestationType::ScenarioCoverage),
            "ScenarioCoverage is a rule-scenario type and must not gate an IntentSpec"
        );
    }

    #[test]
    fn regimepack_keeps_the_strict_three_type_default() {
        let policy = policy_for_kind(ArtifactKind::RegimePack);
        assert_eq!(policy, default_verification_policy());
        assert_eq!(
            required_set(&policy),
            vec![
                AttestationType::SourceFidelity,
                AttestationType::ScenarioCoverage,
                AttestationType::PublicationApproval,
            ],
        );
    }

    #[test]
    fn non_intentspec_kinds_all_use_the_rule_default() {
        for kind in [
            ArtifactKind::RegimePack,
            ArtifactKind::EquivalenceMatrix,
            ArtifactKind::TestCorpus,
            ArtifactKind::PolicyBundle,
        ] {
            assert_eq!(
                policy_for_kind(kind),
                default_verification_policy(),
                "{kind:?} must fall through to the rule-shaped default policy"
            );
        }
    }

    #[test]
    fn resolve_policy_none_selects_by_kind() {
        assert_eq!(
            resolve_policy(None, ArtifactKind::IntentSpec).unwrap(),
            intentspec_verification_policy(),
        );
        assert_eq!(
            resolve_policy(None, ArtifactKind::RegimePack).unwrap(),
            default_verification_policy(),
        );
    }
}
