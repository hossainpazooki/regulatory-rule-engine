//! ed25519 compiler signatures over the **hash-patched envelope prefix**
//! (spec § 8). Compiler authority is structural validity only — never legal
//! truth (spec § 5); attestation signing is Phase 2 and does not live here.
//!
//! The signed range is the envelope prefix `[0, envelope_len)` of the `.kew`
//! bytes *after* the `artifact_hash` patch — verifiers check the signature
//! over the prefix bytes they hold (no re-zeroing for the signature; the
//! re-zeroing applies only to hash recomputation, see [`crate::hash`]).
//!
//! **Key hygiene (windows-gnu toolchain, brief § 3.3):** never use
//! `OsRng`/`getrandom` anywhere in this crate. Deterministic fixed-seed test
//! keys live in [`test_keys`] behind the `test-keys` feature.

use crate::artifact::CompilerSignature;
use crate::ArtifactError;
pub use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};

/// Sign the hash-patched envelope prefix with the compiler key. `key_id`
/// names the key in the resulting [`CompilerSignature`]; fixed-seed test keys
/// must pass [`test_keys::TEST_KEY_ID`] so committed signatures can never be
/// mistaken for ADR-0009 production-key signatures.
pub fn sign_envelope(
    envelope_prefix: &[u8],
    signing_key: &SigningKey,
    key_id: &str,
) -> CompilerSignature {
    let signature = signing_key.sign(envelope_prefix);
    CompilerSignature {
        key_id: key_id.to_string(),
        signature: signature.to_bytes(),
    }
}

/// Verify a compiler signature over the hash-patched envelope prefix.
/// Returns [`ArtifactError::SignatureInvalid`] on any mismatch (tampered
/// bytes, wrong key, corrupted signature) — identifiable per spec § 8.3.
pub fn verify_signature(
    envelope_prefix: &[u8],
    compiler_signature: &CompilerSignature,
    verifying_key: &VerifyingKey,
) -> Result<(), ArtifactError> {
    let signature = Signature::from_bytes(&compiler_signature.signature);
    verifying_key
        .verify(envelope_prefix, &signature)
        .map_err(|_| ArtifactError::SignatureInvalid)
}

/// Deterministic fixed-seed test keys (plan Rev 2 correction #2).
///
/// Gated on `any(test, feature = "test-keys")` because `cfg(test)` is
/// invisible to bin targets — the `gen-golden-artifacts` generator declares
/// `required-features = ["test-keys"]`. The seed is fixed so golden-vector
/// signatures are byte-stable; **no `OsRng`/`getrandom` keygen anywhere**
/// (windows-gnu toolchain cannot build getrandom 0.3, brief § 3.3).
///
/// Every signature produced with these keys must carry
/// [`TEST_KEY_ID`] (`"test-fixed-seed-1"`) so it can never be mistaken for an
/// ADR-0009 production-key signature.
#[cfg(any(test, feature = "test-keys"))]
pub mod test_keys {
    use super::{SigningKey, VerifyingKey};

    /// The `key_id` every fixed-seed signature carries. The `test-` prefix is
    /// asserted by the golden suite.
    pub const TEST_KEY_ID: &str = "test-fixed-seed-1";

    /// Fixed 32-byte ed25519 seed (printable on purpose — loudly a test key).
    pub const FIXED_SEED: [u8; 32] = *b"ke-workbench-test-fixed-seed-1!!";

    /// The fixed-seed signing key. Deterministic; never random.
    pub fn signing_key() -> SigningKey {
        SigningKey::from_bytes(&FIXED_SEED)
    }

    /// The verifying key for [`signing_key`].
    pub fn verifying_key() -> VerifyingKey {
        signing_key().verifying_key()
    }

    // ---- Phase 2: domain-expert test key (attestation signing) ----

    /// The `key_id` every fixed-seed **expert** attestation carries. Distinct
    /// from [`TEST_KEY_ID`] so test attestations can never be mistaken for
    /// compiler signatures or ADR-0009 production expert keys.
    pub const TEST_EXPERT_KEY_ID: &str = "test-expert-fixed-seed-1";

    /// Fixed 32-byte expert seed — distinct from [`FIXED_SEED`] and from the
    /// mock-TSA seed (printable on purpose — loudly a test key).
    pub const EXPERT_FIXED_SEED: [u8; 32] = *b"ke-workbench-test-expert-seed-1!";

    /// The fixed-seed expert signing key. Deterministic; never random.
    pub fn expert_signing_key() -> SigningKey {
        SigningKey::from_bytes(&EXPERT_FIXED_SEED)
    }

    /// The verifying key for [`expert_signing_key`].
    pub fn expert_verifying_key() -> VerifyingKey {
        expert_signing_key().verifying_key()
    }

    // ---- Phase 2: mock-TSA test key (timestamp tokens, ADR 0010) ----

    /// Fixed 32-byte mock-TSA seed — distinct from both signing seeds above.
    /// The mock TSA's authority id is
    /// [`crate::tsa::MOCK_TSA_AUTHORITY_ID`] (`"test-mock-tsa-1"`).
    pub const MOCK_TSA_SEED: [u8; 32] = *b"ke-workbench-test-mock-tsa-seed1";

    /// The mock-TSA signing key. Deterministic; never random.
    pub fn mock_tsa_signing_key() -> SigningKey {
        SigningKey::from_bytes(&MOCK_TSA_SEED)
    }

    /// The verifying key for [`mock_tsa_signing_key`]. Its byte form is
    /// embedded ungated as [`crate::tsa::MOCK_TSA_PUBLIC_KEY`] so token
    /// re-derivation works without the `test-keys` feature; a unit test in
    /// [`crate::tsa`] pins the two to each other.
    pub fn mock_tsa_verifying_key() -> VerifyingKey {
        mock_tsa_signing_key().verifying_key()
    }
}
