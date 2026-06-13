//! `ke attest --hash <h> --type <t> [--type <t> ...]`: build expert-signed
//! attestations and move `ml_checked -> expert_attested` (Phase 3b).
//!
//! Attestations live **outside** the hashed/signed envelope (Phase-2 property).
//! For each `--type`, this builds an [`Attestation`] signed by the fixed-seed
//! expert test key (`test-expert-fixed-seed-1`), with the manifest-bound fields
//! (`regime_id`, effective window, `ir_schema_version`, `compiler_version`,
//! `legal_source_hash = manifest.source_corpus_hash`), scope `WholeArtifact`,
//! mock-TSA stamped at `now_unix`, `attestation_policy_version = "ap-1"`,
//! `test_corpus_hash = None` — the exact golden-attestation shape.
//!
//! It then `decode_artifact`s the stored `.kew`, calls
//! [`Artifact::with_attestations`], **re-writes** `artifact.kew`, and asserts
//! `artifact_hash` is unchanged (spec § 9 — the append never moves the content
//! address). When the set verifies under the strict default policy + local
//! context (precondition: prior == `ml_checked` AND `verify_attestation_set`
//! ok), it appends the `expert_attested` event.
//!
//! *Local-FS note:* re-writing `.kew` in place is fine on local-FS; a future
//! S3-WORM backend would need attestations as separate objects (the trait seam
//! is ready). Flagged, not built.

use crate::registry::backend::RegistryBackend;
use anyhow::Result;
use ke_core::manifest::AttestationType;

/// Arguments for `ke attest`.
pub struct AttestArgs<'a> {
    /// 32-byte artifact content hash (already decoded from hex).
    pub artifact_hash: [u8; 32],
    /// The attestation types to sign (one per `--type`, repeatable).
    pub types: &'a [AttestationType],
    /// Event/attestation clock, unix seconds (sourced at the CLI edge).
    pub now_unix: u64,
}

/// Outcome of an `ke attest` run.
pub struct AttestOutcome {
    pub final_state: crate::registry::LifecycleState,
    /// The number of attestations now on the artifact.
    pub attestation_count: usize,
}

/// Parse a `--type` value (`source_fidelity`, `interpretation`,
/// `scenario_coverage`, `equivalence_claim`, `publication_approval`) into the
/// frozen [`AttestationType`]. Used by the CLI before building [`AttestArgs`].
pub fn parse_attestation_type(s: &str) -> Result<AttestationType> {
    Ok(match s.to_ascii_lowercase().as_str() {
        "source_fidelity" => AttestationType::SourceFidelity,
        "interpretation" => AttestationType::Interpretation,
        "scenario_coverage" => AttestationType::ScenarioCoverage,
        "equivalence_claim" => AttestationType::EquivalenceClaim,
        "publication_approval" => AttestationType::PublicationApproval,
        other => anyhow::bail!(
            "unknown attestation type {other:?}; expected one of source_fidelity, \
             interpretation, scenario_coverage, equivalence_claim, publication_approval"
        ),
    })
}

/// The signer role each attestation type signs under. `PublicationApproval`
/// signs under `PublicationApprover` (rejection rule R7 honors it only with the
/// co-attestations); everything else is a `DomainExpert` claim. Matches the
/// golden set in `gen-golden-artifacts`.
#[cfg(any(test, feature = "test-keys"))]
pub fn signer_role_for(attestation_type: AttestationType) -> ke_artifact::SignerRole {
    use ke_artifact::SignerRole;
    match attestation_type {
        AttestationType::PublicationApproval => SignerRole::PublicationApprover,
        _ => SignerRole::DomainExpert,
    }
}

/// The attestation policy version every Phase-3b attestation is made under
/// (matches the Phase-2 golden set so a freshly-attested artifact verifies under
/// the strict default policy).
pub const ATTESTATION_POLICY_VERSION: &str = "ap-1";

/// Build the in-memory [`KeyDirectory`](ke_artifact::KeyDirectory) the
/// attest/publish verification runs against: a single entry for the fixed-seed
/// expert test key, active, authorized for all five types under both the
/// `DomainExpert` and `PublicationApprover` roles, with a wide validity window.
///
/// This is a **test** directory — the registry-signed key-directory object
/// (ADR 0009, HSM-custodied root) is infra/out-of-scope (plan). Loudly a test
/// key (`test-expert-fixed-seed-1`).
#[cfg(any(test, feature = "test-keys"))]
pub fn test_expert_key_directory() -> ke_artifact::KeyDirectory {
    use ke_artifact::sign::test_keys;
    use ke_artifact::{KeyDirectory, KeyDirectoryEntry, KeyStatus, SignerRole};
    KeyDirectory {
        entries: vec![KeyDirectoryEntry {
            key_id: test_keys::TEST_EXPERT_KEY_ID.to_string(),
            public_key: test_keys::expert_verifying_key().to_bytes(),
            signer_roles: vec![SignerRole::DomainExpert, SignerRole::PublicationApprover],
            authorized_attestation_types: vec![
                AttestationType::SourceFidelity,
                AttestationType::Interpretation,
                AttestationType::ScenarioCoverage,
                AttestationType::EquivalenceClaim,
                AttestationType::PublicationApproval,
            ],
            // Wide window: the fixed test clock (KE_NOW) sits inside it.
            valid_from_unix: 0,
            valid_to_unix: u64::MAX,
            status: KeyStatus::Active,
            revoked_at_unix: None,
            revocation_reason: None,
            revocation_event_hash: None,
        }],
    }
}

/// The local verification context attest/publish use: `environment = "local"`
/// (so mock-TSA timestamps are accepted — R8), the supported policy version,
/// `current_legal_source_hash = None` (the legal source corpus is not loaded
/// here, so R5 is skipped — the same posture the Phase-2 golden verification
/// takes).
#[cfg(any(test, feature = "test-keys"))]
pub fn local_policy_context(now_unix: u64) -> ke_artifact::PolicyContext {
    ke_artifact::PolicyContext {
        environment: "local".to_string(),
        now_unix,
        supported_policy_versions: vec![ATTESTATION_POLICY_VERSION.to_string()],
        current_legal_source_hash: None,
    }
}

/// A permissive verification policy for the attest step: no required types and
/// no minimum counts. `verify_attestation_set` under it still runs every
/// per-attestation rule (R1-R5) and the R7 co-attestation rule, but does not
/// fail on a missing *required* type — that completeness check is the publish
/// policy gate. `t2_t3_mode` is irrelevant to attestation-set verification.
#[cfg(any(test, feature = "test-keys"))]
pub fn permissive_policy() -> ke_core::manifest::VerificationPolicy {
    use ke_core::manifest::{T2T3Mode, VerificationPolicy};
    VerificationPolicy {
        t2_t3_mode: T2T3Mode::Advisory,
        required_attestation_types: Vec::new(),
        minimum_attestation_count_per_type: Vec::new(),
    }
}

#[cfg(any(test, feature = "test-keys"))]
pub fn run<B: RegistryBackend>(backend: &B, args: &AttestArgs<'_>) -> Result<AttestOutcome> {
    use crate::registry::event::test_keys::REGISTRY_ROOT_KEY_ID;
    use crate::registry::{
        build_transition_event, can_transition, head_event, require_current_state, LifecycleState,
        Preconditions, RegistryError,
    };
    use ke_artifact::sign::test_keys;
    use ke_artifact::tsa::MockTsa;
    use ke_artifact::{
        decode_artifact, sign_attestation, verify_attestation_set, Artifact, Attestation,
        AttestationScope, SignerRole,
    };

    let hash = args.artifact_hash;
    if args.types.is_empty() {
        anyhow::bail!("ke attest requires at least one --type");
    }

    // Precondition (state half): prior must be exactly ml_checked.
    let prior_state = require_current_state(backend, &hash)?;
    if prior_state != LifecycleState::MlChecked {
        anyhow::bail!("attest requires prior state ml_checked, found {prior_state:?}");
    }

    // Decode the stored artifact (the .kew is the source of truth).
    let kew = backend.read_artifact_kew(&hash)?;
    let (artifact, _envelope_len) =
        decode_artifact(&kew).map_err(|e| RegistryError::ArtifactDecode(e.to_string()))?;
    let pre_hash = artifact.manifest.artifact_hash;
    let pre_signature = artifact.compiler_signature.clone();

    // Build + sign one attestation per requested type, manifest-bound, mock-TSA
    // stamped at now_unix. Same shape as the Phase-2 golden set.
    let manifest = &artifact.manifest;
    let mut attestations: Vec<Attestation> = Vec::with_capacity(args.types.len());
    for &attestation_type in args.types {
        let payload = Attestation {
            artifact_hash: pre_hash,
            scope: AttestationScope::WholeArtifact,
            attestation_type,
            signer_identity: "Test Expert".to_string(),
            key_id: test_keys::TEST_EXPERT_KEY_ID.to_string(),
            signer_role: signer_role_for(attestation_type),
            regime_id: manifest.regime_id.clone(),
            effective_from: manifest.effective_from,
            effective_to: manifest.effective_to,
            legal_source_hash: manifest.source_corpus_hash,
            ir_schema_version: manifest.ir_schema_version,
            compiler_version: manifest.compiler_version,
            attestation_policy_version: ATTESTATION_POLICY_VERSION.to_string(),
            test_corpus_hash: None,
            timestamp: MockTsa::stamp(&pre_hash, args.now_unix),
            expiration: None,
            reviewer_comments: None,
            signature: [0u8; 64],
        };
        let signed = sign_attestation(payload, &test_keys::expert_signing_key())
            .map_err(|e| anyhow::anyhow!("sign attestation: {e}"))?;
        attestations.push(signed);
    }

    // Append the attestations post-envelope and re-write the .kew. The content
    // address must NOT move (spec § 9) — asserted hard.
    let attestation_count = attestations.len();
    let (attested, attested_kew): (Artifact, Vec<u8>) = artifact
        .with_attestations(attestations)
        .map_err(|e| anyhow::anyhow!("append attestations: {e}"))?;
    if attested.manifest.artifact_hash != pre_hash {
        anyhow::bail!(
            "attestation append moved the content address (spec § 9 violated): {} -> {}",
            crate::registry::hash_hex(&pre_hash),
            crate::registry::hash_hex(&attested.manifest.artifact_hash)
        );
    }
    if attested.compiler_signature != pre_signature {
        anyhow::bail!("attestation append altered the compiler signature (spec § 9 violated)");
    }
    let manifest_json = serde_json::to_string_pretty(&attested.manifest)
        .map_err(|e| anyhow::anyhow!("manifest json: {e}"))?;
    let schema_json = serde_json::to_string_pretty(&attested.compiled_ir)
        .map_err(|e| anyhow::anyhow!("schema json: {e}"))?;
    backend.put_artifact(&pre_hash, &attested_kew, &manifest_json, &schema_json)?;

    // Precondition (evidence half): each attestation is individually valid
    // (R1-R5 + R7 co-attestation) under a **permissive** policy — no required
    // types. Set-completeness (R6: the required publication types) is the
    // PUBLISH policy gate, not attest's: attest records that valid expert
    // attestations exist; publish decides whether the *set* satisfies the
    // environment's verification policy. Using a permissive policy here lets a
    // partial set reach expert_attested and be rejected at publish (the gate the
    // contract specifies), while still rejecting individually-bad attestations.
    let key_directory = test_expert_key_directory();
    let context = local_policy_context(args.now_unix);
    let policy = permissive_policy();
    match verify_attestation_set(&attested, &policy, &key_directory, &context) {
        Ok(()) => {}
        Err(rejections) => {
            // Surfaced as a typed error; the .kew (with attestations) is already
            // written so the operator can inspect what was rejected, but the
            // lifecycle does NOT advance.
            let rendered: Vec<String> = rejections.iter().map(|r| r.to_string()).collect();
            return Err(RegistryError::AttestationSetRejected(rendered).into());
        }
    }

    let pre = Preconditions {
        attestation_set_valid: true,
        ..Preconditions::default()
    };
    if !can_transition(
        LifecycleState::MlChecked,
        LifecycleState::ExpertAttested,
        &pre,
    ) {
        anyhow::bail!("attest precondition failed: attestation set not valid");
    }

    // Append expert_attested, chained onto the validated head.
    let prior = head_event(backend, &hash)?;
    let ts = MockTsa::stamp(&hash, args.now_unix);
    let event = build_transition_event(
        &prior,
        LifecycleState::ExpertAttested,
        REGISTRY_ROOT_KEY_ID,
        SignerRole::Registry,
        ts,
    )?;
    backend.append_event(&hash, &event)?;

    Ok(AttestOutcome {
        final_state: require_current_state(backend, &hash)?,
        attestation_count,
    })
}

/// Without the `test-keys` feature the CLI cannot sign attestations or events,
/// so `ke attest` is unavailable. Typed error.
#[cfg(not(any(test, feature = "test-keys")))]
pub fn run<B: RegistryBackend>(_backend: &B, _args: &AttestArgs<'_>) -> Result<AttestOutcome> {
    anyhow::bail!(
        "`ke attest` requires the `test-keys` feature (it signs expert attestations and a \
         registry-root-signed event). Build with `--features test-keys`. Production expert keys \
         are an infra/ADR-0009 HSM concern."
    )
}
