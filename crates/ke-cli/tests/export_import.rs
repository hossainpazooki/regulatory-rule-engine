//! Gate 5 (workstream 5b) round-trip integration test for `ke export` +
//! `ke import`.
//!
//! Flow: compile a corpus YAML into a tempdir registry (feature `test-keys`),
//! `export::run` the stored `.kew` to a flat temp file, then `import_kew::run`
//! on that flat file OFFLINE and assert:
//!
//! 1. the imported `artifact_hash` equals the compiled hash;
//! 2. the verdict is crypto-clean — `Verified` OR the expected offline
//!    `Rejected(NotPublished { .. })` (offline import has no live registry);
//! 3. `provenance.is_test_key == true` (the fixed-seed compiler key is loud).
//!
//! Then it **corrupts one byte inside the envelope prefix** of the exported
//! file and asserts `import_kew::run` returns `Err` — the re-zero content-hash
//! path (`verify_artifact`'s step 2, never `blake3(raw bytes)`) catches the
//! tamper and the file is not trusted blindly.
//!
//! Determinism mirrors `export_provenance.rs`: fixed test keys (feature
//! unification gives this target the gated modules), a fixed `NOW`, a tempdir
//! backend, and a separate tempdir for the exported flat file.

use ke_artifact::{decode_artifact, RejectionReason, Verdict};
use ke_cli::commands::{compile, export, import_kew};
use ke_cli::registry::backend::LocalFsBackend;
use ke_cli::registry::LifecycleState;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const NOW: u64 = 1_750_000_000;
const FIXTURE_YAML: &str = "../../fixtures/rules/mica_stablecoin.yaml";
const FIXTURE_REGIME: &str = "mica_2023";

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let path = std::env::temp_dir().join(format!("ke-export-import-test-{label}-{pid}-{n}"));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("create tempdir");
        TempDir { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn fixture_path() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(FIXTURE_YAML)
        .to_string_lossy()
        .into_owned()
}

fn compile_into(tmp: &TempDir) -> (LocalFsBackend, [u8; 32]) {
    let backend = LocalFsBackend::open(&tmp.path).expect("open backend");
    let yaml = fixture_path();
    let outcome = compile::run(
        &backend,
        &compile::CompileArgs {
            yaml_path: &yaml,
            regime_id: FIXTURE_REGIME,
            env: "local",
            now_unix: NOW,
        },
    )
    .expect("compile run");
    assert_eq!(outcome.final_state, LifecycleState::StructurallyVerified);
    (backend, outcome.artifact_hash)
}

#[test]
fn export_then_offline_import_round_trip_is_crypto_clean() {
    let tmp = TempDir::new("roundtrip");
    let (backend, hash) = compile_into(&tmp);

    // Export to a flat file in a separate tempdir.
    let out_dir = TempDir::new("flat");
    let out_path = out_dir
        .path
        .join("artifact.kew")
        .to_string_lossy()
        .into_owned();
    let exp = export::run(
        &backend,
        &export::ExportArgs {
            artifact_hash: hash,
            out_path: &out_path,
        },
    )
    .expect("export run");
    assert_eq!(exp.artifact_hash, hash, "export echoes the requested hash");

    // The exported flat file is byte-identical to the stored .kew.
    let stored = {
        use ke_cli::registry::backend::RegistryBackend;
        backend.read_artifact_kew(&hash).expect("stored kew")
    };
    let flat = std::fs::read(&out_path).expect("read flat file");
    assert_eq!(
        flat, stored,
        "exported flat file is byte-identical to the stored .kew (no re-encode)"
    );
    assert_eq!(exp.bytes_written, stored.len());

    // OFFLINE import: re-verify via verify_artifact.
    let outcome = import_kew::run(&import_kew::ImportArgs {
        kew_path: &out_path,
        now_unix: NOW,
    })
    .expect("import run (crypto-clean offline => Ok)");

    assert_eq!(
        outcome.artifact_hash, hash,
        "imported hash equals the compiled hash"
    );
    match &outcome.verdict {
        // Crypto-clean. Offline with no live registry the artifact is not
        // Published, so NotPublished is the expected (accepted) verdict.
        Verdict::Verified | Verdict::Rejected(RejectionReason::NotPublished { .. }) => {}
        other => panic!("expected Verified or Rejected(NotPublished), got {other:?}"),
    }
    assert!(
        outcome.provenance.is_test_key,
        "the fixed-seed compiler key must loudly report is_test_key"
    );
}

#[test]
fn import_rejects_tampered_envelope_byte() {
    let tmp = TempDir::new("tamper");
    let (backend, hash) = compile_into(&tmp);

    let out_dir = TempDir::new("flat-tamper");
    let out_path = out_dir
        .path
        .join("artifact.kew")
        .to_string_lossy()
        .into_owned();
    export::run(
        &backend,
        &export::ExportArgs {
            artifact_hash: hash,
            out_path: &out_path,
        },
    )
    .expect("export run");

    // Corrupt one byte INSIDE the envelope prefix (the canonical body the
    // content hash covers). Decoding the file gives the envelope length; flip a
    // byte well inside [0, envelope_len). The re-zero hash recompute in
    // verify_artifact step 2 must catch this -> HashMismatch -> import Err.
    let mut bytes = std::fs::read(&out_path).expect("read flat file");
    let (_artifact, envelope_len) = decode_artifact(&bytes).expect("decode exported");
    // Pick an offset comfortably inside the envelope, away from the very first
    // length/header bytes whose corruption could fail decode instead of hash.
    let flip_at = envelope_len / 2;
    assert!(flip_at < envelope_len, "flip index inside the envelope");
    bytes[flip_at] ^= 0xFF;
    std::fs::write(&out_path, &bytes).expect("write tampered file");

    let result = import_kew::run(&import_kew::ImportArgs {
        kew_path: &out_path,
        now_unix: NOW,
    });
    assert!(
        result.is_err(),
        "a tampered envelope byte must make offline import reject (not trust blindly); got {result:?}"
    );
}
