//! `ke compile <yaml> --regime <id> [--env <env>]`: compile a YAML rule
//! document, verify it, assemble a signed content-addressed artifact, store it,
//! and record the `draft` (then, if preconditions hold, `structurally_verified`)
//! lifecycle events.
//!
//! # Flow (plan / contract)
//!
//! 1. `ke_compiler::compile_rules(yaml)` -> `RuleIR`s.
//! 2. `ke_compiler::verify::verify(&rules)` — **abort nonzero** and print the
//!    blocking findings if `has_blocking()`.
//! 3. Build a synthetic manifest (`ke_core::examples::synthetic_manifest`) and
//!    `ke_artifact::Artifact::assemble` with the fixed-seed **test compiler
//!    key** (`key_id = "test-fixed-seed-1"`; never a production key, never
//!    `OsRng`/`getrandom`).
//! 4. `backend.put_artifact` (`.kew` + manifest.json + schema.json review
//!    views).
//! 5. Append the `draft` event (`prior_state = None`), registry-root-signed.
//! 6. Evaluate the `draft -> structurally_verified` precondition (verify clean
//!    **and** the compiler signature valid). If met, append the
//!    `structurally_verified` event.
//! 7. Print the content hash + the final lifecycle state.
//!
//! Signing (the compiler key and the registry-root event key) is gated behind
//! the `test-keys` feature; without it, `ke compile` cannot produce signed
//! artifacts and returns a typed error. This keeps `cargo build -p ke-cli`
//! (no features) clean while the smoke script / end-to-end runs use the
//! feature (mirrors `ke-artifact`'s gated golden generator).

use crate::registry::backend::RegistryBackend;
#[cfg(any(test, feature = "test-keys"))]
use crate::registry::RegistryError;
use anyhow::Result;

/// Arguments for `ke compile`.
pub struct CompileArgs<'a> {
    /// Path to the YAML rule document.
    pub yaml_path: &'a str,
    /// Regime id recorded on the synthetic manifest.
    pub regime_id: &'a str,
    /// Named environment (review view only in Phase 3a; default `local`).
    pub env: &'a str,
    /// Resolution / event clock, unix seconds (sourced at the CLI edge).
    pub now_unix: u64,
}

/// Outcome of a `ke compile` run (also used by tests).
pub struct CompileOutcome {
    pub artifact_hash: [u8; 32],
    pub final_state: crate::registry::LifecycleState,
}

#[cfg(any(test, feature = "test-keys"))]
pub fn run<B: RegistryBackend>(backend: &B, args: &CompileArgs<'_>) -> Result<CompileOutcome> {
    use crate::registry::{
        build_draft_event, build_transition_event, can_transition, current_state, structural_clean,
        LifecycleState, Preconditions,
    };
    use ke_artifact::sign::test_keys as artifact_keys;
    use ke_artifact::tsa::MockTsa;
    use ke_artifact::{verify_hash, AuditVersions, SignerRole};
    use ke_compiler::verify::verify;
    use ke_core::examples::synthetic_manifest;
    use ke_core::ir::JurisdictionDate;
    use ke_core::manifest::ArtifactKind;

    // 1. Compile.
    let source = std::fs::read_to_string(args.yaml_path)
        .map_err(|e| anyhow::anyhow!("read {}: {e}", args.yaml_path))?;
    let rules = ke_compiler::compile_rules(&source)
        .map_err(|e| anyhow::anyhow!("compile {}: {e:?}", args.yaml_path))?;

    // 2. Verify — abort if blocking.
    let report = verify(&rules);
    if report.has_blocking() {
        let mut msg =
            String::from("verification produced blocking findings; refusing to assemble:\n");
        for finding in report.findings.iter().filter(|f| f.blocking) {
            msg.push_str(&format!(
                "  [{:?}] {} ({}): {}\n",
                finding.tier, finding.rule_id, finding.code, finding.message
            ));
        }
        for conflict in &report.conflicts {
            if conflict.severity == ke_compiler::verify::Severity::Blocking {
                msg.push_str(&format!("  [T4 conflict] {conflict:?}\n"));
            }
        }
        anyhow::bail!(msg);
    }

    // 3. Synthetic manifest + assemble with the test compiler key.
    //    The effective_from is taken from the first rule's window (corpus
    //    rules carry one); fall back to a floor date if absent.
    let effective_from = rules
        .first()
        .and_then(|r| r.effective_window.as_ref())
        .map(|w| w.effective_from)
        .unwrap_or_else(|| JurisdictionDate::new(1900, 1, 1));
    // The synthetic manifest hashes some bytes for its placeholder slots; the
    // exact bytes are irrelevant because `assemble` re-zeroes and re-derives
    // `artifact_hash`. Use the postcard of the rule set as the seed bytes.
    let seed_bytes = postcard::to_stdvec(&rules)
        .map_err(|e| anyhow::anyhow!("encode rules for manifest seed: {e}"))?;
    let manifest = synthetic_manifest(
        ArtifactKind::RegimePack,
        args.regime_id,
        effective_from,
        &seed_bytes,
    );
    let audit_versions = AuditVersions {
        jurisdiction_resolver_version: None,
        scenario_corpus_version: None,
    };
    let (artifact, kew) = ke_artifact::Artifact::assemble(
        manifest,
        rules.clone(),
        audit_versions,
        &artifact_keys::signing_key(),
        artifact_keys::TEST_KEY_ID,
    )
    .map_err(|e| anyhow::anyhow!("assemble artifact: {e}"))?;
    let artifact_hash = artifact.manifest.artifact_hash;

    // 4. Store the artifact + review views.
    let manifest_json = serde_json::to_string_pretty(&artifact.manifest)
        .map_err(|e| anyhow::anyhow!("manifest json: {e}"))?;
    let schema_json = serde_json::to_string_pretty(&artifact.compiled_ir)
        .map_err(|e| anyhow::anyhow!("schema json: {e}"))?;
    backend.put_artifact(&artifact_hash, &kew, &manifest_json, &schema_json)?;

    // 5. Append the draft event (registry-root-signed; authority = compiler).
    let draft_ts = MockTsa::stamp(&artifact_hash, args.now_unix);
    let draft = build_draft_event(
        artifact_hash,
        crate::registry::event::test_keys::REGISTRY_ROOT_KEY_ID,
        draft_ts,
    )?;
    backend.append_event(&artifact_hash, &draft)?;

    // 6. Evaluate draft -> structurally_verified. Compiler signature validity
    //    is confirmed via the re-zero hash + signature path (`verify_hash`
    //    re-zeroes; the assembled signature is over the hash-patched prefix and
    //    is checked here against the test compiler key).
    let hash_ok = verify_hash(&kew).is_ok();
    let sig_ok = compiler_signature_valid(&kew, &artifact_keys::verifying_key());
    let pre = Preconditions {
        structural_clean: structural_clean(&report),
        compiler_signature_valid: hash_ok && sig_ok,
        ..Preconditions::default()
    };

    let final_state = if can_transition(
        LifecycleState::Draft,
        LifecycleState::StructurallyVerified,
        &pre,
    ) {
        let sv_ts = MockTsa::stamp(&artifact_hash, args.now_unix);
        let sv = build_transition_event(
            &draft,
            LifecycleState::StructurallyVerified,
            // The triggering authority is the compiler/CI (§9); the event is
            // still registry-root-signed.
            artifact_keys::TEST_KEY_ID,
            SignerRole::Registry,
            sv_ts,
        )?;
        backend.append_event(&artifact_hash, &sv)?;
        LifecycleState::StructurallyVerified
    } else {
        LifecycleState::Draft
    };

    // 7. Re-derive the state from the log as the source of truth (ADR 0012 §2).
    let events = backend.read_events(&artifact_hash)?;
    let derived = current_state(&events)?.ok_or_else(|| RegistryError::NotFound {
        selector: "post-compile state".to_string(),
    })?;
    debug_assert_eq!(
        derived, final_state,
        "derived state must match appended state"
    );

    Ok(CompileOutcome {
        artifact_hash,
        final_state: derived,
    })
}

/// Without the `test-keys` feature the CLI cannot sign artifacts or events, so
/// `ke compile` is unavailable. Returns a typed error rather than silently
/// producing an unsigned artifact (CLAUDE.md: nothing here signs without an
/// authorized key).
#[cfg(not(any(test, feature = "test-keys")))]
pub fn run<B: RegistryBackend>(_backend: &B, _args: &CompileArgs<'_>) -> Result<CompileOutcome> {
    anyhow::bail!(
        "`ke compile` requires the `test-keys` feature in Phase 3a (the compiler and \
         registry-root signing keys are fixed-seed test keys). Build with \
         `--features test-keys`. Production signing keys are an infra/ADR-0009 concern \
         (out of Phase 3a)."
    )
}

/// Verify the compiler signature embedded in `.kew` over the hash-patched
/// envelope prefix (the bytes `[0, envelope_len)` as stored).
#[cfg(any(test, feature = "test-keys"))]
fn compiler_signature_valid(kew: &[u8], verifying_key: &ed25519_dalek::VerifyingKey) -> bool {
    let Ok((artifact, envelope_len)) = ke_artifact::decode_artifact(kew) else {
        return false;
    };
    ke_artifact::verify_signature(
        &kew[..envelope_len],
        &artifact.compiler_signature,
        verifying_key,
    )
    .is_ok()
}
