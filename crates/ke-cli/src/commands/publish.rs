//! `ke publish --hash <h> --env <env> [--tag <tag>] [--policy <bundle.json>]`:
//! move `expert_attested -> published` and set the tag pointer (Phase 3b).
//!
//! Loads the verification policy (the strict built-in default, or a
//! `--policy <PolicyBundle.json>` override — see [`crate::policy`]), decodes the
//! stored artifact, and runs `verify_attestation_set`. On a missing required
//! type the publish **fails** with a typed [`RegistryError::AttestationSetRejected`]
//! — this is the policy gate. When the set verifies and the prior state is
//! exactly `expert_attested`, it appends the `published` event and writes the
//! tag pointer `tags/<env>/<tag>` (default tag `current`).

use crate::registry::backend::RegistryBackend;
use anyhow::Result;

/// Arguments for `ke publish`.
pub struct PublishArgs<'a> {
    /// 32-byte artifact content hash (already decoded from hex).
    pub artifact_hash: [u8; 32],
    /// Named environment the tag pointer is written under.
    pub env: &'a str,
    /// Tag to move (default `current`).
    pub tag: &'a str,
    /// Optional `--policy <PolicyBundle.json>` path; `None` uses the strict
    /// built-in default.
    pub policy_path: Option<&'a str>,
    /// Event clock, unix seconds.
    pub now_unix: u64,
}

/// Outcome of an `ke publish` run.
#[derive(Debug)]
pub struct PublishOutcome {
    pub final_state: crate::registry::LifecycleState,
    /// The tag pointer that was set, as `<env>/<tag>`.
    pub tag_ref: String,
}

#[cfg(any(test, feature = "test-keys"))]
pub fn run<B: RegistryBackend>(backend: &B, args: &PublishArgs<'_>) -> Result<PublishOutcome> {
    use crate::commands::attest::{local_policy_context, test_expert_key_directory};
    use crate::registry::event::test_keys::REGISTRY_ROOT_KEY_ID;
    use crate::registry::{
        build_transition_event, can_transition, head_event, require_current_state, LifecycleState,
        Preconditions, RegistryError,
    };
    use ke_artifact::tsa::MockTsa;
    use ke_artifact::{decode_artifact, verify_attestation_set, SignerRole};

    let hash = args.artifact_hash;

    // Precondition (state half): prior must be exactly expert_attested.
    let prior_state = require_current_state(backend, &hash)?;
    if prior_state != LifecycleState::ExpertAttested {
        anyhow::bail!("publish requires prior state expert_attested, found {prior_state:?}");
    }

    // Resolve the verification policy (default or --policy bundle).
    let policy = crate::policy::resolve_policy(args.policy_path)?;

    // Decode the stored artifact and run the policy gate.
    let kew = backend.read_artifact_kew(&hash)?;
    let (artifact, _envelope_len) =
        decode_artifact(&kew).map_err(|e| RegistryError::ArtifactDecode(e.to_string()))?;
    let key_directory = test_expert_key_directory();
    let context = local_policy_context(args.now_unix);
    if let Err(rejections) = verify_attestation_set(&artifact, &policy, &key_directory, &context) {
        let rendered: Vec<String> = rejections.iter().map(|r| r.to_string()).collect();
        return Err(RegistryError::AttestationSetRejected(rendered).into());
    }

    // Precondition (registry-policy half): prior is exactly expert_attested.
    let pre = Preconditions {
        prior_is_expert_attested: true,
        ..Preconditions::default()
    };
    if !can_transition(
        LifecycleState::ExpertAttested,
        LifecycleState::Published,
        &pre,
    ) {
        anyhow::bail!("publish precondition failed");
    }

    // Append published, chained onto the validated head.
    let prior = head_event(backend, &hash)?;
    let ts = MockTsa::stamp(&hash, args.now_unix);
    let event = build_transition_event(
        &prior,
        LifecycleState::Published,
        REGISTRY_ROOT_KEY_ID,
        SignerRole::Registry,
        ts,
    )?;
    backend.append_event(&hash, &event)?;

    // Set the tag pointer to the published hash.
    let event_ref = format!("published@seq{}", event.seq);
    backend.put_pointer(args.env, args.tag, &hash, &event_ref)?;

    Ok(PublishOutcome {
        final_state: require_current_state(backend, &hash)?,
        tag_ref: format!("{}/{}", args.env, args.tag),
    })
}

/// Without the `test-keys` feature the CLI cannot sign events, so `ke publish`
/// is unavailable. Typed error.
#[cfg(not(any(test, feature = "test-keys")))]
pub fn run<B: RegistryBackend>(_backend: &B, _args: &PublishArgs<'_>) -> Result<PublishOutcome> {
    anyhow::bail!(
        "`ke publish` requires the `test-keys` feature (it appends a registry-root-signed \
         lifecycle event). Build with `--features test-keys`."
    )
}
