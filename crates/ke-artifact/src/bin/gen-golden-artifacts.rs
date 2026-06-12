//! Generate the signed golden artifact vectors under `fixtures/artifacts/`
//! (Gate 4 Phase 1).
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
//! 4. writes `fixtures/artifacts/<id>/artifact.kew` (authoritative) plus a
//!    `signature.json` review view (regenerated, never authoritative).
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
use ke_artifact::{decode_artifact, Artifact, AuditVersions};
use ke_core::canonical::decode_rule;
use ke_core::examples;
use ke_core::ir::JurisdictionDate;
use ke_core::manifest::ArtifactKind;
use ke_core::version::{CANONICALIZATION_VERSION, CODEC_VERSION, IR_SCHEMA_VERSION};
use std::fs;
use std::path::{Path, PathBuf};

/// The RegimePack rule fixture inputs (committed `canonical.bin` files).
const RULE_FIXTURE_IDS: [&str; 2] = ["rule_reserve_assets", "rule_significant_thresholds"];

/// The PolicyBundle fixture skipped in Phase 1 (see module doc).
const SKIPPED_POLICY_ID: &str = "policy_production_eu";

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

        // 4. Authoritative .kew + regenerated review view.
        fs::write(dir.join("artifact.kew"), &kew)?;
        let signature_json = format!(
            "{{\n  \"key_id\": \"{}\",\n  \"signature\": \"{}\",\n  \"envelope_len\": {},\n  \"hashed_range\": \"BLAKE3 over [0,envelope_len) with hash slot zeroed\"\n}}\n",
            artifact.compiler_signature.key_id,
            hex(&artifact.compiler_signature.signature),
            envelope_len,
        );
        fs::write(dir.join("signature.json"), signature_json)?;

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
    ledger.push_str("# fixtures/artifacts/ — signed golden artifact ledger (Gate 4 Phase 1)\n\n");
    ledger.push_str(
        "Generated by `cargo run -p ke-artifact --features test-keys --bin gen-golden-artifacts`.\n\
         Do not hand-edit; re-run the generator. `artifact.kew` is authoritative;\n\
         `signature.json` is a regenerated review view, never authoritative.\n\
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
         RegimePack rule fixtures carry signed artifacts.\n\n",
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
