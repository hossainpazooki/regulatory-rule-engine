//! BLAKE3 content addressing over the envelope prefix (spec § 8).
//!
//! **The trap, restated:** `artifact_hash` is BLAKE3 over the envelope prefix
//! with the 32-byte hash slot **zeroed**, then patched in. Therefore
//! `blake3(final .kew bytes) != artifact_hash` *by construction* — every
//! verifier here re-zeroes the slot within the envelope prefix before
//! recomputing. See the [`crate::artifact`] module doc for the full
//! byte-range contract.

use crate::artifact::decode_artifact;
use crate::ArtifactError;
use ke_core::manifest::ArtifactKind;

/// Byte offset of the 32-byte `artifact_hash` slot within the envelope
/// prefix. The manifest is the first envelope field and `artifact_kind` is
/// the first manifest field, so the slot sits immediately after the encoded
/// kind: `offset = postcard::to_stdvec(&kind).len()` (proven idempotent in
/// `ke-core/tests/artifact_hash_offset.rs`).
pub fn artifact_hash_offset(kind: ArtifactKind) -> usize {
    postcard::to_stdvec(&kind)
        .expect("encoding a fieldless enum variant to a Vec cannot fail")
        .len()
}

/// Recompute the content hash of an envelope prefix: copy the prefix,
/// **re-zero the 32-byte `artifact_hash` slot**, and BLAKE3 the zeroed bytes.
///
/// Accepts the prefix in either state (slot zeroed or already hash-patched);
/// zeroing is idempotent, which is exactly what lets verifiers recompute from
/// the final file bytes.
pub fn content_hash(envelope_prefix: &[u8]) -> Result<[u8; 32], ArtifactError> {
    let (kind, _) = postcard::take_from_bytes::<ArtifactKind>(envelope_prefix)
        .map_err(|_| ArtifactError::EnvelopeTruncated)?;
    let offset = artifact_hash_offset(kind);
    if envelope_prefix.len() < offset + 32 {
        return Err(ArtifactError::EnvelopeTruncated);
    }
    let mut zeroed = envelope_prefix.to_vec();
    zeroed[offset..offset + 32].fill(0);
    Ok(*blake3::hash(&zeroed).as_bytes())
}

/// Verify the content address of full `.kew` bytes: decode to recover
/// `envelope_len`, extract the envelope prefix, re-zero the hash slot,
/// recompute BLAKE3, and compare against `manifest.artifact_hash`.
///
/// Never hashes the whole file — `blake3(.kew)` fails for every valid
/// artifact by construction (module doc). Returns the verified hash.
pub fn verify_hash(kew_bytes: &[u8]) -> Result<[u8; 32], ArtifactError> {
    let (artifact, envelope_len) = decode_artifact(kew_bytes)?;
    let got = content_hash(&kew_bytes[..envelope_len])?;
    let expected = artifact.manifest.artifact_hash;
    if got != expected {
        return Err(ArtifactError::HashMismatch { expected, got });
    }
    Ok(got)
}
