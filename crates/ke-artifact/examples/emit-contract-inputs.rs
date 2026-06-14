//! Regenerate the shared three-language contract inputs under
//! `scripts/contract-inputs/` (Gate 4 Phase 4b).
//!
//! These JSON files ARE the shared verifier inputs the Rust, Python, and WASM
//! legs all load, so the contract test can assert they reach the **same**
//! verdict and the **same** canonical provenance over the committed goldens.
//! They mirror — exactly — the Published happy-path that the in-Rust
//! `tests/verify_surface.rs` builds in code:
//!
//! - `keydir.json` — compiler key (resolves `compiler_signature.key_id`) + the expert key authorized for the three golden types under both roles, window over the fixed claimed-time.
//! - `policy.json` — the strict `VerificationPolicy` requiring exactly the three golden types (the publish gate).
//! - `context.json` — environment `"local"`, the fixed clock, supported policy `["ap-1"]`, and `current_legal_source_hash` = the golden's manifest `source_corpus_hash` (so R5 passes).
//! - `registry.json` — `Published`, the fixed embedded event-head, no live head (no staleness check).
//!
//! Keys are fixed-seed test keys (no OsRng/getrandom). Run:
//!   cargo run -p ke-artifact --features test-keys --example emit-contract-inputs
//!
//! NOTE: the inputs live under `scripts/contract-inputs/`, NOT `fixtures/` —
//! `fixtures/` is generator-only and these are contract-test inputs.

use ke_artifact::sign::test_keys;
use ke_artifact::{
    decode_artifact, KeyDirectory, KeyDirectoryEntry, KeyStatus, PolicyContext, RegistryEvidence,
    RegistryStatus, SignerRole,
};
use ke_core::manifest::{AttestationCount, AttestationType, T2T3Mode, VerificationPolicy};
use std::fs;
use std::path::{Path, PathBuf};

/// The committed golden whose `source_corpus_hash` parameterizes the policy
/// context. Both goldens share the same source corpus / clock / keys, so one
/// shared context verifies both (the contract test runs every golden against
/// these same inputs).
const GOLDEN_ID: &str = "rule_reserve_assets";

/// The fixed verification/export clock the golden generator used.
const GOLDEN_NOW: u64 = 1_750_000_000;

/// The non-zero stand-in for the registry event-head as-of-export (matches
/// `tests/verify_surface.rs`).
const EMBEDDED_HEAD: [u8; 32] = [0x11; 32];

/// The three attestation types the goldens carry.
const GOLDEN_TYPES: [AttestationType; 3] = [
    AttestationType::SourceFidelity,
    AttestationType::ScenarioCoverage,
    AttestationType::PublicationApproval,
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn keydir() -> KeyDirectory {
    KeyDirectory {
        entries: vec![
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

fn context() -> PolicyContext {
    let kew_path = repo_root()
        .join("fixtures")
        .join("artifacts")
        .join(GOLDEN_ID)
        .join("artifact.kew");
    let kew = fs::read(&kew_path).unwrap_or_else(|e| panic!("read {}: {e}", kew_path.display()));
    let (artifact, _) = decode_artifact(&kew).expect("decode golden for source hash");
    PolicyContext {
        environment: "local".to_string(),
        now_unix: GOLDEN_NOW,
        supported_policy_versions: vec!["ap-1".to_string()],
        current_legal_source_hash: Some(artifact.manifest.source_corpus_hash),
    }
}

fn registry() -> RegistryEvidence {
    RegistryEvidence {
        status: RegistryStatus::Published,
        event_head_hash: EMBEDDED_HEAD,
        live_event_head: None,
    }
}

fn write_pretty<T: serde::Serialize>(path: &Path, value: &T) {
    let mut json = serde_json::to_string_pretty(value).expect("serialize contract input");
    json.push('\n');
    fs::write(path, json).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
    println!("wrote {}", path.display());
}

fn main() {
    let out = repo_root().join("scripts").join("contract-inputs");
    fs::create_dir_all(&out).expect("create scripts/contract-inputs");

    write_pretty(&out.join("keydir.json"), &keydir());
    write_pretty(&out.join("policy.json"), &strict_policy());
    write_pretty(&out.join("context.json"), &context());
    write_pretty(&out.join("registry.json"), &registry());

    println!(
        "contract inputs regenerated (now_unix={GOLDEN_NOW}, embedded_head=0x11..); \
         exported_at_unix is supplied by the contract test, not stored here."
    );
}
