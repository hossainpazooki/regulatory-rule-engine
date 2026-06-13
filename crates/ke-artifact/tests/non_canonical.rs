//! Identifiable rejection of non-canonical / tampered `.kew` input
//! (spec § 8.3): each failure mode maps to a *named* `ArtifactError`
//! variant, never a generic failure — and tampered **attestations** map to
//! the named `AttestationRejection` variants (spec § 10). Inputs are the
//! committed golden vectors, tampered in memory — fixtures on disk are never
//! touched.

use ke_artifact::sign::{test_keys, verify_signature};
use ke_artifact::tsa::TimestampAuthorityClass;
use ke_artifact::{
    artifact_hash_offset, decode_artifact, sign_attestation, verify_attestation_set, verify_hash,
    ArtifactError, AttestationRejection, KeyDirectory, KeyDirectoryEntry, KeyStatus, PolicyContext,
    SignerRole,
};
use ke_core::manifest::{AttestationType, T2T3Mode, VerificationPolicy};
use std::fs;
use std::path::{Path, PathBuf};

const GOLDEN_IDS: [&str; 2] = ["rule_reserve_assets", "rule_significant_thresholds"];

fn read_kew(id: &str) -> Vec<u8> {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("artifacts")
        .join(id)
        .join("artifact.kew");
    fs::read(&path)
        .unwrap_or_else(|e| panic!("read {id}/artifact.kew (run gen-golden-artifacts): {e}"))
}

#[test]
fn trailing_bytes_are_rejected_as_trailing_bytes() {
    for id in GOLDEN_IDS {
        let mut bytes = read_kew(id);
        bytes.push(0x00);
        let err = decode_artifact(&bytes).expect_err("trailing byte must be rejected");
        assert!(
            matches!(err, ArtifactError::TrailingBytes),
            "{id}: expected TrailingBytes, got {err:?}"
        );
    }
}

#[test]
fn corrupted_hash_slot_is_hash_mismatch() {
    for id in GOLDEN_IDS {
        let mut bytes = read_kew(id);
        let (artifact, _) = decode_artifact(&bytes).expect("golden .kew decodes");
        let offset = artifact_hash_offset(artifact.manifest.artifact_kind);

        // Flip one byte inside the 32-byte patched hash slot. The record
        // still decodes (decode does not verify the hash); verification must
        // report a mismatch between the claimed and recomputed hashes.
        bytes[offset] ^= 0xFF;
        let (tampered, _) = decode_artifact(&bytes).expect("tampered slot still decodes");
        let err = verify_hash(&bytes).expect_err("corrupted hash slot must fail verification");
        match err {
            ArtifactError::HashMismatch { expected, got } => {
                assert_eq!(
                    expected, tampered.manifest.artifact_hash,
                    "{id}: `expected` is the (tampered) manifest claim"
                );
                assert_eq!(
                    got, artifact.manifest.artifact_hash,
                    "{id}: `got` is the recomputed (re-zeroed) hash — zeroing makes the \
                     slot's value irrelevant to the recompute"
                );
            }
            other => panic!("{id}: expected HashMismatch, got {other:?}"),
        }
    }
}

#[test]
fn corrupted_signature_byte_is_signature_invalid() {
    for id in GOLDEN_IDS {
        let bytes = read_kew(id);
        let (artifact, envelope_len) = decode_artifact(&bytes).expect("golden .kew decodes");

        // (a) Tamper the decoded signature value.
        let mut sig = artifact.compiler_signature.clone();
        sig.signature[0] ^= 0x01;
        let err = verify_signature(&bytes[..envelope_len], &sig, &test_keys::verifying_key())
            .expect_err("corrupted signature must fail verification");
        assert!(
            matches!(err, ArtifactError::SignatureInvalid),
            "{id}: expected SignatureInvalid, got {err:?}"
        );

        // (b) Tamper the signature bytes in the raw file. Layout after the
        // envelope prefix: CompilerSignature = key_id (1-byte varint length,
        // since len < 128, + bytes) then the 64 signature bytes.
        let sig_start = envelope_len + 1 + artifact.compiler_signature.key_id.len();
        let mut raw = bytes.clone();
        raw[sig_start + 63] ^= 0x80;
        let (tampered, len) = decode_artifact(&raw).expect("tampered signature still decodes");
        assert_eq!(len, envelope_len, "{id}: envelope untouched");
        assert_ne!(
            tampered.compiler_signature.signature, artifact.compiler_signature.signature,
            "{id}: the flip landed inside the signature field"
        );
        let err = verify_signature(
            &raw[..envelope_len],
            &tampered.compiler_signature,
            &test_keys::verifying_key(),
        )
        .expect_err("file-level signature tamper must fail verification");
        assert!(
            matches!(err, ArtifactError::SignatureInvalid),
            "{id}: expected SignatureInvalid, got {err:?}"
        );
    }
}

// ---- Phase 2: tampered attestations reject identifiably ----

/// The golden expert key's directory entry — signatures verify, so the
/// tamper under test is what rejects, never an R1 key failure.
fn golden_directory() -> KeyDirectory {
    KeyDirectory {
        entries: vec![KeyDirectoryEntry {
            key_id: test_keys::TEST_EXPERT_KEY_ID.to_string(),
            public_key: test_keys::expert_verifying_key().to_bytes(),
            signer_roles: vec![SignerRole::DomainExpert, SignerRole::PublicationApprover],
            authorized_attestation_types: vec![
                AttestationType::SourceFidelity,
                AttestationType::ScenarioCoverage,
                AttestationType::PublicationApproval,
            ],
            valid_from_unix: 1_000_000_000,
            valid_to_unix: 2_000_000_000,
            status: KeyStatus::Active,
            revoked_at_unix: None,
            revocation_reason: None,
            revocation_event_hash: None,
        }],
    }
}

fn golden_context(current_legal_source_hash: [u8; 32]) -> PolicyContext {
    PolicyContext {
        environment: "local".to_string(),
        now_unix: 1_750_000_000, // the generator's fixed claimed_time
        supported_policy_versions: vec!["ap-1".to_string()],
        current_legal_source_hash: Some(current_legal_source_hash),
    }
}

/// A policy requiring nothing, isolating the per-attestation rejection under
/// test from set-level R6 noise.
fn lax_policy() -> VerificationPolicy {
    VerificationPolicy {
        t2_t3_mode: T2T3Mode::Disabled,
        required_attestation_types: vec![],
        minimum_attestation_count_per_type: vec![],
    }
}

#[test]
fn corrupted_attestation_signature_in_raw_kew_is_attestation_signature_invalid() {
    for id in GOLDEN_IDS {
        let bytes = read_kew(id);
        let (artifact, _) = decode_artifact(&bytes).expect("golden .kew decodes");
        assert!(!artifact.attestations.is_empty(), "{id}: attested golden");

        // Locate attestations[0]'s 64 signature bytes in the raw file (the
        // ed25519 signature value is unique in the byte stream) and flip one.
        let sig = artifact.attestations[0].signature;
        let sig_start = bytes
            .windows(64)
            .position(|w| w == sig)
            .unwrap_or_else(|| panic!("{id}: attestation signature bytes located in raw .kew"));
        let mut raw = bytes.clone();
        raw[sig_start] ^= 0x01;

        // The record still decodes (decode does not verify attestations);
        // set verification names the failure exactly.
        let (tampered, _) = decode_artifact(&raw).expect("tampered attestation still decodes");
        assert_ne!(
            tampered.attestations[0].signature, sig,
            "{id}: the flip landed inside the attestation signature field"
        );
        let rejections = verify_attestation_set(
            &tampered,
            &lax_policy(),
            &golden_directory(),
            &golden_context(tampered.manifest.source_corpus_hash),
        )
        .expect_err("corrupted attestation signature must reject");
        assert!(
            rejections
                .iter()
                .any(|r| matches!(r, AttestationRejection::AttestationSignatureInvalid)),
            "{id}: expected AttestationSignatureInvalid, got {rejections:?}"
        );
    }
}

#[test]
fn relabelled_timestamp_class_is_timestamp_class_mismatch() {
    for id in GOLDEN_IDS {
        let bytes = read_kew(id);
        let (artifact, _) = decode_artifact(&bytes).expect("golden .kew decodes");

        // Relabel a genuine mock token as a production RFC 3161 class, then
        // RE-SIGN the attestation so the payload signature stays valid — the
        // class re-derivation (ADR 0010) is what must reject, not the
        // signature check.
        let mut relabelled = artifact.attestations[0].clone();
        relabelled.timestamp.class = TimestampAuthorityClass::Rfc3161External {
            tsa_identity: "acme-tsa".to_string(),
        };
        let relabelled = sign_attestation(relabelled, &test_keys::expert_signing_key())
            .expect("re-sign relabelled attestation");
        let mut attestations = artifact.attestations.clone();
        attestations[0] = relabelled;
        let (tampered, _) = artifact
            .clone()
            .with_attestations(attestations)
            .expect("append relabelled set");

        let rejections = verify_attestation_set(
            &tampered,
            &lax_policy(),
            &golden_directory(),
            &golden_context(tampered.manifest.source_corpus_hash),
        )
        .expect_err("relabelled timestamp class must reject");
        assert!(
            rejections
                .iter()
                .any(|r| matches!(r, AttestationRejection::TimestampClassMismatch)),
            "{id}: expected TimestampClassMismatch, got {rejections:?}"
        );
    }
}

#[test]
fn truncated_envelope_is_envelope_truncated() {
    for id in GOLDEN_IDS {
        let bytes = read_kew(id);
        let (_, envelope_len) = decode_artifact(&bytes).expect("golden .kew decodes");

        // Cut one byte short of the envelope: the five envelope fields can
        // no longer complete decoding.
        let err = decode_artifact(&bytes[..envelope_len - 1])
            .expect_err("truncated envelope must be rejected");
        assert!(
            matches!(err, ArtifactError::EnvelopeTruncated),
            "{id}: expected EnvelopeTruncated at envelope_len-1, got {err:?}"
        );

        // A deep truncation (mid-manifest) is the same identifiable error.
        let err = decode_artifact(&bytes[..5]).expect_err("deep truncation must be rejected");
        assert!(
            matches!(err, ArtifactError::EnvelopeTruncated),
            "{id}: expected EnvelopeTruncated at 5 bytes, got {err:?}"
        );
    }
}
