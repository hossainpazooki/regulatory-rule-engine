//! Revocation runtime-decision (Gate 6, ADR-0024).
//!
//! Operationalizes the ADR-0009 § 4 reason-class → policy table and the
//! ADR-0013 floor semantics as a pure, consumer-agnostic function: an
//! environment-configured policy may only *raise* strictness above the
//! reason-class floor, never lower it.
//!
//! This layer **informs** a consumer; it never loosens `verify`, which stays
//! fail-closed on any non-`Published` artifact regardless of the decision
//! (ADR-0024 invariant). No orchestrator consumer runs the decision yet —
//! honest groundwork, stated in ADR-0024.
//!
//! [`RevocationPolicy`] lives in [`crate::manifest`] and is part of the
//! canonical `PolicyBundle` encoding; nothing here changes it. The types added
//! here appear only in the registry's `revocations/<hash>.json` sidecar and
//! serve DTOs, which are outside the canonical envelope.

use crate::manifest::RevocationPolicy;
use serde::{Deserialize, Serialize};

/// Why an artifact was revoked (ADR-0009 § 4 reason classes).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RevocationReasonClass {
    /// An expert or registry key is compromised — trust in signatures is void.
    KeyCompromise,
    /// The rule content is legally invalid (misread source, court reversal).
    LegalInvalidity,
    /// Superseded by a newer artifact in the ordinary course.
    RoutineSupersession,
    /// Informational; no defect in the artifact itself.
    Advisory,
}

/// The minimum policy the reason class demands (ADR-0009 § 4 table).
pub fn revocation_floor(reason: RevocationReasonClass) -> RevocationPolicy {
    match reason {
        RevocationReasonClass::KeyCompromise | RevocationReasonClass::LegalInvalidity => {
            RevocationPolicy::HardStop
        }
        RevocationReasonClass::RoutineSupersession => RevocationPolicy::FinishPinned,
        RevocationReasonClass::Advisory => RevocationPolicy::AuditOnly,
    }
}

/// Strictness rank: `HardStop` > `FinishPinned` > `AuditOnly`. Kept here (not
/// as `Ord` on [`RevocationPolicy`]) so the canonical enum stays untouched.
pub fn strictness_rank(policy: RevocationPolicy) -> u8 {
    match policy {
        RevocationPolicy::HardStop => 2,
        RevocationPolicy::FinishPinned => 1,
        RevocationPolicy::AuditOnly => 0,
    }
}

/// The effective policy for a revocation: **stricter-of(floor, configured)**.
/// A configured environment policy may only raise strictness above the
/// reason-class floor, never lower it (ADR-0009 § 4 / ADR-0013).
pub fn revocation_decision(
    reason: RevocationReasonClass,
    configured: Option<RevocationPolicy>,
) -> RevocationPolicy {
    let floor = revocation_floor(reason);
    match configured {
        Some(c) if strictness_rank(c) > strictness_rank(floor) => c,
        _ => floor,
    }
}
