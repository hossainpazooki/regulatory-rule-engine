//! The `Artifact` record, its `.kew` byte layout, and assembly/decoding.
//!
//! # Byte-range contract (load-bearing — spec § 8, plan Rev 2 correction #1)
//!
//! A `.kew` file is exactly `postcard::to_stdvec(&Artifact)`. Postcard
//! serializes struct fields in **declaration order with no framing**, so the
//! field order of [`Artifact`] *is* the byte contract:
//!
//! ```text
//! [0, envelope_len)             ENVELOPE  — hashed + signed prefix
//!   manifest                    (13 Gate-1-frozen fields; the 32-byte
//!                                artifact_hash slot immediately follows
//!                                artifact_kind)
//!   compiled_ir                 Vec<RuleIR>
//!   source_span_index           SourceSpanIndex
//!   audit_versions              AuditVersions (ADR 0014 static slots)
//!   consistency_block           Option<ConsistencyBlock> (None in Phase 1)
//! [envelope_len, EOF)           OUTSIDE the envelope — never hashed/signed
//!   compiler_signature          CompilerSignature
//!   attestations                Vec<Attestation> (empty in Phase 1)
//!   registry_state_metadata     RegistryStateMetadata (::Draft in Phase 1)
//! ```
//!
//! The **envelope serialization is the literal byte prefix** `[0,
//! envelope_len)` of the file. `envelope_len` is recovered by decoding a
//! private `EnvelopeView` (only the five envelope fields) with
//! `postcard::take_from_bytes` and taking the cursor position:
//! `envelope_len = bytes.len() - remaining.len()`.
//!
//! ## Hash derivation (zero-then-patch)
//!
//! `artifact_hash` = BLAKE3 over the envelope prefix **with the 32-byte hash
//! slot zeroed**, then patched in at
//! `offset = postcard::to_stdvec(&manifest.artifact_kind).len()` (idempotence
//! proven in `ke-core/tests/artifact_hash_offset.rs`).
//!
//! **Consequence — the trap:** `blake3(final .kew bytes) != artifact_hash` *by
//! construction* (and `blake3(final envelope prefix) != artifact_hash` too,
//! because the patched hash is part of those bytes). Every verifier must
//! **re-zero the 32-byte slot within the envelope prefix** before recomputing.
//! A naive whole-file hash check rejects every valid artifact; the golden
//! suite negatively asserts this.
//!
//! ## Signature
//!
//! `compiler_signature` = ed25519 over the **hash-patched envelope prefix**.
//! Assembly order: encode-zeroed → hash prefix → patch → sign prefix → append
//! signature/attestations/metadata → write `.kew`.
//!
//! ## Post-envelope evolution
//!
//! `compiler_signature`, `attestations`, and `registry_state_metadata` live
//! **outside** the hashed+signed envelope (spec § 9: state transitions never
//! mutate artifact bytes), so their shapes may still evolve in Phase 2/3
//! without breaking content addresses. Envelope field order, by contrast, is
//! frozen.

use crate::sign::sign_envelope;
use crate::ArtifactError;
use ed25519_dalek::SigningKey;
use ke_core::canonical::decode::{validate_manifest, validate_rule};
use ke_core::canonical::encode::{canonicalize_manifest, canonicalize_rule};
use ke_core::canonical::CanonicalError;
use ke_core::ir::{DecisionEntry, JurisdictionDate, RuleIR, SourceSpan};
use ke_core::manifest::{AttestationType, Manifest, SemVer, T2T3Mode};
use ke_core::version::SchemaVersion;
use serde::{Deserialize, Serialize};

/// The signed, content-addressed artifact record (spec § 8). **Field
/// declaration order is the `.kew` byte contract** — see the module doc.
/// The first five fields are the envelope (hashed + signed prefix); the last
/// three are appended outside it and are never hashed or signed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    // ---- ENVELOPE (hashed + signed prefix) ----
    pub manifest: Manifest,
    pub compiled_ir: Vec<RuleIR>,
    pub source_span_index: SourceSpanIndex,
    pub audit_versions: AuditVersions,
    /// `None` in Phase 1; ConsistencyBlock *behavior* is Phase 2.
    pub consistency_block: Option<ConsistencyBlock>,
    // ---- OUTSIDE the envelope (appended; never hashed/signed) ----
    pub compiler_signature: CompilerSignature,
    /// Empty in Phase 1; attestation binding is Phase 2.
    pub attestations: Vec<Attestation>,
    /// `::Draft` in Phase 1; the registry state machine is Phase 3.
    pub registry_state_metadata: RegistryStateMetadata,
}

/// Index from `rule_id` to the source spans its IR carries, built from
/// `compiled_ir` at assembly time. Entries are sorted by `rule_id`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSpanIndex {
    pub entries: Vec<SpanIndexEntry>,
}

/// One rule's source spans, in deterministic pre-order traversal order
/// (decision tree, then rule-level obligations).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpanIndexEntry {
    pub rule_id: String,
    pub spans: Vec<SourceSpan>,
}

/// ADR 0014's named audit-version slots, frozen at Phase 1 inside the
/// envelope so they are bound by the content hash before any attestation
/// exists. Both are `None` until the corresponding subsystems land.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditVersions {
    pub jurisdiction_resolver_version: Option<String>,
    pub scenario_corpus_version: Option<String>,
}

/// T2/T3 verification evidence (spec § 11). **Phase-2 BEHAVIOR** — the shape
/// is a minimal stub frozen now so Phase 2 fills behavior, not bytes; in
/// Phase 1 every artifact carries `consistency_block: None`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsistencyBlock {
    /// Tier result (T2/T3 outcome).
    pub tier_result: String,
    /// Publication-policy mode the evidence was produced under.
    pub policy_mode: T2T3Mode,
    /// Model name for T2/T3.
    pub model_name: String,
    /// Model version for T2/T3.
    pub model_version: String,
    /// Prompt or scoring profile version, where applicable.
    pub scoring_profile_version: Option<String>,
    /// Evidence references.
    pub evidence_references: Vec<String>,
    /// Reviewer overrides.
    pub reviewer_overrides: Vec<String>,
    /// Reviewer rationale.
    pub reviewer_rationale: Option<String>,
    /// Timestamp (representation pending the trusted-timestamp ADR 0010).
    pub timestamp: String,
    /// Execution environment the verification ran in.
    pub execution_environment: String,
}

/// A typed expert attestation — **Phase-2 shell** per
/// `docs/attestation-schema.md` § 3 (spec § 10 bound fields). In Phase 1 the
/// `attestations` vec is always empty; no signing, verification, or rejection
/// rules (R1–R8) are implemented here.
///
/// This type lives **outside** the hashed+signed envelope, so its shape may
/// still evolve in Phase 2 without breaking content addresses. Fields marked
/// "pending ADR" use provisional representations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attestation {
    /// BLAKE3 content hash of the exact artifact being attested (binds R2).
    pub artifact_hash: [u8; 32],
    /// Which rules / whole artifact the claim covers.
    pub scope: AttestationScope,
    /// The single named claim (one of the five frozen ke-core types).
    pub attestation_type: AttestationType,
    /// Who signed (form pending ADR 0009).
    pub signer_identity: String,
    /// Which key signed; resolves to the key-directory entry (pending ADR 0009).
    pub key_id: String,
    /// Authorization basis for the type (role enum pending ADR 0009).
    pub signer_role: String,
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
    /// Trusted-timestamp envelope (pending ADR 0010; mock TSA => R8).
    pub timestamp: TimestampEnvelope,
    /// Optional validity horizon (past => R4).
    pub expiration: Option<JurisdictionDate>,
    /// Free-text rationale / stated conditions.
    pub reviewer_comments: Option<String>,
    /// ed25519 over the canonical encoding of the bound fields (Phase 2).
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

/// Trusted-timestamp envelope stub (pending ADR 0010): the TSA token plus the
/// authority identifier so verifiers can distinguish mock from production.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimestampEnvelope {
    pub authority_id: String,
    pub token: Vec<u8>,
}

/// The compiler's ed25519 signature over the **hash-patched envelope prefix**
/// (structural validity only — never legal truth; spec § 5). Lives outside
/// the envelope: signing cannot alter the bytes it signs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompilerSignature {
    /// Identifies the signing key. Fixed-seed test signatures always carry
    /// `"test-fixed-seed-1"` so they can never be mistaken for ADR-0009
    /// production-key signatures.
    pub key_id: String,
    #[serde(with = "serde_bytes_64")]
    pub signature: [u8; 64],
}

/// Registry lifecycle marker appended outside the envelope. Phase 1 knows
/// only `Draft`; Phase 3 grows the real state machine (spec § 9) — possible
/// because this field is never hashed or signed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegistryStateMetadata {
    Draft,
}

/// Serde support for `[u8; 64]` (serde's derive covers arrays only up to 32).
/// Encodes as a 64-element tuple — byte-identical under postcard to a native
/// fixed array (no length prefix).
mod serde_bytes_64 {
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

/// The five envelope fields, by reference, for encoding the envelope prefix.
/// Postcard has no framing, so serializing this equals the prefix of
/// `postcard::to_stdvec(&Artifact)` exactly.
#[derive(Serialize)]
struct EnvelopeRef<'a> {
    manifest: &'a Manifest,
    compiled_ir: &'a [RuleIR],
    source_span_index: &'a SourceSpanIndex,
    audit_versions: &'a AuditVersions,
    consistency_block: &'a Option<ConsistencyBlock>,
}

/// The five envelope fields, owned, for recovering `envelope_len` from a
/// `.kew` byte stream via `postcard::take_from_bytes` cursor arithmetic.
#[derive(Deserialize)]
struct EnvelopeView {
    manifest: Manifest,
    compiled_ir: Vec<RuleIR>,
    source_span_index: SourceSpanIndex,
    audit_versions: AuditVersions,
    consistency_block: Option<ConsistencyBlock>,
}

fn codec(e: postcard::Error) -> ArtifactError {
    ArtifactError::Canonical(CanonicalError::Codec(e))
}

impl Artifact {
    /// Assemble a signed, content-addressed artifact from compiled IR.
    ///
    /// Order (module doc): canonicalize envelope content (reusing ke-core's
    /// canonical profile — no duplicated NFC/decimal/set logic) → encode the
    /// envelope with a **zeroed** hash slot → BLAKE3 the zeroed prefix →
    /// patch the hash in at the derived offset → ed25519-sign the
    /// hash-patched prefix → append signature, empty attestations, and
    /// `Draft` metadata.
    ///
    /// Returns the assembled record and the full `.kew` bytes (the envelope
    /// prefix of which is the hashed+signed range). `key_id` names the
    /// signing key in the embedded [`CompilerSignature`]; fixed-seed test
    /// keys must pass [`crate::sign::test_keys::TEST_KEY_ID`].
    pub fn assemble(
        manifest: Manifest,
        rules: Vec<RuleIR>,
        audit_versions: AuditVersions,
        signing_key: &SigningKey,
        key_id: &str,
    ) -> Result<(Artifact, Vec<u8>), ArtifactError> {
        // 1. Canonicalize envelope content via ke-core's public surface.
        let mut manifest = manifest;
        manifest.artifact_hash = [0u8; 32];
        canonicalize_manifest(&mut manifest)?;
        let mut compiled_ir = rules;
        for rule in &mut compiled_ir {
            canonicalize_rule(rule)?;
        }
        let source_span_index = build_span_index(&compiled_ir);
        let consistency_block: Option<ConsistencyBlock> = None;

        // 2. Encode the envelope prefix with the hash slot zeroed.
        let mut envelope = postcard::to_stdvec(&EnvelopeRef {
            manifest: &manifest,
            compiled_ir: &compiled_ir,
            source_span_index: &source_span_index,
            audit_versions: &audit_versions,
            consistency_block: &consistency_block,
        })
        .map_err(codec)?;

        // 3. BLAKE3 over the zeroed prefix, then patch the hash in at the
        //    offset derived from the encoded artifact_kind.
        let offset = crate::hash::artifact_hash_offset(manifest.artifact_kind);
        let hash: [u8; 32] = *blake3::hash(&envelope).as_bytes();
        envelope[offset..offset + 32].copy_from_slice(&hash);
        manifest.artifact_hash = hash;

        // 4. Sign the hash-patched envelope prefix.
        let compiler_signature = sign_envelope(&envelope, signing_key, key_id);

        // 5. Append the post-envelope fields (never hashed/signed).
        let attestations: Vec<Attestation> = Vec::new();
        let registry_state_metadata = RegistryStateMetadata::Draft;
        let mut kew = envelope;
        kew.extend(postcard::to_stdvec(&compiler_signature).map_err(codec)?);
        kew.extend(postcard::to_stdvec(&attestations).map_err(codec)?);
        kew.extend(postcard::to_stdvec(&registry_state_metadata).map_err(codec)?);

        let artifact = Artifact {
            manifest,
            compiled_ir,
            source_span_index,
            audit_versions,
            consistency_block,
            compiler_signature,
            attestations,
            registry_state_metadata,
        };
        debug_assert_eq!(
            kew,
            postcard::to_stdvec(&artifact).map_err(codec)?,
            "envelope-prefix concatenation must equal postcard(Artifact)"
        );
        Ok((artifact, kew))
    }
}

/// Strictly decode a `.kew` byte stream into an [`Artifact`], returning the
/// recovered `envelope_len` (the hashed+signed prefix is
/// `&bytes[..envelope_len]`).
///
/// Rejects trailing bytes after the full record and re-validates the
/// canonical profile on the envelope's manifest and rules (reusing ke-core's
/// strict decode validation). Does **not** verify the content hash or the
/// signature — use [`crate::hash::verify_hash`] and
/// [`crate::sign::verify_signature`]; both must re-derive over the envelope
/// prefix, re-zeroing the hash slot for the hash check (module doc trap:
/// `blake3(.kew)` never equals `artifact_hash`).
pub fn decode_artifact(bytes: &[u8]) -> Result<(Artifact, usize), ArtifactError> {
    let (envelope, rest) =
        postcard::take_from_bytes::<EnvelopeView>(bytes).map_err(|e| match e {
            postcard::Error::DeserializeUnexpectedEnd => ArtifactError::EnvelopeTruncated,
            other => codec_decode(other),
        })?;
    let envelope_len = bytes.len() - rest.len();

    let (compiler_signature, rest) =
        postcard::take_from_bytes::<CompilerSignature>(rest).map_err(codec_decode)?;
    let (attestations, rest) =
        postcard::take_from_bytes::<Vec<Attestation>>(rest).map_err(codec_decode)?;
    let (registry_state_metadata, rest) =
        postcard::take_from_bytes::<RegistryStateMetadata>(rest).map_err(codec_decode)?;
    if !rest.is_empty() {
        return Err(ArtifactError::TrailingBytes);
    }

    validate_manifest(&envelope.manifest)?;
    for rule in &envelope.compiled_ir {
        validate_rule(rule)?;
    }

    let artifact = Artifact {
        manifest: envelope.manifest,
        compiled_ir: envelope.compiled_ir,
        source_span_index: envelope.source_span_index,
        audit_versions: envelope.audit_versions,
        consistency_block: envelope.consistency_block,
        compiler_signature,
        attestations,
        registry_state_metadata,
    };
    Ok((artifact, envelope_len))
}

fn codec_decode(e: postcard::Error) -> ArtifactError {
    ArtifactError::CanonicalDecode(ke_core::canonical::CanonicalDecodeError::Codec(e))
}

/// Build the [`SourceSpanIndex`] for a set of compiled rules: each rule's
/// spans are collected by deterministic pre-order traversal of its decision
/// tree (node span, true branch, false branch; leaf span, then leaf
/// obligations) followed by rule-level obligations. Entries are sorted by
/// `rule_id`.
pub fn build_span_index(rules: &[RuleIR]) -> SourceSpanIndex {
    let mut entries: Vec<SpanIndexEntry> = rules
        .iter()
        .map(|rule| SpanIndexEntry {
            rule_id: rule.rule_id.clone(),
            spans: collect_rule_spans(rule),
        })
        .collect();
    entries.sort_by(|a, b| a.rule_id.cmp(&b.rule_id));
    SourceSpanIndex { entries }
}

fn collect_rule_spans(rule: &RuleIR) -> Vec<SourceSpan> {
    let mut spans = Vec::new();
    collect_entry_spans(&rule.decision_tree, &mut spans);
    for obligation in &rule.obligations {
        if let Some(span) = &obligation.source_span {
            spans.push(span.clone());
        }
    }
    spans
}

fn collect_entry_spans(entry: &DecisionEntry, out: &mut Vec<SourceSpan>) {
    match entry {
        DecisionEntry::Node(node) => {
            if let Some(span) = &node.source_span {
                out.push(span.clone());
            }
            collect_entry_spans(&node.true_branch, out);
            collect_entry_spans(&node.false_branch, out);
        }
        DecisionEntry::Leaf(leaf) => {
            if let Some(span) = &leaf.source_span {
                out.push(span.clone());
            }
            if let Some(obligations) = &leaf.obligations {
                for obligation in obligations {
                    if let Some(span) = &obligation.source_span {
                        out.push(span.clone());
                    }
                }
            }
        }
    }
}
