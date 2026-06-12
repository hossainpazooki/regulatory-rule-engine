//! Identifiable rejection of non-canonical / tampered `.kew` input
//! (spec § 8.3): each failure mode maps to a *named* `ArtifactError`
//! variant, never a generic failure. Inputs are the committed golden
//! vectors, tampered in memory — fixtures on disk are never touched.

use ke_artifact::sign::{test_keys, verify_signature};
use ke_artifact::{artifact_hash_offset, decode_artifact, verify_hash, ArtifactError};
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
