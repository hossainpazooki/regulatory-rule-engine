//! Generate the signed golden artifact vectors under `fixtures/artifacts/`
//! (Gate 4 Phase 1 envelope + Phase 2 attestation set).
//!
//! Mirrors the `ke-core` `gen-fixtures` write pattern: deterministic,
//! idempotent — re-running produces byte-identical output. Requires the
//! `test-keys` feature (`required-features` on this bin):
//!
//! ```text
//! cargo run -p ke-artifact --features test-keys --bin gen-golden-artifacts
//! ```
//!
//! For each **RegimePack rule** fixture input the generator:
//! 1. decodes `fixtures/artifacts/<id>/canonical.bin` via ke-core's strict
//!    canonical decode (the committed bytes are the authoritative input);
//! 2. rebuilds the synthetic manifest exactly as `gen-fixtures` does
//!    (`ke_core::examples::synthetic_manifest`; `assemble` re-zeroes and
//!    re-derives `artifact_hash`, so the synthetic hash slot is discarded);
//! 3. assembles a signed artifact via [`Artifact::assemble`] with the
//!    fixed-seed test key (`key_id = "test-fixed-seed-1"`, **never** a
//!    production key; no `OsRng`/`getrandom` anywhere) and
//!    `AuditVersions { None, None }`;
//! 4. appends a deterministic **three-type attestation set** (Phase 2):
//!    `SourceFidelity` + `ScenarioCoverage` + `PublicationApproval`, each
//!    signed by the fixed-seed **expert** test key
//!    (`test-expert-fixed-seed-1`), scope `WholeArtifact`, manifest-copied
//!    binding fields, mock-TSA stamped at the fixed
//!    [`GOLDEN_CLAIMED_TIME_UNIX`]. The append is post-envelope
//!    ([`Artifact::with_attestations`]) — `artifact_hash`, `envelope_len`,
//!    and the compiler signature are asserted **unchanged** (spec § 9);
//! 5. writes `fixtures/artifacts/<id>/artifact.kew` (authoritative) plus
//!    `signature.json` and `attestations.json` review views (regenerated,
//!    never authoritative).
//!
//! It then writes its own ledger `fixtures/artifacts/GOLDEN.md`. It does NOT
//! touch ke-core's `MANIFEST.md` — two generators, two ledgers, no clobber.
//!
//! ## Scope note: `policy_production_eu` is skipped
//!
//! The Phase-1 [`Artifact`] envelope is **RuleIR-oriented** (`compiled_ir:
//! Vec<RuleIR>`); a `PolicyBundle` has no representation in it. The
//! `policy_production_eu` fixture is therefore skipped with a logged note —
//! signed artifacts are built only for the two RegimePack rule fixtures.
//! A PolicyBundle-bearing artifact shape is a later-phase decision.
//!
//! ## Byte-range contract (repeated because it is load-bearing)
//!
//! `artifact_hash` = BLAKE3 over the envelope prefix `[0, envelope_len)` of
//! the `.kew` bytes **with the 32-byte hash slot zeroed**, then patched in.
//! Consequently `blake3(raw .kew bytes) != artifact_hash` by construction;
//! verifiers must re-zero the slot within the envelope prefix before
//! recomputing. The compiler signature is ed25519 over the **hash-patched**
//! envelope prefix.

use ke_artifact::sign::test_keys;
use ke_artifact::tsa::{MockTsa, TimestampAuthorityClass, MOCK_TSA_AUTHORITY_ID};
use ke_artifact::{
    decode_artifact, sign_attestation, Artifact, Attestation, AttestationScope, AuditVersions,
    SignerRole,
};
use ke_core::canonical::decode_rule;
use ke_core::examples;
use ke_core::ir::JurisdictionDate;
use ke_core::manifest::{ArtifactKind, AttestationType, Manifest};
use ke_core::version::{CANONICALIZATION_VERSION, CODEC_VERSION, IR_SCHEMA_VERSION};
use std::fs;
use std::path::{Path, PathBuf};

/// The RegimePack rule fixture inputs (committed `canonical.bin` files).
const RULE_FIXTURE_IDS: [&str; 2] = ["rule_reserve_assets", "rule_significant_thresholds"];

/// The PolicyBundle fixture skipped in Phase 1 (see module doc).
const SKIPPED_POLICY_ID: &str = "policy_production_eu";

/// The fixed `claimed_time_unix` every golden attestation's mock-TSA token
/// carries: **1_750_000_000** (2025-06-15T15:06:40Z). A constant — never a
/// clock read — so re-running the generator is byte-identical.
const GOLDEN_CLAIMED_TIME_UNIX: u64 = 1_750_000_000;

/// The attestation policy version every golden attestation is made under.
const GOLDEN_ATTESTATION_POLICY_VERSION: &str = "ap-1";

/// The signer identity string on every golden attestation (a test persona,
/// loudly not a real ADR-0009 subject).
const GOLDEN_SIGNER_IDENTITY: &str = "Test Expert";

/// The deterministic golden attestation set: the three types a strict
/// publication policy requires, in this fixed order. `PublicationApproval`
/// signs under the `PublicationApprover` role (rejection rule R7 honors it
/// only alongside the two co-attestations also present here).
const GOLDEN_ATTESTATION_SET: [(AttestationType, SignerRole); 3] = [
    (AttestationType::SourceFidelity, SignerRole::DomainExpert),
    (AttestationType::ScenarioCoverage, SignerRole::DomainExpert),
    (
        AttestationType::PublicationApproval,
        SignerRole::PublicationApprover,
    ),
];

fn repo_root() -> PathBuf {
    // crates/ke-artifact -> repo root
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate is two levels below repo root")
        .to_path_buf()
}

/// Lowercase hex (review views and the ledger).
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

struct Row {
    id: String,
    artifact_hash: String,
    envelope_len: usize,
}

/// One golden attestation payload, bound to the assembled artifact: scope
/// `WholeArtifact`; regime/effective/version fields copied from the manifest;
/// `legal_source_hash = manifest.source_corpus_hash` (the hash the encoding
/// was reviewed against); `test_corpus_hash = None` (slot frozen, semantics
/// not yet authoritative); mock-TSA stamped over the **artifact hash** at
/// [`GOLDEN_CLAIMED_TIME_UNIX`]; no expiration. Signed with the fixed-seed
/// expert key — deterministic per RFC 8032.
fn golden_attestation(
    manifest: &Manifest,
    attestation_type: AttestationType,
    signer_role: SignerRole,
) -> Attestation {
    let payload = Attestation {
        artifact_hash: manifest.artifact_hash,
        scope: AttestationScope::WholeArtifact,
        attestation_type,
        signer_identity: GOLDEN_SIGNER_IDENTITY.to_string(),
        key_id: test_keys::TEST_EXPERT_KEY_ID.to_string(),
        signer_role,
        regime_id: manifest.regime_id.clone(),
        effective_from: manifest.effective_from,
        effective_to: manifest.effective_to,
        legal_source_hash: manifest.source_corpus_hash,
        ir_schema_version: manifest.ir_schema_version,
        compiler_version: manifest.compiler_version,
        attestation_policy_version: GOLDEN_ATTESTATION_POLICY_VERSION.to_string(),
        test_corpus_hash: None,
        timestamp: MockTsa::stamp(&manifest.artifact_hash, GOLDEN_CLAIMED_TIME_UNIX),
        expiration: None,
        reviewer_comments: None,
        signature: [0u8; 64],
    };
    sign_attestation(payload, &test_keys::expert_signing_key()).expect("golden attestation signs")
}

/// Render the `attestations.json` review view (regenerated, never
/// authoritative — `artifact.kew` is the source of truth).
fn attestations_json(attestations: &[Attestation]) -> String {
    let mut out = String::from("[\n");
    for (i, att) in attestations.iter().enumerate() {
        let class = match att.timestamp.class {
            TimestampAuthorityClass::Mock => "Mock",
            TimestampAuthorityClass::Rfc3161External { .. } => "Rfc3161External",
            TimestampAuthorityClass::Rfc3161Internal { .. } => "Rfc3161Internal",
        };
        out.push_str(&format!(
            "  {{\n    \"attestation_type\": \"{:?}\",\n    \"key_id\": \"{}\",\n    \
             \"signer_role\": \"{:?}\",\n    \"tsa_authority_id\": \"{}\",\n    \
             \"tsa_class\": \"{}\",\n    \"claimed_time_unix\": {},\n    \
             \"signature\": \"{}\"\n  }}{}\n",
            att.attestation_type,
            att.key_id,
            att.signer_role,
            MOCK_TSA_AUTHORITY_ID,
            class,
            att.timestamp.claimed_time_unix,
            hex(&att.signature),
            if i + 1 < attestations.len() { "," } else { "" },
        ));
    }
    out.push_str("]\n");
    out
}

fn main() -> std::io::Result<()> {
    let artifacts_dir = repo_root().join("fixtures").join("artifacts");
    let mut rows: Vec<Row> = Vec::new();

    for id in RULE_FIXTURE_IDS {
        let dir = artifacts_dir.join(id);

        // 1. The committed canonical bytes are the authoritative input.
        let canonical = fs::read(dir.join("canonical.bin"))?;
        let rule = decode_rule(&canonical).expect("committed canonical.bin decodes strictly");

        // 2. Synthetic manifest, exactly as ke-core's gen-fixtures builds it.
        //    `assemble` zeroes and re-derives `artifact_hash`, so the
        //    blake3(canonical.bin) value this puts in the slot is discarded.
        let effective_from = rule
            .effective_window
            .as_ref()
            .map(|w| w.effective_from)
            .unwrap_or_else(|| JurisdictionDate::new(1900, 1, 1));
        let manifest = examples::synthetic_manifest(
            ArtifactKind::RegimePack,
            "mica_2023",
            effective_from,
            &canonical,
        );

        // 3. Assemble + sign with the fixed-seed test key (deterministic;
        //    never OsRng/getrandom). ADR 0014 audit slots are None in Phase 1.
        let audit_versions = AuditVersions {
            jurisdiction_resolver_version: None,
            scenario_corpus_version: None,
        };
        let (artifact, kew) = Artifact::assemble(
            manifest,
            vec![rule],
            audit_versions,
            &test_keys::signing_key(),
            test_keys::TEST_KEY_ID,
        )
        .expect("golden artifact assembles");

        // Self-check + recover envelope_len for the review view/ledger.
        let (decoded, envelope_len) = decode_artifact(&kew).expect("assembled .kew self-decodes");
        assert_eq!(decoded, artifact, "decode round-trips the assembled record");

        // 4. Phase 2: append the deterministic three-type attestation set
        //    post-envelope. Spec § 9: the append must never move the content
        //    address — asserted hard before anything is written.
        let pre_hash = artifact.manifest.artifact_hash;
        let pre_signature = artifact.compiler_signature.clone();
        let attestation_set: Vec<Attestation> = GOLDEN_ATTESTATION_SET
            .iter()
            .map(|(ty, role)| golden_attestation(&artifact.manifest, *ty, *role))
            .collect();
        let (artifact, attested_kew) = artifact
            .with_attestations(attestation_set)
            .expect("attestation set appends");
        let (decoded, attested_envelope_len) =
            decode_artifact(&attested_kew).expect("attested .kew self-decodes");
        assert_eq!(decoded, artifact, "decode round-trips the attested record");
        assert_eq!(
            attested_envelope_len, envelope_len,
            "attestation append must not move envelope_len"
        );
        assert_eq!(
            attested_kew[..envelope_len],
            kew[..envelope_len],
            "attestation append must not touch the envelope prefix bytes"
        );
        assert_eq!(
            artifact.manifest.artifact_hash, pre_hash,
            "attestation append must not move the content address (spec § 9)"
        );
        assert_eq!(
            artifact.compiler_signature, pre_signature,
            "attestation append must not alter the compiler signature"
        );

        // 5. Authoritative .kew + regenerated review views.
        fs::write(dir.join("artifact.kew"), &attested_kew)?;
        let signature_json = format!(
            "{{\n  \"key_id\": \"{}\",\n  \"signature\": \"{}\",\n  \"envelope_len\": {},\n  \"hashed_range\": \"BLAKE3 over [0,envelope_len) with hash slot zeroed\"\n}}\n",
            artifact.compiler_signature.key_id,
            hex(&artifact.compiler_signature.signature),
            envelope_len,
        );
        fs::write(dir.join("signature.json"), signature_json)?;
        fs::write(
            dir.join("attestations.json"),
            attestations_json(&artifact.attestations),
        )?;

        rows.push(Row {
            id: id.to_string(),
            artifact_hash: hex(&artifact.manifest.artifact_hash),
            envelope_len,
        });
    }

    eprintln!(
        "note: `{SKIPPED_POLICY_ID}` (PolicyBundle) skipped — the Phase-1 Artifact \
         envelope is RuleIR-oriented (compiled_ir: Vec<RuleIR>); no PolicyBundle \
         artifact shape exists yet (see GOLDEN.md)"
    );

    // Ledger (deterministic order). Does NOT touch MANIFEST.md.
    rows.sort_by(|a, b| a.id.cmp(&b.id));
    let mut ledger = String::new();
    ledger.push_str(
        "# fixtures/artifacts/ — signed golden artifact ledger (Gate 4 Phase 1 + Phase 2)\n\n",
    );
    ledger.push_str(
        "Generated by `cargo run -p ke-artifact --features test-keys --bin gen-golden-artifacts`.\n\
         Do not hand-edit; re-run the generator. `artifact.kew` is authoritative;\n\
         `signature.json` and `attestations.json` are regenerated review views, never\n\
         authoritative.\n\
         This ledger is separate from `MANIFEST.md` (ke-core's `gen-fixtures` ledger).\n\n",
    );
    ledger.push_str(&format!(
        "- ir_schema_version: {IR_SCHEMA_VERSION}\n- codec_version: {CODEC_VERSION}\n\
         - canonicalization_version: {CANONICALIZATION_VERSION}\n\
         - signing key: fixed-seed test key `test-fixed-seed-1` — loudly a test key,\n  \
         never an ADR-0009 production key\n\
         - `artifact_hash` = BLAKE3 over the envelope prefix `[0, envelope_len)` with\n  \
         the 32-byte hash slot zeroed, then patched in. `blake3(raw .kew bytes)` does\n  \
         NOT equal `artifact_hash` — by construction; verifiers re-zero the slot first.\n\
         - `{SKIPPED_POLICY_ID}` (PolicyBundle) is skipped: the Phase-1 Artifact\n  \
         envelope is RuleIR-oriented (`compiled_ir: Vec<RuleIR>`), so only the two\n  \
         RegimePack rule fixtures carry signed artifacts.\n\
         - Phase 2 attestation set: each rule artifact carries three attestations\n  \
         (`SourceFidelity`, `ScenarioCoverage`, `PublicationApproval`) signed by the\n  \
         fixed-seed expert test key `{expert_key_id}`, scope `WholeArtifact`,\n  \
         attestation_policy_version `{GOLDEN_ATTESTATION_POLICY_VERSION}`, mock-TSA\n  \
         (`{MOCK_TSA_AUTHORITY_ID}`) stamped at fixed claimed_time_unix\n  \
         {GOLDEN_CLAIMED_TIME_UNIX}. Attestations live OUTSIDE the hashed+signed\n  \
         envelope (spec § 9), so every `artifact_hash` and `envelope_len` below is\n  \
         UNCHANGED from Phase 1 — the golden suite pins them as constants.\n\n",
        expert_key_id = test_keys::TEST_EXPERT_KEY_ID,
    ));
    ledger.push_str("| artifact_id | artifact_hash | envelope_len |\n");
    ledger.push_str("| ----------- | ------------- | ------------ |\n");
    for r in &rows {
        ledger.push_str(&format!(
            "| `{}` | `{}` | {} |\n",
            r.id, r.artifact_hash, r.envelope_len
        ));
    }
    fs::write(artifacts_dir.join("GOLDEN.md"), ledger)?;

    eprintln!(
        "wrote {} signed artifact(s) + GOLDEN.md to {}",
        rows.len(),
        artifacts_dir.display()
    );
    Ok(())
}
