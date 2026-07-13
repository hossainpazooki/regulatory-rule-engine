//! The rejection matrix: one named test per `AttestationRejection` variant
//! (`docs/attestation-schema.md` § 7, R1–R8, plus the signature/timestamp
//! integrity failures), each asserting the **exact** variant; plus
//! payload-prefix signing determinism and the happy-path attestation set.
//!
//! All keys are fixed-seed test keys (`test-` prefixed key ids; no
//! OsRng/getrandom anywhere); the clock is a constant — verification is pure.

use ke_artifact::sign::{test_keys, Signer};
use ke_artifact::tsa::{MockTsa, TimestampAuthorityClass};
use ke_artifact::{
    sign_attestation, verify_attestation, verify_attestation_set, Artifact, ArtifactPayload,
    Attestation, AttestationRejection, AttestationScope, AuditVersions, KeyDirectory,
    KeyDirectoryEntry, KeyStatus, PolicyContext, SignerRole,
};
use ke_core::examples;
use ke_core::ir::{IdempotencyDef, IntentSpecIR, JurisdictionDate};
use ke_core::manifest::{
    ArtifactKind, AttestationCount, AttestationType, T2T3Mode, VerificationPolicy,
};

/// The fixed verification clock (unix seconds, mid-2025).
const NOW: u64 = 1_750_000_000;
/// When the mock TSA stamped (one day before NOW).
const STAMP_TIME: u64 = NOW - 86_400;
/// The legal source hash every test attestation binds.
const LEGAL_SOURCE_HASH: [u8; 32] = [0xAB; 32];

const ALL_TYPES: [AttestationType; 5] = [
    AttestationType::SourceFidelity,
    AttestationType::Interpretation,
    AttestationType::ScenarioCoverage,
    AttestationType::EquivalenceClaim,
    AttestationType::PublicationApproval,
];

fn assembled() -> Artifact {
    let manifest = examples::synthetic_manifest(
        ArtifactKind::RegimePack,
        "mica_2023",
        JurisdictionDate::new(2024, 6, 30),
        b"phase-2 attestation tests",
    );
    let rules: Vec<_> = examples::rules().into_iter().map(|(_, r)| r).collect();
    let (artifact, _) = Artifact::assemble(
        manifest,
        rules,
        AuditVersions::default(),
        &test_keys::signing_key(),
        test_keys::TEST_KEY_ID,
    )
    .expect("assemble");
    artifact
}

fn entry(key_id: &str, status: KeyStatus) -> KeyDirectoryEntry {
    KeyDirectoryEntry {
        key_id: key_id.to_string(),
        public_key: test_keys::expert_verifying_key().to_bytes(),
        signer_roles: vec![SignerRole::DomainExpert, SignerRole::PublicationApprover],
        authorized_attestation_types: ALL_TYPES.to_vec(),
        valid_from_unix: 1_000_000_000,
        valid_to_unix: 2_000_000_000,
        status,
        revoked_at_unix: None,
        revocation_reason: None,
        revocation_event_hash: None,
    }
}

/// Expert key (active, all roles/types) plus the R1 negative entries:
/// status-expired, window-lapsed, revoked, and interpretation-only keys —
/// all carrying the expert public key so signatures verify and the R1 rule
/// itself is what rejects.
fn directory() -> KeyDirectory {
    let mut revoked = entry("test-revoked-key-1", KeyStatus::Revoked);
    revoked.revoked_at_unix = Some(NOW - 10);
    revoked.revocation_reason = Some("compromised in test".to_string());
    revoked.revocation_event_hash = Some([0xEE; 32]);

    let mut lapsed = entry("test-window-lapsed-key-1", KeyStatus::Active);
    lapsed.valid_to_unix = NOW - 1;

    let mut interp_only = entry("test-interp-only-key-1", KeyStatus::Active);
    interp_only.signer_roles = vec![SignerRole::DomainExpert];
    interp_only.authorized_attestation_types = vec![AttestationType::Interpretation];

    KeyDirectory {
        entries: vec![
            entry(test_keys::TEST_EXPERT_KEY_ID, KeyStatus::Active),
            entry("test-status-expired-key-1", KeyStatus::Expired),
            lapsed,
            revoked,
            interp_only,
        ],
    }
}

fn local_ctx() -> PolicyContext {
    PolicyContext {
        environment: "local".to_string(),
        now_unix: NOW,
        supported_policy_versions: vec!["ap-1".to_string()],
        current_legal_source_hash: Some(LEGAL_SOURCE_HASH),
    }
}

/// An unsigned attestation payload bound to the artifact, mock-stamped over
/// its artifact hash. Tests mutate fields, then sign.
fn payload(artifact: &Artifact, attestation_type: AttestationType) -> Attestation {
    let manifest = &artifact.manifest;
    Attestation {
        artifact_hash: manifest.artifact_hash,
        scope: AttestationScope::WholeArtifact,
        attestation_type,
        signer_identity: "Test Expert".to_string(),
        key_id: test_keys::TEST_EXPERT_KEY_ID.to_string(),
        signer_role: SignerRole::DomainExpert,
        regime_id: manifest.regime_id.clone(),
        effective_from: manifest.effective_from,
        effective_to: manifest.effective_to,
        legal_source_hash: LEGAL_SOURCE_HASH,
        ir_schema_version: manifest.ir_schema_version,
        compiler_version: manifest.compiler_version,
        attestation_policy_version: "ap-1".to_string(),
        test_corpus_hash: None,
        timestamp: MockTsa::stamp(&manifest.artifact_hash, STAMP_TIME),
        expiration: None,
        reviewer_comments: None,
        signature: [0u8; 64],
    }
}

fn signed(payload: Attestation) -> Attestation {
    sign_attestation(payload, &test_keys::expert_signing_key()).expect("sign")
}

fn verify(artifact: &Artifact, attestation: &Attestation) -> Result<(), AttestationRejection> {
    verify_attestation(
        attestation,
        &artifact.manifest.artifact_hash,
        &artifact.manifest,
        &directory(),
        &local_ctx(),
    )
}

// ---- R1 family: key unknown / expired / revoked / unauthorized ----

#[test]
fn r1a_unknown_key_rejected() {
    let artifact = assembled();
    let mut att = payload(&artifact, AttestationType::SourceFidelity);
    att.key_id = "test-nobody-key-1".to_string();
    let err = verify(&artifact, &signed(att)).expect_err("unknown key");
    assert!(
        matches!(err, AttestationRejection::KeyUnknown { ref key_id } if key_id == "test-nobody-key-1"),
        "expected KeyUnknown, got {err:?}"
    );
}

#[test]
fn r1b_expired_key_rejected() {
    let artifact = assembled();
    // Status-expired key.
    let mut att = payload(&artifact, AttestationType::SourceFidelity);
    att.key_id = "test-status-expired-key-1".to_string();
    let err = verify(&artifact, &signed(att)).expect_err("status-expired key");
    assert!(
        matches!(err, AttestationRejection::KeyExpired { .. }),
        "expected KeyExpired (status), got {err:?}"
    );
    // Active key whose validity window has lapsed.
    let mut att = payload(&artifact, AttestationType::SourceFidelity);
    att.key_id = "test-window-lapsed-key-1".to_string();
    let err = verify(&artifact, &signed(att)).expect_err("window-lapsed key");
    assert!(
        matches!(err, AttestationRejection::KeyExpired { .. }),
        "expected KeyExpired (window), got {err:?}"
    );
}

#[test]
fn r1c_revoked_key_rejected() {
    let artifact = assembled();
    let mut att = payload(&artifact, AttestationType::SourceFidelity);
    att.key_id = "test-revoked-key-1".to_string();
    let err = verify(&artifact, &signed(att)).expect_err("revoked key");
    assert!(
        matches!(err, AttestationRejection::KeyRevoked { ref key_id } if key_id == "test-revoked-key-1"),
        "expected KeyRevoked, got {err:?}"
    );
}

#[test]
fn r1d_unauthorized_key_rejected() {
    let artifact = assembled();
    // Key authorized only for Interpretation signs SourceFidelity.
    let mut att = payload(&artifact, AttestationType::SourceFidelity);
    att.key_id = "test-interp-only-key-1".to_string();
    let err = verify(&artifact, &signed(att)).expect_err("unauthorized type");
    assert!(
        matches!(
            err,
            AttestationRejection::KeyUnauthorizedForType {
                attestation_type: AttestationType::SourceFidelity,
                ..
            }
        ),
        "expected KeyUnauthorizedForType, got {err:?}"
    );
    // Same key under a role it does not hold.
    let mut att = payload(&artifact, AttestationType::Interpretation);
    att.key_id = "test-interp-only-key-1".to_string();
    att.signer_role = SignerRole::PublicationApprover;
    let err = verify(&artifact, &signed(att)).expect_err("unauthorized role");
    assert!(
        matches!(err, AttestationRejection::KeyUnauthorizedForType { .. }),
        "expected KeyUnauthorizedForType (role), got {err:?}"
    );
}

// ---- R2: not bound to the artifact being executed ----

#[test]
fn r2_unbound_artifact_hash_rejected() {
    let artifact = assembled();
    let real = artifact.manifest.artifact_hash;
    let fake = [0x42; 32];
    let mut att = payload(&artifact, AttestationType::SourceFidelity);
    att.artifact_hash = fake;
    // Re-stamp over the bound hash so the timestamp re-derivation passes and
    // R2 is the rule that rejects.
    att.timestamp = MockTsa::stamp(&fake, STAMP_TIME);
    let err = verify(&artifact, &signed(att)).expect_err("unbound hash");
    assert!(
        matches!(
            err,
            AttestationRejection::NotBoundToArtifact { expected, got }
                if expected == real && got == fake
        ),
        "expected NotBoundToArtifact, got {err:?}"
    );
}

// ---- R3: unsupported attestation policy version ----

#[test]
fn r3_unsupported_policy_version_rejected() {
    let artifact = assembled();
    let mut att = payload(&artifact, AttestationType::SourceFidelity);
    att.attestation_policy_version = "ap-999".to_string();
    let err = verify(&artifact, &signed(att)).expect_err("unsupported policy version");
    assert!(
        matches!(
            err,
            AttestationRejection::PolicyVersionUnsupported { ref version } if version == "ap-999"
        ),
        "expected PolicyVersionUnsupported, got {err:?}"
    );
}

// ---- R4: attestation expired ----

#[test]
fn r4_expired_attestation_rejected() {
    let artifact = assembled();
    let mut att = payload(&artifact, AttestationType::SourceFidelity);
    att.expiration = Some(JurisdictionDate::new(2020, 1, 1));
    let err = verify(&artifact, &signed(att)).expect_err("expired attestation");
    assert!(
        matches!(err, AttestationRejection::Expired),
        "expected Expired, got {err:?}"
    );
    // A future horizon is fine.
    let mut att = payload(&artifact, AttestationType::SourceFidelity);
    att.expiration = Some(JurisdictionDate::new(2030, 1, 1));
    verify(&artifact, &signed(att)).expect("future expiration accepted");
}

// ---- R5: legal source hash changed after attestation ----

#[test]
fn r5_legal_source_hash_change_rejected() {
    let artifact = assembled();
    let att = signed(payload(&artifact, AttestationType::SourceFidelity));
    let mut ctx = local_ctx();
    ctx.current_legal_source_hash = Some([0xCD; 32]);
    let err = verify_attestation(
        &att,
        &artifact.manifest.artifact_hash,
        &artifact.manifest,
        &directory(),
        &ctx,
    )
    .expect_err("changed source hash");
    assert!(
        matches!(err, AttestationRejection::LegalSourceHashChanged),
        "expected LegalSourceHashChanged, got {err:?}"
    );
}

// ---- R6: required attestation type missing ----

#[test]
fn r6_required_type_missing_rejected() {
    let artifact = assembled();
    let (artifact, _) = artifact
        .clone()
        .with_attestations(vec![signed(payload(
            &artifact,
            AttestationType::SourceFidelity,
        ))])
        .expect("append");
    let policy = VerificationPolicy {
        t2_t3_mode: T2T3Mode::Disabled,
        required_attestation_types: vec![
            AttestationType::SourceFidelity,
            AttestationType::ScenarioCoverage,
        ],
        minimum_attestation_count_per_type: vec![AttestationCount {
            attestation_type: AttestationType::ScenarioCoverage,
            count: 1,
        }],
    };
    let rejections = verify_attestation_set(&artifact, &policy, &directory(), &local_ctx())
        .expect_err("missing required type");
    assert!(
        rejections.iter().any(|r| matches!(
            r,
            AttestationRejection::RequiredTypeMissing {
                attestation_type: AttestationType::ScenarioCoverage,
                required: 1,
                found: 0,
            }
        )),
        "expected RequiredTypeMissing for ScenarioCoverage, got {rejections:?}"
    );
    assert_eq!(rejections.len(), 1, "SourceFidelity itself is satisfied");
}

// ---- R7: publication approval needs its co-attestations ----

#[test]
fn r7_publication_approval_without_coattestations_rejected() {
    let artifact = assembled();
    let mut approval = payload(&artifact, AttestationType::PublicationApproval);
    approval.signer_role = SignerRole::PublicationApprover;
    let (artifact, _) = artifact
        .clone()
        .with_attestations(vec![signed(approval)])
        .expect("append");
    let policy = VerificationPolicy {
        t2_t3_mode: T2T3Mode::Disabled,
        required_attestation_types: vec![AttestationType::PublicationApproval],
        minimum_attestation_count_per_type: vec![],
    };
    let rejections = verify_attestation_set(&artifact, &policy, &directory(), &local_ctx())
        .expect_err("approval without co-attestations");
    for missing in [
        AttestationType::ScenarioCoverage,
        AttestationType::SourceFidelity,
    ] {
        assert!(
            rejections.iter().any(
                |r| matches!(r, AttestationRejection::CoAttestationAbsent { missing: m } if *m == missing)
            ),
            "expected CoAttestationAbsent for {missing:?}, got {rejections:?}"
        );
    }
}

#[test]
fn r7_publication_approval_with_coattestations_accepted() {
    let artifact = assembled();
    let mut approval = payload(&artifact, AttestationType::PublicationApproval);
    approval.signer_role = SignerRole::PublicationApprover;
    let attestations = vec![
        signed(payload(&artifact, AttestationType::SourceFidelity)),
        signed(payload(&artifact, AttestationType::ScenarioCoverage)),
        signed(approval),
    ];
    let (artifact, _) = artifact
        .clone()
        .with_attestations(attestations)
        .expect("append");
    let policy = VerificationPolicy {
        t2_t3_mode: T2T3Mode::Disabled,
        required_attestation_types: vec![AttestationType::PublicationApproval],
        minimum_attestation_count_per_type: vec![],
    };
    verify_attestation_set(&artifact, &policy, &directory(), &local_ctx())
        .expect("approval honored with both co-attestations present");
}

/// A signed `IntentSpec` artifact assembled through the polymorphic entry
/// point, mirroring the gen-golden path (ADR-0021).
fn assembled_intentspec() -> Artifact {
    let intent = IntentSpecIR {
        action_class: "payment".to_string(),
        criteria: Vec::new(),
        idempotency: IdempotencyDef {
            key_fields: Vec::new(),
            scope: "payer".to_string(),
        },
        source_spans: Vec::new(),
    };
    let intent_bytes = postcard::to_stdvec(&intent).expect("intentspec payload encodes");
    let manifest = examples::synthetic_manifest(
        ArtifactKind::IntentSpec,
        "treasury_payments",
        JurisdictionDate::new(2025, 1, 1),
        &intent_bytes,
    );
    let (artifact, _) = Artifact::assemble_payload(
        manifest,
        ArtifactPayload::IntentSpec(intent),
        AuditVersions::default(),
        &test_keys::signing_key(),
        test_keys::TEST_KEY_ID,
    )
    .expect("assemble intentspec");
    artifact
}

#[test]
fn r7_intentspec_approval_with_source_fidelity_accepted() {
    // ADR-0022: an IntentSpec's R7 co-attestation set is SourceFidelity only —
    // ADR-0021 § 5 pins its attestation set to SourceFidelity +
    // PublicationApproval, so demanding ScenarioCoverage would make every
    // IntentSpec unpublishable and unverifiable by construction.
    let artifact = assembled_intentspec();
    let mut approval = payload(&artifact, AttestationType::PublicationApproval);
    approval.signer_role = SignerRole::PublicationApprover;
    let attestations = vec![
        signed(payload(&artifact, AttestationType::SourceFidelity)),
        signed(approval),
    ];
    let (artifact, _) = artifact
        .clone()
        .with_attestations(attestations)
        .expect("append");
    let policy = VerificationPolicy {
        t2_t3_mode: T2T3Mode::Disabled,
        required_attestation_types: vec![
            AttestationType::SourceFidelity,
            AttestationType::PublicationApproval,
        ],
        minimum_attestation_count_per_type: vec![],
    };
    verify_attestation_set(&artifact, &policy, &directory(), &local_ctx())
        .expect("the ADR-0021 two-type set must be honorable for an IntentSpec");
}

#[test]
fn r7_intentspec_approval_without_source_fidelity_rejected() {
    // The IntentSpec co-attestation rule still bites: approval alone is
    // rejected for the missing SourceFidelity — and ScenarioCoverage is NOT
    // demanded of an IntentSpec.
    let artifact = assembled_intentspec();
    let mut approval = payload(&artifact, AttestationType::PublicationApproval);
    approval.signer_role = SignerRole::PublicationApprover;
    let (artifact, _) = artifact
        .clone()
        .with_attestations(vec![signed(approval)])
        .expect("append");
    let policy = VerificationPolicy {
        t2_t3_mode: T2T3Mode::Disabled,
        required_attestation_types: vec![AttestationType::PublicationApproval],
        minimum_attestation_count_per_type: vec![],
    };
    let rejections = verify_attestation_set(&artifact, &policy, &directory(), &local_ctx())
        .expect_err("approval without a source-fidelity co-attestation");
    assert!(
        rejections.iter().any(|r| matches!(
            r,
            AttestationRejection::CoAttestationAbsent {
                missing: AttestationType::SourceFidelity
            }
        )),
        "expected CoAttestationAbsent(SourceFidelity), got {rejections:?}"
    );
    assert!(
        !rejections.iter().any(|r| matches!(
            r,
            AttestationRejection::CoAttestationAbsent {
                missing: AttestationType::ScenarioCoverage
            }
        )),
        "ScenarioCoverage must not be demanded of an IntentSpec, got {rejections:?}"
    );
}

// ---- R8: mock TSA under non-local policy ----

#[test]
fn r8_mock_tsa_non_local_rejected_local_accepted() {
    let artifact = assembled();
    let att = signed(payload(&artifact, AttestationType::SourceFidelity));
    // Mock-stamped + "local" => accepted.
    verify(&artifact, &att).expect("mock TSA accepted under local policy");
    // Mock-stamped + any other environment => rejected.
    let mut ctx = local_ctx();
    ctx.environment = "production-eu".to_string();
    let err = verify_attestation(
        &att,
        &artifact.manifest.artifact_hash,
        &artifact.manifest,
        &directory(),
        &ctx,
    )
    .expect_err("mock TSA under non-local policy");
    assert!(
        matches!(err, AttestationRejection::MockTsaNonLocal),
        "expected MockTsaNonLocal, got {err:?}"
    );
}

// ---- Signature integrity ----

#[test]
fn tampered_payload_is_signature_invalid() {
    let artifact = assembled();
    let mut att = signed(payload(&artifact, AttestationType::SourceFidelity));
    // Tamper one payload field after signing.
    att.reviewer_comments = Some("inserted after signing".to_string());
    let err = verify(&artifact, &att).expect_err("tampered payload");
    assert!(
        matches!(err, AttestationRejection::AttestationSignatureInvalid),
        "expected AttestationSignatureInvalid, got {err:?}"
    );
}

// ---- Timestamp class integrity (ADR 0010) ----

#[test]
fn timestamp_class_mismatch_rejected() {
    let artifact = assembled();
    let hash = artifact.manifest.artifact_hash;

    // (a) Token signed by the wrong key but labelled Mock: the token does
    // not re-derive to Mock.
    let mut forged = MockTsa::stamp(&hash, STAMP_TIME);
    let mut message = Vec::with_capacity(41);
    message.push(2u8); // Mock class discriminant
    message.extend_from_slice(&hash);
    message.extend_from_slice(&STAMP_TIME.to_le_bytes());
    forged.token = test_keys::expert_signing_key()
        .sign(&message)
        .to_bytes()
        .to_vec();
    let mut att = payload(&artifact, AttestationType::SourceFidelity);
    att.timestamp = forged;
    let err = verify(&artifact, &signed(att)).expect_err("wrong-key token");
    assert!(
        matches!(err, AttestationRejection::TimestampClassMismatch),
        "expected TimestampClassMismatch (wrong key), got {err:?}"
    );

    // (b) Genuine mock token relabelled as a production class: re-derivation
    // says Mock, the claimed class says RFC 3161.
    let mut relabelled = MockTsa::stamp(&hash, STAMP_TIME);
    relabelled.class = TimestampAuthorityClass::Rfc3161External {
        tsa_identity: "acme-tsa".to_string(),
    };
    let mut att = payload(&artifact, AttestationType::SourceFidelity);
    att.timestamp = relabelled;
    let err = verify(&artifact, &signed(att)).expect_err("relabelled mock token");
    assert!(
        matches!(err, AttestationRejection::TimestampClassMismatch),
        "expected TimestampClassMismatch (relabelled), got {err:?}"
    );
}

#[test]
fn tsa_unsupported_rejected() {
    let artifact = assembled();
    let mut att = payload(&artifact, AttestationType::SourceFidelity);
    att.timestamp.class = TimestampAuthorityClass::Rfc3161External {
        tsa_identity: "acme-tsa".to_string(),
    };
    att.timestamp.token = vec![0xDE, 0xAD, 0xBE, 0xEF]; // not a mock token
    let err = verify(&artifact, &signed(att)).expect_err("rfc3161 unverifiable");
    assert!(
        matches!(
            err,
            AttestationRejection::TsaUnsupported {
                class: TimestampAuthorityClass::Rfc3161External { .. }
            }
        ),
        "expected TsaUnsupported, got {err:?}"
    );
}

// ---- Payload-prefix signing determinism ----

#[test]
fn signing_is_deterministic_over_payload_prefix() {
    let artifact = assembled();
    let a = signed(payload(&artifact, AttestationType::SourceFidelity));
    let b = signed(payload(&artifact, AttestationType::SourceFidelity));
    assert_eq!(
        a.signature, b.signature,
        "same payload + same key => identical signature bytes (RFC 8032)"
    );
    assert_eq!(a, b, "the whole attestation is deterministic");
    // A pre-existing signature value must not leak into the signed bytes.
    let mut prefilled = payload(&artifact, AttestationType::SourceFidelity);
    prefilled.signature = [0xFF; 64];
    let c = signed(prefilled);
    assert_eq!(a.signature, c.signature, "signature slot is not signed");
    verify(&artifact, &a).expect("deterministic signature verifies");
}

// ---- Happy path: full three-type set under a strict policy ----

#[test]
fn happy_path_three_type_set_passes_strict_policy() {
    let artifact = assembled();
    let mut approval = payload(&artifact, AttestationType::PublicationApproval);
    approval.signer_role = SignerRole::PublicationApprover;
    let attestations = vec![
        signed(payload(&artifact, AttestationType::SourceFidelity)),
        signed(payload(&artifact, AttestationType::ScenarioCoverage)),
        signed(approval),
    ];
    let (artifact, _) = artifact
        .clone()
        .with_attestations(attestations)
        .expect("append");
    let policy = VerificationPolicy {
        t2_t3_mode: T2T3Mode::Strict,
        required_attestation_types: vec![
            AttestationType::SourceFidelity,
            AttestationType::ScenarioCoverage,
            AttestationType::PublicationApproval,
        ],
        minimum_attestation_count_per_type: vec![
            AttestationCount {
                attestation_type: AttestationType::SourceFidelity,
                count: 1,
            },
            AttestationCount {
                attestation_type: AttestationType::ScenarioCoverage,
                count: 1,
            },
            AttestationCount {
                attestation_type: AttestationType::PublicationApproval,
                count: 1,
            },
        ],
    };
    verify_attestation_set(&artifact, &policy, &directory(), &local_ctx())
        .expect("three-type set passes the strict policy");
}
