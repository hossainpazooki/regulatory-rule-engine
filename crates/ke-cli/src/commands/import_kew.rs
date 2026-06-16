//! `ke import --in <path>`: OFFLINE re-verification of a flat `.kew` file
//! (Gate 5, workstream 5b). (Module named `import_kew` because `import` is a
//! reserved-word-adjacent name; the CLI subcommand is still `import`.)
//!
//! NON-AUTHORITATIVE and offline. Import reads the `.kew` bytes from disk
//! (`std::fs::read` — no backend, no network) and re-verifies them by calling
//! [`ke_artifact::verify_artifact`] **verbatim**. It never trusts the file
//! blindly, never calls `blake3` directly, and never reimplements
//! decode/hash/signature checks: the content-hash re-zero recompute is
//! `verify_artifact`'s internal step 2 (`verify_hash`). Import signs nothing,
//! publishes nothing, transitions nothing, and appends no events.
//!
//! # Why a `test-keys`-gated key directory
//!
//! `verify_artifact` resolves the compiler verifying key from a caller-supplied
//! [`KeyDirectory`] by the signature's `key_id`. The artifacts this repo
//! produces are signed with the fixed-seed compiler test key
//! (`ke_artifact::sign::test_keys::TEST_KEY_ID`), so offline import builds the
//! directory from that entry — which only exists under the `test-keys` feature.
//! Without the feature, `run` returns a typed "requires --features test-keys"
//! error (mirroring `compile.rs`). Import *verifies* a signature; it still signs
//! nothing.
//!
//! # The offline crypto-vs-authority split (documented contract)
//!
//! Offline import has no live registry, so it supplies
//! `RegistryEvidence { status: Unknown, .. }` and a permissive
//! [`VerificationPolicy`] (per-attestation validity only — no required types,
//! no minimum counts). Consequently `verify_artifact` step 5 returns
//! `Rejected(NotPublished{..})`. That is **expected and correct** offline: the
//! import's success criterion is **cryptographic integrity**, not publish-time
//! completeness. So the verdict split is:
//!
//! - `Verified` and `Rejected(NotPublished{..})` / `Rejected(StaleEventHead{..})`
//!   are treated as "crypto OK, just not authoritative offline" — `run` returns
//!   `Ok`, the CLI exits 0 (with a warning for the non-`Verified` cases).
//! - `Rejected(Decode(..))`, `Rejected(HashMismatch)`,
//!   `Rejected(CompilerSignatureInvalid)`, and `Rejected(Attestations(..))` are
//!   **hard rejections**: the bytes failed crypto/integrity, so `run` returns
//!   `Err` and the CLI exits nonzero. The file is never trusted blindly.

use anyhow::Result;

/// Arguments for `ke import`.
pub struct ImportArgs<'a> {
    /// Path to the flat `.kew` file being imported (offline; no backend read).
    pub kew_path: &'a str,
    /// Verification clock, unix seconds (sourced at the CLI edge).
    pub now_unix: u64,
}

/// Outcome of an `ke import` run: the re-verification verdict plus the
/// always-built provenance and the registry status the verdict considered.
#[cfg(any(test, feature = "test-keys"))]
#[derive(Clone, Debug)]
pub struct ImportOutcome {
    pub artifact_hash: [u8; 32],
    pub verdict: ke_artifact::Verdict,
    pub provenance: ke_artifact::ArtifactProvenance,
    pub registry_state: ke_artifact::RegistryStatus,
}

/// Reads `kew_path` from disk and OFFLINE re-verifies via
/// [`ke_artifact::verify_artifact`]. Returns `Err` (hard rejection) for
/// integrity failures (decode / hash / compiler signature / attestations);
/// returns `Ok` with the verdict for crypto-clean results, including the
/// expected offline `Rejected(NotPublished)` / `Rejected(StaleEventHead)`.
/// Never calls `blake3` directly; never trusts the file blindly.
#[cfg(any(test, feature = "test-keys"))]
pub fn run(args: &ImportArgs<'_>) -> Result<ImportOutcome> {
    use ke_artifact::sign::test_keys;
    use ke_artifact::verify_artifact;
    use ke_artifact::{decode_artifact, RejectionReason, Verdict};
    use ke_artifact::{
        KeyDirectory, KeyDirectoryEntry, KeyStatus, PolicyContext, RegistryEvidence,
        RegistryStatus, SignerRole,
    };
    use ke_core::manifest::{T2T3Mode, VerificationPolicy};

    // Offline: read the flat file directly from disk (no backend, no network).
    let kew =
        std::fs::read(args.kew_path).map_err(|e| anyhow::anyhow!("read {}: {e}", args.kew_path))?;

    // Key directory: the fixed-seed compiler test key, so the compiler
    // signature's key_id resolves to a verifying key. The expert key is added
    // too so any attestations on the file can be checked (R1 resolution); the
    // permissive policy below means absent attestations are not a failure.
    let keydir = KeyDirectory {
        entries: vec![
            KeyDirectoryEntry {
                key_id: test_keys::TEST_KEY_ID.to_string(),
                public_key: test_keys::verifying_key().to_bytes(),
                signer_roles: vec![SignerRole::Registry],
                authorized_attestation_types: vec![],
                valid_from_unix: 0,
                valid_to_unix: u64::MAX,
                status: KeyStatus::Active,
                revoked_at_unix: None,
                revocation_reason: None,
                revocation_event_hash: None,
            },
            KeyDirectoryEntry {
                key_id: test_keys::TEST_EXPERT_KEY_ID.to_string(),
                public_key: test_keys::expert_verifying_key().to_bytes(),
                signer_roles: vec![SignerRole::DomainExpert, SignerRole::PublicationApprover],
                authorized_attestation_types: vec![
                    ke_core::manifest::AttestationType::SourceFidelity,
                    ke_core::manifest::AttestationType::Interpretation,
                    ke_core::manifest::AttestationType::ScenarioCoverage,
                    ke_core::manifest::AttestationType::EquivalenceClaim,
                    ke_core::manifest::AttestationType::PublicationApproval,
                ],
                // A wide window so a valid attestation's claimed time is not
                // rejected as out-of-window during an offline crypto check.
                valid_from_unix: 0,
                valid_to_unix: u64::MAX,
                status: KeyStatus::Active,
                revoked_at_unix: None,
                revocation_reason: None,
                revocation_event_hash: None,
            },
        ],
    };

    // Context: "local" so mock-TSA-stamped attestations are accepted (R8); the
    // CLI-edge clock; the recomputed legal-source hash bound to the manifest's
    // own source corpus hash when the file decodes (None skips R5 otherwise).
    let current_legal_source_hash = decode_artifact(&kew)
        .ok()
        .map(|(artifact, _)| artifact.manifest.source_corpus_hash);
    let ctx = PolicyContext {
        environment: "local".to_string(),
        now_unix: args.now_unix,
        supported_policy_versions: vec!["ap-1".to_string()],
        current_legal_source_hash,
    };

    // Permissive policy: offline import asserts crypto + hash integrity, not
    // publish-time completeness (no required types, no minimum counts).
    let policy = VerificationPolicy {
        t2_t3_mode: T2T3Mode::Disabled,
        required_attestation_types: vec![],
        minimum_attestation_count_per_type: vec![],
    };

    // No live registry offline: Unknown status + zero head, no live head (skips
    // the staleness check). NotPublished is the expected, accepted result.
    let registry = RegistryEvidence {
        status: RegistryStatus::Unknown,
        event_head_hash: [0u8; 32],
        live_event_head: None,
    };

    let outcome = verify_artifact(&kew, &keydir, &ctx, &policy, &registry, args.now_unix);

    // The crypto-vs-authority split: integrity failures are hard rejections
    // (Err); "not authoritative offline" passes through as Ok with the verdict.
    match &outcome.verdict {
        Verdict::Verified
        | Verdict::Rejected(RejectionReason::NotPublished { .. })
        | Verdict::Rejected(RejectionReason::StaleEventHead { .. }) => {}
        Verdict::Rejected(RejectionReason::Decode(msg)) => {
            anyhow::bail!("import rejected: {} did not decode: {msg}", args.kew_path);
        }
        Verdict::Rejected(RejectionReason::HashMismatch) => {
            anyhow::bail!(
                "import rejected: {} content hash does not match (tampered / corrupt)",
                args.kew_path
            );
        }
        Verdict::Rejected(RejectionReason::CompilerSignatureInvalid) => {
            anyhow::bail!(
                "import rejected: {} compiler signature did not verify",
                args.kew_path
            );
        }
        Verdict::Rejected(RejectionReason::Attestations(rejections)) => {
            anyhow::bail!(
                "import rejected: {} attestation set invalid: {rejections:?}",
                args.kew_path
            );
        }
    }

    Ok(ImportOutcome {
        artifact_hash: outcome.provenance.artifact_hash,
        verdict: outcome.verdict,
        provenance: outcome.provenance,
        registry_state: outcome.registry_state,
    })
}

/// Without the `test-keys` feature the compiler verifying-key entry the
/// directory needs does not exist, so offline import cannot resolve the
/// signature's key. Returns a typed error rather than silently skipping
/// verification (the whole point of import is to verify). Mirrors
/// `compile.rs`'s no-feature stub.
#[cfg(not(any(test, feature = "test-keys")))]
pub fn run(_args: &ImportArgs<'_>) -> Result<ImportOutcomeStub> {
    anyhow::bail!(
        "`ke import` requires the `test-keys` feature: offline re-verification resolves the \
         compiler verifying key from the fixed-seed test key directory. Build with \
         `--features test-keys`. (Import verifies signatures; it signs nothing.)"
    )
}

/// Placeholder outcome for the no-feature build so the `run` signature stays
/// total. Never constructed (the stub always returns `Err`).
#[cfg(not(any(test, feature = "test-keys")))]
pub struct ImportOutcomeStub;
