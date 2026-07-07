//! Kind <-> payload agreement (ADR-0021 §Decision-4): `decode_artifact` must
//! reject a `.kew` whose `manifest.artifact_kind` disagrees with the envelope
//! payload variant. Without this, a crafted artifact carrying an `IntentSpec`
//! payload under a rule-shaped kind (or rules under an `IntentSpec` kind) would
//! decode cleanly and be dispatched under the wrong kind's policy downstream —
//! the silent failure Stage A exists to close.

use ke_artifact::sign::test_keys;
use ke_artifact::{decode_artifact, Artifact, ArtifactError, ArtifactPayload, AuditVersions};
use ke_core::examples;
use ke_core::ir::JurisdictionDate;
use ke_core::ir::{IdempotencyDef, IntentSpecIR};
use ke_core::manifest::ArtifactKind;

/// A real, signed `RegimePack` artifact (manifest kind = RegimePack, payload =
/// Rules), assembled via the canonical path.
fn regime_pack() -> Artifact {
    let manifest = examples::synthetic_manifest(
        ArtifactKind::RegimePack,
        "mica_2023",
        JurisdictionDate::new(2024, 6, 30),
        b"kind-payload agreement test",
    );
    let rules: Vec<_> = examples::rules().into_iter().map(|(_, r)| r).collect();
    let (artifact, _kew) = Artifact::assemble(
        manifest,
        rules,
        AuditVersions::default(),
        &test_keys::signing_key(),
        test_keys::TEST_KEY_ID,
    )
    .expect("assemble");
    artifact
}

fn minimal_intentspec() -> IntentSpecIR {
    IntentSpecIR {
        action_class: "payment".to_string(),
        criteria: Vec::new(),
        idempotency: IdempotencyDef {
            key_fields: Vec::new(),
            scope: "payer".to_string(),
        },
        source_spans: Vec::new(),
    }
}

/// A `RegimePack` manifest carrying an `IntentSpec` payload must be rejected at
/// decode. (Decode does not verify the hash/signature, so the stale hash left by
/// the swap does not mask the kind/payload check — the agreement check must fire
/// on its own.)
#[test]
fn regimepack_kind_with_intentspec_payload_is_rejected() {
    let mismatched = Artifact {
        payload: ArtifactPayload::IntentSpec(minimal_intentspec()),
        ..regime_pack()
    };
    let bytes = postcard::to_stdvec(&mismatched).expect("encode");
    let err = decode_artifact(&bytes).expect_err("kind<->payload mismatch must be rejected");
    assert!(
        matches!(
            err,
            ArtifactError::KindPayloadMismatch(ArtifactKind::RegimePack)
        ),
        "expected KindPayloadMismatch(RegimePack), got {err:?}"
    );
}

/// The valid pairing (RegimePack kind + Rules payload) still decodes cleanly —
/// the agreement check must not over-reject the happy path.
#[test]
fn regimepack_kind_with_rules_payload_still_decodes() {
    let artifact = regime_pack();
    let bytes = postcard::to_stdvec(&artifact).expect("encode");
    decode_artifact(&bytes).expect("valid RegimePack+Rules must decode");
}
