//! `ke ml-check --hash <h>`: the DEV STAND-IN T2/T3 step (Phase 3b).
//!
//! Real T2/T3 model verification is **platform-owned** (ADR 0011). This command
//! is a loudly non-authoritative stand-in: it builds a dev [`ConsistencyBlock`]
//! (`execution_environment = "local-dev-standin"`), writes it to the registry
//! **sidecar** `consistency/<hash>.json` (never the artifact envelope — that
//! field is part of the hashed/signed bytes and stays `None`), and appends an
//! `ml_checked` lifecycle event.
//!
//! Precondition (`structurally_verified -> ml_checked`): the prior state is
//! exactly `StructurallyVerified` **and** the consistency sidecar is present.
//! The sidecar is written first, then the precondition reads its presence — the
//! same fail-closed model the precondition table encodes.
//!
//! ## Flagged for a possible follow-up ADR (spec § 8.1 vs § 9)
//!
//! Spec § 8.1 lists `consistency_block` as an in-envelope artifact component,
//! but the § 9 lifecycle attaches T2/T3 evidence *after* compile. Populating the
//! in-envelope slot post-compile would change `artifact_hash` and break § 9
//! immutability + the Phase-1/2 content-address pins. So the in-envelope slot is
//! reserved for compile-time T0/T1/T4 evidence (left `None` in Phase 1) and
//! runtime T2/T3 lives in this registry sidecar. No contract change here.

use crate::registry::backend::RegistryBackend;
#[cfg(any(test, feature = "test-keys"))]
use crate::registry::LifecycleState;
use anyhow::Result;

/// Arguments for `ke ml-check`.
pub struct MlCheckArgs {
    /// 32-byte artifact content hash (already decoded from hex).
    pub artifact_hash: [u8; 32],
    /// Event clock, unix seconds (sourced at the CLI edge).
    pub now_unix: u64,
}

/// Outcome of an `ke ml-check` run.
pub struct MlCheckOutcome {
    pub final_state: crate::registry::LifecycleState,
}

/// The loud, non-authoritative execution-environment marker the dev stand-in
/// consistency block carries. Asserted by tests + the smoke script.
pub const DEV_STANDIN_ENV: &str = "local-dev-standin";

#[cfg(any(test, feature = "test-keys"))]
pub fn run<B: RegistryBackend>(backend: &B, args: &MlCheckArgs) -> Result<MlCheckOutcome> {
    use crate::registry::event::test_keys::REGISTRY_ROOT_KEY_ID;
    use crate::registry::{
        build_transition_event, can_transition, head_event, require_current_state, Preconditions,
    };
    use ke_artifact::tsa::MockTsa;
    use ke_artifact::{ConsistencyBlockBuilder, SignerRole};
    use ke_core::manifest::T2T3Mode;

    let hash = args.artifact_hash;

    // Precondition (state half): prior must be exactly structurally_verified.
    let prior_state = require_current_state(backend, &hash)?;
    if prior_state != LifecycleState::StructurallyVerified {
        anyhow::bail!("ml-check requires prior state structurally_verified, found {prior_state:?}");
    }

    // Build + write the dev stand-in consistency block SIDECAR. Loudly
    // non-authoritative; real T2/T3 is platform-owned (ADR 0011).
    let block = ConsistencyBlockBuilder::new(
        "dev_standin_pass",
        T2T3Mode::Advisory,
        "dev-standin",
        "0.0.0",
        args.now_unix.to_string(),
        DEV_STANDIN_ENV,
    )
    .reviewer_rationale(
        "DEV STAND-IN: non-authoritative local ml-check; real T2/T3 is platform-owned (ADR 0011)",
    )
    .build()
    .map_err(|e| anyhow::anyhow!("build dev consistency block: {e}"))?;
    backend.put_consistency(&hash, &block)?;

    // Precondition (evidence half): the sidecar is now present.
    let pre = Preconditions {
        consistency_block_present: backend.read_consistency(&hash)?.is_some(),
        ..Preconditions::default()
    };
    if !can_transition(
        LifecycleState::StructurallyVerified,
        LifecycleState::MlChecked,
        &pre,
    ) {
        anyhow::bail!("ml-check precondition failed: consistency block not present");
    }

    // Append the ml_checked event, chained onto the validated head, registry-
    // root-signed. The triggering authority is the registry (dev stand-in).
    let prior = head_event(backend, &hash)?;
    let ts = MockTsa::stamp(&hash, args.now_unix);
    let event = build_transition_event(
        &prior,
        LifecycleState::MlChecked,
        REGISTRY_ROOT_KEY_ID,
        SignerRole::Registry,
        ts,
    )?;
    backend.append_event(&hash, &event)?;

    Ok(MlCheckOutcome {
        final_state: require_current_state(backend, &hash)?,
    })
}

/// Without the `test-keys` feature the CLI cannot sign events, so `ke ml-check`
/// is unavailable (it appends a registry-root-signed event). Typed error.
#[cfg(not(any(test, feature = "test-keys")))]
pub fn run<B: RegistryBackend>(_backend: &B, _args: &MlCheckArgs) -> Result<MlCheckOutcome> {
    anyhow::bail!(
        "`ke ml-check` requires the `test-keys` feature (it appends a registry-root-signed \
         lifecycle event). Build with `--features test-keys`. Production signing keys are an \
         infra/ADR-0009 concern."
    )
}
