//! Revocation runtime-decision tests (Gate 6, ADR-0024; policy source:
//! ADR-0009 § 4 reason-class table + ADR-0013 floor semantics).
//!
//! The decision layer is pure and consumer-agnostic: it maps a reason class to
//! a policy floor and combines it with an optionally-configured policy by
//! *stricter-of* — an environment policy may raise strictness above the floor,
//! never lower it. It informs a consumer; `verify` stays fail-closed regardless.

use ke_core::manifest::RevocationPolicy;
use ke_core::revocation::{
    revocation_decision, revocation_floor, strictness_rank, RevocationReasonClass,
};

/// ADR-0009 § 4: the reason-class → policy-floor matrix.
#[test]
fn reason_class_floor_matrix() {
    assert_eq!(
        revocation_floor(RevocationReasonClass::KeyCompromise),
        RevocationPolicy::HardStop
    );
    assert_eq!(
        revocation_floor(RevocationReasonClass::LegalInvalidity),
        RevocationPolicy::HardStop
    );
    assert_eq!(
        revocation_floor(RevocationReasonClass::RoutineSupersession),
        RevocationPolicy::FinishPinned
    );
    assert_eq!(
        revocation_floor(RevocationReasonClass::Advisory),
        RevocationPolicy::AuditOnly
    );
}

/// HardStop > FinishPinned > AuditOnly.
#[test]
fn strictness_ordering() {
    assert!(
        strictness_rank(RevocationPolicy::HardStop)
            > strictness_rank(RevocationPolicy::FinishPinned)
    );
    assert!(
        strictness_rank(RevocationPolicy::FinishPinned)
            > strictness_rank(RevocationPolicy::AuditOnly)
    );
}

/// No configured policy: the decision is exactly the floor.
#[test]
fn decision_without_configured_policy_is_the_floor() {
    for reason in [
        RevocationReasonClass::KeyCompromise,
        RevocationReasonClass::LegalInvalidity,
        RevocationReasonClass::RoutineSupersession,
        RevocationReasonClass::Advisory,
    ] {
        assert_eq!(revocation_decision(reason, None), revocation_floor(reason));
    }
}

/// A configured policy may only RAISE strictness above the floor.
#[test]
fn configured_policy_raises_above_floor() {
    // Advisory floor is AuditOnly; configured HardStop wins.
    assert_eq!(
        revocation_decision(
            RevocationReasonClass::Advisory,
            Some(RevocationPolicy::HardStop)
        ),
        RevocationPolicy::HardStop
    );
    // RoutineSupersession floor is FinishPinned; configured HardStop wins.
    assert_eq!(
        revocation_decision(
            RevocationReasonClass::RoutineSupersession,
            Some(RevocationPolicy::HardStop)
        ),
        RevocationPolicy::HardStop
    );
}

/// A configured policy can never LOWER strictness below the floor.
#[test]
fn configured_policy_cannot_lower_below_floor() {
    // KeyCompromise floor is HardStop; a weaker configured policy is ignored.
    assert_eq!(
        revocation_decision(
            RevocationReasonClass::KeyCompromise,
            Some(RevocationPolicy::AuditOnly)
        ),
        RevocationPolicy::HardStop
    );
    assert_eq!(
        revocation_decision(
            RevocationReasonClass::LegalInvalidity,
            Some(RevocationPolicy::FinishPinned)
        ),
        RevocationPolicy::HardStop
    );
    // Floor equal to configured: unchanged.
    assert_eq!(
        revocation_decision(
            RevocationReasonClass::RoutineSupersession,
            Some(RevocationPolicy::FinishPinned)
        ),
        RevocationPolicy::FinishPinned
    );
}

/// The reason class round-trips through serde JSON as a plain string — the
/// shape the `revocations/<hash>.json` sidecar and serve DTOs use.
#[test]
fn reason_class_serde_round_trip() {
    for (reason, expected_json) in [
        (RevocationReasonClass::KeyCompromise, "\"KeyCompromise\""),
        (
            RevocationReasonClass::LegalInvalidity,
            "\"LegalInvalidity\"",
        ),
        (
            RevocationReasonClass::RoutineSupersession,
            "\"RoutineSupersession\"",
        ),
        (RevocationReasonClass::Advisory, "\"Advisory\""),
    ] {
        let json = serde_json::to_string(&reason).expect("serialize");
        assert_eq!(json, expected_json);
        let back: RevocationReasonClass = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, reason);
    }
}
