//! `ke sql --query <sql>`: READ-ONLY SQL over registry/artifact metadata
//! (Gate 5, workstream 5b SQL views; acceptance criterion **G5-3**).
//!
//! NON-AUTHORITATIVE and read-only. This surface never signs, attests,
//! publishes, revokes, deprecates, transitions lifecycle state, or appends
//! events. It opens an **in-memory** DuckDB connection, registers an
//! `artifacts` table whose rows are a **projection of canonical contents**, runs
//! the caller's query, and returns the result rows as JSON. The registry on disk
//! is **read** (every stored `.kew` is decoded and its event log walked), never
//! mutated.
//!
//! # Rows == canonical artifact contents (G5-3)
//!
//! Each row is built by decoding the stored `.kew` via
//! [`ke_artifact::decode_artifact`] and deriving the lifecycle state from the
//! event log via the existing [`current_state`](crate::registry::current_state)
//! (the same read path `export-provenance` uses). The metadata columns are the
//! RECON `metadataFields` set, sourced from the [`Manifest`], the event-log-
//! derived lifecycle state, and the [`Artifact::compiler_signature`] /
//! `attestations` — **never re-derived** and never re-encoded. The SQL layer is
//! a view *over* canonical bytes, not a parallel source of truth.
//!
//! # Feature-gated-defer (probe verdict — authoritative for this workstream)
//!
//! The DuckDB probe is unambiguous: on the `1.85.0-x86_64-pc-windows-gnu`
//! toolchain there is **no C/C++ compiler on PATH** (`g++.exe` not found), so
//! `libduckdb-sys`'s `cc-rs` build script fails (`ToolNotFound`, `EXITCODE=101`)
//! even after pinning around every Rust-version / getrandom obstacle. Native
//! DuckDB does **not** build here. So this command ships **feature-gated,
//! default-off** behind `sql-views`:
//!
//! - The [`Sql`](crate::cli::Command::Sql) CLI variant is **always** present
//!   (so `ke --help` lists it).
//! - With `--features sql-views` (built on the Linux CI leg, where a C++
//!   toolchain exists), [`run`] opens DuckDB and answers the query.
//! - On the default windows-gnu build, [`run`] is a stub returning a typed
//!   "requires --features sql-views" error — the same gating pattern
//!   `compile`/`import` use for `test-keys`. The default build links **no**
//!   `duckdb`.

use crate::registry::backend::RegistryBackend;
use anyhow::Result;

/// Arguments for `ke sql`.
pub struct SqlArgs<'a> {
    /// Read-only SQL query against the registry metadata views.
    pub query: &'a str,
}

/// Outcome of an `ke sql` run.
pub struct SqlOutcome {
    /// Canonical JSON array of result rows (one object per row).
    pub rows_json: String,
}

/// The RECON `metadataFields` projection of one artifact — a single row of the
/// `artifacts` view. Built purely from canonical contents (the decoded
/// [`Artifact`](ke_artifact::Artifact) + the event-log-derived lifecycle state),
/// never re-derived and never re-encoded.
///
/// This struct (and [`project_rows`]) are feature-independent so the projection
/// logic is testable and reviewable without DuckDB; only the DuckDB binding in
/// [`run`] is gated.
#[derive(Clone, Debug)]
pub struct ArtifactRow {
    pub artifact_hash: String,
    pub regime_id: String,
    pub artifact_kind: String,
    pub compiler_version: String,
    pub ir_schema_version: String,
    pub codec_version: String,
    pub canonicalization_version: String,
    pub effective_from: String,
    pub effective_to: Option<String>,
    pub corpus_root_hash: String,
    pub source_corpus_hash: String,
    pub attestation_policy_version: String,
    /// Event-log-derived lifecycle state (e.g. `published`); `unknown` if the
    /// event log is empty.
    pub lifecycle_state: String,
    pub signer_key_id: String,
    /// JSON array string of attestation types present on the artifact, in
    /// canonical order (e.g. `["SourceFidelity","Interpretation"]`).
    pub attestations: String,
    /// The registry event-head chain hash as of this read; zero if no events.
    pub registry_event_head_hash: String,
    /// The unix time this read was taken (caller-supplied at the CLI edge via
    /// the query's run; here it mirrors the export-time semantics).
    pub exported_at_unix: i64,
}

/// Lowercase hex of a 32-byte hash.
fn hex32(bytes: &[u8; 32]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::with_capacity(64), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// Render an `ArtifactKind` to its stable variant name.
fn artifact_kind_name(kind: ke_core::manifest::ArtifactKind) -> &'static str {
    use ke_core::manifest::ArtifactKind::*;
    match kind {
        RegimePack => "RegimePack",
        EquivalenceMatrix => "EquivalenceMatrix",
        TestCorpus => "TestCorpus",
        PolicyBundle => "PolicyBundle",
    }
}

/// Render a `JurisdictionDate` as `YYYY-MM-DD`.
fn date_str(d: ke_core::ir::JurisdictionDate) -> String {
    format!("{:04}-{:02}-{:02}", d.year, d.month, d.day)
}

/// The event-log-derived lifecycle state string for an artifact (mirrors
/// [`LifecycleState::event_kind`](crate::registry::LifecycleState::event_kind)),
/// or `"unknown"` if the log is empty. Walks + validates the chain via
/// [`current_state`](crate::registry::current_state) — the same read path the
/// rest of the registry uses; an invalid chain surfaces as an error, never a
/// silently-trusted row.
fn lifecycle_state_str<B: RegistryBackend>(
    backend: &B,
    hash: &[u8; 32],
) -> Result<(String, [u8; 32])> {
    use crate::registry::{current_state, head_event};
    let events = backend.read_events(hash)?;
    let state = match current_state(&events)? {
        Some(s) => s.event_kind().to_string(),
        None => "unknown".to_string(),
    };
    let head = if events.is_empty() {
        [0u8; 32]
    } else {
        head_event(backend, hash)?.chain_hash()?
    };
    Ok((state, head))
}

/// Project every stored artifact into one [`ArtifactRow`] by **decoding the
/// canonical `.kew`** and deriving lifecycle state from the event log. Pure read
/// path: `read_artifact_kew` + `decode_artifact` + `current_state`. No
/// re-encode, no re-hash beyond the manifest's own self-reported hash, no
/// mutation. `exported_at_unix` stamps each row with the read time.
///
/// Feature-independent so the projection is unit-testable without DuckDB.
pub fn project_rows<B: RegistryBackend>(
    backend: &B,
    exported_at_unix: u64,
) -> Result<Vec<ArtifactRow>> {
    let manifests = backend.list_manifests()?;
    let mut rows = Vec::with_capacity(manifests.len());
    for (hash, manifest) in &manifests {
        // Decode the canonical .kew (source of truth) to read the compiler
        // signature + attestations, which list_manifests does not surface.
        let kew = backend.read_artifact_kew(hash)?;
        let (artifact, _envelope_len) = ke_artifact::decode_artifact(&kew)
            .map_err(|e| anyhow::anyhow!("decode artifact {}: {e}", hex32(hash)))?;

        let (lifecycle_state, registry_event_head_hash) = lifecycle_state_str(backend, hash)?;

        let attestation_types: Vec<String> = artifact
            .attestations
            .iter()
            .map(|a| format!("{:?}", a.attestation_type))
            .collect();
        let attestations = serde_json::to_string(&attestation_types)
            .map_err(|e| anyhow::anyhow!("encode attestations column: {e}"))?;

        rows.push(ArtifactRow {
            artifact_hash: hex32(&manifest.artifact_hash),
            regime_id: manifest.regime_id.clone(),
            artifact_kind: artifact_kind_name(manifest.artifact_kind).to_string(),
            compiler_version: format!(
                "{}.{}.{}",
                manifest.compiler_version.major,
                manifest.compiler_version.minor,
                manifest.compiler_version.patch
            ),
            ir_schema_version: manifest.ir_schema_version.to_string(),
            codec_version: manifest.codec_version.0.clone(),
            canonicalization_version: manifest.canonicalization_version.0.clone(),
            effective_from: date_str(manifest.effective_from),
            effective_to: manifest.effective_to.map(date_str),
            corpus_root_hash: hex32(&manifest.corpus_root_hash),
            source_corpus_hash: hex32(&manifest.source_corpus_hash),
            attestation_policy_version: manifest.attestation_policy_version.clone(),
            lifecycle_state,
            signer_key_id: artifact.compiler_signature.key_id.clone(),
            attestations,
            registry_event_head_hash: hex32(&registry_event_head_hash),
            exported_at_unix: exported_at_unix as i64,
        });
    }
    Ok(rows)
}

/// READ-ONLY. Opens an in-memory DuckDB, registers the registry/artifact
/// metadata as a read-only `artifacts` view projecting the canonical
/// Manifest/Artifact/event fields, runs `args.query`, and returns the result
/// rows as a canonical JSON array. Writes nothing, signs nothing, transitions
/// nothing. The DuckDB connection is in-memory; the registry on disk is read but
/// never mutated.
#[cfg(feature = "sql-views")]
pub fn run<B: RegistryBackend>(backend: &B, args: &SqlArgs<'_>) -> Result<SqlOutcome> {
    use duckdb::types::Value as DuckValue;
    use duckdb::Connection;

    // Build the canonical rows off-DuckDB (decode + event-log derive). The read
    // time stamps each row; sourced at this edge like export-provenance.
    let exported_at_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let rows = project_rows(backend, exported_at_unix)?;

    // In-memory connection: no file is opened, nothing on disk is written.
    let conn =
        Connection::open_in_memory().map_err(|e| anyhow::anyhow!("open in-memory duckdb: {e}"))?;

    // Create the read-only projection table and insert the canonical rows. The
    // column set is exactly the RECON metadataFields.
    conn.execute_batch(
        "CREATE TABLE artifacts (
            artifact_hash TEXT,
            regime_id TEXT,
            artifact_kind TEXT,
            compiler_version TEXT,
            ir_schema_version TEXT,
            codec_version TEXT,
            canonicalization_version TEXT,
            effective_from TEXT,
            effective_to TEXT,
            corpus_root_hash TEXT,
            source_corpus_hash TEXT,
            attestation_policy_version TEXT,
            lifecycle_state TEXT,
            signer_key_id TEXT,
            attestations TEXT,
            registry_event_head_hash TEXT,
            exported_at_unix BIGINT
        );",
    )
    .map_err(|e| anyhow::anyhow!("create artifacts view: {e}"))?;

    {
        let mut stmt = conn
            .prepare(
                "INSERT INTO artifacts VALUES \
                 (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
            )
            .map_err(|e| anyhow::anyhow!("prepare insert: {e}"))?;
        for row in &rows {
            stmt.execute(duckdb::params![
                row.artifact_hash,
                row.regime_id,
                row.artifact_kind,
                row.compiler_version,
                row.ir_schema_version,
                row.codec_version,
                row.canonicalization_version,
                row.effective_from,
                row.effective_to,
                row.corpus_root_hash,
                row.source_corpus_hash,
                row.attestation_policy_version,
                row.lifecycle_state,
                row.signer_key_id,
                row.attestations,
                row.registry_event_head_hash,
                row.exported_at_unix,
            ])
            .map_err(|e| anyhow::anyhow!("insert artifact row: {e}"))?;
        }
    }

    // Run the caller's query and serialize the result set to JSON.
    let mut stmt = conn
        .prepare(args.query)
        .map_err(|e| anyhow::anyhow!("prepare query: {e}"))?;
    let column_names: Vec<String> = stmt.column_names().to_vec();
    let mut json_rows: Vec<serde_json::Map<String, serde_json::Value>> = Vec::new();
    let mut duck_rows = stmt
        .query([])
        .map_err(|e| anyhow::anyhow!("run query: {e}"))?;
    while let Some(r) = duck_rows
        .next()
        .map_err(|e| anyhow::anyhow!("read query row: {e}"))?
    {
        let mut obj = serde_json::Map::new();
        for (i, name) in column_names.iter().enumerate() {
            let v: DuckValue = r
                .get(i)
                .map_err(|e| anyhow::anyhow!("read column {name}: {e}"))?;
            obj.insert(name.clone(), duck_value_to_json(&v));
        }
        json_rows.push(obj);
    }

    let rows_json = serde_json::to_string(&json_rows)
        .map_err(|e| anyhow::anyhow!("serialize result rows: {e}"))?;
    Ok(SqlOutcome { rows_json })
}

/// Convert a DuckDB value to a JSON value for the result projection. Only the
/// types our `artifacts` view produces (text, bigint, null) need precise
/// handling; anything else falls back to its display form so an arbitrary query
/// (e.g. `COUNT(*)`) still serializes.
#[cfg(feature = "sql-views")]
fn duck_value_to_json(v: &duckdb::types::Value) -> serde_json::Value {
    use duckdb::types::Value;
    match v {
        Value::Null => serde_json::Value::Null,
        Value::Boolean(b) => serde_json::Value::Bool(*b),
        Value::Text(s) => serde_json::Value::String(s.clone()),
        Value::BigInt(n) => serde_json::Value::Number((*n).into()),
        Value::Int(n) => serde_json::Value::Number((*n as i64).into()),
        Value::UBigInt(n) => serde_json::Value::Number((*n).into()),
        Value::SmallInt(n) => serde_json::Value::Number((*n as i64).into()),
        Value::TinyInt(n) => serde_json::Value::Number((*n as i64).into()),
        other => serde_json::Value::String(format!("{other:?}")),
    }
}

/// Without the `sql-views` feature the `duckdb` dependency is not linked (it
/// cannot build on the windows-gnu toolchain: no C++ compiler for
/// `libduckdb-sys`). Returns a typed error rather than silently doing nothing —
/// mirrors `compile`/`import` under `test-keys`. The `Sql` CLI variant still
/// parses so `ke --help` lists it; only this `run` body is gated.
#[cfg(not(feature = "sql-views"))]
pub fn run<B: RegistryBackend>(_backend: &B, _args: &SqlArgs<'_>) -> Result<SqlOutcome> {
    anyhow::bail!(
        "`ke sql` requires --features sql-views (blocked on windows-gnu: no C++ toolchain for \
         libduckdb-sys — build on the Linux CI leg). The read-only SQL views over registry \
         metadata are feature-gated, default-off (Gate 5 workstream 5b)."
    )
}
