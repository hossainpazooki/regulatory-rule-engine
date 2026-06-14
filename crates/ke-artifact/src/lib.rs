//! ke-artifact: canonical encoding + content addressing + signatures + attestations.
//!
//! Gate 4. Turns compiled IR into a **signed, content-addressed artifact**
//! per spec § 8: the `.kew` byte format, the BLAKE3 zero-then-patch
//! `artifact_hash` derivation, and the ed25519 compiler signature over the
//! envelope prefix (Phase 1; see [`artifact`] for the byte-range contract).
//! Phase 2 adds typed expert attestations — payload-prefix signing,
//! verification, and the spec § 10 rejection rules ([`attestation`]), the
//! ADR 0010 timestamp-token model ([`tsa`]), the ADR 0009 key-directory
//! shapes ([`keydir`]), and the spec § 11 `ConsistencyBlock` builder
//! ([`consistency`]).
//!
//! Authority boundaries (spec § 5, § 10, § 13): the compiler key signs
//! *structural validity* only — never legal truth. Attestations are signed
//! exclusively by domain-expert keys resolved through the key directory.
//! This crate does **not** publish or transition registry lifecycle state
//! (Phase 3). No AI/LLM code participates in any path here.
//!
//! The PyO3 binding for the platform (`ke-artifact-py`) lives behind the
//! `pyo3` feature; see spec § 14 (Phase 4).

#![deny(unsafe_code)]

pub mod artifact;
pub mod attestation;
pub mod consistency;
pub mod hash;
pub mod keydir;
/// PyO3 binding for `ke-artifact-py` (spec § 14, Phase 4b). Behind the `pyo3`
/// feature so the default workspace build never links libpython. RNG-free —
/// it wraps the pure 4a verify surface verbatim and exposes verification +
/// provenance reading only (no signing/publish in this module).
#[cfg(feature = "pyo3")]
pub mod python;
pub mod sign;
pub mod tsa;
pub mod verify;

pub use artifact::{
    build_span_index, decode_artifact, Artifact, AuditVersions, CompilerSignature,
    RegistryStateMetadata, SourceSpanIndex, SpanIndexEntry,
};
pub use attestation::{
    sign_attestation, verify_attestation, verify_attestation_set, Attestation,
    AttestationRejection, AttestationScope, PolicyContext,
};
pub use consistency::{ConsistencyBlock, ConsistencyBlockBuilder, ConsistencyError};
pub use hash::{artifact_hash_offset, content_hash, verify_hash};
pub use keydir::{KeyDirectory, KeyDirectoryEntry, KeyStatus, SignerRole};
pub use sign::{sign_envelope, verify_signature};
pub use tsa::{derive_class, TimestampAuthorityClass, TimestampToken, TsaError};
pub use verify::{
    artifact_provenance, verify_artifact, ArtifactProvenance, AttestationSummary, RegistryEvidence,
    RegistryStatus, RejectionReason, Verdict, VerificationOutcome,
};

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
pub(crate) fn hex32(bytes: &[u8; 32]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::with_capacity(64), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}
