//! Artifact-level shapes whose **layout is frozen in Gate 1** so canonical
//! encoding tests can be written, even though the authority, signing, content
//! addressing, and registry behavior land in Gate 4 (`ke-artifact`). See spec
//! § 8.1 and brief § 3 / § 7.
//!
//! Nothing here signs, hashes-as-authority, or publishes. The `artifact_hash`
//! field is a *shape* slot; the zero-then-patch derivation is exercised only at
//! the byte-offset level (brief § 12 risk) and wired for real in Gate 4.

use crate::ir::time::EffectiveWindow;
use crate::ir::JurisdictionDate;
use crate::version::{CanonicalizationVersion, CodecVersion, SchemaVersion};
use serde::{Deserialize, Serialize};

/// A plain semantic version (compiler version, etc.).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemVer {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

/// The artifact kinds (spec § 8.2, amended by ADR-0021). Variants are
/// **append-only**: a mid-list insert changes the encoded discriminant value of
/// later variants, which changes the content hash of existing artifacts and
/// mis-decodes committed goldens (ADR-0002). `IntentSpec` is therefore appended
/// last.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArtifactKind {
    RegimePack,
    EquivalenceMatrix,
    TestCorpus,
    PolicyBundle,
    /// Authorization criteria for an action class (ADR-0021). Its payload is
    /// [`crate::ir::IntentSpecIR`], not `Vec<RuleIR>`.
    IntentSpec,
}

/// The artifact manifest (spec § 8.1). `artifact_kind` precedes
/// `artifact_hash`, and `artifact_hash` is fixed-width, so the hash's byte
/// offset is a pure function of the encoded `artifact_kind` prefix — this is
/// what makes the Gate 4 zero-then-patch derivation determinable (brief § 3).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub artifact_kind: ArtifactKind,
    /// BLAKE3 of the canonical bytes; self-referential, computed last (Gate 4).
    /// Encoded as 32 zero bytes during the first pass.
    pub artifact_hash: [u8; 32],
    pub regime_id: String,
    pub effective_from: JurisdictionDate,
    pub effective_to: Option<JurisdictionDate>,
    pub compiler_version: SemVer,
    pub compiler_build_hash: [u8; 32],
    pub ir_schema_version: SchemaVersion,
    pub codec_version: CodecVersion,
    pub canonicalization_version: CanonicalizationVersion,
    pub corpus_root_hash: [u8; 32],
    pub source_corpus_hash: [u8; 32],
    pub attestation_policy_version: String,
}

// ---------------------------------------------------------------------------
// PolicyBundle (informational; brief § 7)
//
// Sketched now so the IR and canonical encoding don't have to be revisited at
// Gate 4. The open decisions (spec § 21: key authority, T2/T3 default,
// revocation default) do not block Gate 1; the fields merely need to round-trip.
// ---------------------------------------------------------------------------

/// T2/T3 publication-policy mode (spec § 11 policy modes).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum T2T3Mode {
    Strict,
    ReviewOverride,
    Advisory,
    Disabled,
}

/// Typed attestation kinds (spec § 10).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AttestationType {
    SourceFidelity,
    Interpretation,
    ScenarioCoverage,
    EquivalenceClaim,
    PublicationApproval,
}

/// Revocation behavior for already-running and new workflows (spec § 15).
/// Variant order is the canonical discriminant order; declared in § 15 order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RevocationPolicy {
    /// § 15 "hard stop": fail any workflow attempting to execute.
    HardStop,
    /// § 15 "finish pinned": allow already-started workflows to finish; block new starts.
    FinishPinned,
    /// § 15 "audit-only": allow execution; emit a high-severity audit event.
    AuditOnly,
}

/// One entry of the `minimum_attestation_count_per_type` map. Encoded as a
/// canonically-sorted sequence of entries (brief § 4.3 map ordering), keyed by
/// [`AttestationType`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttestationCount {
    pub attestation_type: AttestationType,
    pub count: u8,
}

/// Verification policy block.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationPolicy {
    pub t2_t3_mode: T2T3Mode,
    pub required_attestation_types: Vec<AttestationType>,
    pub minimum_attestation_count_per_type: Vec<AttestationCount>,
}

/// A publication/runtime-enforcement policy for a named environment
/// (artifact kind `PolicyBundle`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyBundle {
    pub environment: String,
    pub verification_policy: VerificationPolicy,
    pub revocation_policy: RevocationPolicy,
    pub effective_window: EffectiveWindow,
}
