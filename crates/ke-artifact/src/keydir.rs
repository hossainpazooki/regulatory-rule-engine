//! Key directory (ADR 0009 sketch): signer roles, key lifecycle status, and
//! the per-key entry the attestation verifier resolves `key_id` against
//! (rejection rule R1: unknown / expired / revoked / unauthorized).
//!
//! Phase 2 ships the **types plus verification against an in-memory
//! instance** only. The registry-signed directory *object* (a directory
//! signed by the registry root, with custody and rotation of that root) is
//! **Phase 3** — nothing here signs or transitions lifecycle state, and a
//! `KeyDirectory` value carries no authority of its own.

use ke_core::manifest::AttestationType;
use serde::{Deserialize, Serialize};

/// Authorization basis for an attestation signature (ADR 0009). The role ->
/// allowed-types map lives on the [`KeyDirectoryEntry`], not here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignerRole {
    /// A domain expert — the only authority that can sign typed attestations
    /// (spec § 5, § 10).
    DomainExpert,
    /// May sign `publication_approval` (honored only with required
    /// co-attestations — rejection rule R7).
    PublicationApprover,
    /// The registry itself (directory/lifecycle signatures — Phase 3).
    Registry,
}

/// Key lifecycle status (ADR 0009; rejection rule R1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyStatus {
    Active,
    Expired,
    Revoked,
}

/// One key's directory entry (ADR 0009 sketch). The attestation verifier
/// checks status, validity window, roles, and per-type authorization here
/// before trusting a signature (rejection rule R1 family).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyDirectoryEntry {
    /// The identifier attestations carry in `key_id`.
    pub key_id: String,
    /// The ed25519 public key signatures are verified against.
    pub public_key: [u8; 32],
    /// Roles this key may sign under.
    pub signer_roles: Vec<SignerRole>,
    /// Attestation types this key is authorized for (R1 "unauthorized").
    pub authorized_attestation_types: Vec<AttestationType>,
    /// Validity window, unix seconds, `[from, to)` half-open.
    pub valid_from_unix: u64,
    pub valid_to_unix: u64,
    /// Lifecycle status (R1 "expired" / "revoked").
    pub status: KeyStatus,
    /// When the key was revoked, if it was.
    pub revoked_at_unix: Option<u64>,
    /// Why the key was revoked, if it was.
    pub revocation_reason: Option<String>,
    /// Hash of the registry revocation event (event log is Phase 3).
    pub revocation_event_hash: Option<[u8; 32]>,
}

/// An in-memory key directory. **Not** the registry-signed directory object
/// (Phase 3) — this is the lookup structure the verifier runs against.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyDirectory {
    pub entries: Vec<KeyDirectoryEntry>,
}

impl KeyDirectory {
    /// Resolve a `key_id` to its entry; `None` is rejection R1 "unknown".
    pub fn lookup(&self, key_id: &str) -> Option<&KeyDirectoryEntry> {
        self.entries.iter().find(|entry| entry.key_id == key_id)
    }
}
