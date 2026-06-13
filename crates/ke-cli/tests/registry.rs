//! Gate 4 Phase 3a registry integration tests.
//!
//! Covers (plan): (a) the compile-equivalent flow building `draft` +
//! `structurally_verified`, `current_state == StructurallyVerified`; (b)
//! hash-chain tamper -> typed error; (c) seq gap -> typed error; (d)
//! transition-authority precondition rejection; (e) resolution ByHash + ByTag +
//! ByRegime with the §18 record fields; (f) `is_rollback_eligible`; (g) bad-sig
//! event rejected; (h) the canonical-event-head-hash pin.
//!
//! Determinism: fixed registry-root + compiler test keys, fixed `KE_NOW`
//! (`NOW`), tempdir backends. `cfg(test)`/feature unification gives this target
//! the gated test-key modules (see ke-cli Cargo `[dev-dependencies]`).

use ke_artifact::tsa::MockTsa;
use ke_artifact::SignerRole;
use ke_cli::commands::compile::{self, CompileArgs};
use ke_cli::registry::backend::{LocalFsBackend, RegistryBackend};
use ke_cli::registry::event::test_keys;
use ke_cli::registry::{
    build_draft_event, build_transition_event, can_transition, current_state, is_rollback_eligible,
    resolve, LifecycleState, Preconditions, RegistryError, Selector,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

/// Fixed clock for every test (deterministic events). 2025-06-15T15:06:40Z.
const NOW: u64 = 1_750_000_000;

/// A corpus fixture that compiles cleanly with no blocking findings.
const FIXTURE_YAML: &str = "../../fixtures/rules/mica_stablecoin.yaml";
const FIXTURE_REGIME: &str = "mica_2023";

/// A unique tempdir under the OS temp root; removed on drop.
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let path = std::env::temp_dir().join(format!("ke-registry-test-{label}-{pid}-{n}"));
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

/// Resolve the fixture path relative to the crate manifest dir.
fn fixture_path() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(FIXTURE_YAML)
        .to_string_lossy()
        .into_owned()
}

/// Compile the fixture into a fresh backend and return (backend, hash).
fn compile_into(tmp: &TempDir) -> (LocalFsBackend, [u8; 32]) {
    let backend = LocalFsBackend::open(&tmp.path).expect("open backend");
    let yaml = fixture_path();
    let args = CompileArgs {
        yaml_path: &yaml,
        regime_id: FIXTURE_REGIME,
        env: "local",
        now_unix: NOW,
    };
    let outcome = compile::run(&backend, &args).expect("compile run");
    assert_eq!(
        outcome.final_state,
        LifecycleState::StructurallyVerified,
        "clean corpus rule must reach structurally_verified"
    );
    (backend, outcome.artifact_hash)
}

// ---- (a) compile-equivalent flow -> draft + structurally_verified ---------

#[test]
fn compile_builds_draft_then_structurally_verified() {
    let tmp = TempDir::new("compile");
    let (backend, hash) = compile_into(&tmp);

    let events = backend.read_events(&hash).expect("read events");
    assert_eq!(events.len(), 2, "draft + structurally_verified");
    assert_eq!(events[0].new_state, LifecycleState::Draft);
    assert_eq!(events[0].prior_state, None);
    assert_eq!(events[0].prev_event_hash, None);
    assert_eq!(events[1].new_state, LifecycleState::StructurallyVerified);
    assert_eq!(events[1].prior_state, Some(LifecycleState::Draft));

    // Every event is registry-root-signed and records the registry role.
    for e in &events {
        assert_eq!(e.authority_role, SignerRole::Registry);
    }
    // The draft event names the registry-root test key as authority.
    assert!(events[0].authority_key_id.starts_with("test-"));

    assert_eq!(
        current_state(&events).expect("derive state"),
        Some(LifecycleState::StructurallyVerified)
    );

    // The non-authoritative marker exists (ADR 0012 §6).
    let marker = ke_cli::registry::backend::read_marker(&tmp.path).expect("marker");
    assert!(marker.contains("NON-AUTHORITATIVE"));
}

// ---- (b) hash-chain tamper -> typed error ---------------------------------

#[test]
fn tampering_event_zero_breaks_the_chain() {
    let tmp = TempDir::new("tamper");
    let (backend, hash) = compile_into(&tmp);
    let mut events = backend.read_events(&hash).expect("read events");

    // Flip a byte in event 0's artifact_hash. This both breaks the seq-0
    // signature (payload changed) and the chain link event 1 recorded over
    // event 0's bytes. current_state must reject with a typed error.
    events[0].artifact_hash[0] ^= 0x01;
    let err = current_state(&events).expect_err("tampered chain must be rejected");
    assert!(
        matches!(
            err,
            RegistryError::SignatureInvalid { .. } | RegistryError::ChainBroken { .. }
        ),
        "expected SignatureInvalid/ChainBroken, got {err:?}"
    );
}

// ---- (c) seq gap -> typed error -------------------------------------------

#[test]
fn sequence_gap_is_a_typed_error() {
    let tmp = TempDir::new("seqgap");
    let (backend, hash) = compile_into(&tmp);
    let mut events = backend.read_events(&hash).expect("read events");

    // Drop event 0; the remaining event has seq 1 where seq 0 is expected.
    events.remove(0);
    let err = current_state(&events).expect_err("seq gap must be rejected");
    assert!(
        matches!(
            err,
            RegistryError::SeqGap {
                expected: 0,
                found: 1
            }
        ),
        "expected SeqGap{{0,1}}, got {err:?}"
    );
}

// ---- (d) transition-authority precondition rejection ----------------------

#[test]
fn published_without_expert_attested_is_rejected() {
    // can_transition is the §9 authority gate. Attempting to publish from a
    // non-expert-attested prior must be refused.
    let pre = Preconditions {
        prior_is_expert_attested: false,
        ..Preconditions::default()
    };
    assert!(
        !can_transition(
            LifecycleState::StructurallyVerified,
            LifecycleState::Published,
            &pre
        ),
        "publish requires prior == expert_attested"
    );
    // Even from ExpertAttested, the precondition bit must be set.
    assert!(!can_transition(
        LifecycleState::ExpertAttested,
        LifecycleState::Published,
        &Preconditions::default()
    ));
    let ok = Preconditions {
        prior_is_expert_attested: true,
        ..Preconditions::default()
    };
    assert!(can_transition(
        LifecycleState::ExpertAttested,
        LifecycleState::Published,
        &ok
    ));
    // structurally_verified -> ml_checked is deferred behavior: gated on a
    // consistency block (table entry only in 3a).
    assert!(!can_transition(
        LifecycleState::StructurallyVerified,
        LifecycleState::MlChecked,
        &Preconditions::default()
    ));
    assert!(can_transition(
        LifecycleState::StructurallyVerified,
        LifecycleState::MlChecked,
        &Preconditions {
            consistency_block_present: true,
            ..Preconditions::default()
        }
    ));
}

// ---- (e) resolution ByHash + ByTag + ByRegime + §18 record ----------------

/// Hand-append a `published` event to the log via core (publish CLI is 3b) so
/// ByTag/ByRegime can be exercised. Walks draft -> structurally_verified (from
/// compile) -> ... -> published, signing each with the registry-root key.
fn drive_to_published(backend: &LocalFsBackend, hash: &[u8; 32]) {
    let mut events = backend.read_events(hash).expect("read events");
    // The compile flow leaves us at structurally_verified (seq 1).
    let mut prior = events.last().expect("has events").clone();
    for next in [
        LifecycleState::MlChecked,
        LifecycleState::ExpertAttested,
        LifecycleState::Published,
    ] {
        let ts = MockTsa::stamp(hash, NOW);
        let ev = build_transition_event(
            &prior,
            next,
            test_keys::REGISTRY_ROOT_KEY_ID,
            SignerRole::Registry,
            ts,
        )
        .expect("build transition");
        backend.append_event(hash, &ev).expect("append");
        prior = ev;
    }
    events = backend.read_events(hash).expect("re-read");
    assert_eq!(
        current_state(&events).expect("state"),
        Some(LifecycleState::Published)
    );
}

#[test]
fn resolution_by_hash_tag_and_regime() {
    let tmp = TempDir::new("resolve");
    let (backend, hash) = compile_into(&tmp);

    // ByHash resolves at structurally_verified.
    let (rh, record) = resolve(&backend, &Selector::ByHash(hash), NOW).expect("by hash");
    assert_eq!(rh, hash);
    assert_eq!(record.artifact_hash, hash);
    assert_eq!(
        record.registry_state_at_resolution,
        LifecycleState::StructurallyVerified
    );
    assert_eq!(record.resolving_event_key, "structurally_verified");
    assert!(record.selector_desc.starts_with("by-hash:"));
    assert_eq!(record.attestation_policy_version, "ap-1");
    assert_eq!(record.resolution_timestamp_unix, NOW);

    // Drive to published, set a tag, resolve by tag + by regime.
    drive_to_published(&backend, &hash);
    backend
        .put_pointer("prod", "current", &hash, "published@seq4")
        .expect("put pointer");

    let (rt, trec) = resolve(
        &backend,
        &Selector::ByTag {
            env: "prod".to_string(),
            tag: "current".to_string(),
        },
        NOW,
    )
    .expect("by tag");
    assert_eq!(rt, hash);
    assert_eq!(trec.registry_state_at_resolution, LifecycleState::Published);
    assert_eq!(trec.resolving_event_key, "published");
    assert_eq!(trec.selector_desc, "by-tag:prod/current");

    // ByRegime: effective date inside the manifest window (rule effective_from
    // 2024-06-30, open-ended), Published state.
    let (rr, rrec) = resolve(
        &backend,
        &Selector::ByRegime {
            regime_id: FIXTURE_REGIME.to_string(),
            effective: ke_core::ir::JurisdictionDate::new(2025, 1, 1),
            env: "prod".to_string(),
        },
        NOW,
    )
    .expect("by regime");
    assert_eq!(rr, hash);
    assert_eq!(rrec.registry_state_at_resolution, LifecycleState::Published);
    assert!(rrec
        .selector_desc
        .starts_with("by-regime:mica_2023@2025-01-01"));

    // A date BEFORE the effective window must not match (closed-open [from,to)).
    let before = resolve(
        &backend,
        &Selector::ByRegime {
            regime_id: FIXTURE_REGIME.to_string(),
            effective: ke_core::ir::JurisdictionDate::new(2000, 1, 1),
            env: "prod".to_string(),
        },
        NOW,
    );
    assert!(
        matches!(before, Err(RegistryError::NotFound { .. })),
        "pre-effective date must not resolve, got {before:?}"
    );

    // Unknown regime -> NotFound.
    let unknown = resolve(
        &backend,
        &Selector::ByRegime {
            regime_id: "no_such_regime".to_string(),
            effective: ke_core::ir::JurisdictionDate::new(2025, 1, 1),
            env: "prod".to_string(),
        },
        NOW,
    );
    assert!(matches!(unknown, Err(RegistryError::NotFound { .. })));

    // Missing tag -> NotFound.
    let missing = resolve(
        &backend,
        &Selector::ByTag {
            env: "prod".to_string(),
            tag: "absent".to_string(),
        },
        NOW,
    );
    assert!(matches!(missing, Err(RegistryError::NotFound { .. })));
}

// ---- (f) rollback-eligibility predicate -----------------------------------

#[test]
fn rollback_eligibility_is_published_only() {
    assert!(is_rollback_eligible(LifecycleState::Published));
    for s in [
        LifecycleState::Draft,
        LifecycleState::StructurallyVerified,
        LifecycleState::MlChecked,
        LifecycleState::ExpertAttested,
        LifecycleState::Deprecated,
        LifecycleState::Revoked,
    ] {
        assert!(
            !is_rollback_eligible(s),
            "{s:?} must not be rollback-eligible"
        );
    }
}

// ---- (g) bad-sig event rejected -------------------------------------------

#[test]
fn bad_signature_event_is_rejected() {
    let tmp = TempDir::new("badsig");
    let (backend, hash) = compile_into(&tmp);
    let mut events = backend.read_events(&hash).expect("read events");

    // Corrupt the signature bytes of the head event (without touching the
    // payload): the registry-root verification must fail.
    let last = events.last_mut().unwrap();
    last.signature[0] ^= 0xff;
    let err = current_state(&events).expect_err("bad sig must be rejected");
    assert!(
        matches!(err, RegistryError::SignatureInvalid { .. }),
        "expected SignatureInvalid, got {err:?}"
    );

    // An event signed by a non-registry key is also rejected: build a draft
    // event then re-sign its payload with the WRONG key.
    let ts = MockTsa::stamp(&hash, NOW);
    let mut forged = build_draft_event(hash, test_keys::REGISTRY_ROOT_KEY_ID, ts).expect("draft");
    // Re-sign with the compiler key (wrong authority for events).
    use ed25519_dalek::Signer;
    let prefix = forged.payload_prefix().expect("prefix");
    forged.signature = ke_artifact::sign::test_keys::signing_key()
        .sign(&prefix)
        .to_bytes();
    assert!(
        matches!(
            forged.verify_signature(),
            Err(RegistryError::SignatureInvalid { .. })
        ),
        "an event signed by a non-registry key must be rejected"
    );
}

// ---- (h) canonical-event-head-hash pin ------------------------------------

/// The blake3 of the highest-seq event's canonical bytes (signature included)
/// for the fixed-key, fixed-NOW compile of `mica_stablecoin.yaml`. This locks
/// the event encoding + signing scheme: any accidental shape/field-order change
/// flips this constant and fails the test.
///
/// Value computed during the first run and hardcoded (see the assert message if
/// it ever needs re-pinning after an intentional change).
const PINNED_HEAD_CHAIN_HASH_HEX: &str =
    "3ded38e468316b59cf8afe2cd46fe36bb13632ca2b159085324dd3102282ce3e";

#[test]
fn canonical_event_head_hash_is_pinned() {
    let tmp = TempDir::new("pin");
    let (backend, hash) = compile_into(&tmp);
    let events = backend.read_events(&hash).expect("read events");
    let head = events.last().expect("has head event");
    let head_hash = head.chain_hash().expect("chain hash");
    let hex = ke_cli::registry::hash_hex(&head_hash);
    assert_eq!(
        hex, PINNED_HEAD_CHAIN_HASH_HEX,
        "canonical-event-head-hash changed — if intentional, re-pin to: {hex}"
    );
}

// ---- determinism: re-compile -> byte-identical head chain hash ------------

#[test]
fn recompiling_is_deterministic() {
    let a = TempDir::new("det-a");
    let b = TempDir::new("det-b");
    let (ba, ha) = compile_into(&a);
    let (bb, hb) = compile_into(&b);
    assert_eq!(ha, hb, "same input + keys -> same content hash");
    let head_a = ba
        .read_events(&ha)
        .unwrap()
        .last()
        .unwrap()
        .chain_hash()
        .unwrap();
    let head_b = bb
        .read_events(&hb)
        .unwrap()
        .last()
        .unwrap()
        .chain_hash()
        .unwrap();
    assert_eq!(head_a, head_b, "deterministic event chain head");
}
