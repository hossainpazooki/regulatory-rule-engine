//! Gate 4 hash-offset and zero-then-patch tests over the real assembled
//! artifact (the Gate-1 `ke-core/tests/artifact_hash_offset.rs` proved the
//! offset on a bare manifest; here the BLAKE3 wiring is live).
//!
//! Byte-range contract under test: `artifact_hash` = BLAKE3 over the envelope
//! prefix with the 32-byte slot zeroed, patched in at
//! `offset = postcard::to_stdvec(&artifact_kind).len()`. Consequence:
//! `blake3(hash-patched bytes) != artifact_hash` — verifiers must re-zero.

use ke_artifact::sign::test_keys;
use ke_artifact::{
    artifact_hash_offset, content_hash, decode_artifact, verify_hash, Artifact, AuditVersions,
};
use ke_core::canonical::encode_manifest;
use ke_core::examples;
use ke_core::ir::JurisdictionDate;
use ke_core::manifest::ArtifactKind;

const ALL_KINDS: [ArtifactKind; 4] = [
    ArtifactKind::RegimePack,
    ArtifactKind::EquivalenceMatrix,
    ArtifactKind::TestCorpus,
    ArtifactKind::PolicyBundle,
];

fn assembled() -> (Artifact, Vec<u8>) {
    let manifest = examples::synthetic_manifest(
        ArtifactKind::RegimePack,
        "mica_2023",
        JurisdictionDate::new(2024, 6, 30),
        b"phase-1 hash tests",
    );
    let rules: Vec<_> = examples::rules().into_iter().map(|(_, r)| r).collect();
    Artifact::assemble(
        manifest,
        rules,
        AuditVersions::default(),
        &test_keys::signing_key(),
        test_keys::TEST_KEY_ID,
    )
    .expect("assemble")
}

#[test]
fn offset_is_correct_for_all_four_artifact_kinds() {
    for kind in ALL_KINDS {
        let manifest = examples::synthetic_manifest(
            kind,
            "mica_2023",
            JurisdictionDate::new(2024, 6, 30),
            b"offset probe",
        );
        let bytes = encode_manifest(&manifest).expect("encode manifest");
        let offset = artifact_hash_offset(kind);

        assert!(offset + 32 <= bytes.len(), "{kind:?}: slot fits");
        assert_eq!(
            &bytes[offset..offset + 32],
            &manifest.artifact_hash[..],
            "{kind:?}: artifact_hash sits at the derived offset"
        );
    }
}

#[test]
fn zero_then_patch_is_idempotent_on_an_assembled_artifact() {
    let (artifact, kew) = assembled();
    let (_, envelope_len) = decode_artifact(&kew).expect("decode");
    let prefix = &kew[..envelope_len];
    let offset = artifact_hash_offset(artifact.manifest.artifact_kind);

    // The patched slot holds the manifest hash.
    assert_eq!(
        &prefix[offset..offset + 32],
        &artifact.manifest.artifact_hash[..]
    );

    // Zero the slot (as every verifier must) and recompute BLAKE3.
    let mut zeroed = prefix.to_vec();
    zeroed[offset..offset + 32].fill(0);
    let recomputed: [u8; 32] = *blake3::hash(&zeroed).as_bytes();
    assert_eq!(
        recomputed, artifact.manifest.artifact_hash,
        "BLAKE3 over the re-zeroed envelope prefix is the artifact hash"
    );

    // Patch the hash back in: exact original prefix bytes (idempotence).
    zeroed[offset..offset + 32].copy_from_slice(&artifact.manifest.artifact_hash);
    assert_eq!(zeroed, prefix, "zero-then-patch restores the exact prefix");

    // content_hash performs the same re-zeroing internally.
    assert_eq!(
        content_hash(prefix).expect("content_hash"),
        artifact.manifest.artifact_hash
    );

    // verify_hash accepts the full .kew bytes.
    assert_eq!(
        verify_hash(&kew).expect("verify_hash"),
        artifact.manifest.artifact_hash
    );
}

#[test]
fn naive_whole_bytes_hash_fails_by_construction() {
    let (artifact, kew) = assembled();
    let (_, envelope_len) = decode_artifact(&kew).expect("decode");

    // The trap: hashing the patched bytes (file or prefix) never reproduces
    // the artifact hash, because the patched hash is part of those bytes.
    assert_ne!(
        *blake3::hash(&kew).as_bytes(),
        artifact.manifest.artifact_hash,
        "blake3(final .kew bytes) must differ from artifact_hash"
    );
    assert_ne!(
        *blake3::hash(&kew[..envelope_len]).as_bytes(),
        artifact.manifest.artifact_hash,
        "blake3(hash-patched envelope prefix) must differ from artifact_hash"
    );
}
