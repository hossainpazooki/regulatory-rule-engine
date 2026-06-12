//! ke-artifact: canonical encoding + content addressing + signatures + attestations.
//!
//! Gate 4 Phase 1. Turns compiled IR into a **signed, content-addressed
//! artifact** per spec § 8: the `.kew` byte format, the BLAKE3 zero-then-patch
//! `artifact_hash` derivation, and the ed25519 compiler signature over the
//! envelope prefix. See [`artifact`] for the full byte-range contract.
//!
//! Authority boundaries (spec § 5, § 10, § 13): this crate signs *compiler*
//! signatures only — structural validity, never legal truth. It does **not**
//! attest, publish, or transition registry lifecycle state (Phases 2/3). No
//! AI/LLM code participates in any path here.
//!
//! The PyO3 binding for the platform (`ke-artifact-py`) lives behind the
//! `pyo3` feature; see spec § 14 (Phase 4).

#![deny(unsafe_code)]

pub mod artifact;
pub mod hash;
pub mod sign;

pub use artifact::{
    build_span_index, decode_artifact, Artifact, Attestation, AttestationScope, AuditVersions,
    CompilerSignature, ConsistencyBlock, RegistryStateMetadata, SourceSpanIndex, SpanIndexEntry,
    TimestampEnvelope,
};
pub use hash::{artifact_hash_offset, content_hash, verify_hash};
pub use sign::{sign_envelope, verify_signature};

use ke_core::canonical::{CanonicalDecodeError, CanonicalError};
use thiserror::Error;

/// Errors raised while assembling, encoding, decoding, or verifying an
/// artifact. Each variant is identifiable per spec § 8.3 so non-canonical or
/// tampered input is diagnosable, never a generic failure.
#[derive(Debug, Error)]
pub enum ArtifactError {
    /// Envelope content could not be canonicalized/encoded (wraps ke-core's
    /// canonical-encoding profile errors; the profile is reused, not
    /// duplicated).
    #[error("canonical encode failed: {0}")]
    Canonical(#[from] CanonicalError),

    /// Decoded bytes violate the canonical profile (wraps ke-core's strict
    /// decode validation).
    #[error("canonical decode failed: {0}")]
    CanonicalDecode(#[from] CanonicalDecodeError),

    /// Bytes remain after the full `Artifact` record (envelope + signature +
    /// attestations + registry-state metadata). Canonical `.kew` files have
    /// no trailing bytes.
    #[error("trailing bytes after the artifact record")]
    TrailingBytes,

    /// The recomputed content hash (BLAKE3 over the envelope prefix with the
    /// 32-byte `artifact_hash` slot re-zeroed) does not match the hash the
    /// manifest claims.
    #[error(
        "artifact hash mismatch: manifest claims {}, recomputed {}",
        hex32(.expected),
        hex32(.got)
    )]
    HashMismatch {
        /// The hash recorded in `manifest.artifact_hash`.
        expected: [u8; 32],
        /// The hash recomputed from the (re-zeroed) envelope prefix.
        got: [u8; 32],
    },

    /// The ed25519 compiler signature does not verify over the hash-patched
    /// envelope prefix with the given key.
    #[error("compiler signature invalid for envelope prefix")]
    SignatureInvalid,

    /// The byte stream ended (or was malformed) before the five envelope
    /// fields completed decoding, so no envelope prefix can be recovered.
    #[error("artifact bytes truncated before the envelope completed")]
    EnvelopeTruncated,
}

/// Lowercase hex for a 32-byte hash (error display only).
fn hex32(bytes: &[u8; 32]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::with_capacity(64), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}
