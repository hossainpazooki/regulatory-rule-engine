//! `ke rollback --env <env> [--tag <tag>] --to <hash>`: move a tag pointer back
//! to a prior **published** artifact (ADR 0013, Phase 3b).
//!
//! Eligibility: `is_rollback_eligible(current_state(--to)) == Published`. A
//! `Deprecated` or `Revoked` target is rejected with a typed
//! [`RegistryError::RollbackIneligible`]. On success the tag pointer
//! `tags/<env>/<tag>` (default `current`) is moved to `--to`, and a `tag_moved`
//! event is appended to the target's log (the target stays `Published`).

use crate::registry::backend::RegistryBackend;
use anyhow::Result;

/// Arguments for `ke rollback`.
pub struct RollbackArgs<'a> {
    /// Named environment the tag pointer lives under.
    pub env: &'a str,
    /// Tag to move (default `current`).
    pub tag: &'a str,
    /// The rollback target's 32-byte content hash (already decoded from hex).
    pub to_hash: [u8; 32],
    /// Event clock, unix seconds.
    pub now_unix: u64,
}

/// Outcome of an `ke rollback` run.
#[derive(Debug)]
pub struct RollbackOutcome {
    /// The target's state (always `Published` on success).
    pub target_state: crate::registry::LifecycleState,
    /// The tag pointer that was moved, as `<env>/<tag>`.
    pub tag_ref: String,
}

#[cfg(any(test, feature = "test-keys"))]
pub fn run<B: RegistryBackend>(backend: &B, args: &RollbackArgs<'_>) -> Result<RollbackOutcome> {
    use crate::registry::event::test_keys::REGISTRY_ROOT_KEY_ID;
    use crate::registry::{
        build_tag_moved_event, head_event, is_rollback_eligible, require_current_state,
        RegistryError,
    };
    use ke_artifact::tsa::MockTsa;
    use ke_artifact::SignerRole;

    let target = args.to_hash;

    // Eligibility (ADR 0013): only a Published target is a valid rollback target.
    let state = require_current_state(backend, &target)?;
    if !is_rollback_eligible(state) {
        return Err(RegistryError::RollbackIneligible { state }.into());
    }

    // Append a tag_moved event to the target's log (state stays Published).
    let prior = head_event(backend, &target)?;
    let ts = MockTsa::stamp(&target, args.now_unix);
    let event = build_tag_moved_event(&prior, REGISTRY_ROOT_KEY_ID, SignerRole::Registry, ts)?;
    backend.append_event(&target, &event)?;

    // Move the tag pointer to the target.
    let event_ref = format!("tag_moved@seq{}", event.seq);
    backend.put_pointer(args.env, args.tag, &target, &event_ref)?;

    Ok(RollbackOutcome {
        target_state: require_current_state(backend, &target)?,
        tag_ref: format!("{}/{}", args.env, args.tag),
    })
}

/// Without the `test-keys` feature the CLI cannot sign events. Typed error.
#[cfg(not(any(test, feature = "test-keys")))]
pub fn run<B: RegistryBackend>(_backend: &B, _args: &RollbackArgs<'_>) -> Result<RollbackOutcome> {
    anyhow::bail!(
        "`ke rollback` requires the `test-keys` feature (it appends a registry-root-signed \
         tag_moved event). Build with `--features test-keys`."
    )
}
