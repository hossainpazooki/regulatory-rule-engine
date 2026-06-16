//! Gate 5 (workstream 5b SQL views) integration test — acceptance **G5-3**:
//! SQL rows == canonical artifact contents.
//!
//! The whole file is guarded `#![cfg(feature = "sql-views")]` so it is **inert**
//! on the default windows-gnu build (where `duckdb`/`libduckdb-sys` cannot
//! compile — no C++ toolchain). It runs on the Linux CI leg
//! (`cargo test -p ke-cli --features sql-views` on ubuntu-latest), where a C++
//! compiler exists.
//!
//! Flow: compile a corpus YAML into a tempdir registry (the `sql-views` test
//! build also unifies `test-keys` via the dev-dependency, so `compile::run` can
//! sign), then run `sql::run` and assert the projected row matches the canonical
//! contents:
//!
//! 1. `artifact_hash` (hex) == the compiled hash;
//! 2. `regime_id` == the manifest's regime;
//! 3. `lifecycle_state` == the event-log-derived `current_state` (the same read
//!    path the registry uses) rendered to its `event_kind` string;
//! 4. `signer_key_id` == the compiler signature's key id (and is the loud
//!    fixed-seed test key).
//!
//! That is the G5-3 spot-check: the SQL projection is a view *over* canonical
//! bytes, never a parallel source of truth.
#![cfg(feature = "sql-views")]

use ke_cli::commands::{compile, sql};
use ke_cli::registry::backend::{LocalFsBackend, RegistryBackend};
use ke_cli::registry::{current_state, LifecycleState};
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
        let path = std::env::temp_dir().join(format!("ke-sql-views-test-{label}-{pid}-{n}"));
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

fn hex32(bytes: &[u8; 32]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::with_capacity(64), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

#[test]
fn sql_rows_equal_canonical_artifact_contents() {
    let tmp = TempDir::new("g5-3");
    let (backend, hash) = compile_into(&tmp);

    // Independent oracle: the event-log-derived lifecycle state, read straight
    // through the registry's own path (not the SQL layer).
    let events = backend.read_events(&hash).expect("read events");
    let oracle_state = current_state(&events)
        .expect("current_state")
        .expect("a recorded state")
        .event_kind()
        .to_string();
    // And the canonical signer key id, decoded from the stored .kew.
    let kew = backend.read_artifact_kew(&hash).expect("read kew");
    let (artifact, _) = ke_artifact::decode_artifact(&kew).expect("decode");
    let oracle_signer = artifact.compiler_signature.key_id.clone();

    let outcome = sql::run(
        &backend,
        &sql::SqlArgs {
            query: "SELECT artifact_hash, regime_id, lifecycle_state, signer_key_id \
                    FROM artifacts",
        },
    )
    .expect("sql run");

    let rows: Vec<serde_json::Value> = serde_json::from_str(&outcome.rows_json).expect("rows json");
    assert_eq!(
        rows.len(),
        1,
        "exactly one compiled artifact in the registry"
    );
    let row = &rows[0];

    assert_eq!(
        row["artifact_hash"].as_str().unwrap(),
        hex32(&hash),
        "SQL artifact_hash == canonical compiled hash (G5-3)"
    );
    assert_eq!(
        row["regime_id"].as_str().unwrap(),
        FIXTURE_REGIME,
        "SQL regime_id == manifest regime"
    );
    assert_eq!(
        row["lifecycle_state"].as_str().unwrap(),
        oracle_state,
        "SQL lifecycle_state == event-log-derived current_state"
    );
    assert_eq!(
        row["signer_key_id"].as_str().unwrap(),
        oracle_signer,
        "SQL signer_key_id == canonical compiler signature key id"
    );
    assert!(
        row["signer_key_id"].as_str().unwrap().starts_with("test-"),
        "the fixed-seed compiler key is loudly a test key"
    );
}

#[test]
fn sql_is_read_only_count_query() {
    let tmp = TempDir::new("count");
    let (backend, _hash) = compile_into(&tmp);
    let outcome = sql::run(
        &backend,
        &sql::SqlArgs {
            query: "SELECT COUNT(*) AS n FROM artifacts",
        },
    )
    .expect("sql count run");
    let rows: Vec<serde_json::Value> = serde_json::from_str(&outcome.rows_json).expect("rows json");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["n"].as_i64(), Some(1));
}
