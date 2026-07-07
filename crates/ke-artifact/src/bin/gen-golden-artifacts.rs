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
//!    `manifest.json`, `signature.json`, and `attestations.json` review views
//!    (regenerated, never authoritative). The `manifest.json` review view is
//!    rewritten here so consumers (e.g. COMPASS `sync:atlas`) read the CURRENT
//!    canon triplet / artifact_kind, not a stale copy from before a canon bump.
//!
//! It **also** builds one **IntentSpec** golden (ADR-0021 / Stage-A canon-5).
//! The envelope payload is now polymorphic ([`ArtifactPayload`]), so alongside
//! the rule fixtures the generator authors a synthetic payment IntentSpec IR in
//! Rust (analogous to `ke_core::examples`' Rust-authored rules — there is no
//! committed `canonical.bin` input for it), assembles it via
//! [`Artifact::assemble_payload`] with the same fixed-seed test key, and appends
//! the **kind-selected** two-type attestation set (`SourceFidelity` +
//! `PublicationApproval` — NOT the rule three-type set) that the IntentSpec
//! publish policy requires. Everything else (zero-then-patch hash, ed25519
//! envelope signature, post-envelope §9 attestation append) is identical to the
//! rule path.
//!
//! It then writes its own ledger `fixtures/artifacts/GOLDEN.md`. It does NOT
//! touch ke-core's `MANIFEST.md` — two generators, two ledgers, no clobber.
//!
//! ## Scope note: `policy_production_eu` is still skipped
//!
//! `IntentSpec` now has a first-class payload variant, but `PolicyBundle`
//! (and `EquivalenceMatrix` / `TestCorpus`) still have **no** `ArtifactPayload`
//! representation. The `policy_production_eu` fixture is therefore skipped with
//! a logged note — signed artifacts are built for the two RegimePack rule
//! fixtures plus the one IntentSpec fixture. A PolicyBundle-bearing payload
//! variant is a later-phase decision.
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
    decode_artifact, sign_attestation, Artifact, ArtifactPayload, Attestation, AttestationScope,
    AuditVersions, SignerRole,
};
use ke_core::canonical::decode_rule;
use ke_core::examples;
use ke_core::ir::{
    AuthorizationCriterion, IdempotencyDef, IntentSpecIR, JurisdictionDate, ScalarValue,
    SourceSpan, Volatility,
};
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

/// The **kind-selected** attestation set an `IntentSpec` publishes under
/// (ADR-0021 / Stage-A contract): `SourceFidelity` + `PublicationApproval`
/// only — deliberately **not** the rule three-type set (no `ScenarioCoverage`).
/// This is the set the IntentSpec publish policy requires; `PublicationApproval`
/// signs under `PublicationApprover` and is honored alongside the co-present
/// `SourceFidelity`.
const GOLDEN_INTENTSPEC_ATTESTATION_SET: [(AttestationType, SignerRole); 2] = [
    (AttestationType::SourceFidelity, SignerRole::DomainExpert),
    (
        AttestationType::PublicationApproval,
        SignerRole::PublicationApprover,
    ),
];

/// The IntentSpec golden fixture id (its own `fixtures/artifacts/<id>/` dir,
/// created by this generator — there is no committed `canonical.bin` input,
/// since the IntentSpec payload is Rust-authored, not decoded from bytes).
const INTENTSPEC_FIXTURE_ID: &str = "intentspec_payment";

/// The regime id the IntentSpec golden is authored under — a synthetic
/// treasury/payment marker, loudly not a production regime.
const INTENTSPEC_REGIME_ID: &str = "treasury_payments_v1";

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

/// The synthetic payment IntentSpec payload authored for the golden — analogous
/// to `ke_core::examples`' Rust-authored rule IRs (there is no committed
/// `canonical.bin` input for an IntentSpec). A couple of authorization criteria
/// mixing a **stable** and a **volatile** threshold, an idempotency definition
/// (payer-scoped key fields + scope), and the source spans the criteria derive
/// from. Float-free per ADR-0003 — thresholds are exact `ScalarValue::Decimal`s.
fn intentspec_golden_payload() -> IntentSpecIR {
    IntentSpecIR {
        action_class: "treasury.payment.outbound".to_string(),
        criteria: vec![
            // Stable: a fixed per-payment ceiling (EUR 1,000,000 — exact integer).
            AuthorizationCriterion {
                name: "amount_under_ceiling".to_string(),
                threshold: ScalarValue::int(1_000_000),
                volatility: Volatility::Stable,
            },
            // Volatile: an FX deviation bound that moves with the market
            // (1.05 — exact decimal {mantissa: 105, scale: 2}).
            AuthorizationCriterion {
                name: "fx_rate_within_band".to_string(),
                threshold: ScalarValue::Decimal {
                    mantissa: 105,
                    scale: 2,
                },
                volatility: Volatility::Volatile,
            },
        ],
        idempotency: IdempotencyDef {
            key_fields: vec!["payer_id".to_string(), "payment_reference".to_string()],
            scope: "treasury.payment.outbound".to_string(),
        },
        source_spans: vec![
            SourceSpan {
                document_id: "treasury_authorization_policy_2025".to_string(),
                article: Some("4".to_string()),
                section: Some("2".to_string()),
                paragraph: None,
                pages: Some(vec![12]),
                byte_range: None,
                text_hash: None,
            },
            SourceSpan {
                document_id: "treasury_authorization_policy_2025".to_string(),
                article: Some("7".to_string()),
                section: Some("1".to_string()),
                paragraph: None,
                pages: Some(vec![19, 20]),
                byte_range: None,
                text_hash: None,
            },
        ],
    }
}

/// Phase 2 (shared by the rule and IntentSpec goldens): append the
/// kind-selected attestation set post-envelope, assert the spec § 9 invariants
/// (the append must never move the content address, the envelope prefix bytes,
/// `envelope_len`, or the compiler signature) **before** anything is written,
/// then write the authoritative `.kew` plus the regenerated review views. One
/// code path so both kinds prove the identical post-envelope invariants.
fn attest_and_write(
    id: &str,
    dir: &Path,
    artifact: Artifact,
    kew: Vec<u8>,
    envelope_len: usize,
    attestation_set_spec: &[(AttestationType, SignerRole)],
) -> std::io::Result<Row> {
    let pre_hash = artifact.manifest.artifact_hash;
    let pre_signature = artifact.compiler_signature.clone();
    let attestation_set: Vec<Attestation> = attestation_set_spec
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

    // Authoritative .kew + regenerated review views. `create_dir_all` is a
    // no-op for the rule fixtures (their dir carries the committed
    // `canonical.bin`) and creates the IntentSpec fixture dir on first run.
    fs::create_dir_all(dir)?;
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
    // manifest.json review view (regenerated): the canonical Manifest as pretty
    // JSON, so downstream consumers (e.g. COMPASS `sync:atlas`) read the CURRENT
    // canon triplet / artifact_kind rather than a stale copy. Regenerating this
    // alongside the .kew keeps the review views from drifting on a canon bump.
    let manifest_json =
        serde_json::to_string_pretty(&artifact.manifest).expect("serialize manifest.json");
    fs::write(dir.join("manifest.json"), manifest_json + "\n")?;

    Ok(Row {
        id: id.to_string(),
        artifact_hash: hex(&artifact.manifest.artifact_hash),
        envelope_len,
    })
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

        // 4 + 5. Phase 2 attestation append (deterministic three-type set) +
        //    write. The §9 post-envelope invariants are asserted inside the
        //    shared helper, hard, before anything is written.
        rows.push(attest_and_write(
            id,
            &dir,
            artifact,
            kew,
            envelope_len,
            &GOLDEN_ATTESTATION_SET,
        )?);
    }

    // --- IntentSpec golden (ADR-0021 / canon-5) -------------------------------
    // No committed `canonical.bin` input: the payload is Rust-authored (like the
    // rule examples in `ke_core::examples`), assembled through the polymorphic
    // `assemble_payload` entry point, and carries the kind-selected two-type
    // attestation set.
    {
        let id = INTENTSPEC_FIXTURE_ID;
        let dir = artifacts_dir.join(id);

        // 1. Author the payload. Its postcard bytes stand in for the "canonical
        //    input" the rule path reads from disk, feeding the synthetic
        //    manifest's placeholder corpus/source hashes deterministically.
        let intent = intentspec_golden_payload();
        let intent_bytes =
            postcard::to_stdvec(&intent).expect("intentspec payload postcard-encodes");

        // 2. Synthetic manifest, kind = IntentSpec (append-only discriminant 4).
        let manifest = examples::synthetic_manifest(
            ArtifactKind::IntentSpec,
            INTENTSPEC_REGIME_ID,
            JurisdictionDate::new(2025, 1, 1),
            &intent_bytes,
        );

        // 3. Assemble + sign the polymorphic payload with the same fixed-seed
        //    test key (deterministic; never OsRng/getrandom). ADR 0014 audit
        //    slots are None in Phase 1, exactly as the rule path.
        let audit_versions = AuditVersions {
            jurisdiction_resolver_version: None,
            scenario_corpus_version: None,
        };
        let (artifact, kew) = Artifact::assemble_payload(
            manifest,
            ArtifactPayload::IntentSpec(intent),
            audit_versions,
            &test_keys::signing_key(),
            test_keys::TEST_KEY_ID,
        )
        .expect("golden IntentSpec artifact assembles");

        let (decoded, envelope_len) = decode_artifact(&kew).expect("assembled .kew self-decodes");
        assert_eq!(decoded, artifact, "decode round-trips the assembled record");
        assert!(
            matches!(artifact.payload, ArtifactPayload::IntentSpec(_)),
            "IntentSpec golden carries an IntentSpec payload"
        );

        // 4 + 5. Phase 2 append (kind-selected SourceFidelity + PublicationApproval)
        //    + write, through the same helper the rule goldens use.
        rows.push(attest_and_write(
            id,
            &dir,
            artifact,
            kew,
            envelope_len,
            &GOLDEN_INTENTSPEC_ATTESTATION_SET,
        )?);
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
         - Payload is polymorphic (ADR-0021): `ArtifactPayload::Rules(_)` for the\n  \
         two RegimePack fixtures, `ArtifactPayload::IntentSpec(_)` for the\n  \
         `{INTENTSPEC_FIXTURE_ID}` fixture. This is the canon-5 re-pin — every\n  \
         `artifact_hash`/`envelope_len` below is a fresh canon-5 value, NOT the\n  \
         canon-4 Phase-1 value.\n\
         - `{SKIPPED_POLICY_ID}` (PolicyBundle) is still skipped: `PolicyBundle`\n  \
         (like `EquivalenceMatrix`/`TestCorpus`) has no `ArtifactPayload` variant\n  \
         yet, so only the two RegimePack rule fixtures and the one IntentSpec\n  \
         fixture carry signed artifacts.\n\
         - Phase 2 attestation set: each **rule** artifact carries three\n  \
         attestations (`SourceFidelity`, `ScenarioCoverage`, `PublicationApproval`);\n  \
         the **IntentSpec** artifact carries the kind-selected two-type set\n  \
         (`SourceFidelity`, `PublicationApproval` — no `ScenarioCoverage`). All are\n  \
         signed by the fixed-seed expert test key `{expert_key_id}`, scope\n  \
         `WholeArtifact`, attestation_policy_version\n  \
         `{GOLDEN_ATTESTATION_POLICY_VERSION}`, mock-TSA (`{MOCK_TSA_AUTHORITY_ID}`)\n  \
         stamped at fixed claimed_time_unix {GOLDEN_CLAIMED_TIME_UNIX}. Attestations\n  \
         live OUTSIDE the hashed+signed envelope (spec § 9), so appending them does\n  \
         not change any `artifact_hash`/`envelope_len` below — the golden suite pins\n  \
         them as constants.\n\n",
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
