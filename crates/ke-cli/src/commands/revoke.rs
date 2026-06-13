//! `ke revoke --hash <h> --policy <hardstop|finishpinned|auditonly> [--reason <s>]`:
//! move `{published, deprecated} -> revoked` and record a revocation-policy
//! SIDECAR (Phase 3b).
//!
//! Appends a standard registry-root-signed `revoked` event (the event shape is
//! frozen — no policy field) **plus** a `revocations/<hash>.json` sidecar
//! `{policy, reason, event_ref, severity}`. For `AuditOnly` the severity is
//! `"high"` (§ 15 emits a high-severity audit event).
//!
//! The registry **records** the policy. **Runtime enforcement**
//! (fail / block-new / audit-emit) is platform/Gate 6 — NOT implemented here.
//! This boundary is deliberate: nothing in this command fails, blocks, or emits
//! a runtime audit event; it only writes the record a platform consumer reads.

use crate::registry::backend::RegistryBackend;
use anyhow::Result;
use ke_core::manifest::RevocationPolicy;

/// Arguments for `ke revoke`.
pub struct RevokeArgs<'a> {
    /// 32-byte artifact content hash (already decoded from hex).
    pub artifact_hash: [u8; 32],
    /// The revocation policy to **record** (not enforce).
    pub policy: RevocationPolicy,
    /// Optional human reason.
    pub reason: Option<&'a str>,
    /// Event clock, unix seconds.
    pub now_unix: u64,
}

/// Outcome of an `ke revoke` run.
pub struct RevokeOutcome {
    pub final_state: crate::registry::LifecycleState,
    /// The severity recorded in the sidecar (`"high"` for AuditOnly).
    pub severity: String,
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
    let severity = severity_for(args.policy).to_string();
    let record = RevocationRecord {
        policy: args.policy,
        reason: args.reason.map(str::to_string),
        event_ref: format!("revoked@seq{}", event.seq),
        severity: severity.clone(),
    };
    backend.put_revocation(&hash, &record)?;

    Ok(RevokeOutcome {
        final_state: require_current_state(backend, &hash)?,
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
