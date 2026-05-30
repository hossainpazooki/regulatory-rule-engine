//! Self-referential `artifact_hash` offset (brief § 3, § 12 risk).
//!
//! Gate 1 only proves the offset is *determinable* and the zero-then-patch is
//! idempotent. The real BLAKE3 wiring lands in Gate 4. The manifest is laid out
//! so `artifact_hash` (fixed 32 bytes) immediately follows `artifact_kind`,
//! whose encoded length is the offset.

use ke_core::canonical::encode_manifest;
use ke_core::examples;
use ke_core::ir::time::JurisdictionDate;
use ke_core::manifest::ArtifactKind;

fn hash_offset(kind: ArtifactKind) -> usize {
    // The hash sits immediately after the encoded `artifact_kind` prefix.
    postcard::to_stdvec(&kind).expect("encode kind").len()
}

#[test]
fn hash_sits_at_the_derived_offset() {
    let manifest = examples::synthetic_manifest(
        ArtifactKind::RegimePack,
        "mica_2023",
        JurisdictionDate::new(2024, 6, 30),
        b"payload",
    );
    let bytes = encode_manifest(&manifest).expect("encode");
    let offset = hash_offset(ArtifactKind::RegimePack);

    assert!(offset + 32 <= bytes.len());
    assert_eq!(
        &bytes[offset..offset + 32],
        &manifest.artifact_hash[..],
        "artifact_hash bytes are at the derived offset"
    );
}

#[test]
fn zero_then_patch_is_idempotent() {
    let manifest = examples::synthetic_manifest(
        ArtifactKind::RegimePack,
        "mica_2023",
        JurisdictionDate::new(2024, 6, 30),
        b"payload",
    );
    let original = encode_manifest(&manifest).expect("encode");
    let offset = hash_offset(ArtifactKind::RegimePack);

    // Zero the hash region (as the Gate 4 first pass would).
    let mut work = original.clone();
    for b in &mut work[offset..offset + 32] {
        *b = 0;
    }
    assert_ne!(work, original, "zeroing changes the bytes");

    // Patch the real hash back in (as the Gate 4 second pass would).
    work[offset..offset + 32].copy_from_slice(&manifest.artifact_hash);
    assert_eq!(work, original, "zero-then-patch restores the exact bytes");
}
