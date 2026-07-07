//! Golden-vector suite over the committed signed artifacts
//! (`fixtures/artifacts/<id>/artifact.kew`, ledger `GOLDEN.md`).
//!
//! Pins the byte-range contract (plan Rev 2 correction #1):
//! - `.kew` round-trips byte-stably through `decode_artifact` + postcard;
//! - `artifact_hash` = BLAKE3 over the envelope prefix `[0, envelope_len)`
//!   **with the 32-byte hash slot re-zeroed** — recomputed here independently
//!   (manual zero + blake3, not just via `verify_hash`) and compared against
//!   BOTH `manifest.artifact_hash` AND the `GOLDEN.md` ledger;
//! - **negative assertion:** `blake3(raw .kew bytes) != artifact_hash` — the
//!   naive whole-file check fails *by construction*, so the zero-then-patch
//!   semantics cannot be "fixed" by weakening the design;
//! - the compiler signature verifies over the hash-patched envelope prefix
//!   with the fixed-seed test verifying key;
//! - key hygiene: every committed key id (compiler signature, expert
//!   attestations, mock-TSA authority) starts with `test-`, never a
//!   production key id;
//! - **Phase 2 — hash-stability pins:** the two Phase-1 content addresses are
//!   hardcoded below ([`PHASE_1_PINS`]) and asserted against both the decoded
//!   manifests and the `GOLDEN.md` ledger. The committed goldens now carry a
//!   three-type attestation set appended **post-envelope**, so these pins
//!   mechanically prove that attestation append never moves the content
//!   address (spec § 9);
//! - the committed attestation set round-trips and passes
//!   `verify_attestation_set` under a strict local policy.

use ke_artifact::sign::{test_keys, verify_signature};
use ke_artifact::tsa::{TimestampAuthorityClass, MOCK_TSA_AUTHORITY_ID};
use ke_artifact::{
    artifact_hash_offset, decode_artifact, verify_attestation_set, verify_hash, KeyDirectory,
    KeyDirectoryEntry, KeyStatus, PolicyContext, RegistryStateMetadata, SignerRole,
};
use ke_core::manifest::{AttestationCount, AttestationType, T2T3Mode, VerificationPolicy};
use std::fs;
use std::path::{Path, PathBuf};

const GOLDEN_IDS: [&str; 2] = ["rule_reserve_assets", "rule_significant_thresholds"];

/// The Phase-1 content addresses, hardcoded as regression pins:
/// `(artifact_id, artifact_hash hex, envelope_len)`. These values were
/// recorded **before** the Phase-2 attestation set was appended; if either
/// ever moves, the spec § 9 append property (state never mutates envelope
/// bytes) has been broken.
const PHASE_1_PINS: [(&str, &str, usize); 2] = [
    (
        "rule_reserve_assets",
        "13a414cf7f6b25c6b6049c0953a83ff5697044aabafbd44b87e87fc4ed90f8a9",
        863,
    ),
    (
        "rule_significant_thresholds",
        "72a60976bcd55fc9a9b088cada4aae10cfbb4aabf066a8e85c403dbeae893d94",
        599,
    ),
];

/// The fixed verification clock for the golden attestation set — the same
/// instant the generator's mock-TSA tokens claim (`GOLDEN_CLAIMED_TIME_UNIX`).
const GOLDEN_NOW: u64 = 1_750_000_000;

/// The three attestation types every golden artifact must carry.
const GOLDEN_TYPES: [AttestationType; 3] = [
    AttestationType::SourceFidelity,
    AttestationType::ScenarioCoverage,
    AttestationType::PublicationApproval,
];

fn artifacts_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("artifacts")
}

fn read_kew(id: &str) -> Vec<u8> {
    fs::read(artifacts_dir().join(id).join("artifact.kew"))
        .unwrap_or_else(|e| panic!("read {id}/artifact.kew (run gen-golden-artifacts): {e}"))
}

/// One `GOLDEN.md` ledger row: (artifact_hash, envelope_len).
fn ledger_row(id: &str) -> ([u8; 32], usize) {
    let ledger = fs::read_to_string(artifacts_dir().join("GOLDEN.md")).expect("read GOLDEN.md");
    let needle = format!("| `{id}` |");
    let line = ledger
        .lines()
        .find(|l| l.starts_with(&needle))
        .unwrap_or_else(|| panic!("GOLDEN.md has a row for `{id}`"));
    let cols: Vec<&str> = line.split('|').map(str::trim).collect();
    // ["", "`id`", "`hash`", "len", ""]
    let hash_hex = cols[2].trim_matches('`');
    let envelope_len: usize = cols[3].parse().expect("ledger envelope_len parses");
    (unhex32(hash_hex), envelope_len)
}

fn unhex32(s: &str) -> [u8; 32] {
    assert_eq!(s.len(), 64, "ledger hash is 64 hex chars");
    let mut out = [0u8; 32];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).expect("ledger hash is hex");
    }
    out
}

#[test]
fn golden_kew_round_trips_byte_stably() {
    for id in GOLDEN_IDS {
        let bytes = read_kew(id);
        let (artifact, envelope_len) = decode_artifact(&bytes).expect("golden .kew decodes");
        assert!(
            envelope_len > 0 && envelope_len < bytes.len(),
            "{id}: envelope is a strict prefix"
        );
        let reencoded = postcard::to_stdvec(&artifact).expect("artifact re-encodes");
        assert_eq!(reencoded, bytes, "{id}: decode -> encode is byte-stable");
    }
}

#[test]
fn recomputed_hash_matches_manifest_and_ledger() {
    for id in GOLDEN_IDS {
        let bytes = read_kew(id);
        let (artifact, envelope_len) = decode_artifact(&bytes).expect("golden .kew decodes");
        let (ledger_hash, ledger_len) = ledger_row(id);
        assert_eq!(
            envelope_len, ledger_len,
            "{id}: ledger envelope_len matches"
        );

        // Independent recompute: extract the envelope prefix, RE-ZERO the
        // 32-byte hash slot, BLAKE3 the zeroed bytes.
        let offset = artifact_hash_offset(artifact.manifest.artifact_kind);
        let mut zeroed = bytes[..envelope_len].to_vec();
        zeroed[offset..offset + 32].fill(0);
        let recomputed: [u8; 32] = *blake3::hash(&zeroed).as_bytes();

        assert_eq!(
            recomputed, artifact.manifest.artifact_hash,
            "{id}: hash matches manifest"
        );
        assert_eq!(
            recomputed, ledger_hash,
            "{id}: hash matches GOLDEN.md ledger"
        );
        // The crate's own verifier agrees with the independent recompute.
        assert_eq!(
            verify_hash(&bytes).expect("verify_hash passes"),
            recomputed,
            "{id}"
        );
    }
}

#[test]
fn naive_whole_file_hash_fails_by_construction() {
    for id in GOLDEN_IDS {
        let bytes = read_kew(id);
        let (artifact, envelope_len) = decode_artifact(&bytes).expect("golden .kew decodes");
        let claimed = artifact.manifest.artifact_hash;
        // The trap, locked in: hashing the raw file — or even the patched
        // envelope prefix without re-zeroing — never matches.
        assert_ne!(
            *blake3::hash(&bytes).as_bytes(),
            claimed,
            "{id}: blake3(.kew) must NOT match"
        );
        assert_ne!(
            *blake3::hash(&bytes[..envelope_len]).as_bytes(),
            claimed,
            "{id}: blake3(patched prefix) must NOT match"
        );
    }
}

#[test]
fn compiler_signature_verifies_over_patched_prefix() {
    for id in GOLDEN_IDS {
        let bytes = read_kew(id);
        let (artifact, envelope_len) = decode_artifact(&bytes).expect("golden .kew decodes");
        verify_signature(
            &bytes[..envelope_len],
            &artifact.compiler_signature,
            &test_keys::verifying_key(),
        )
        .unwrap_or_else(|e| panic!("{id}: golden signature verifies with the test key: {e}"));
    }
}

#[test]
fn committed_key_ids_are_loudly_test_keys() {
    for id in GOLDEN_IDS {
        let bytes = read_kew(id);
        let (artifact, _) = decode_artifact(&bytes).expect("golden .kew decodes");
        let key_id = &artifact.compiler_signature.key_id;
        assert!(
            key_id.starts_with("test-"),
            "{id}: key_id `{key_id}` must start with `test-`"
        );
        assert_eq!(
            key_id,
            test_keys::TEST_KEY_ID,
            "{id}: exactly test-fixed-seed-1"
        );

        // Every committed attestation key id is the loud expert test key.
        for att in &artifact.attestations {
            assert!(
                att.key_id.starts_with("test-"),
                "{id}: attestation key_id `{}` must start with `test-`",
                att.key_id
            );
            assert_eq!(
                att.key_id,
                test_keys::TEST_EXPERT_KEY_ID,
                "{id}: exactly test-expert-fixed-seed-1"
            );
        }
        // The mock-TSA authority id is loudly a test authority.
        assert!(
            MOCK_TSA_AUTHORITY_ID.starts_with("test-"),
            "mock TSA authority id must start with `test-`"
        );

        // The review views carry the same loud key ids.
        let view = fs::read_to_string(artifacts_dir().join(id).join("signature.json"))
            .expect("read signature.json");
        assert!(
            view.contains("\"key_id\": \"test-fixed-seed-1\""),
            "{id}: signature.json review view carries the test key_id"
        );
        let view = fs::read_to_string(artifacts_dir().join(id).join("attestations.json"))
            .expect("read attestations.json");
        assert!(
            view.contains("\"key_id\": \"test-expert-fixed-seed-1\""),
            "{id}: attestations.json review view carries the expert test key_id"
        );
        assert!(
            view.contains("\"tsa_authority_id\": \"test-mock-tsa-1\""),
            "{id}: attestations.json review view carries the mock-TSA authority id"
        );
    }
}

#[test]
fn golden_artifacts_carry_phase_2_slots() {
    for id in GOLDEN_IDS {
        let bytes = read_kew(id);
        let (artifact, _) = decode_artifact(&bytes).expect("golden .kew decodes");
        assert!(
            artifact.consistency_block.is_none(),
            "{id}: no T2/T3 evidence path exists yet (platform-owned, ADR 0011)"
        );
        assert_eq!(
            artifact.attestations.len(),
            3,
            "{id}: Phase 2 appended the three-type attestation set"
        );
        assert_eq!(
            artifact.registry_state_metadata,
            RegistryStateMetadata::Draft,
            "{id}: registry state machine is Phase 3"
        );
    }
}

// ---- Phase 2: hash-stability pins + attested round-trip ----

/// A key directory holding exactly the golden expert key, authorized for the
/// three golden attestation types under both signing roles.
fn golden_directory() -> KeyDirectory {
    KeyDirectory {
        entries: vec![KeyDirectoryEntry {
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
        }],
    }
}

fn golden_context(current_legal_source_hash: [u8; 32]) -> PolicyContext {
    PolicyContext {
        environment: "local".to_string(),
        now_unix: GOLDEN_NOW,
        supported_policy_versions: vec!["ap-1".to_string()],
        current_legal_source_hash: Some(current_legal_source_hash),
    }
}

/// The §9 append property, mechanically pinned: the Phase-1 content
/// addresses (recorded before any attestation existed) still match BOTH the
/// decoded manifest AND the `GOLDEN.md` ledger, even though the committed
/// `.kew` files now carry the attestation set. If a pin moves, appending
/// attestations mutated envelope bytes — a contract break, not a refresh.
#[test]
fn hash_stability_pins_attestation_append_never_moves_the_content_address() {
    for (id, pin_hex, pin_len) in PHASE_1_PINS {
        let bytes = read_kew(id);
        let (artifact, envelope_len) = decode_artifact(&bytes).expect("golden .kew decodes");
        let pin = unhex32(pin_hex);

        assert_eq!(
            artifact.manifest.artifact_hash, pin,
            "{id}: manifest.artifact_hash must equal the Phase-1 pin"
        );
        assert_eq!(
            envelope_len, pin_len,
            "{id}: envelope_len must equal the Phase-1 pin"
        );
        let (ledger_hash, ledger_len) = ledger_row(id);
        assert_eq!(
            ledger_hash, pin,
            "{id}: GOLDEN.md ledger hash must equal the Phase-1 pin"
        );
        assert_eq!(ledger_len, pin_len, "{id}: GOLDEN.md ledger envelope_len");

        // The pinned address holds even though attestations are present and
        // the file is strictly longer than the envelope they bind to.
        assert!(
            !artifact.attestations.is_empty(),
            "{id}: the committed golden carries the attestation set"
        );
        assert!(
            bytes.len() > pin_len,
            "{id}: attested tail extends past the pinned envelope"
        );
    }
}

#[test]
fn golden_attestation_set_round_trips_and_passes_strict_local_policy() {
    for id in GOLDEN_IDS {
        let bytes = read_kew(id);
        let (artifact, _) = decode_artifact(&bytes).expect("golden .kew decodes");

        // Exactly the three expected types, each mock-stamped at the fixed
        // generator time and bound to this artifact's hash.
        let types: Vec<AttestationType> = artifact
            .attestations
            .iter()
            .map(|a| a.attestation_type)
            .collect();
        assert_eq!(types, GOLDEN_TYPES, "{id}: three-type set in fixed order");
        for att in &artifact.attestations {
            assert_eq!(
                att.artifact_hash, artifact.manifest.artifact_hash,
                "{id}: attestation binds the committed artifact hash"
            );
            assert_eq!(
                att.timestamp.class,
                TimestampAuthorityClass::Mock,
                "{id}: mock-TSA stamped"
            );
            assert_eq!(
                att.timestamp.claimed_time_unix, GOLDEN_NOW,
                "{id}: fixed generator claimed_time"
            );
            assert!(att.test_corpus_hash.is_none(), "{id}: slot not ratified");
        }

        // The full set passes a strict policy requiring all three types,
        // verified purely (fixed clock, local environment, recomputed legal
        // source hash = the manifest's source corpus hash the generator bound).
        let policy = VerificationPolicy {
            t2_t3_mode: T2T3Mode::Strict,
            required_attestation_types: GOLDEN_TYPES.to_vec(),
            minimum_attestation_count_per_type: GOLDEN_TYPES
                .iter()
                .map(|ty| AttestationCount {
                    attestation_type: *ty,
                    count: 1,
                })
                .collect(),
        };
        verify_attestation_set(
            &artifact,
            &policy,
            &golden_directory(),
            &golden_context(artifact.manifest.source_corpus_hash),
        )
        .unwrap_or_else(|rejections| {
            panic!("{id}: committed attestation set must verify, got {rejections:?}")
        });
    }
}
