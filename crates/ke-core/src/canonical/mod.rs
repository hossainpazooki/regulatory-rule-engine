//! Deterministic canonical byte encoding and a strict decoder.
//!
//! Canonical bytes are the input to BLAKE3 content addressing (Gate 4) and the
//! basis of the cross-language Rust ⇄ Python round-trip. The wire codec is
//! **postcard** (ADR 0002) with an explicit ordering/normalization profile
//! layered on top, documented in `docs/canonical-encoding.md`:
//!
//! - struct field order = declaration order (§ 4.2);
//! - sets/maps as sequences sorted by canonical-encoded element/key bytes,
//!   duplicates forbidden (§ 4.3–4.4);
//! - `Option` as a 1-byte tag + payload (postcard-native, § 4.5);
//! - fixed-width integers, decimals as `mantissa × 10^-scale` with no trailing
//!   zeros, **floats forbidden** (§ 4.6, ADR 0003);
//! - NFC-normalized UTF-8 strings (§ 4.7);
//! - structurally-validated jurisdiction dates (§ 4.8).
//!
//! [`encode`] normalizes then serializes; [`decode`] deserializes then
//! re-validates every invariant, returning a specific [`CanonicalDecodeError`]
//! for non-canonical input.

pub mod decode;
pub mod encode;
pub mod ordering;

use crate::ir::RuleIR;
use crate::manifest::{Manifest, PolicyBundle};
use thiserror::Error;

/// Error raised while *producing* canonical bytes (the input was un-encodable
/// or could not be normalized into canonical form).
#[derive(Debug, Error)]
pub enum CanonicalError {
    #[error("postcard encode failed: {0}")]
    Codec(postcard::Error),
    #[error("string is not NFC-normalized: {value:?}")]
    NonNfcString { value: String },
    #[error("structurally invalid date: {year:04}-{month:02}-{day:02}")]
    InvalidDate { year: i16, month: u8, day: u8 },
    #[error("unknown IANA time zone: {0}")]
    UnknownTimeZone(String),
    #[error("decimal overflow normalizing mantissa {mantissa} scale {scale}")]
    DecimalOverflow { mantissa: i128, scale: i8 },
    #[error("duplicate element in set")]
    DuplicateSetElement,
}

/// Error raised while *decoding* canonical bytes. Each variant names a specific
/// canonical-form violation so non-canonical input is diagnosable (spec § 8.3,
/// brief § 8.4).
#[derive(Debug, Error)]
pub enum CanonicalDecodeError {
    #[error("postcard decode failed: {0}")]
    Codec(postcard::Error),
    #[error("trailing bytes after canonical payload")]
    TrailingBytes,
    #[error("set is not sorted in canonical order")]
    UnsortedSet,
    #[error("duplicate element in set")]
    DuplicateSetElement,
    #[error("string is not NFC-normalized")]
    NonNfcString,
    #[error("structurally invalid date: {year:04}-{month:02}-{day:02}")]
    InvalidDate { year: i16, month: u8, day: u8 },
    #[error("unknown IANA time zone: {0}")]
    UnknownTimeZone(String),
    #[error("non-canonical decimal (trailing zeros or negative scale): mantissa {mantissa} scale {scale}")]
    NonCanonicalDecimal { mantissa: i128, scale: i8 },
}

/// Encode a [`RuleIR`] to canonical bytes.
pub fn encode_rule(rule: &RuleIR) -> Result<Vec<u8>, CanonicalError> {
    let mut c = rule.clone();
    encode::canonicalize_rule(&mut c)?;
    postcard::to_stdvec(&c).map_err(CanonicalError::Codec)
}

/// Strictly decode canonical bytes into a [`RuleIR`], rejecting any
/// non-canonical encoding.
pub fn decode_rule(bytes: &[u8]) -> Result<RuleIR, CanonicalDecodeError> {
    let (rule, rest) =
        postcard::take_from_bytes::<RuleIR>(bytes).map_err(CanonicalDecodeError::Codec)?;
    if !rest.is_empty() {
        return Err(CanonicalDecodeError::TrailingBytes);
    }
    decode::validate_rule(&rule)?;
    Ok(rule)
}

/// Encode a [`PolicyBundle`] to canonical bytes.
pub fn encode_policy(policy: &PolicyBundle) -> Result<Vec<u8>, CanonicalError> {
    let mut c = policy.clone();
    encode::canonicalize_policy(&mut c)?;
    postcard::to_stdvec(&c).map_err(CanonicalError::Codec)
}

/// Strictly decode canonical bytes into a [`PolicyBundle`].
pub fn decode_policy(bytes: &[u8]) -> Result<PolicyBundle, CanonicalDecodeError> {
    let (policy, rest) =
        postcard::take_from_bytes::<PolicyBundle>(bytes).map_err(CanonicalDecodeError::Codec)?;
    if !rest.is_empty() {
        return Err(CanonicalDecodeError::TrailingBytes);
    }
    decode::validate_policy(&policy)?;
    Ok(policy)
}

/// Encode a [`Manifest`] to canonical bytes. Gate 1 validates structure only;
/// the self-referential `artifact_hash` patch is Gate 4 (see
/// [`crate::manifest`]).
pub fn encode_manifest(manifest: &Manifest) -> Result<Vec<u8>, CanonicalError> {
    let mut c = manifest.clone();
    encode::canonicalize_manifest(&mut c)?;
    postcard::to_stdvec(&c).map_err(CanonicalError::Codec)
}

/// Strictly decode canonical bytes into a [`Manifest`].
pub fn decode_manifest(bytes: &[u8]) -> Result<Manifest, CanonicalDecodeError> {
    let (manifest, rest) =
        postcard::take_from_bytes::<Manifest>(bytes).map_err(CanonicalDecodeError::Codec)?;
    if !rest.is_empty() {
        return Err(CanonicalDecodeError::TrailingBytes);
    }
    decode::validate_manifest(&manifest)?;
    Ok(manifest)
}
