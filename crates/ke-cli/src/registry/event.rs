//! Append-only, hash-chained, registry-root-signed lifecycle events
//! (ADR 0009, ADR 0012 §2/§3).
//!
//! # State lives in the event log, not the artifact (ADR 0012 §2)
//!
//! The current lifecycle state of an artifact is **the `new_state` of the
//! highest-`seq` event**, never a mutable field. `ke-artifact`'s
//! `RegistryStateMetadata` stays the inert `Draft` marker; the real state
//! machine ([`super::LifecycleState`]) is derived by walking the event log.
//!
//! # Signed bytes (payload-prefix; one canonicalization scheme, reused)
//!
//! `signature` is the **last** field of [`LifecycleEvent`], so the signed
//! bytes are the postcard serialization of every field before it — the literal
//! byte prefix of `postcard(LifecycleEvent)`. The prefix length is recovered by
//! decoding a private [`EventPayloadView`] (all fields minus the signature)
//! with `postcard::take_from_bytes` — the exact `EnvelopeView`/attestation
//! technique from `ke-artifact`. ed25519 over that prefix; no second
//! canonicalization scheme exists.
//!
//! # Authority (ADR 0009, ADR 0012 §2; CLAUDE.md authority boundaries)
//!
//! **Every** lifecycle event is signed by the **registry-root key**
//! ([`SignerRole::Registry`]). The triggering authority (the compiler/CI that
//! drove `structurally_verified`, the experts behind `expert_attested`, etc.)
//! is recorded in `authority_role` + `authority_key_id` but does **not** sign
//! the event — the §9 transition-authority rules are enforced as
//! *preconditions* the registry checks before appending (see
//! [`super::can_transition`]). This keeps the §9 authority semantics without
//! inventing per-actor event signing.
//!
//! # Hash chain (ADR 0012 §3)
//!
//! `prev_event_hash = blake3(canonical bytes of the prior event, INCLUDING its
//! signature)`; the first event carries `None`. A gap, reorder, broken link,
//! or bad signature is a typed [`super::RegistryError`], surfaced by
//! [`super::current_state`].
//!
//! # Key hygiene (review-lessons memory; windows-gnu toolchain)
//!
//! The registry-root test key is **fixed-seed**, gated `any(test, feature =
//! "test-keys")`, and loudly named ([`test_keys::REGISTRY_ROOT_KEY_ID`] =
//! `"test-registry-fixed-seed-1"`). The ungated [`REGISTRY_ROOT_PUBLIC_KEY`]
//! const lets verification compile without the feature (mirrors
//! `ke_artifact::tsa::MOCK_TSA_PUBLIC_KEY`). **Never** `OsRng`/`getrandom`.

use crate::registry::RegistryError;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use ke_artifact::{SignerRole, TimestampToken};
use serde::{Deserialize, Serialize};

use super::LifecycleState;

/// The registry-root ed25519 **public** key — the verifying key derived from
/// `test_keys::REGISTRY_ROOT_SEED`, embedded ungated so event verification
/// works in builds without the `test-keys` feature. A gated unit test pins it
/// to the seed-derived key (mirrors `ke_artifact::tsa::MOCK_TSA_PUBLIC_KEY`).
///
/// This is a **test** key directory entry: a real ADR-0009 registry root lives
/// in HSM custody (infra; out of Phase 3a). Local-FS registry objects signed
/// by this key are non-authoritative (ADR 0012 §6).
pub const REGISTRY_ROOT_PUBLIC_KEY: [u8; 32] = REGISTRY_ROOT_PUBLIC_KEY_BYTES;

// Derived from the gated test seed `REGISTRY_ROOT_SEED`; the unit test
// `registry_root_public_key_const_matches_seed` pins it (so it can never drift).
const REGISTRY_ROOT_PUBLIC_KEY_BYTES: [u8; 32] = [
    0xab, 0x4b, 0x87, 0x6b, 0x11, 0x80, 0x33, 0x9d, 0xfe, 0x2a, 0xec, 0xec, 0x7b, 0x1f, 0x60, 0x3a,
    0xe6, 0xba, 0xba, 0x31, 0x72, 0x08, 0xe2, 0x21, 0x39, 0x51, 0x2d, 0x1c, 0x06, 0x51, 0x32, 0xbf,
];

/// One append-only lifecycle event (ADR 0012 §2/§3). **Field declaration order
/// is the byte contract** — the signed payload is the postcard prefix of every
/// field before `signature`, and `prev_event_hash` chains the whole canonical
/// bytes (signature included) of the prior event. Do not reorder or insert
/// fields without re-pinning the canonical-event-head-hash test.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleEvent {
    /// BLAKE3 content hash of the artifact this event transitions.
    pub artifact_hash: [u8; 32],
    /// 0-based contiguous sequence number within this artifact's event log.
    pub seq: u32,
    /// The state before this transition; `None` only for the first (draft)
    /// event.
    pub prior_state: Option<LifecycleState>,
    /// The state after this transition (the current state if this is the
    /// highest-`seq` event).
    pub new_state: LifecycleState,
    /// One of the §9 transition kinds (plus an initial `draft`): `draft |
    /// structurally_verified | ml_checked | expert_attested | published |
    /// deprecated | revoked | tag_moved | policy_moved`.
    pub event_kind: String,
    /// The triggering authority's key id (ADR 0012 §2 event body). The event
    /// itself is registry-root-signed; this records *who drove* the transition.
    pub authority_key_id: String,
    /// The triggering authority's role (compiler/CI, expert, registry policy).
    pub authority_role: SignerRole,
    /// Trusted-timestamp token for this event (ADR 0010; mock TSA in Phase 3a).
    pub timestamp: TimestampToken,
    /// `blake3(canonical bytes of the prior event, INCLUDING its signature)`;
    /// `None` for the first event.
    pub prev_event_hash: Option<[u8; 32]>,
    /// ed25519 by the **registry-root** key over the payload prefix (all
    /// fields above). **Must stay the last field** — the prefix-signing scheme
    /// depends on it.
    #[serde(with = "serde_bytes_64")]
    pub signature: [u8; 64],
}

/// All [`LifecycleEvent`] fields **minus the signature**, used only to recover
/// the payload-prefix length from serialized bytes via
/// `postcard::take_from_bytes` cursor arithmetic. Never constructed for its
/// field values.
#[derive(Deserialize)]
#[allow(dead_code)]
struct EventPayloadView {
    artifact_hash: [u8; 32],
    seq: u32,
    prior_state: Option<LifecycleState>,
    new_state: LifecycleState,
    event_kind: String,
    authority_key_id: String,
    authority_role: SignerRole,
    timestamp: TimestampToken,
    prev_event_hash: Option<[u8; 32]>,
}

impl LifecycleEvent {
    /// The signed payload prefix: the postcard bytes of every field before
    /// `signature`. Recovered via [`EventPayloadView`] cursor arithmetic, so it
    /// is the literal prefix of `postcard(LifecycleEvent)` regardless of the
    /// current `signature` value (the last 64 bytes are simply truncated).
    pub fn payload_prefix(&self) -> Result<Vec<u8>, RegistryError> {
        let mut bytes = postcard::to_stdvec(self).map_err(RegistryError::event_encode)?;
        let (_, rest) = postcard::take_from_bytes::<EventPayloadView>(&bytes)
            .map_err(RegistryError::event_encode)?;
        let prefix_len = bytes.len() - rest.len();
        bytes.truncate(prefix_len);
        Ok(bytes)
    }

    /// Full canonical bytes of this event, **including** the signature — the
    /// bytes the *next* event's `prev_event_hash` is computed over.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, RegistryError> {
        postcard::to_stdvec(self).map_err(RegistryError::event_encode)
    }

    /// `blake3(canonical bytes including signature)` — the link the next event
    /// records as its `prev_event_hash` (ADR 0012 §3).
    pub fn chain_hash(&self) -> Result<[u8; 32], RegistryError> {
        Ok(*blake3::hash(&self.canonical_bytes()?).as_bytes())
    }

    /// Verify this event's registry-root signature over its payload prefix.
    /// Returns [`RegistryError::SignatureInvalid`] on any mismatch.
    pub fn verify_signature(&self) -> Result<(), RegistryError> {
        let verifying_key = VerifyingKey::from_bytes(&REGISTRY_ROOT_PUBLIC_KEY)
            .map_err(|_| RegistryError::SignatureInvalid { seq: self.seq })?;
        let prefix = self.payload_prefix()?;
        verifying_key
            .verify(&prefix, &Signature::from_bytes(&self.signature))
            .map_err(|_| RegistryError::SignatureInvalid { seq: self.seq })
    }
}

/// Sign a lifecycle-event payload with the registry-root key: ed25519 over the
/// payload prefix (module doc). Any pre-existing `signature` value is ignored
/// and overwritten. Deterministic per RFC 8032 — same payload + same key =>
/// identical signature bytes. Gated like all signing entry points: only the
/// registry root signs events.
#[cfg(any(test, feature = "test-keys"))]
pub fn sign_event(mut event: LifecycleEvent) -> Result<LifecycleEvent, RegistryError> {
    use ed25519_dalek::Signer;
    let prefix = event.payload_prefix()?;
    event.signature = test_keys::registry_root_signing_key()
        .sign(&prefix)
        .to_bytes();
    Ok(event)
}

/// Serde support for `[u8; 64]` (serde's derive covers arrays only up to 32).
/// Encodes as a 64-element tuple — byte-identical under postcard to a native
/// fixed array (no length prefix). Mirrors `ke_artifact::artifact`'s
/// `serde_bytes_64` (that one is `pub(crate)`, so this is a local twin with the
/// same byte behavior).
pub(crate) mod serde_bytes_64 {
    use serde::de::{Error, SeqAccess, Visitor};
    use serde::ser::SerializeTuple;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S: Serializer>(value: &[u8; 64], serializer: S) -> Result<S::Ok, S::Error> {
        let mut tuple = serializer.serialize_tuple(64)?;
        for byte in value {
            tuple.serialize_element(byte)?;
        }
        tuple.end()
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<[u8; 64], D::Error> {
        struct ArrayVisitor;
        impl<'de> Visitor<'de> for ArrayVisitor {
            type Value = [u8; 64];
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an array of 64 bytes")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut out = [0u8; 64];
                for (i, slot) in out.iter_mut().enumerate() {
                    *slot = seq
                        .next_element()?
                        .ok_or_else(|| A::Error::invalid_length(i, &self))?;
                }
                Ok(out)
            }
        }
        deserializer.deserialize_tuple(64, ArrayVisitor)
    }
}

/// Deterministic fixed-seed registry-root test key (review-lessons memory:
/// loud key_id, no getrandom). Gated `any(test, feature = "test-keys")` because
/// `cfg(test)` is invisible to the `ke` bin target — `ke compile` signs events
/// under the `test-keys` feature.
///
/// Every event signed with this key carries `authority_role =
/// SignerRole::Registry` and is verifiable against the ungated
/// [`REGISTRY_ROOT_PUBLIC_KEY`]. The `test-` prefix marks it loudly as
/// non-production; a real ADR-0009 registry root is HSM-custodied (infra).
/// **Never** `OsRng`/`getrandom`.
#[cfg(any(test, feature = "test-keys"))]
pub mod test_keys {
    use ed25519_dalek::{SigningKey, VerifyingKey};

    /// The `key_id` the registry-root test key carries on every signed event.
    /// The `test-` prefix is asserted by the registry suite.
    pub const REGISTRY_ROOT_KEY_ID: &str = "test-registry-fixed-seed-1";

    /// Fixed 32-byte ed25519 seed (printable on purpose — loudly a test key).
    /// Distinct from every `ke_artifact` test seed.
    pub const REGISTRY_ROOT_SEED: [u8; 32] = *b"ke-workbench-test-registry-seed1";

    /// The registry-root signing key. Deterministic; never random.
    pub fn registry_root_signing_key() -> SigningKey {
        SigningKey::from_bytes(&REGISTRY_ROOT_SEED)
    }

    /// The verifying key for [`registry_root_signing_key`]. Its byte form is
    /// embedded ungated as [`super::REGISTRY_ROOT_PUBLIC_KEY`].
    pub fn registry_root_verifying_key() -> VerifyingKey {
        registry_root_signing_key().verifying_key()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the ungated [`REGISTRY_ROOT_PUBLIC_KEY`] const to the key actually
    /// derived from the gated fixed seed — the two can never drift.
    #[test]
    fn registry_root_public_key_const_matches_seed() {
        let derived = test_keys::registry_root_verifying_key().to_bytes();
        let hex = crate::registry::backend::hex32(&derived);
        assert_eq!(
            REGISTRY_ROOT_PUBLIC_KEY, derived,
            "REGISTRY_ROOT_PUBLIC_KEY must equal the seed-derived verifying key (hex: {hex})"
        );
    }

    #[test]
    fn registry_root_key_id_is_test_prefixed() {
        assert!(test_keys::REGISTRY_ROOT_KEY_ID.starts_with("test-"));
    }
}
