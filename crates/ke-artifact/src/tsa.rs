//! Trusted-timestamp tokens (ADR 0010, spec § 10 "Timestamp authority").
//!
//! A [`TimestampToken`] carries its claimed [`TimestampAuthorityClass`]
//! **inside the signed attestation payload**, and the class is **re-derived
//! from the token bytes at verification** ([`derive_class`]) — a claimed
//! class that the token does not actually re-derive to is a
//! `TimestampClassMismatch` rejection. This is ADR 0010's tamper-evident
//! binding: relabelling a mock token as production (or vice versa) is
//! mechanically detectable.
//!
//! Only the deterministic [`MockTsa`] is verifiable today. RFC 3161 token
//! parsing/verification waits for vendor onboarding (ADR 0010 blocker), so
//! `Rfc3161External` / `Rfc3161Internal` claims yield a typed
//! [`TsaError::Unsupported`] — honest, because production publish is blocked
//! by ADR 0010 anyway.
//!
//! **Key hygiene:** no `OsRng`/`getrandom` anywhere; the mock TSA key is
//! fixed-seed (`crate::sign::test_keys`, gated `any(test, feature =
//! "test-keys")`). The token *types* and *verify logic* here are normal
//! (ungated) — the mock **public** key is embedded as
//! [`MOCK_TSA_PUBLIC_KEY`] so re-derivation compiles without the feature;
//! only the secret seed is gated.

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The mock TSA's authority identifier. The `test-` prefix marks every
/// mock-stamped token as non-production (rejection R8 under non-local
/// policy).
pub const MOCK_TSA_AUTHORITY_ID: &str = "test-mock-tsa-1";

/// The mock TSA's ed25519 **public** key — the verifying key derived from
/// `crate::sign::test_keys::MOCK_TSA_SEED`, embedded ungated so token
/// re-derivation works in builds without the `test-keys` feature. A unit
/// test below pins it to the seed-derived key.
pub const MOCK_TSA_PUBLIC_KEY: [u8; 32] = [
    0x09, 0x57, 0xc8, 0x3d, 0x33, 0xc4, 0xe4, 0xe7, 0xc1, 0x3b, 0x3c, 0x8f, 0x52, 0x24, 0xe8, 0xa6,
    0x99, 0x94, 0xae, 0x46, 0xe3, 0xb2, 0xea, 0x31, 0xed, 0x8a, 0x47, 0x86, 0xd7, 0x50, 0x82, 0x21,
];

/// Which authority class stamped a token (ADR 0010). The class is part of
/// the signed attestation payload **and** re-derivable from the token bytes;
/// the two must agree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimestampAuthorityClass {
    /// An external RFC 3161 TSA (production; vendor onboarding pending).
    Rfc3161External { tsa_identity: String },
    /// An internally-operated RFC 3161 TSA (vendor onboarding pending).
    Rfc3161Internal { tsa_identity: String },
    /// The deterministic mock TSA — local development only (rejection R8
    /// under non-local policy).
    Mock,
}

/// A trusted-timestamp token bound into the attestation payload (ADR 0010).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimestampToken {
    /// The claimed authority class — re-derived from `token` at
    /// verification; mismatch is rejected.
    pub class: TimestampAuthorityClass,
    /// The authority's token bytes. For [`TimestampAuthorityClass::Mock`]
    /// this is a 64-byte ed25519 signature by the mock TSA key over
    /// `(class discriminant byte, payload hash, claimed time LE)`; for the
    /// RFC 3161 classes it will be the DER `TimeStampToken` once a vendor is
    /// onboarded.
    pub token: Vec<u8>,
    /// The signing time the authority claims, unix seconds (caller-supplied
    /// clock — never read from the environment here).
    pub claimed_time_unix: u64,
}

/// Timestamp re-derivation failures, mapped onto
/// `AttestationRejection::{TsaUnsupported, TimestampClassMismatch}` by the
/// attestation verifier.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum TsaError {
    /// The claimed class cannot be verified yet (RFC 3161 vendor onboarding
    /// pending, ADR 0010).
    #[error(
        "timestamp authority class {0:?} not yet verifiable (ADR 0010 vendor onboarding pending)"
    )]
    Unsupported(TimestampAuthorityClass),
    /// The token bytes do not re-derive to the claimed class (wrong key,
    /// tampered token, or relabelled class).
    #[error("timestamp token does not re-derive to its claimed authority class")]
    ClassMismatch,
}

/// The class's postcard variant discriminant, used as the first byte of the
/// mock-signed message so a token can never be replayed under a different
/// class.
fn class_discriminant(class: &TimestampAuthorityClass) -> u8 {
    match class {
        TimestampAuthorityClass::Rfc3161External { .. } => 0,
        TimestampAuthorityClass::Rfc3161Internal { .. } => 1,
        TimestampAuthorityClass::Mock => 2,
    }
}

/// The 41-byte message a mock token signs:
/// `discriminant(1) || payload_hash(32) || claimed_time_unix LE(8)`.
fn mock_message(payload_hash: &[u8; 32], claimed_time_unix: u64) -> [u8; 41] {
    let mut msg = [0u8; 41];
    msg[0] = class_discriminant(&TimestampAuthorityClass::Mock);
    msg[1..33].copy_from_slice(payload_hash);
    msg[33..41].copy_from_slice(&claimed_time_unix.to_le_bytes());
    msg
}

/// Re-derive the authority class from the token bytes (ADR 0010).
///
/// `payload_hash` is the hash the authority stamped — for attestations, the
/// attested `artifact_hash`. A token that verifies as a mock-TSA signature
/// over `(Mock, payload_hash, claimed_time)` derives to `Mock` regardless of
/// its claimed class (the caller compares derived vs claimed and rejects a
/// mismatch). Otherwise: RFC 3161 claims are [`TsaError::Unsupported`] until
/// vendor onboarding; a `Mock` claim that fails mock verification is
/// [`TsaError::ClassMismatch`].
pub fn derive_class(
    token: &TimestampToken,
    payload_hash: &[u8; 32],
) -> Result<TimestampAuthorityClass, TsaError> {
    if verifies_as_mock(token, payload_hash) {
        return Ok(TimestampAuthorityClass::Mock);
    }
    match &token.class {
        TimestampAuthorityClass::Rfc3161External { .. }
        | TimestampAuthorityClass::Rfc3161Internal { .. } => {
            Err(TsaError::Unsupported(token.class.clone()))
        }
        TimestampAuthorityClass::Mock => Err(TsaError::ClassMismatch),
    }
}

/// True iff the token bytes are a valid mock-TSA signature over
/// `(Mock, payload_hash, claimed_time)`.
fn verifies_as_mock(token: &TimestampToken, payload_hash: &[u8; 32]) -> bool {
    let Ok(sig_bytes) = <[u8; 64]>::try_from(token.token.as_slice()) else {
        return false;
    };
    let Ok(verifying_key) = VerifyingKey::from_bytes(&MOCK_TSA_PUBLIC_KEY) else {
        return false;
    };
    let msg = mock_message(payload_hash, token.claimed_time_unix);
    verifying_key
        .verify(&msg, &Signature::from_bytes(&sig_bytes))
        .is_ok()
}

/// The deterministic mock TSA (local development only — rejection R8 under
/// non-local policy). Stamping needs the fixed-seed secret key, so it is
/// gated like all test key material; verification ([`derive_class`]) is not.
#[cfg(any(test, feature = "test-keys"))]
pub struct MockTsa;

#[cfg(any(test, feature = "test-keys"))]
impl MockTsa {
    /// Stamp a payload hash at a **caller-supplied** time (no clock is read
    /// here — determinism is the point of the mock).
    pub fn stamp(payload_hash: &[u8; 32], claimed_time_unix: u64) -> TimestampToken {
        use ed25519_dalek::Signer;
        let msg = mock_message(payload_hash, claimed_time_unix);
        let signature = crate::sign::test_keys::mock_tsa_signing_key().sign(&msg);
        TimestampToken {
            class: TimestampAuthorityClass::Mock,
            token: signature.to_bytes().to_vec(),
            claimed_time_unix,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sign::test_keys;

    /// Pins the ungated [`MOCK_TSA_PUBLIC_KEY`] const to the key actually
    /// derived from the gated fixed seed — the two can never drift.
    #[test]
    fn mock_public_key_const_matches_seed_derived_key() {
        let derived = test_keys::mock_tsa_verifying_key().to_bytes();
        let hex = crate::hex32(&derived);
        assert_eq!(
            MOCK_TSA_PUBLIC_KEY, derived,
            "MOCK_TSA_PUBLIC_KEY must equal the seed-derived verifying key (hex: {hex})"
        );
    }

    #[test]
    fn stamp_round_trips_to_mock_class() {
        let hash = [7u8; 32];
        let token = MockTsa::stamp(&hash, 1_750_000_000);
        assert_eq!(
            derive_class(&token, &hash),
            Ok(TimestampAuthorityClass::Mock)
        );
    }

    #[test]
    fn authority_id_is_test_prefixed() {
        assert!(MOCK_TSA_AUTHORITY_ID.starts_with("test-"));
    }
}
