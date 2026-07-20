//! Gate 4 Phase 4a integration test for `ke export-provenance` (ADR 0016).
//!
//! Drives an artifact to `published`, then to `revoked`, through the existing
//! command `run` helpers in a tempdir registry with fixed keys + a fixed clock,
//! and after each transition runs `export_provenance::run` and asserts the
//! emitted canonical JSON's `registry_state` and `registry_event_head_hash`
//! match what `current_state` / `head_event` report directly off the event log
//! (independent recompute — never trusting the command's self-report).
//!
//! Determinism mirrors `lifecycle.rs`: fixed registry-root + compiler + expert
//! test keys (feature unification gives this target the gated modules), fixed
//! `NOW`, tempdir backend.

use ke_artifact::RegistryStatus;
use ke_cli::commands::export_provenance::{self, status_for};
use ke_cli::commands::{attest, compile, ml_check, publish, revoke};
use ke_cli::registry::backend::{LocalFsBackend, RegistryBackend};
use ke_cli::registry::{current_state, hash_hex, head_event, LifecycleState};
use ke_core::manifest::{AttestationType, RevocationPolicy};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const NOW: u64 = 1_750_000_000;
const FIXTURE_YAML: &str = "../../fixtures/rules/mica_stablecoin.yaml";
const FIXTURE_REGIME: &str = "mica_2023";

const FULL_SET: [AttestationType; 3] = [
    AttestationType::SourceFidelity,
    AttestationType::ScenarioCoverage,
    AttestationType::PublicationApproval,
];

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let path = std::env::temp_dir().join(format!("ke-provenance-test-{label}-{pid}-{n}"));
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

/// Export provenance and parse the canonical JSON back to the typed value.
fn export(backend: &LocalFsBackend, hash: [u8; 32]) -> ke_artifact::ArtifactProvenance {
    let outcome = export_provenance::run(
        backend,
        &export_provenance::ExportProvenanceArgs {
            artifact_hash: hash,
            exported_at_unix: NOW,
            write_root: None,
        },
    )
    .expect("export-provenance run");
    // The returned provenance and its canonical JSON must agree.
    let parsed: ke_artifact::ArtifactProvenance =
        serde_json::from_str(&outcome.canonical_json).expect("canonical JSON parses");
    assert_eq!(
        parsed, outcome.provenance,
        "canonical JSON round-trips to the returned provenance"
    );
    parsed
}

/// The event-head chain hash read directly off the log (independent of the
/// command).
fn event_head_hex(backend: &LocalFsBackend, hash: &[u8; 32]) -> String {
    hash_hex(
        &head_event(backend, hash)
            .expect("head event")
            .chain_hash()
            .expect("chain hash"),
    )
}

/// The current state read directly off the log.
fn state(backend: &LocalFsBackend, hash: &[u8; 32]) -> LifecycleState {
    current_state(&backend.read_events(hash).expect("events"))
        .expect("derive state")
        .expect("has state")
}

#[test]
fn export_provenance_tracks_published_then_revoked() {
    let tmp = TempDir::new("lifecycle");
    let (backend, hash) = compile_into(&tmp);

    // --- pre-publish: state is Unknown to an execution consumer ---
    let prov = export(&backend, hash);
    assert_eq!(prov.registry_state, RegistryStatus::Unknown);
    assert_eq!(
        prov.registry_state,
        status_for(state(&backend, &hash)),
        "status mapping matches the log-derived state"
    );
    assert_eq!(hash_hex(&prov.artifact_hash), hash_hex(&hash));
    assert!(prov.is_test_key, "compiler key is the fixed-seed test key");

    // --- drive to published ---
    ml_check::run(
        &backend,
        &ml_check::MlCheckArgs {
            artifact_hash: hash,
            now_unix: NOW,
        },
    )
    .expect("ml-check");
    attest::run(
        &backend,
        &attest::AttestArgs {
            artifact_hash: hash,
            types: &FULL_SET,
            now_unix: NOW,
        },
    )
    .expect("attest");
    publish::run(
        &backend,
        &publish::PublishArgs {
            artifact_hash: hash,
            env: "staging",
            tag: "current",
            policy_path: None,
            now_unix: NOW,
        },
    )
    .expect("publish");
    assert_eq!(state(&backend, &hash), LifecycleState::Published);

    let prov = export(&backend, hash);
    assert_eq!(prov.registry_state, RegistryStatus::Published);
    assert_eq!(
        hash_hex(&prov.registry_event_head_hash),
        event_head_hex(&backend, &hash),
        "exported event-head matches head_event off the log (published)"
    );
    assert_eq!(prov.attestations.len(), 3);

    // --- revoke ---
    revoke::run(
        &backend,
        &revoke::RevokeArgs {
            artifact_hash: hash,
            policy: Some(RevocationPolicy::HardStop),
            reason_class: None,
            reason: Some("superseded"),
            now_unix: NOW,
        },
    )
    .expect("revoke");
    assert_eq!(state(&backend, &hash), LifecycleState::Revoked);

    let prov = export(&backend, hash);
    assert_eq!(
        prov.registry_state,
        RegistryStatus::Revoked,
        "a revoked artifact exports Revoked (the COMPASS staleness/revocation fix)"
    );
    assert_eq!(
        hash_hex(&prov.registry_event_head_hash),
        event_head_hex(&backend, &hash),
        "exported event-head matches head_event off the log (revoked)"
    );
    assert_eq!(
        prov.registry_state,
        status_for(state(&backend, &hash)),
        "status mapping matches the log-derived state after revoke"
    );
}

#[test]
fn export_provenance_write_root_writes_sidecar() {
    let tmp = TempDir::new("sidecar");
    let (backend, hash) = compile_into(&tmp);
    let root = tmp.path.to_string_lossy().into_owned();

    let outcome = export_provenance::run(
        &backend,
        &export_provenance::ExportProvenanceArgs {
            artifact_hash: hash,
            exported_at_unix: NOW,
            write_root: Some(root.as_str()),
        },
    )
    .expect("export with sidecar");

    let sidecar = tmp
        .path
        .join("artifacts")
        .join(hash_hex(&hash))
        .join("provenance.json");
    let written = std::fs::read_to_string(&sidecar).expect("sidecar written");
    assert_eq!(
        written, outcome.canonical_json,
        "sidecar bytes equal the canonical JSON"
    );
}
