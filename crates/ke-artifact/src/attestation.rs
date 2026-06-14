//! Typed expert attestations: payload-prefix signing, verification, and the
//! spec § 10 rejection rules (`docs/attestation-schema.md` § 7, R1–R8).
//!
//! # Signed bytes (one canonicalization scheme, reused)
//!
//! `signature` is the **last** field of [`Attestation`], so the signed bytes
//! are the postcard serialization of every field before it — the literal
//! byte prefix of `postcard(Attestation)`. The prefix length is recovered by
//! decoding a private [`AttestationPayloadView`] (all fields minus the
//! signature) with `postcard::take_from_bytes` — the exact `EnvelopeView`
//! technique from [`crate::artifact`]. ed25519 over that prefix; no second
//! canonicalization scheme exists.
//!
//! # Authority boundaries (spec § 5, § 10, § 13)
//!
//! Only a domain expert's key signs attestations; the compiler never does
//! (structural validity only), and no AI/LLM code participates in any path
//! here. Verification is **pure and deterministic** — clock, environment,
//! and current source hash all arrive explicitly via [`PolicyContext`];
//! nothing reads the system clock or environment.

use crate::artifact::serde_bytes_64;
use crate::keydir::{KeyDirectory, KeyStatus, SignerRole};
use crate::tsa::{derive_class, TimestampAuthorityClass, TimestampToken, TsaError};
use crate::ArtifactError;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use ke_core::canonical::CanonicalError;
use ke_core::ir::JurisdictionDate;
use ke_core::manifest::{AttestationType, Manifest, SemVer, VerificationPolicy};
use ke_core::version::SchemaVersion;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A typed expert attestation per `docs/attestation-schema.md` § 3 (spec
/// § 10 bound fields). **Field declaration order is the byte contract** —
/// the signed payload is the serialization prefix of all fields before
/// `signature`. Shape frozen in Phase 2 (the first attested goldens).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attestation {
    /// BLAKE3 content hash of the exact artifact being attested (binds R2).
    pub artifact_hash: [u8; 32],
    /// Which rules / whole artifact the claim covers.
    pub scope: AttestationScope,
    /// The single named claim (one of the five frozen ke-core types).
    pub attestation_type: AttestationType,
    /// Who signed (subject form per ADR 0009).
    pub signer_identity: String,
    /// Which key signed; resolves to the key-directory entry (ADR 0009, R1).
    pub key_id: String,
    /// Authorization basis for the type (ADR 0009).
    pub signer_role: SignerRole,
    /// The regime the claim is scoped to (must match the manifest).
    pub regime_id: String,
    /// Effective period the claim covers — `[from, to)` half-open (ADR 0007).
    pub effective_from: JurisdictionDate,
    pub effective_to: Option<JurisdictionDate>,
    /// The legal source the encoding was reviewed against (hash-only, R5).
    pub legal_source_hash: [u8; 32],
    /// The IR schema the artifact was compiled under.
    pub ir_schema_version: SchemaVersion,
    /// The compiler that produced the artifact (audit reconstruction, § 18).
    pub compiler_version: SemVer,
    /// The attestation policy version the attestation was made under (R3).
    pub attestation_policy_version: String,
    /// proposed §10 amendment (attestation-schema §6A) — slot frozen,
    /// binding semantics NOT yet authoritative; `None` until ratified.
    pub test_corpus_hash: Option<[u8; 32]>,
    /// Trusted-timestamp token (ADR 0010; mock TSA => R8). The class inside
    /// is re-derived from the token at verification.
    pub timestamp: TimestampToken,
    /// Optional validity horizon (past => R4).
    pub expiration: Option<JurisdictionDate>,
    /// Free-text rationale / stated conditions.
    pub reviewer_comments: Option<String>,
    /// ed25519 over the payload prefix (all fields above). **Must stay the
    /// last field** — the prefix-signing scheme depends on it.
    #[serde(with = "serde_bytes_64")]
    pub signature: [u8; 64],
}

/// Attestation scope: an explicit rule-id set or the whole artifact
/// (`docs/attestation-schema.md` § 3 — exactly one of the two).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttestationScope {
    RuleIds(Vec<String>),
    WholeArtifact,
}

/// All [`Attestation`] fields **minus the signature**, used only to recover
/// the payload-prefix length from serialized bytes via
/// `postcard::take_from_bytes` cursor arithmetic (the `EnvelopeView`
/// technique). Never constructed for its field values.
#[derive(Deserialize)]
#[allow(dead_code)]
struct AttestationPayloadView {
    artifact_hash: [u8; 32],
    scope: AttestationScope,
    attestation_type: AttestationType,
    signer_identity: String,
    key_id: String,
    signer_role: SignerRole,
    regime_id: String,
    effective_from: JurisdictionDate,
    effective_to: Option<JurisdictionDate>,
    legal_source_hash: [u8; 32],
    ir_schema_version: SchemaVersion,
    compiler_version: SemVer,
    attestation_policy_version: String,
    test_corpus_hash: Option<[u8; 32]>,
    timestamp: TimestampToken,
    expiration: Option<JurisdictionDate>,
    reviewer_comments: Option<String>,
}

/// Explicit verification context — clock, environment, supported policy
/// versions, and the recomputed legal-source hash all arrive here so
/// verification stays pure (no system clock, no environment reads).
///
/// `Deserialize` (Phase 4b): the PyO3 / WASM bindings take this as JSON, so the
/// three contract languages share one verifier context (`scripts/contract-inputs/policy.json`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyContext {
    /// Named environment the verification runs under (R8: mock TSA is
    /// accepted only when this is `"local"`).
    pub environment: String,
    /// Verification time, unix seconds (key windows, R4 expiry).
    pub now_unix: u64,
    /// Attestation policy versions the platform supports (R3).
    pub supported_policy_versions: Vec<String>,
    /// The legal source hash as recomputed *now*, if the source is
    /// available; `None` skips R5 (nothing to compare against).
    pub current_legal_source_hash: Option<[u8; 32]>,
}

impl PolicyContext {
    /// True iff the environment is `"local"` (the only environment where
    /// mock-TSA-stamped attestations are accepted — rejection R8).
    pub fn is_local(&self) -> bool {
        self.environment == "local"
    }
}

/// One variant per rejection rule of `docs/attestation-schema.md` § 7
/// (spec § 10), plus the signature/timestamp integrity failures. The
/// platform consumer mirrors this enumeration.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum AttestationRejection {
    /// R1: the signing key is unknown to the key directory.
    #[error("R1: signing key `{key_id}` unknown to the key directory")]
    KeyUnknown { key_id: String },
    /// R1: the signing key is expired (status or validity window).
    #[error("R1: signing key `{key_id}` expired")]
    KeyExpired { key_id: String },
    /// R1: the signing key is revoked.
    #[error("R1: signing key `{key_id}` revoked")]
    KeyRevoked { key_id: String },
    /// R1: the key is not authorized for the attestation type under the
    /// stated signer role.
    #[error("R1: signing key `{key_id}` not authorized for {attestation_type:?}")]
    KeyUnauthorizedForType {
        key_id: String,
        attestation_type: AttestationType,
    },
    /// R2: the attestation is not bound to the artifact being executed
    /// (hash mismatch, or a manifest-binding field — `regime_id`,
    /// `compiler_version` — disagrees with the artifact's manifest; schema
    /// § 7 folds those binding failures into R2).
    #[error(
        "R2: attestation not bound to artifact (expected {}, got {})",
        crate::hex32(.expected),
        crate::hex32(.got)
    )]
    NotBoundToArtifact { expected: [u8; 32], got: [u8; 32] },
    /// R3: the attestation policy version (or the IR schema version it was
    /// made under — schema § 3 folds schema drift into R3) is unsupported.
    #[error("R3: attestation policy version `{version}` unsupported")]
    PolicyVersionUnsupported { version: String },
    /// R4: the attestation's `expiration` is in the past.
    #[error("R4: attestation expired")]
    Expired,
    /// R5: the legal source hash changed after attestation.
    #[error("R5: legal source hash changed after attestation")]
    LegalSourceHashChanged,
    /// R6: a required attestation type is missing or under its minimum
    /// count for the environment's verification policy.
    #[error("R6: required type {attestation_type:?} missing ({found} found, {required} required)")]
    RequiredTypeMissing {
        attestation_type: AttestationType,
        required: usize,
        found: usize,
    },
    /// R7: `publication_approval` without its required co-attestation
    /// (`scenario_coverage` + `source_fidelity` over the same hash).
    #[error("R7: publication approval without a valid {missing:?} co-attestation")]
    CoAttestationAbsent { missing: AttestationType },
    /// R8: mock-TSA-stamped attestation under a non-local policy.
    #[error("R8: mock TSA timestamp rejected under non-local policy")]
    MockTsaNonLocal,
    /// The ed25519 signature does not verify over the payload prefix.
    #[error("attestation signature invalid over the payload prefix")]
    AttestationSignatureInvalid,
    /// The timestamp class re-derived from the token does not match the
    /// class claimed inside the signed payload (ADR 0010).
    #[error("timestamp class re-derived from token mismatches the claimed class")]
    TimestampClassMismatch,
    /// The claimed timestamp authority class cannot be verified yet
    /// (RFC 3161 vendor onboarding pending, ADR 0010).
    #[error("timestamp authority class {class:?} unsupported (ADR 0010 onboarding pending)")]
    TsaUnsupported { class: TimestampAuthorityClass },
}

fn codec(e: postcard::Error) -> ArtifactError {
    ArtifactError::Canonical(CanonicalError::Codec(e))
}

/// The signed payload prefix: the postcard bytes of every field before
/// `signature`. Recovered via [`AttestationPayloadView`] cursor arithmetic,
/// so it is the literal prefix of `postcard(Attestation)` regardless of the
/// current `signature` value (the last 64 bytes are simply truncated).
fn payload_prefix(attestation: &Attestation) -> Result<Vec<u8>, ArtifactError> {
    let mut bytes = postcard::to_stdvec(attestation).map_err(codec)?;
    let (_, rest) = postcard::take_from_bytes::<AttestationPayloadView>(&bytes).map_err(codec)?;
    let prefix_len = bytes.len() - rest.len();
    bytes.truncate(prefix_len);
    Ok(bytes)
}

/// Sign an attestation payload with a domain expert's key: ed25519 over the
/// payload prefix (module doc). Any pre-existing `signature` value is
/// ignored and overwritten. Deterministic per RFC 8032 — same payload +
/// same key => identical signature bytes.
pub fn sign_attestation(
    payload: Attestation,
    signing_key: &SigningKey,
) -> Result<Attestation, ArtifactError> {
    let mut attestation = payload;
    let prefix = payload_prefix(&attestation)?;
    attestation.signature = signing_key.sign(&prefix).to_bytes();
    Ok(attestation)
}

/// Verify one attestation against the artifact it claims to bind
/// (`expected_artifact_hash` + its `manifest`), the key directory, and the
/// policy context. Returns the **first** rejection in the contract order:
/// signature, timestamp class re-derivation, R8, R1, R2, R3, R4, R5
/// (key lookup is a prerequisite of the signature check, so R1 "unknown"
/// surfaces first by necessity). Set-level rules R6/R7 live in
/// [`verify_attestation_set`].
pub fn verify_attestation(
    attestation: &Attestation,
    expected_artifact_hash: &[u8; 32],
    manifest: &Manifest,
    key_directory: &KeyDirectory,
    context: &PolicyContext,
) -> Result<(), AttestationRejection> {
    // Key lookup — prerequisite for signature verification (R1 "unknown").
    let entry =
        key_directory
            .lookup(&attestation.key_id)
            .ok_or(AttestationRejection::KeyUnknown {
                key_id: attestation.key_id.clone(),
            })?;

    // 1. Signature over the payload prefix.
    let verifying_key = VerifyingKey::from_bytes(&entry.public_key)
        .map_err(|_| AttestationRejection::AttestationSignatureInvalid)?;
    let prefix = payload_prefix(attestation)
        .map_err(|_| AttestationRejection::AttestationSignatureInvalid)?;
    verifying_key
        .verify(&prefix, &Signature::from_bytes(&attestation.signature))
        .map_err(|_| AttestationRejection::AttestationSignatureInvalid)?;

    // 2. Timestamp class re-derivation (ADR 0010). The mock TSA stamps the
    //    attested artifact_hash.
    let derived =
        derive_class(&attestation.timestamp, &attestation.artifact_hash).map_err(|error| {
            match error {
                TsaError::Unsupported(class) => AttestationRejection::TsaUnsupported { class },
                TsaError::ClassMismatch => AttestationRejection::TimestampClassMismatch,
            }
        })?;
    if derived != attestation.timestamp.class {
        return Err(AttestationRejection::TimestampClassMismatch);
    }

    // 3. R8 — mock TSA only under local policy.
    if derived == TimestampAuthorityClass::Mock && !context.is_local() {
        return Err(AttestationRejection::MockTsaNonLocal);
    }

    // 4. R1 — key status, validity window, role + type authorization.
    match entry.status {
        KeyStatus::Expired => {
            return Err(AttestationRejection::KeyExpired {
                key_id: attestation.key_id.clone(),
            })
        }
        KeyStatus::Revoked => {
            return Err(AttestationRejection::KeyRevoked {
                key_id: attestation.key_id.clone(),
            })
        }
        KeyStatus::Active => {}
    }
    if context.now_unix < entry.valid_from_unix || context.now_unix >= entry.valid_to_unix {
        return Err(AttestationRejection::KeyExpired {
            key_id: attestation.key_id.clone(),
        });
    }
    if !entry.signer_roles.contains(&attestation.signer_role)
        || !entry
            .authorized_attestation_types
            .contains(&attestation.attestation_type)
    {
        return Err(AttestationRejection::KeyUnauthorizedForType {
            key_id: attestation.key_id.clone(),
            attestation_type: attestation.attestation_type,
        });
    }

    // 5. R2 — bound to the artifact being executed. Manifest-binding fields
    //    (schema § 7 footnote) fold into R2.
    if attestation.artifact_hash != *expected_artifact_hash {
        return Err(AttestationRejection::NotBoundToArtifact {
            expected: *expected_artifact_hash,
            got: attestation.artifact_hash,
        });
    }
    if attestation.regime_id != manifest.regime_id
        || attestation.compiler_version != manifest.compiler_version
    {
        return Err(AttestationRejection::NotBoundToArtifact {
            expected: *expected_artifact_hash,
            got: attestation.artifact_hash,
        });
    }

    // 6. R3 — supported policy version; IR schema drift folds into R3
    //    (schema § 3).
    if !context
        .supported_policy_versions
        .contains(&attestation.attestation_policy_version)
    {
        return Err(AttestationRejection::PolicyVersionUnsupported {
            version: attestation.attestation_policy_version.clone(),
        });
    }
    if attestation.ir_schema_version != manifest.ir_schema_version {
        return Err(AttestationRejection::PolicyVersionUnsupported {
            version: format!("ir-schema-{}", attestation.ir_schema_version),
        });
    }

    // 7. R4 — expiration. `[.., expiration)` half-open per ADR 0007: the
    //    attestation is already invalid at 00:00 UTC of the expiration date.
    if let Some(expiration) = &attestation.expiration {
        if i64::try_from(context.now_unix).unwrap_or(i64::MAX) >= unix_start_of(expiration) {
            return Err(AttestationRejection::Expired);
        }
    }

    // 8. R5 — legal source hash unchanged since attestation (skipped when
    //    the source is unavailable for recomputation).
    if let Some(current) = &context.current_legal_source_hash {
        if *current != attestation.legal_source_hash {
            return Err(AttestationRejection::LegalSourceHashChanged);
        }
    }

    Ok(())
}

/// Verify an artifact's full attestation set under a verification policy.
///
/// Each attestation is checked with [`verify_attestation`]; then the
/// set-level rules run over the **valid** subset, in contract order:
///
/// - **R6** — every type in `required_attestation_types` must have at least
///   its `minimum_attestation_count_per_type` count (default 1) of valid
///   attestations.
/// - **R7** — a valid `publication_approval` is honored only when a valid,
///   non-expired `scenario_coverage` **and** `source_fidelity` over the
///   **same** `artifact_hash` exist.
///
/// Returns **all** rejections (per-attestation + set-level), not just the
/// first.
pub fn verify_attestation_set(
    artifact: &crate::artifact::Artifact,
    policy: &VerificationPolicy,
    key_directory: &KeyDirectory,
    context: &PolicyContext,
) -> Result<(), Vec<AttestationRejection>> {
    let expected_hash = artifact.manifest.artifact_hash;
    let mut rejections = Vec::new();
    let mut valid: Vec<&Attestation> = Vec::new();
    for attestation in &artifact.attestations {
        match verify_attestation(
            attestation,
            &expected_hash,
            &artifact.manifest,
            key_directory,
            context,
        ) {
            Ok(()) => valid.push(attestation),
            Err(rejection) => rejections.push(rejection),
        }
    }

    // R6 — required types at their minimum counts, valid attestations only.
    for required_type in &policy.required_attestation_types {
        let required = policy
            .minimum_attestation_count_per_type
            .iter()
            .find(|count| count.attestation_type == *required_type)
            .map(|count| usize::from(count.count))
            .unwrap_or(1)
            .max(1);
        let found = valid
            .iter()
            .filter(|attestation| attestation.attestation_type == *required_type)
            .count();
        if found < required {
            rejections.push(AttestationRejection::RequiredTypeMissing {
                attestation_type: *required_type,
                required,
                found,
            });
        }
    }

    // R7 — publication approval requires scenario-coverage + source-fidelity
    // co-attestations over the same artifact hash (schema § 6B).
    if let Some(approval) = valid
        .iter()
        .find(|attestation| attestation.attestation_type == AttestationType::PublicationApproval)
    {
        for co_type in [
            AttestationType::ScenarioCoverage,
            AttestationType::SourceFidelity,
        ] {
            let present = valid.iter().any(|attestation| {
                attestation.attestation_type == co_type
                    && attestation.artifact_hash == approval.artifact_hash
            });
            if !present {
                rejections.push(AttestationRejection::CoAttestationAbsent { missing: co_type });
            }
        }
    }

    if rejections.is_empty() {
        Ok(())
    } else {
        Err(rejections)
    }
}

/// Unix seconds at 00:00 UTC of a jurisdiction date (Howard Hinnant's
/// `days_from_civil`). Used only for the R4 expiry comparison; full
/// jurisdiction-time resolution lives in `ke-runtime` (ADR 0001).
fn unix_start_of(date: &JurisdictionDate) -> i64 {
    let year = i64::from(date.year) - i64::from(date.month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month = i64::from(date.month);
    let day_of_year =
        (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + i64::from(date.day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    (era * 146_097 + day_of_era - 719_468) * 86_400
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_start_of_known_dates() {
        assert_eq!(unix_start_of(&JurisdictionDate::new(1970, 1, 1)), 0);
        assert_eq!(
            unix_start_of(&JurisdictionDate::new(2000, 3, 1)),
            951_868_800
        );
        assert_eq!(
            unix_start_of(&JurisdictionDate::new(2026, 6, 12)),
            1_781_222_400
        );
    }
}
