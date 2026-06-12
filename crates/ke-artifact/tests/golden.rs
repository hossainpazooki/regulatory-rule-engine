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
//! - key hygiene: every committed signature carries `key_id` starting with
//!   `test-` (exactly `test-fixed-seed-1`), never a production key id.

use ke_artifact::sign::{test_keys, verify_signature};
use ke_artifact::{artifact_hash_offset, decode_artifact, verify_hash, RegistryStateMetadata};
use std::fs;
use std::path::{Path, PathBuf};

const GOLDEN_IDS: [&str; 2] = ["rule_reserve_assets", "rule_significant_thresholds"];

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
fn committed_key_id_is_loudly_a_test_key() {
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

        // The signature.json review view carries the same loud key id.
        let view = fs::read_to_string(artifacts_dir().join(id).join("signature.json"))
            .expect("read signature.json");
        assert!(
            view.contains("\"key_id\": \"test-fixed-seed-1\""),
            "{id}: signature.json review view carries the test key_id"
        );
    }
}

#[test]
fn golden_artifacts_carry_phase_1_inert_slots() {
    for id in GOLDEN_IDS {
        let bytes = read_kew(id);
        let (artifact, _) = decode_artifact(&bytes).expect("golden .kew decodes");
        assert!(
            artifact.consistency_block.is_none(),
            "{id}: ConsistencyBlock is Phase 2"
        );
        assert!(
            artifact.attestations.is_empty(),
            "{id}: attestation binding is Phase 2"
        );
        assert_eq!(
            artifact.registry_state_metadata,
            RegistryStateMetadata::Draft,
            "{id}: registry state machine is Phase 3"
        );
    }
}
