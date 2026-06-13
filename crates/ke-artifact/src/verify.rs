//! The **consumer-agnostic verification surface** (Gate 4 Phase 4a, ADR 0016).
//!
//! One call — [`verify_artifact`] — folds the four pure Phase 1–2 verifiers
//! ([`decode_artifact`](crate::decode_artifact), [`verify_hash`](crate::verify_hash),
//! [`verify_signature`](crate::verify_signature),
//! [`verify_attestation_set`](crate::verify_attestation_set)) together with the
//! **registry lifecycle state** into a single [`VerificationOutcome`], and always
//! emits a canonical [`ArtifactProvenance`] export. This is the surface the
//! PyO3 and WASM bindings (Phase 4b) wrap verbatim.
//!
//! # Purity (WASM-ready; ADR 0016)
//!
//! This module performs **no I/O and no RNG**. The registry state arrives as
//! **data** ([`RegistryEvidence`]) — the registry read happens at the `ke-cli`
//! edge (`ke export-provenance`), never here. ed25519 *verify* is deterministic;
//! the only RNG-adjacent code in the crate is signing / fixed-seed test keys
//! (feature-gated), and nothing on this path touches it. The compiler verifying
//! key is resolved from the caller-supplied [`KeyDirectory`] by the signature's
//! `key_id`, so this surface never embeds or generates a key.
//!
//! # Registry state is part of the verdict (the COMPASS correctness fix)
//!
//! An offline consumer (COMPASS on Vercel) must refuse a **non-`Published`**
//! pack even when its crypto is valid, and must detect a **stale** embedded
//! event-head against a freshly-fetched live head. [`verify_artifact`] folds
//! both checks in **after** the cryptographic checks pass.

use crate::artifact::{decode_artifact, Artifact};
use crate::attestation::{verify_attestation_set, AttestationRejection, PolicyContext};
use crate::hash::verify_hash;
use crate::keydir::KeyDirectory;
use crate::sign::{verify_signature, VerifyingKey};
use crate::tsa::TimestampAuthorityClass;
use ke_core::manifest::{AttestationType, VerificationPolicy};
use serde::{Deserialize, Serialize};

/// The `ke-artifact`-local mirror of the `ke-cli` registry `LifecycleState`
/// (ADR 0016). `ke-artifact` stays backend-free, so it cannot depend on
/// `ke-cli`; the `LifecycleState -> RegistryStatus` mapping lives at the
/// `ke-cli` boundary (`ke export-provenance`). Only `Published` is
/// authoritative for execution; everything else makes [`verify_artifact`]
/// reject.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegistryStatus {
    /// The artifact is the live, published content for its selector.
    Published,
    /// Superseded but not revoked (still readable; not authoritative to start
    /// new work).
    Deprecated,
    /// Revoked — must never be treated as authoritative (spec § 15).
    Revoked,
    /// No recorded lifecycle state, or a pre-`Published` state
    /// (draft / structurally_verified / ml_checked / expert_attested).
    Unknown,
}

/// Registry evidence carried into [`verify_artifact`] as **data** (no I/O on
/// this path). `status` and `event_head_hash` are the registry state and the
/// chain-hash of the head event **as of export**; `live_event_head`, if
/// `Some`, is a freshly-fetched head the consumer supplies for **staleness
/// detection** — a mismatch is [`RejectionReason::StaleEventHead`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegistryEvidence {
    /// The registry lifecycle status as of export.
    pub status: RegistryStatus,
    /// `blake3(canonical head event bytes incl. signature)` as of export.
    pub event_head_hash: [u8; 32],
    /// A freshly-fetched live head, if the consumer has one; `None` skips the
    /// staleness check.
    pub live_event_head: Option<[u8; 32]>,
}

/// Why [`verify_artifact`] rejected. First failure short-circuits in the order
/// decode → hash → compiler signature → attestations → registry state →
/// staleness.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RejectionReason {
    /// The `.kew` bytes did not decode (carries the underlying error string).
    Decode(String),
    /// The recomputed content hash did not match `manifest.artifact_hash`.
    HashMismatch,
    /// The ed25519 compiler signature did not verify over the envelope prefix
    /// (also covers an unresolvable / malformed compiler key in the directory).
    CompilerSignatureInvalid,
    /// One or more attestation rejections (per-attestation R1–R5/R8 + set-level
    /// R6/R7). Carries the full list.
    Attestations(Vec<AttestationRejection>),
    /// The registry status is not `Published` (the COMPASS correctness fix:
    /// refuse deprecated / revoked / unknown packs even with valid crypto).
    NotPublished { status: RegistryStatus },
    /// The embedded event-head hash does not match the supplied live head — the
    /// export is stale (the registry advanced since it was produced).
    StaleEventHead { embedded: [u8; 32], live: [u8; 32] },
}

/// The verdict of [`verify_artifact`]: verified, or rejected with the first
/// failing reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    Verified,
    Rejected(RejectionReason),
}

/// The full outcome of [`verify_artifact`]: the [`Verdict`], the canonical
/// [`ArtifactProvenance`] (**always** built, even on rejection — a consumer
/// surfaces provenance regardless), and the registry status the verdict
/// considered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerificationOutcome {
    pub verdict: Verdict,
    pub provenance: ArtifactProvenance,
    pub registry_state: RegistryStatus,
}

/// One attestation, summarized for the provenance export (no signature bytes).
/// `is_test_key` is `signer_key_id.starts_with("test-")` — it loudly surfaces a
/// fixed-seed test key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttestationSummary {
    pub attestation_type: AttestationType,
    pub signer_key_id: String,
    pub is_test_key: bool,
    pub tsa_class: TimestampAuthorityClass,
    pub claimed_time_unix: u64,
}

/// The canonical, consumer-agnostic provenance export (ADR 0016). Plain serde
/// over stable struct field order, so [`to_canonical_json`](ArtifactProvenance::to_canonical_json)
/// is byte-stable across runs and both bindings emit/read the same JSON.
///
/// Carries the **registry state** + the **event-head hash** so an offline
/// consumer refuses non-`Published` packs and detects staleness.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactProvenance {
    /// The regime the artifact encodes (from the manifest).
    pub regime_id: String,
    /// The artifact content hash (`manifest.artifact_hash`).
    pub artifact_hash: [u8; 32],
    /// IR schema version, rendered `major.minor.patch`.
    pub ir_schema_version: String,
    /// Wire codec version (e.g. `postcard-1`).
    pub codec_version: String,
    /// Canonicalization-profile version (e.g. `ke-canon-4`).
    pub canonicalization_version: String,
    /// The compiler signature's `key_id`.
    pub signer_key_id: String,
    /// `signer_key_id.starts_with("test-")` — `test-*` keys are not production.
    pub is_test_key: bool,
    /// One summary per attestation (no signature bytes).
    pub attestations: Vec<AttestationSummary>,
    /// The registry lifecycle status as of export.
    pub registry_state: RegistryStatus,
    /// The registry event-head hash as of export.
    pub registry_event_head_hash: [u8; 32],
    /// When the export was produced, unix seconds (caller-supplied at the CLI
    /// edge — never read from the environment here).
    pub exported_at_unix: u64,
}

impl ArtifactProvenance {
    /// Serialize to one canonical JSON string. Key order is the struct field
    /// declaration order (serde_json preserves it), so the output is byte-stable
    /// for a given value.
    pub fn to_canonical_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

/// True iff `key_id` is a fixed-seed test key (loud `test-` prefix). Surfaced
/// in the provenance so a consumer can refuse non-production keys.
fn is_test_key(key_id: &str) -> bool {
    key_id.starts_with("test-")
}

/// Summarize one attestation for the export (drops the signature bytes).
fn summarize_attestation(att: &crate::attestation::Attestation) -> AttestationSummary {
    AttestationSummary {
        attestation_type: att.attestation_type,
        signer_key_id: att.key_id.clone(),
        is_test_key: is_test_key(&att.key_id),
        tsa_class: att.timestamp.class.clone(),
        claimed_time_unix: att.timestamp.claimed_time_unix,
    }
}

/// Build the canonical [`ArtifactProvenance`] from a decoded [`Artifact`] plus
/// the registry evidence and the export time. **Pure** — no I/O, no RNG. Always
/// succeeds (it is a projection of already-decoded data).
pub fn artifact_provenance(
    artifact: &Artifact,
    registry: &RegistryEvidence,
    exported_at_unix: u64,
) -> ArtifactProvenance {
    let manifest = &artifact.manifest;
    ArtifactProvenance {
        regime_id: manifest.regime_id.clone(),
        artifact_hash: manifest.artifact_hash,
        ir_schema_version: manifest.ir_schema_version.to_string(),
        codec_version: manifest.codec_version.0.clone(),
        canonicalization_version: manifest.canonicalization_version.0.clone(),
        signer_key_id: artifact.compiler_signature.key_id.clone(),
        is_test_key: is_test_key(&artifact.compiler_signature.key_id),
        attestations: artifact
            .attestations
            .iter()
            .map(summarize_attestation)
            .collect(),
        registry_state: registry.status,
        registry_event_head_hash: registry.event_head_hash,
        exported_at_unix,
    }
}

/// Verify a `.kew` artifact end-to-end against a key directory, a policy
/// context, and registry evidence — the **one consumer call** (ADR 0016).
///
/// Order (first failure short-circuits to `Rejected`):
/// 1. [`decode_artifact`](crate::decode_artifact) — `Decode` on error;
/// 2. [`verify_hash`](crate::verify_hash) — `HashMismatch`;
/// 3. [`verify_signature`](crate::verify_signature) over the envelope prefix
///    `[0, envelope_len)`, with the compiler verifying key resolved from
///    `keydir` by the signature's `key_id` — `CompilerSignatureInvalid`;
/// 4. [`verify_attestation_set`](crate::verify_attestation_set) under `policy`,
///    `keydir`, `ctx` — `Attestations`;
/// 5. registry status must be `Published` — else `NotPublished`;
/// 6. if `registry.live_event_head` is `Some` and differs from
///    `registry.event_head_hash` — `StaleEventHead`.
///
/// The provenance is **always** built (even on rejection). When decode itself
/// fails there is no artifact to project, so the provenance is a minimal record
/// carrying the registry evidence and export time; otherwise it is the full
/// projection. **No I/O, no RNG.**
///
/// `policy` is the [`VerificationPolicy`] the attestation set is checked
/// against; pass a strict policy (required types + minimum counts) to enforce
/// completeness, or a permissive one to only enforce per-attestation validity.
pub fn verify_artifact(
    kew: &[u8],
    keydir: &KeyDirectory,
    ctx: &PolicyContext,
    policy: &VerificationPolicy,
    registry: &RegistryEvidence,
    exported_at_unix: u64,
) -> VerificationOutcome {
    // 1. Decode. On failure there is no artifact to project — emit a minimal
    //    provenance carrying the registry evidence so a consumer still sees
    //    state/head, and reject with Decode.
    let (artifact, envelope_len) = match decode_artifact(kew) {
        Ok(decoded) => decoded,
        Err(e) => {
            return VerificationOutcome {
                verdict: Verdict::Rejected(RejectionReason::Decode(e.to_string())),
                provenance: decode_failed_provenance(registry, exported_at_unix),
                registry_state: registry.status,
            };
        }
    };

    // Provenance is built from the decoded artifact and is returned regardless
    // of the verdict below.
    let provenance = artifact_provenance(&artifact, registry, exported_at_unix);

    let verdict = verdict_for(&artifact, kew, envelope_len, keydir, ctx, policy, registry);

    VerificationOutcome {
        verdict,
        provenance,
        registry_state: registry.status,
    }
}

/// The cryptographic + registry checks, short-circuiting to the first failure.
/// Split out so [`verify_artifact`] can always build provenance from the
/// decoded artifact first.
fn verdict_for(
    artifact: &Artifact,
    kew: &[u8],
    envelope_len: usize,
    keydir: &KeyDirectory,
    ctx: &PolicyContext,
    policy: &VerificationPolicy,
    registry: &RegistryEvidence,
) -> Verdict {
    // 2. Content hash (re-zero recompute; never blake3(raw .kew)).
    if verify_hash(kew).is_err() {
        return Verdict::Rejected(RejectionReason::HashMismatch);
    }

    // 3. Compiler signature over the envelope prefix. The verifying key is
    //    resolved from the directory by the signature's key_id (consumer-
    //    supplied; the surface embeds no key). An unknown or malformed key is a
    //    signature failure — there is no trusted key to check against.
    let compiler_key = keydir
        .lookup(&artifact.compiler_signature.key_id)
        .and_then(|entry| VerifyingKey::from_bytes(&entry.public_key).ok());
    let Some(compiler_key) = compiler_key else {
        return Verdict::Rejected(RejectionReason::CompilerSignatureInvalid);
    };
    if verify_signature(
        &kew[..envelope_len],
        &artifact.compiler_signature,
        &compiler_key,
    )
    .is_err()
    {
        return Verdict::Rejected(RejectionReason::CompilerSignatureInvalid);
    }

    // 4. Attestation set (R1–R8, set-level R6/R7).
    if let Err(rejections) = verify_attestation_set(artifact, policy, keydir, ctx) {
        return Verdict::Rejected(RejectionReason::Attestations(rejections));
    }

    // 5. Registry state must be Published (refuse deprecated/revoked/unknown
    //    even with valid crypto — the COMPASS correctness fix).
    if registry.status != RegistryStatus::Published {
        return Verdict::Rejected(RejectionReason::NotPublished {
            status: registry.status,
        });
    }

    // 6. Staleness: a supplied live head that differs from the embedded head.
    if let Some(live) = registry.live_event_head {
        if live != registry.event_head_hash {
            return Verdict::Rejected(RejectionReason::StaleEventHead {
                embedded: registry.event_head_hash,
                live,
            });
        }
    }

    Verdict::Verified
}

/// Minimal provenance when the `.kew` did not decode: no artifact fields are
/// available, so the regime/version/signer fields are empty and the hash is
/// zero. Carries the registry evidence and export time so a consumer still sees
/// the state/head it asked about.
fn decode_failed_provenance(
    registry: &RegistryEvidence,
    exported_at_unix: u64,
) -> ArtifactProvenance {
    ArtifactProvenance {
        regime_id: String::new(),
        artifact_hash: [0u8; 32],
        ir_schema_version: String::new(),
        codec_version: String::new(),
        canonicalization_version: String::new(),
        signer_key_id: String::new(),
        is_test_key: false,
        attestations: Vec::new(),
        registry_state: registry.status,
        registry_event_head_hash: registry.event_head_hash,
        exported_at_unix,
    }
}
