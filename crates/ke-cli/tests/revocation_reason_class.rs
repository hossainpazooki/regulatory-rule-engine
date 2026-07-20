//! Gate 6 (ADR-0024): `ke revoke --reason-class` — the reason-class → policy
//! decision wired into the revoke command.
//!
//! - `--reason-class` alone records the class + its floor-derived policy;
//! - a configured `--policy` may raise strictness above the floor, never lower
//!   it (lowering is rejected, not silently raised);
//! - the legacy `--policy`-only path is byte-compatible: the sidecar JSON
//!   carries no `reason_class` key at all;
//! - the revoked EVENT stays frozen either way (ADR-0012) — only the sidecar
//!   grows.

use ke_cli::commands::{attest, compile, ml_check, publish, revoke};
use ke_cli::registry::backend::{LocalFsBackend, RegistryBackend};
use ke_cli::registry::LifecycleState;
use ke_core::manifest::{AttestationType, RevocationPolicy};
use ke_core::revocation::RevocationReasonClass;
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
        let path = std::env::temp_dir().join(format!("ke-revclass-test-{label}-{pid}-{n}"));
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

/// compile → ml-check → attest×3 → publish: a Published artifact to revoke.
fn published_artifact(tmp: &TempDir) -> (LocalFsBackend, [u8; 32]) {
    let backend = LocalFsBackend::open(&tmp.path).expect("open backend");
    let yaml = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(FIXTURE_YAML)
        .to_string_lossy()
        .into_owned();
    let out = compile::run(
        &backend,
        &compile::CompileArgs {
            yaml_path: &yaml,
            regime_id: FIXTURE_REGIME,
            env: "local",
            now_unix: NOW,
        },
    )
    .expect("compile");
    let hash = out.artifact_hash;
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
    (backend, hash)
}

// ---- reason-class records class + floor-derived policy ---------------------

#[test]
fn reason_class_alone_records_class_and_floor_policy() {
    let tmp = TempDir::new("floor");
    let (backend, hash) = published_artifact(&tmp);

    let out = revoke::run(
        &backend,
        &revoke::RevokeArgs {
            artifact_hash: hash,
            policy: None,
            reason_class: Some(RevocationReasonClass::KeyCompromise),
            reason: Some("expert key leaked"),
            now_unix: NOW,
        },
    )
    .expect("revoke with reason-class");
    assert_eq!(out.final_state, LifecycleState::Revoked);
    assert_eq!(out.recorded_policy, RevocationPolicy::HardStop);

    let record = backend
        .read_revocation(&hash)
        .expect("read sidecar")
        .expect("sidecar present");
    assert_eq!(record.policy, RevocationPolicy::HardStop);
    assert_eq!(
        record.reason_class,
        Some(RevocationReasonClass::KeyCompromise)
    );
}

/// Advisory floor is AuditOnly — and AuditOnly keeps its § 15 high severity.
#[test]
fn advisory_reason_class_floors_to_auditonly_with_high_severity() {
    let tmp = TempDir::new("advisory");
    let (backend, hash) = published_artifact(&tmp);

    let out = revoke::run(
        &backend,
        &revoke::RevokeArgs {
            artifact_hash: hash,
            policy: None,
            reason_class: Some(RevocationReasonClass::Advisory),
            reason: None,
            now_unix: NOW,
        },
    )
    .expect("revoke advisory");
    assert_eq!(out.recorded_policy, RevocationPolicy::AuditOnly);
    assert_eq!(out.severity, "high");
}

// ---- configured policy: raise OK, lower REJECTED ---------------------------

#[test]
fn configured_policy_above_floor_is_recorded() {
    let tmp = TempDir::new("raise");
    let (backend, hash) = published_artifact(&tmp);

    let out = revoke::run(
        &backend,
        &revoke::RevokeArgs {
            artifact_hash: hash,
            policy: Some(RevocationPolicy::HardStop),
            reason_class: Some(RevocationReasonClass::Advisory),
            reason: None,
            now_unix: NOW,
        },
    )
    .expect("revoke raising above floor");
    assert_eq!(out.recorded_policy, RevocationPolicy::HardStop);

    let record = backend
        .read_revocation(&hash)
        .expect("read sidecar")
        .expect("sidecar present");
    assert_eq!(record.policy, RevocationPolicy::HardStop);
    assert_eq!(record.reason_class, Some(RevocationReasonClass::Advisory));
}

#[test]
fn configured_policy_below_floor_is_rejected() {
    let tmp = TempDir::new("lower");
    let (backend, hash) = published_artifact(&tmp);

    let err = revoke::run(
        &backend,
        &revoke::RevokeArgs {
            artifact_hash: hash,
            policy: Some(RevocationPolicy::AuditOnly),
            reason_class: Some(RevocationReasonClass::KeyCompromise),
            reason: None,
            now_unix: NOW,
        },
    )
    .expect_err("policy below the reason-class floor must be rejected");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("floor"),
        "error should name the floor violation, got: {msg}"
    );

    // Rejected BEFORE any state transition: still Published, no sidecar.
    let record = backend.read_revocation(&hash).expect("read sidecar");
    assert!(record.is_none(), "no sidecar after a rejected revoke");
}

// ---- legacy --policy path unchanged ----------------------------------------

#[test]
fn legacy_policy_only_path_has_no_reason_class_key() {
    let tmp = TempDir::new("legacy");
    let (backend, hash) = published_artifact(&tmp);

    let out = revoke::run(
        &backend,
        &revoke::RevokeArgs {
            artifact_hash: hash,
            policy: Some(RevocationPolicy::FinishPinned),
            reason_class: None,
            reason: Some("superseded"),
            now_unix: NOW,
        },
    )
    .expect("legacy revoke");
    assert_eq!(out.recorded_policy, RevocationPolicy::FinishPinned);

    let record = backend
        .read_revocation(&hash)
        .expect("read sidecar")
        .expect("sidecar present");
    assert_eq!(record.reason_class, None);

    // Byte-compat: the sidecar JSON must not carry a `reason_class` key at all.
    let raw = std::fs::read_to_string(
        tmp.path
            .join("revocations")
            .join(format!("{}.json", ke_cli::registry::hash_hex(&hash))),
    )
    .expect("read raw sidecar json");
    assert!(
        !raw.contains("reason_class"),
        "legacy sidecar JSON must be shape-identical, got: {raw}"
    );
}

#[test]
fn neither_policy_nor_reason_class_is_rejected() {
    let tmp = TempDir::new("neither");
    let (backend, hash) = published_artifact(&tmp);

    revoke::run(
        &backend,
        &revoke::RevokeArgs {
            artifact_hash: hash,
            policy: None,
            reason_class: None,
            reason: None,
            now_unix: NOW,
        },
    )
    .expect_err("revoke needs --policy or --reason-class");
}

// ---- CLI string parsing ----------------------------------------------------

#[test]
fn parse_reason_class_accepts_documented_spellings() {
    for (s, expected) in [
        ("key_compromise", RevocationReasonClass::KeyCompromise),
        ("keycompromise", RevocationReasonClass::KeyCompromise),
        ("legal_invalidity", RevocationReasonClass::LegalInvalidity),
        ("legalinvalidity", RevocationReasonClass::LegalInvalidity),
        (
            "routine_supersession",
            RevocationReasonClass::RoutineSupersession,
        ),
        (
            "routinesupersession",
            RevocationReasonClass::RoutineSupersession,
        ),
        ("advisory", RevocationReasonClass::Advisory),
        ("ADVISORY", RevocationReasonClass::Advisory),
    ] {
        assert_eq!(
            revoke::parse_reason_class(s).expect("parse"),
            expected,
            "spelling {s:?}"
        );
    }
    assert!(revoke::parse_reason_class("bogus").is_err());
}
