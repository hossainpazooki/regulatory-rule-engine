//! Compiler-signature tests: RFC 8032 determinism, tamper detection, and
//! wrong-key rejection — all over the hash-patched envelope prefix, using the
//! fixed-seed test keys (no OsRng/getrandom anywhere; brief § 3.3).

use ke_artifact::sign::{sign_envelope, test_keys, verify_signature, SigningKey};
use ke_artifact::{decode_artifact, Artifact, ArtifactError, AuditVersions};
use ke_core::examples;
use ke_core::ir::JurisdictionDate;
use ke_core::manifest::ArtifactKind;

/// A second deterministic key, distinct from the fixed test seed.
const OTHER_SEED: [u8; 32] = *b"ke-workbench-wrong-key-seed-0!!!";

fn assembled() -> (Artifact, Vec<u8>, usize) {
    let manifest = examples::synthetic_manifest(
        ArtifactKind::RegimePack,
        "mica_2023",
        JurisdictionDate::new(2024, 6, 30),
        b"phase-1 sign tests",
    );
    let rules: Vec<_> = examples::rules().into_iter().map(|(_, r)| r).collect();
    let (artifact, kew) = Artifact::assemble(
        manifest,
        rules,
        AuditVersions::default(),
        &test_keys::signing_key(),
        test_keys::TEST_KEY_ID,
    )
    .expect("assemble");
    let (_, envelope_len) = decode_artifact(&kew).expect("decode");
    (artifact, kew, envelope_len)
}

#[test]
fn signing_is_deterministic_rfc8032() {
    let (artifact, kew, envelope_len) = assembled();
    let prefix = &kew[..envelope_len];

    // Same bytes + same key => bit-identical signature, twice over.
    let a = sign_envelope(prefix, &test_keys::signing_key(), test_keys::TEST_KEY_ID);
    let b = sign_envelope(prefix, &test_keys::signing_key(), test_keys::TEST_KEY_ID);
    assert_eq!(a, b, "ed25519 (RFC 8032) signing is deterministic");

    // And it matches the signature embedded at assembly time.
    assert_eq!(a, artifact.compiler_signature);
    assert_eq!(a.key_id, "test-fixed-seed-1");

    verify_signature(prefix, &a, &test_keys::verifying_key()).expect("signature verifies");
}

#[test]
fn tampered_byte_is_signature_invalid() {
    let (artifact, kew, envelope_len) = assembled();
    let mut prefix = kew[..envelope_len].to_vec();
    // Flip one byte in the middle of the signed range.
    let i = prefix.len() / 2;
    prefix[i] ^= 0x01;

    let err = verify_signature(
        &prefix,
        &artifact.compiler_signature,
        &test_keys::verifying_key(),
    )
    .expect_err("tampered prefix must not verify");
    assert!(
        matches!(err, ArtifactError::SignatureInvalid),
        "expected SignatureInvalid, got {err:?}"
    );
}

#[test]
fn wrong_key_is_signature_invalid() {
    let (artifact, kew, envelope_len) = assembled();
    let prefix = &kew[..envelope_len];
    let wrong = SigningKey::from_bytes(&OTHER_SEED).verifying_key();
    assert_ne!(wrong, test_keys::verifying_key(), "keys actually differ");

    let err = verify_signature(prefix, &artifact.compiler_signature, &wrong)
        .expect_err("wrong key must not verify");
    assert!(
        matches!(err, ArtifactError::SignatureInvalid),
        "expected SignatureInvalid, got {err:?}"
    );
}
