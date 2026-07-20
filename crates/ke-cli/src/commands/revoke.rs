//! `ke revoke --hash <h> [--policy <hardstop|finishpinned|auditonly>]
//! [--reason-class <key_compromise|legal_invalidity|routine_supersession|advisory>]
//! [--reason <s>]`: move `{published, deprecated} -> revoked` and record a
//! revocation-policy SIDECAR (Phase 3b; reason-class decision Gate 6/ADR-0024).
//!
//! Appends a standard registry-root-signed `revoked` event (the event shape is
//! frozen — no policy field) **plus** a `revocations/<hash>.json` sidecar
//! `{policy, reason, event_ref, severity[, reason_class]}`. For `AuditOnly` the
//! severity is `"high"` (§ 15 emits a high-severity audit event).
//!
//! With `--reason-class`, the recorded policy is
//! [`revocation_decision`](ke_core::revocation::revocation_decision)`(class,
//! --policy?)` — a configured `--policy` may raise strictness above the
//! class's floor (ADR-0009 § 4), and one that would LOWER it is rejected
//! outright. The legacy `--policy`-only path is unchanged and its sidecar JSON
//! is shape-identical (no `reason_class` key).
//!
//! The registry **records** the policy. **Runtime enforcement**
//! (fail / block-new / audit-emit) stays outside this repo — the decision
//! layer only informs a consumer (ADR-0024); nothing in this command fails,
//! blocks, or emits a runtime audit event.

use crate::registry::backend::RegistryBackend;
use anyhow::Result;
use ke_core::manifest::RevocationPolicy;
use ke_core::revocation::{
    revocation_decision, revocation_floor, strictness_rank, RevocationReasonClass,
};

/// Arguments for `ke revoke`.
pub struct RevokeArgs<'a> {
    /// 32-byte artifact content hash (already decoded from hex).
    pub artifact_hash: [u8; 32],
    /// The revocation policy to **record** (not enforce). Optional when
    /// `reason_class` supplies a floor; required without one.
    pub policy: Option<RevocationPolicy>,
    /// Why the artifact is revoked (ADR-0009 § 4). Drives the policy floor.
    pub reason_class: Option<RevocationReasonClass>,
    /// Optional human reason.
    pub reason: Option<&'a str>,
    /// Event clock, unix seconds.
    pub now_unix: u64,
}

/// Outcome of an `ke revoke` run.
#[derive(Debug)]
pub struct RevokeOutcome {
    pub final_state: crate::registry::LifecycleState,
    /// The policy actually recorded (post reason-class decision).
    pub recorded_policy: RevocationPolicy,
    /// The severity recorded in the sidecar (`"high"` for AuditOnly).
    pub severity: String,
}

/// The policy to record for the given args: legacy `--policy` verbatim, or the
/// reason-class decision (stricter-of floor and configured). Rejects a
/// configured policy strictly below the class floor, and rejects args carrying
/// neither input.
pub fn recorded_policy(
    policy: Option<RevocationPolicy>,
    reason_class: Option<RevocationReasonClass>,
) -> Result<RevocationPolicy> {
    match (policy, reason_class) {
        (Some(p), None) => Ok(p),
        (configured, Some(class)) => {
            let floor = revocation_floor(class);
            if let Some(c) = configured {
                if strictness_rank(c) < strictness_rank(floor) {
                    anyhow::bail!(
                        "--policy {c:?} is below the {class:?} floor {floor:?}; \
                         a configured policy may only raise strictness above the floor \
                         (ADR-0009 § 4)"
                    );
                }
            }
            Ok(revocation_decision(class, configured))
        }
        (None, None) => anyhow::bail!("revoke requires --policy or --reason-class"),
    }
}

/// Parse a `--policy` value into the recorded [`RevocationPolicy`].
pub fn parse_revocation_policy(s: &str) -> Result<RevocationPolicy> {
    Ok(match s.to_ascii_lowercase().as_str() {
        "hardstop" | "hard_stop" => RevocationPolicy::HardStop,
        "finishpinned" | "finish_pinned" => RevocationPolicy::FinishPinned,
        "auditonly" | "audit_only" => RevocationPolicy::AuditOnly,
        other => anyhow::bail!(
            "unknown revocation policy {other:?}; expected hardstop, finishpinned, or auditonly"
        ),
    })
}

/// Parse a `--reason-class` value into a [`RevocationReasonClass`].
pub fn parse_reason_class(s: &str) -> Result<RevocationReasonClass> {
    Ok(match s.to_ascii_lowercase().as_str() {
        "key_compromise" | "keycompromise" => RevocationReasonClass::KeyCompromise,
        "legal_invalidity" | "legalinvalidity" => RevocationReasonClass::LegalInvalidity,
        "routine_supersession" | "routinesupersession" => {
            RevocationReasonClass::RoutineSupersession
        }
        "advisory" => RevocationReasonClass::Advisory,
        other => anyhow::bail!(
            "unknown reason class {other:?}; expected key_compromise, legal_invalidity, \
             routine_supersession, or advisory"
        ),
    })
}

/// The severity recorded for a policy: `"high"` for `AuditOnly` (§ 15 high-
/// severity audit event), else `"normal"`.
pub fn severity_for(policy: RevocationPolicy) -> &'static str {
    match policy {
        RevocationPolicy::AuditOnly => "high",
        _ => "normal",
    }
}

#[cfg(any(test, feature = "test-keys"))]
pub fn run<B: RegistryBackend>(backend: &B, args: &RevokeArgs<'_>) -> Result<RevokeOutcome> {
    use crate::registry::event::test_keys::REGISTRY_ROOT_KEY_ID;
    use crate::registry::{
        build_transition_event, can_transition, head_event, require_current_state, LifecycleState,
        Preconditions, RevocationRecord,
    };
    use ke_artifact::tsa::MockTsa;
    use ke_artifact::SignerRole;

    // Decide the recorded policy FIRST: a floor violation (or missing inputs)
    // rejects before any state transition or sidecar write.
    let policy = recorded_policy(args.policy, args.reason_class)?;

    let hash = args.artifact_hash;
    let prior_state = require_current_state(backend, &hash)?;
    if !matches!(
        prior_state,
        LifecycleState::Published | LifecycleState::Deprecated
    ) {
        anyhow::bail!("revoke requires prior state published or deprecated, found {prior_state:?}");
    }
    if !can_transition(
        prior_state,
        LifecycleState::Revoked,
        &Preconditions::default(),
    ) {
        anyhow::bail!("revoke precondition failed");
    }

    // Append the (policy-free) revoked event.
    let prior = head_event(backend, &hash)?;
    let ts = MockTsa::stamp(&hash, args.now_unix);
    let event = build_transition_event(
        &prior,
        LifecycleState::Revoked,
        REGISTRY_ROOT_KEY_ID,
        SignerRole::Registry,
        ts,
    )?;
    backend.append_event(&hash, &event)?;

    // Record the policy/reason/severity SIDECAR (recorded, NOT enforced).
    let severity = severity_for(policy).to_string();
    let record = RevocationRecord {
        policy,
        reason: args.reason.map(str::to_string),
        event_ref: format!("revoked@seq{}", event.seq),
        severity: severity.clone(),
        reason_class: args.reason_class,
    };
    backend.put_revocation(&hash, &record)?;

    Ok(RevokeOutcome {
        final_state: require_current_state(backend, &hash)?,
        recorded_policy: policy,
        severity,
    })
}

/// Without the `test-keys` feature the CLI cannot sign events. Typed error.
#[cfg(not(any(test, feature = "test-keys")))]
pub fn run<B: RegistryBackend>(_backend: &B, _args: &RevokeArgs<'_>) -> Result<RevokeOutcome> {
    anyhow::bail!(
        "`ke revoke` requires the `test-keys` feature (it appends a registry-root-signed \
         lifecycle event). Build with `--features test-keys`."
    )
}
