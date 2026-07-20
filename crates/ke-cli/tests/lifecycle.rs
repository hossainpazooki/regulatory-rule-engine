//! Gate 4 Phase 3b lifecycle integration tests.
//!
//! Drives the full § 9 lifecycle end-to-end through the command `run` functions
//! in a tempdir registry with fixed keys + a fixed clock (`NOW`), covering:
//!
//! - (a) compile -> ml-check -> attest×3 -> publish -> query(--tag) -> deprecate
//!   -> revoke, asserting the state after each step;
//! - (b) publish REJECTED when a required attestation type is missing (the
//!   policy gate) -> typed `AttestationSetRejected`;
//! - (c) rollback to a Published hash OK (pointer moves, tag_moved appended) and
//!   rollback to a Deprecated/Revoked hash -> `RollbackIneligible`;
//! - (d) revoke with `--policy auditonly` records severity=high in the sidecar;
//! - (e) attest does NOT change `artifact_hash`;
//! - (f) the 3a canonical_event_head_hash pin is unaffected (event shape
//!   unchanged) — re-asserted indirectly by the registry suite; here we add
//! - (g) published + revoked event-head-hash PINS.
//!
//! Determinism: fixed registry-root + compiler + expert test keys, fixed `NOW`,
//! tempdir backends (feature unification gives this target the gated test-key
//! modules — see ke-cli Cargo `[dev-dependencies]`).

use ke_cli::commands::{attest, compile, deprecate, ml_check, publish, revoke, rollback};
use ke_cli::registry::backend::{LocalFsBackend, RegistryBackend};
use ke_cli::registry::{current_state, hash_hex, LifecycleState, RegistryError, Selector};
use ke_core::manifest::{AttestationType, RevocationPolicy};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

/// Fixed clock for every test (deterministic events). 2025-06-15T15:06:40Z —
/// the same `NOW` the registry suite + the golden generator use.
const NOW: u64 = 1_750_000_000;

/// A corpus fixture that compiles cleanly with no blocking findings.
const FIXTURE_YAML: &str = "../../fixtures/rules/mica_stablecoin.yaml";
const FIXTURE_REGIME: &str = "mica_2023";

/// The three types a strict publication requires.
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
        let path = std::env::temp_dir().join(format!("ke-lifecycle-test-{label}-{pid}-{n}"));
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

/// Compile the fixture into a fresh backend, asserting structurally_verified.
fn compile_into(tmp: &TempDir) -> (LocalFsBackend, [u8; 32]) {
    let backend = LocalFsBackend::open(&tmp.path).expect("open backend");
    let yaml = fixture_path();
    let args = compile::CompileArgs {
        yaml_path: &yaml,
        regime_id: FIXTURE_REGIME,
        env: "local",
        now_unix: NOW,
    };
    let outcome = compile::run(&backend, &args).expect("compile run");
    assert_eq!(outcome.final_state, LifecycleState::StructurallyVerified);
    (backend, outcome.artifact_hash)
}

/// Variant of `compile_into` that compiles a *named* fixture into an existing
/// backend (so a second, distinct artifact can share one registry). Used by the
/// C4 prior-distinct-hash test.
fn compile_named(backend: &LocalFsBackend, yaml_rel: &str, regime: &str) -> [u8; 32] {
    let yaml = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(yaml_rel)
        .to_string_lossy()
        .into_owned();
    let args = compile::CompileArgs {
        yaml_path: &yaml,
        regime_id: regime,
        env: "local",
        now_unix: NOW,
    };
    let outcome = compile::run(backend, &args).expect("compile run (named)");
    assert_eq!(outcome.final_state, LifecycleState::StructurallyVerified);
    outcome.artifact_hash
}

/// Resolve `<env>/<tag>` to its current content hash via the registry view.
fn resolve_tag(backend: &LocalFsBackend, env: &str, tag: &str) -> [u8; 32] {
    let (resolved, _record) = ke_cli::registry::resolve(
        backend,
        &Selector::ByTag {
            env: env.to_string(),
            tag: tag.to_string(),
        },
        NOW,
    )
    .expect("resolve by tag");
    resolved
}

fn state_of(backend: &LocalFsBackend, hash: &[u8; 32]) -> LifecycleState {
    let events = backend.read_events(hash).expect("read events");
    current_state(&events)
        .expect("derive state")
        .expect("has state")
}

fn ml_check(backend: &LocalFsBackend, hash: [u8; 32]) {
    let out = ml_check::run(
        backend,
        &ml_check::MlCheckArgs {
            artifact_hash: hash,
            now_unix: NOW,
        },
    )
    .expect("ml-check run");
    assert_eq!(out.final_state, LifecycleState::MlChecked);
}

fn attest_with(backend: &LocalFsBackend, hash: [u8; 32], types: &[AttestationType]) {
    let out = attest::run(
        backend,
        &attest::AttestArgs {
            artifact_hash: hash,
            types,
            now_unix: NOW,
        },
    )
    .expect("attest run");
    assert_eq!(out.final_state, LifecycleState::ExpertAttested);
    assert_eq!(out.attestation_count, types.len());
}

fn publish(backend: &LocalFsBackend, hash: [u8; 32], env: &str) {
    let out = publish::run(
        backend,
        &publish::PublishArgs {
            artifact_hash: hash,
            env,
            tag: "current",
            policy_path: None,
            now_unix: NOW,
        },
    )
    .expect("publish run");
    assert_eq!(out.final_state, LifecycleState::Published);
}

// ---- (a) full happy path + (e) hash-stable attest -------------------------

#[test]
fn full_lifecycle_happy_path() {
    let tmp = TempDir::new("happy");
    let (backend, hash) = compile_into(&tmp);
    assert_eq!(
        state_of(&backend, &hash),
        LifecycleState::StructurallyVerified
    );

    ml_check(&backend, hash);
    assert_eq!(state_of(&backend, &hash), LifecycleState::MlChecked);
    // The consistency sidecar is the dev stand-in (non-authoritative).
    let block = backend
        .read_consistency(&hash)
        .expect("read consistency")
        .expect("consistency present after ml-check");
    assert_eq!(block.execution_environment, ml_check::DEV_STANDIN_ENV);

    // (e) attest must NOT move the content address.
    let kew_before = backend.read_artifact_kew(&hash).expect("kew before");
    let (artifact_before, _) = ke_artifact::decode_artifact(&kew_before).expect("decode before");
    let hash_before = artifact_before.manifest.artifact_hash;
    assert!(
        artifact_before.attestations.is_empty(),
        "no attestations before attest"
    );

    attest_with(&backend, hash, &FULL_SET);
    assert_eq!(state_of(&backend, &hash), LifecycleState::ExpertAttested);

    let kew_after = backend.read_artifact_kew(&hash).expect("kew after");
    let (artifact_after, _) = ke_artifact::decode_artifact(&kew_after).expect("decode after");
    assert_eq!(
        artifact_after.manifest.artifact_hash, hash_before,
        "attest must not move the content address (spec § 9)"
    );
    assert_eq!(artifact_after.manifest.artifact_hash, hash);
    assert_eq!(
        artifact_after.attestations.len(),
        3,
        "three attestations appended"
    );

    publish(&backend, hash, "staging");
    assert_eq!(state_of(&backend, &hash), LifecycleState::Published);

    // query --tag resolves to the published hash, state Published.
    let (resolved, record) = ke_cli::registry::resolve(
        &backend,
        &Selector::ByTag {
            env: "staging".to_string(),
            tag: "current".to_string(),
        },
        NOW,
    )
    .expect("resolve by tag");
    assert_eq!(resolved, hash);
    assert_eq!(
        record.registry_state_at_resolution,
        LifecycleState::Published
    );
    assert_eq!(record.resolving_event_key, "published");

    // deprecate.
    let dep = deprecate::run(
        &backend,
        &deprecate::DeprecateArgs {
            artifact_hash: hash,
            now_unix: NOW,
        },
    )
    .expect("deprecate run");
    assert_eq!(dep.final_state, LifecycleState::Deprecated);
    assert_eq!(state_of(&backend, &hash), LifecycleState::Deprecated);

    // revoke (from deprecated) with hardstop.
    let rev = revoke::run(
        &backend,
        &revoke::RevokeArgs {
            artifact_hash: hash,
            policy: Some(RevocationPolicy::HardStop),
            reason_class: None,
            reason: Some("superseded"),
            now_unix: NOW,
        },
    )
    .expect("revoke run");
    assert_eq!(rev.final_state, LifecycleState::Revoked);
    assert_eq!(rev.severity, "normal");
    assert_eq!(state_of(&backend, &hash), LifecycleState::Revoked);

    let rec = backend
        .read_revocation(&hash)
        .expect("read revocation")
        .expect("revocation sidecar present");
    assert_eq!(rec.policy, RevocationPolicy::HardStop);
    assert_eq!(rec.reason.as_deref(), Some("superseded"));
    assert_eq!(rec.severity, "normal");
}

// ---- (b) publish policy gate: missing required type rejected --------------

#[test]
fn publish_rejected_when_required_type_missing() {
    let tmp = TempDir::new("policygate");
    let (backend, hash) = compile_into(&tmp);
    ml_check(&backend, hash);
    // Attest only two of the three required types: omit PublicationApproval.
    attest_with(
        &backend,
        hash,
        &[
            AttestationType::SourceFidelity,
            AttestationType::ScenarioCoverage,
        ],
    );
    assert_eq!(state_of(&backend, &hash), LifecycleState::ExpertAttested);

    let err = publish::run(
        &backend,
        &publish::PublishArgs {
            artifact_hash: hash,
            env: "staging",
            tag: "current",
            policy_path: None,
            now_unix: NOW,
        },
    )
    .expect_err("publish must be rejected without PublicationApproval");
    // The typed policy-gate error.
    let downcast = err
        .downcast_ref::<RegistryError>()
        .expect("publish error is a RegistryError");
    assert!(
        matches!(downcast, RegistryError::AttestationSetRejected(_)),
        "expected AttestationSetRejected, got {downcast:?}"
    );
    // State must NOT have advanced.
    assert_eq!(state_of(&backend, &hash), LifecycleState::ExpertAttested);
    // No tag pointer was written.
    assert!(backend
        .read_pointer("staging", "current")
        .expect("read pointer")
        .is_none());
}

// ---- (c) rollback eligibility ---------------------------------------------

#[test]
fn rollback_to_published_ok_and_to_revoked_ineligible() {
    let tmp = TempDir::new("rollback");
    let (backend, hash) = compile_into(&tmp);
    ml_check(&backend, hash);
    attest_with(&backend, hash, &FULL_SET);
    publish(&backend, hash, "staging");

    // Move the tag away first (simulate a later publish moved current); then
    // rollback to the published hash. Here we just rollback to the same hash:
    // eligibility is what's under test (pointer + tag_moved event).
    let events_before = backend.read_events(&hash).expect("events").len();
    let out = rollback::run(
        &backend,
        &rollback::RollbackArgs {
            env: "staging",
            tag: "current",
            to_hash: hash,
            now_unix: NOW,
        },
    )
    .expect("rollback to published ok");
    assert_eq!(out.target_state, LifecycleState::Published);

    // A tag_moved event was appended; state stays Published.
    let events_after = backend.read_events(&hash).expect("events");
    assert_eq!(events_after.len(), events_before + 1);
    assert_eq!(events_after.last().unwrap().event_kind, "tag_moved");
    assert_eq!(
        events_after.last().unwrap().new_state,
        LifecycleState::Published
    );
    assert_eq!(state_of(&backend, &hash), LifecycleState::Published);
    // Pointer points at the target.
    let ptr = backend
        .read_pointer("staging", "current")
        .expect("read pointer")
        .expect("pointer present");
    assert_eq!(ptr.target_hash().expect("target"), hash);

    // Now drive to revoked and assert rollback is ineligible.
    deprecate::run(
        &backend,
        &deprecate::DeprecateArgs {
            artifact_hash: hash,
            now_unix: NOW,
        },
    )
    .expect("deprecate");
    revoke::run(
        &backend,
        &revoke::RevokeArgs {
            artifact_hash: hash,
            policy: Some(RevocationPolicy::HardStop),
            reason_class: None,
            reason: None,
            now_unix: NOW,
        },
    )
    .expect("revoke");
    assert_eq!(state_of(&backend, &hash), LifecycleState::Revoked);

    let err = rollback::run(
        &backend,
        &rollback::RollbackArgs {
            env: "staging",
            tag: "current",
            to_hash: hash,
            now_unix: NOW,
        },
    )
    .expect_err("rollback to revoked must be ineligible");
    let downcast = err
        .downcast_ref::<RegistryError>()
        .expect("rollback error is a RegistryError");
    assert!(
        matches!(
            downcast,
            RegistryError::RollbackIneligible {
                state: LifecycleState::Revoked
            }
        ),
        "expected RollbackIneligible(Revoked), got {downcast:?}"
    );
}

// ---- (d) revoke auditonly -> severity=high --------------------------------

#[test]
fn revoke_auditonly_records_high_severity() {
    let tmp = TempDir::new("auditonly");
    let (backend, hash) = compile_into(&tmp);
    ml_check(&backend, hash);
    attest_with(&backend, hash, &FULL_SET);
    publish(&backend, hash, "staging");

    let out = revoke::run(
        &backend,
        &revoke::RevokeArgs {
            artifact_hash: hash,
            policy: Some(RevocationPolicy::AuditOnly),
            reason_class: None,
            reason: Some("audit"),
            now_unix: NOW,
        },
    )
    .expect("revoke run");
    assert_eq!(out.severity, "high");

    let rec = backend
        .read_revocation(&hash)
        .expect("read revocation")
        .expect("revocation present");
    assert_eq!(rec.policy, RevocationPolicy::AuditOnly);
    assert_eq!(rec.severity, "high");
}

// ---- (g) published + revoked event-head-hash PINS -------------------------

/// The blake3 of the `published` event's canonical bytes (signature included)
/// for the fixed-key, fixed-NOW lifecycle of `mica_stablecoin.yaml`. Locks the
/// event encoding + signing scheme across the ml_checked/expert_attested/
/// published chain. Re-pin (see assert message) only on an intentional change.
const PINNED_PUBLISHED_HEAD_HEX: &str =
    "bff874d0bb75c42aae0d35249817753e849368fc997d15ee6154f78261f38ffd";

/// The blake3 of the `revoked` event's canonical bytes for the same fixed
/// lifecycle, revoked from Published with HardStop (the revoke event shape is
/// policy-free; the policy lives in the sidecar).
const PINNED_REVOKED_HEAD_HEX: &str =
    "7edee48a59108338eb2501a6ce5e6ddd249d4b915aa30a676aff91a4f6d8f8ee";

#[test]
fn published_and_revoked_event_heads_are_pinned() {
    let tmp = TempDir::new("pins");
    let (backend, hash) = compile_into(&tmp);
    ml_check(&backend, hash);
    attest_with(&backend, hash, &FULL_SET);
    publish(&backend, hash, "staging");

    // Head after publish is the `published` event.
    let published_head = backend
        .read_events(&hash)
        .expect("events")
        .last()
        .expect("head")
        .chain_hash()
        .expect("chain hash");
    let published_hex = hash_hex(&published_head);
    assert_eq!(
        published_hex, PINNED_PUBLISHED_HEAD_HEX,
        "published event-head-hash changed — if intentional, re-pin to: {published_hex}"
    );

    // Revoke directly from Published, then pin the revoked event head.
    revoke::run(
        &backend,
        &revoke::RevokeArgs {
            artifact_hash: hash,
            policy: Some(RevocationPolicy::HardStop),
            reason_class: None,
            reason: None,
            now_unix: NOW,
        },
    )
    .expect("revoke");
    let revoked_head = backend
        .read_events(&hash)
        .expect("events")
        .last()
        .expect("head")
        .chain_hash()
        .expect("chain hash");
    let revoked_hex = hash_hex(&revoked_head);
    assert_eq!(
        revoked_hex, PINNED_REVOKED_HEAD_HEX,
        "revoked event-head-hash changed — if intentional, re-pin to: {revoked_hex}"
    );
}

// ---- (h) C4: rollback re-resolves to the PREVIOUS DISTINCT hash -----------

/// The literal spec §19 / brief C4 criterion: after a tag is moved to a newer
/// artifact B and then rolled back, resolve-by-tag returns the **previous
/// distinct** published hash A — not merely the same hash (which only proves
/// eligibility + pointer-move). Two distinct artifacts (A = `mica_stablecoin`,
/// B = `mica_authorization`, both `mica_2023`) share one registry; the tag
/// `staging/current` moves A -> B on B's publish, then back to A on rollback.
#[test]
fn rollback_reresolves_to_previous_distinct_hash() {
    let tmp = TempDir::new("c4_prior_distinct");

    // A -> published; staging/current points at A.
    let (backend, a) = compile_into(&tmp);
    ml_check(&backend, a);
    attest_with(&backend, a, &FULL_SET);
    publish(&backend, a, "staging");
    assert_eq!(resolve_tag(&backend, "staging", "current"), a);

    // B (DISTINCT) -> published under the SAME env+tag -> pointer moves to B.
    let b = compile_named(
        &backend,
        "../../fixtures/rules/mica_authorization.yaml",
        "mica_2023",
    );
    assert_ne!(a, b, "B must be a distinct artifact from A");
    ml_check(&backend, b);
    attest_with(&backend, b, &FULL_SET);
    publish(&backend, b, "staging");
    assert_eq!(
        resolve_tag(&backend, "staging", "current"),
        b,
        "publishing B under the same env+tag must move the pointer to B"
    );

    // Rollback the tag to A (A is Published => is_rollback_eligible).
    let out = rollback::run(
        &backend,
        &rollback::RollbackArgs {
            env: "staging",
            tag: "current",
            to_hash: a,
            now_unix: NOW,
        },
    )
    .expect("rollback to prior published A");
    assert_eq!(out.target_state, LifecycleState::Published);

    // LITERAL C4 CRITERION: resolve-by-tag now returns the PREVIOUS DISTINCT hash.
    assert_eq!(
        resolve_tag(&backend, "staging", "current"),
        a,
        "after rollback, resolve-by-tag must return the previous distinct hash A"
    );

    // The chain still validates and a tag_moved event was appended to A's log.
    let a_events = backend.read_events(&a).expect("events for A");
    assert_eq!(a_events.last().unwrap().event_kind, "tag_moved");
    assert!(
        current_state(&a_events).expect("derive state").is_some(),
        "A's event chain still validates after the rollback"
    );
}
