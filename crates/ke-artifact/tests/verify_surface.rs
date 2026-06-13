//! The consumer-agnostic verification surface (Gate 4 Phase 4a, ADR 0016),
//! exercised over the committed Phase-2 golden `.kew`.
//!
//! Covers, with a keydir/ctx built in-test (mirroring the lifecycle/attestation
//! tests: the expert key authorized for the attested types, valid window over
//! the fixed claimed-time, environment "local", supported policy `["ap-1"]`,
//! plus a compiler-key entry so the compiler signature resolves):
//!
//! - (a) `verified` -> `Verdict::Verified` with `RegistryStatus::Published` and
//!   a matching event-head;
//! - (b) `rejected_bad_sig` (flip a compiler-signature byte) -> `CompilerSignatureInvalid`;
//! - (c) `rejected_missing_attestation` (strict policy requiring an absent type)
//!   -> `Attestations(..)`;
//! - (d) `rejected_when_revoked` (`RegistryStatus::Revoked` with valid crypto)
//!   -> `Verdict::Rejected(NotPublished)` (the COMPASS correctness fix);
//! - (e) `stale_event_head` (`live_event_head = Some(different)`) -> `StaleEventHead`.
//!
//! Plus: `is_test_key == true` for the test-keyed golden, and canonical-JSON
//! serialization is byte-stable.
//!
//! All keys are fixed-seed test keys (no OsRng/getrandom); the clock is a
//! constant. The verify path is pure and RNG-free.

use ke_artifact::sign::test_keys;
use ke_artifact::{
    decode_artifact, verify_artifact, KeyDirectory, KeyDirectoryEntry, KeyStatus, PolicyContext,
    RegistryEvidence, RegistryStatus, RejectionReason, Verdict,
};
use ke_core::manifest::{AttestationCount, AttestationType, T2T3Mode, VerificationPolicy};
use std::fs;
use std::path::Path;

/// The committed Phase-2 golden the suite verifies.
const GOLDEN_ID: &str = "rule_reserve_assets";

/// The fixed verification/export clock the golden generator used.
const GOLDEN_NOW: u64 = 1_750_000_000;

/// The three attestation types the golden carries.
const GOLDEN_TYPES: [AttestationType; 3] = [
    AttestationType::SourceFidelity,
    AttestationType::ScenarioCoverage,
    AttestationType::PublicationApproval,
];

/// A non-zero stand-in for the registry event-head as-of-export.
const EMBEDDED_HEAD: [u8; 32] = [0x11; 32];

fn golden_kew() -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("artifacts")
        .join(GOLDEN_ID)
        .join("artifact.kew");
    fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// A keydir with BOTH the compiler key (so the compiler signature resolves and
/// verifies) and the expert key (so the attestation set verifies). The expert
/// key is authorized for the three golden types under both signing roles with a
/// window over the fixed claimed-time; the compiler key carries the compiler
/// verifying key.
fn keydir() -> KeyDirectory {
    use ke_artifact::SignerRole;
    KeyDirectory {
        entries: vec![
            // Compiler key: resolves `compiler_signature.key_id` -> verifying key.
            KeyDirectoryEntry {
                key_id: test_keys::TEST_KEY_ID.to_string(),
                public_key: test_keys::verifying_key().to_bytes(),
                signer_roles: vec![SignerRole::Registry],
                authorized_attestation_types: vec![],
                valid_from_unix: 0,
                valid_to_unix: u64::MAX,
                status: KeyStatus::Active,
                revoked_at_unix: None,
                revocation_reason: None,
                revocation_event_hash: None,
            },
            // Expert key: signs the three golden attestations.
            KeyDirectoryEntry {
                key_id: test_keys::TEST_EXPERT_KEY_ID.to_string(),
                public_key: test_keys::expert_verifying_key().to_bytes(),
                signer_roles: vec![SignerRole::DomainExpert, SignerRole::PublicationApprover],
                authorized_attestation_types: GOLDEN_TYPES.to_vec(),
                valid_from_unix: 1_000_000_000,
                valid_to_unix: 2_000_000_000,
                status: KeyStatus::Active,
                revoked_at_unix: None,
                revocation_reason: None,
                revocation_event_hash: None,
            },
        ],
    }
}

/// The local context the golden verifies under: environment "local" (mock-TSA
/// accepted), the fixed clock, supported policy `["ap-1"]`, and the recomputed
/// legal-source hash = the manifest's source corpus hash the generator bound.
fn ctx(kew: &[u8]) -> PolicyContext {
    let (artifact, _) = decode_artifact(kew).expect("decode golden");
    PolicyContext {
        environment: "local".to_string(),
        now_unix: GOLDEN_NOW,
        supported_policy_versions: vec!["ap-1".to_string()],
        current_legal_source_hash: Some(artifact.manifest.source_corpus_hash),
    }
}

/// A strict policy requiring exactly the three golden types (the publish gate).
fn strict_policy() -> VerificationPolicy {
    VerificationPolicy {
        t2_t3_mode: T2T3Mode::Strict,
        required_attestation_types: GOLDEN_TYPES.to_vec(),
        minimum_attestation_count_per_type: GOLDEN_TYPES
            .iter()
            .map(|ty| AttestationCount {
                attestation_type: *ty,
                count: 1,
            })
            .collect(),
    }
}

/// Published registry evidence with no live head (no staleness check).
fn published_evidence() -> RegistryEvidence {
    RegistryEvidence {
        status: RegistryStatus::Published,
        event_head_hash: EMBEDDED_HEAD,
        live_event_head: None,
    }
}

// ---- (a) verified happy path ---------------------------------------------

#[test]
fn verified_published_golden() {
    let kew = golden_kew();
    let outcome = verify_artifact(
        &kew,
        &keydir(),
        &ctx(&kew),
        &strict_policy(),
        &published_evidence(),
        GOLDEN_NOW,
    );
    assert_eq!(
        outcome.verdict,
        Verdict::Verified,
        "valid crypto + Published + strict policy must verify; got {:?}",
        outcome.verdict
    );
    assert_eq!(outcome.registry_state, RegistryStatus::Published);
    assert_eq!(outcome.provenance.registry_state, RegistryStatus::Published);
    assert_eq!(outcome.provenance.registry_event_head_hash, EMBEDDED_HEAD);
    // The golden is test-keyed: the flag must loudly say so.
    assert!(
        outcome.provenance.is_test_key,
        "golden compiler key is a test key"
    );
    assert_eq!(outcome.provenance.signer_key_id, test_keys::TEST_KEY_ID);
    assert_eq!(outcome.provenance.attestations.len(), 3);
    assert!(
        outcome
            .provenance
            .attestations
            .iter()
            .all(|a| a.is_test_key),
        "every golden attestation is expert-test-keyed"
    );
}

// ---- (b) bad compiler signature ------------------------------------------

#[test]
fn rejected_bad_sig() {
    let mut kew = golden_kew();
    // Flip a byte inside the post-envelope compiler signature. Find the
    // signature's location by decoding the envelope length, then flip a byte a
    // few past it (the signature key_id + bytes follow the envelope prefix).
    let (artifact, envelope_len) = decode_artifact(&kew).expect("decode golden");
    // The compiler signature is the first post-envelope field; its 64 signature
    // bytes are the load-bearing part. Flip a byte well inside the post-envelope
    // region to corrupt the signature.
    let flip_at = envelope_len + artifact.compiler_signature.key_id.len() + 5;
    assert!(flip_at < kew.len(), "flip index inside the file");
    kew[flip_at] ^= 0xFF;

    let outcome = verify_artifact(
        &kew,
        &keydir(),
        &ctx(&golden_kew()),
        &strict_policy(),
        &published_evidence(),
        GOLDEN_NOW,
    );
    // Decoding may still succeed (the signature bytes are opaque); the signature
    // check must reject. If the flip lands such that decode fails, that is also
    // a rejection but of a different reason — assert the signature path by
    // checking it is not Verified and prefer CompilerSignatureInvalid.
    match outcome.verdict {
        Verdict::Rejected(RejectionReason::CompilerSignatureInvalid) => {}
        other => panic!("expected CompilerSignatureInvalid, got {other:?}"),
    }
}

// ---- (c) strict policy with an absent required type ----------------------

#[test]
fn rejected_missing_attestation() {
    let kew = golden_kew();
    // Require a type the golden does NOT carry (Interpretation is absent).
    let policy = VerificationPolicy {
        t2_t3_mode: T2T3Mode::Strict,
        required_attestation_types: vec![AttestationType::Interpretation],
        minimum_attestation_count_per_type: vec![AttestationCount {
            attestation_type: AttestationType::Interpretation,
            count: 1,
        }],
    };
    let outcome = verify_artifact(
        &kew,
        &keydir(),
        &ctx(&kew),
        &policy,
        &published_evidence(),
        GOLDEN_NOW,
    );
    match outcome.verdict {
        Verdict::Rejected(RejectionReason::Attestations(rejections)) => {
            assert!(
                rejections.iter().any(|r| matches!(
                    r,
                    ke_artifact::AttestationRejection::RequiredTypeMissing {
                        attestation_type: AttestationType::Interpretation,
                        ..
                    }
                )),
                "expected RequiredTypeMissing(Interpretation), got {rejections:?}"
            );
        }
        other => panic!("expected Attestations(..), got {other:?}"),
    }
}

// ---- (d) revoked registry state rejects even with valid crypto -----------

#[test]
fn rejected_when_revoked() {
    let kew = golden_kew();
    let evidence = RegistryEvidence {
        status: RegistryStatus::Revoked,
        event_head_hash: EMBEDDED_HEAD,
        live_event_head: None,
    };
    let outcome = verify_artifact(
        &kew,
        &keydir(),
        &ctx(&kew),
        &strict_policy(),
        &evidence,
        GOLDEN_NOW,
    );
    // The COMPASS correctness fix: valid crypto, but revoked -> rejected.
    assert_eq!(
        outcome.verdict,
        Verdict::Rejected(RejectionReason::NotPublished {
            status: RegistryStatus::Revoked
        }),
        "a revoked pack with valid crypto must be rejected NotPublished(Revoked)"
    );
    assert_eq!(outcome.registry_state, RegistryStatus::Revoked);
    // Provenance is still built and surfaces the revoked state.
    assert_eq!(outcome.provenance.registry_state, RegistryStatus::Revoked);
}

// ---- (e) stale embedded event-head ---------------------------------------

#[test]
fn stale_event_head() {
    let kew = golden_kew();
    let live = [0x22; 32];
    let evidence = RegistryEvidence {
        status: RegistryStatus::Published,
        event_head_hash: EMBEDDED_HEAD,
        live_event_head: Some(live),
    };
    let outcome = verify_artifact(
        &kew,
        &keydir(),
        &ctx(&kew),
        &strict_policy(),
        &evidence,
        GOLDEN_NOW,
    );
    assert_eq!(
        outcome.verdict,
        Verdict::Rejected(RejectionReason::StaleEventHead {
            embedded: EMBEDDED_HEAD,
            live,
        }),
        "a Published pack whose embedded head != live head must be StaleEventHead"
    );
    // A matching live head verifies (no staleness).
    let fresh = RegistryEvidence {
        status: RegistryStatus::Published,
        event_head_hash: EMBEDDED_HEAD,
        live_event_head: Some(EMBEDDED_HEAD),
    };
    let outcome = verify_artifact(
        &kew,
        &keydir(),
        &ctx(&kew),
        &strict_policy(),
        &fresh,
        GOLDEN_NOW,
    );
    assert_eq!(outcome.verdict, Verdict::Verified);
}

// ---- canonical JSON is byte-stable ---------------------------------------

#[test]
fn provenance_canonical_json_is_stable() {
    let kew = golden_kew();
    let (artifact, _) = decode_artifact(&kew).expect("decode golden");
    let evidence = published_evidence();
    let a = ke_artifact::artifact_provenance(&artifact, &evidence, GOLDEN_NOW);
    let b = ke_artifact::artifact_provenance(&artifact, &evidence, GOLDEN_NOW);
    let ja = a.to_canonical_json().expect("json a");
    let jb = b.to_canonical_json().expect("json b");
    assert_eq!(ja, jb, "same inputs -> byte-identical canonical JSON");
    // Round-trips back to an equal value.
    let parsed: ke_artifact::ArtifactProvenance =
        serde_json::from_str(&ja).expect("provenance round-trips");
    assert_eq!(parsed, a, "canonical JSON round-trips to an equal value");
    // Field order is stable: regime_id is the first key.
    assert!(
        ja.starts_with("{\"regime_id\":"),
        "canonical JSON leads with regime_id (stable field order); got: {}",
        &ja[..ja.len().min(40)]
    );
}
