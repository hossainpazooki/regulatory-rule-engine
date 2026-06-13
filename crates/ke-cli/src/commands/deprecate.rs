//! `ke deprecate --hash <h>`: move `published -> deprecated` (Phase 3b).
//!
//! A plain lifecycle end-state transition: precondition prior == `published`,
//! append a registry-root-signed `deprecated` event. No sidecar, no tag move.

use crate::registry::backend::RegistryBackend;
use anyhow::Result;

/// Arguments for `ke deprecate`.
pub struct DeprecateArgs {
    /// 32-byte artifact content hash (already decoded from hex).
    pub artifact_hash: [u8; 32],
    /// Event clock, unix seconds.
    pub now_unix: u64,
}

/// Outcome of an `ke deprecate` run.
pub struct DeprecateOutcome {
    pub final_state: crate::registry::LifecycleState,
}

#[cfg(any(test, feature = "test-keys"))]
pub fn run<B: RegistryBackend>(backend: &B, args: &DeprecateArgs) -> Result<DeprecateOutcome> {
    use crate::registry::event::test_keys::REGISTRY_ROOT_KEY_ID;
    use crate::registry::{
        build_transition_event, can_transition, head_event, require_current_state, LifecycleState,
        Preconditions,
    };
    use ke_artifact::tsa::MockTsa;
    use ke_artifact::SignerRole;

    let hash = args.artifact_hash;
    let prior_state = require_current_state(backend, &hash)?;
    if prior_state != LifecycleState::Published {
        anyhow::bail!("deprecate requires prior state published, found {prior_state:?}");
    }
    // (Published, Deprecated) is an unconditional edge in the table.
    if !can_transition(
        LifecycleState::Published,
        LifecycleState::Deprecated,
        &Preconditions::default(),
    ) {
        anyhow::bail!("deprecate precondition failed");
    }

    let prior = head_event(backend, &hash)?;
    let ts = MockTsa::stamp(&hash, args.now_unix);
    let event = build_transition_event(
        &prior,
        LifecycleState::Deprecated,
        REGISTRY_ROOT_KEY_ID,
        SignerRole::Registry,
        ts,
    )?;
    backend.append_event(&hash, &event)?;

    Ok(DeprecateOutcome {
        final_state: require_current_state(backend, &hash)?,
    })
}

/// Without the `test-keys` feature the CLI cannot sign events. Typed error.
#[cfg(not(any(test, feature = "test-keys")))]
pub fn run<B: RegistryBackend>(_backend: &B, _args: &DeprecateArgs) -> Result<DeprecateOutcome> {
    anyhow::bail!(
        "`ke deprecate` requires the `test-keys` feature (it appends a registry-root-signed \
         lifecycle event). Build with `--features test-keys`."
    )
}
