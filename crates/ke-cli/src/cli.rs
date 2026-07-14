//! The clap v4 (derive) CLI surface for `ke` and its dispatcher.
//!
//! Global options:
//! - `--registry <dir>` (or `KE_REGISTRY_BACKEND=local` + `KE_REGISTRY_DIR`):
//!   the local-FS registry root. Phase 3a has one backend (local); the flag is
//!   the seam an S3 backend slots behind later.
//! - `--now <unix>` (or `KE_NOW`): the deterministic clock. Falls back to the
//!   system clock **only at this CLI edge** — the registry core is clock-free
//!   (plan §4).
//!
//! Implemented subcommands: `compile`, `query` (Phase 3a) and the Phase-3b
//! lifecycle set `ml-check`, `attest`, `publish`, `deprecate`, `revoke`,
//! `rollback`. Without the `test-keys` feature the signing commands return a
//! typed "requires --features test-keys" error (the command surface still
//! parses). `verify` remains a deferred exit-2 stub (standalone attestation-set
//! verification is a later surface). `lint` (Gate 5) runs the non-authoritative
//! T5 lint tier; it needs no registry and no `test-keys` (it signs nothing).

use crate::commands::{
    attest, compile, deprecate, export, export_provenance, graph_export, import_kew, lint,
    ml_check, publish, query, revoke, rollback, sql,
};
use crate::registry::backend::LocalFsBackend;
use crate::registry::Selector;
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

/// ke-workbench command-line — Gate 4 registry + verification.
///
/// Implemented phases (shown per command in `[Gate 4 · Phase N]` tags below):
/// 3a = compile/query; 3b = the lifecycle commands (ml-check/attest/publish/
/// deprecate/revoke/rollback); 4a = export-provenance. `serve` (Gate 5) is the
/// thin, non-authoritative read/preview HTTP adapter (resolve/verify/preview/
/// dry-run + a read-only event feed); it signs/attests/publishes NOTHING — the
/// authoritative compile path stays on `ke compile`.
#[derive(Parser, Debug)]
#[command(name = "ke", version, about, long_about = None)]
pub struct Cli {
    /// Local-FS registry root directory. Overrides `KE_REGISTRY_DIR`.
    #[arg(long, global = true)]
    pub registry: Option<String>,

    /// Deterministic clock, unix seconds. Overrides `KE_NOW`; falls back to the
    /// system clock at this edge only.
    #[arg(long, global = true)]
    pub now: Option<u64>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// [Gate 4 · Phase 3a] Compile a YAML rule document into a signed artifact
    /// and record its draft / structurally_verified lifecycle events.
    Compile {
        /// Path to the YAML rule document.
        yaml: String,
        /// Regime id recorded on the synthetic manifest.
        #[arg(long)]
        regime: String,
        /// Named environment (default `local`).
        #[arg(long, default_value = "local")]
        env: String,
    },
    /// [Gate 4 · ADR-0021] Author an IntentSpec artifact from a JSON document
    /// (action_class + criteria + idempotency), signing and recording its
    /// draft / structurally_verified lifecycle on the same test-keys path as
    /// `compile`. The input is JSON IntentSpec, not rule YAML.
    #[command(name = "compile-intent")]
    CompileIntent {
        /// Path to the JSON IntentSpec document.
        json: String,
        /// Regime id recorded on the synthetic manifest.
        #[arg(long)]
        regime: String,
        /// Named environment (default `local`).
        #[arg(long, default_value = "local")]
        env: String,
    },
    /// [Gate 4 · Phase 3a] Resolve an artifact and print its hash, current
    /// state, and §18 record.
    Query {
        /// Resolve a specific content hash (64-char lowercase hex).
        #[arg(long, conflicts_with_all = ["tag", "regime"])]
        hash: Option<String>,
        /// Resolve a tag, given as `<env>/<tag>`.
        #[arg(long, conflicts_with_all = ["hash", "regime"])]
        tag: Option<String>,
        /// Resolve a regime's published artifact effective on `--effective`.
        #[arg(long, requires = "effective")]
        regime: Option<String>,
        /// Effective date `YYYY-MM-DD` for `--regime`.
        #[arg(long)]
        effective: Option<String>,
        /// Environment for `--regime` (default `local`).
        #[arg(long, default_value = "local")]
        env: String,
    },
    /// [Gate 4 · Phase 3b] Dev stand-in T2/T3 step: write a non-authoritative
    /// consistency-block sidecar and move structurally_verified -> ml_checked.
    #[command(name = "ml-check")]
    MlCheck {
        /// Artifact content hash (64-char lowercase hex).
        #[arg(long)]
        hash: String,
    },
    /// [Gate 4 · Phase 3b] Sign typed expert attestations and move
    /// ml_checked -> expert_attested.
    Attest {
        /// Artifact content hash (64-char lowercase hex).
        #[arg(long)]
        hash: String,
        /// Attestation type (repeatable): source_fidelity | interpretation |
        /// scenario_coverage | equivalence_claim | publication_approval.
        #[arg(long = "type", value_name = "TYPE", required = true)]
        types: Vec<String>,
    },
    /// [Gate 4 · Phase 3b] Transition an expert-attested artifact to published
    /// and set its tag.
    Publish {
        /// Artifact content hash (64-char lowercase hex).
        #[arg(long)]
        hash: String,
        /// Named environment the tag pointer is written under.
        #[arg(long)]
        env: String,
        /// Tag to move (default `current`).
        #[arg(long, default_value = "current")]
        tag: String,
        /// Optional PolicyBundle JSON; default is the strict built-in policy.
        #[arg(long)]
        policy: Option<String>,
    },
    /// [Gate 4 · Phase 3b] Deprecate a published artifact.
    Deprecate {
        /// Artifact content hash (64-char lowercase hex).
        #[arg(long)]
        hash: String,
    },
    /// [Gate 4 · Phase 3b] Revoke a published or deprecated artifact; records
    /// (not enforces) policy.
    Revoke {
        /// Artifact content hash (64-char lowercase hex).
        #[arg(long)]
        hash: String,
        /// Recorded revocation policy: hardstop | finishpinned | auditonly.
        #[arg(long)]
        policy: String,
        /// Optional human reason recorded in the sidecar.
        #[arg(long)]
        reason: Option<String>,
    },
    /// [Gate 4 · Phase 3b] Roll a tag back to a prior published artifact (ADR 0013).
    Rollback {
        /// Named environment the tag pointer lives under.
        #[arg(long)]
        env: String,
        /// Tag to move (default `current`).
        #[arg(long, default_value = "current")]
        tag: String,
        /// Rollback target's content hash (64-char lowercase hex).
        #[arg(long = "to")]
        to: String,
    },
    /// [Gate 4 · Phase 4a] Export the consumer-agnostic provenance JSON for an
    /// artifact: regime, content hash, version triplet, signer key
    /// (+ is-test-key), attestation summaries, and the registry state +
    /// event-head hash as-of-export (ADR 0016). Reads the registry; signs
    /// nothing (no `test-keys` needed).
    #[command(name = "export-provenance")]
    ExportProvenance {
        /// Artifact content hash (64-char lowercase hex).
        #[arg(long)]
        hash: String,
        /// Also write `<registry>/artifacts/<hash>/provenance.json`.
        #[arg(long)]
        write: bool,
    },
    /// [Gate 4 · Phase 4b — deferred] Standalone verification of an artifact's
    /// attestation set.
    Verify,
    /// [Gate 5] Serve a thin, NON-AUTHORITATIVE HTTP read/preview adapter over
    /// the canonical registry: resolve/verify (canonical registry view, G5-1),
    /// compile/preview + dry-run (non-authoritative), and a read-only SSE event
    /// feed. Signs, attests, publishes, revokes, and assembles NOTHING — the
    /// authoritative compile path stays on `ke compile` (CLAUDE.md §5/§10/§13).
    Serve {
        /// Bind host (default `127.0.0.1`; loopback only).
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        /// Bind port. `0` (default) picks an ephemeral port — read the actual
        /// bound port from the log line / `server_addr()` (tests bind `0`).
        #[arg(long, default_value_t = 0)]
        port: u16,
    },
    /// [Gate 5] Lint a YAML rule document with the T5 lint tier (advisory by
    /// default). NON-AUTHORITATIVE: compiles to IR and reports style/quality
    /// findings; signs nothing, writes nothing, touches no registry.
    Lint {
        /// Path to the YAML rule document.
        yaml: String,
        /// Exit nonzero (2) if any blocking T5 finding fires.
        #[arg(long)]
        deny: bool,
    },
    /// [Gate 5] Export an artifact's `.kew` to a flat file (byte-identical copy).
    /// NON-AUTHORITATIVE: reads stored bytes, re-hashes nothing, signs nothing.
    Export {
        /// Artifact content hash (64-char lowercase hex).
        #[arg(long)]
        hash: String,
        /// Destination path for the flat `.kew` file.
        #[arg(long)]
        out: String,
    },
    /// [Gate 5] OFFLINE import: re-verify a flat `.kew` file's hash + signatures
    /// via ke_artifact::verify_artifact and reject anything that does not verify.
    /// NON-AUTHORITATIVE: trusts nothing blindly; signs/publishes nothing.
    Import {
        /// Path to the flat `.kew` file to re-verify offline.
        #[arg(long = "in")]
        in_path: String,
    },
    /// [Gate 5] Run a READ-ONLY SQL query over registry/artifact metadata views
    /// (G5-3). NON-AUTHORITATIVE and read-only. Requires `--features sql-views`
    /// (blocked on the windows-gnu toolchain: no C++ compiler for libduckdb-sys
    /// — built on the Linux CI leg).
    Sql {
        /// The read-only SQL query.
        #[arg(long)]
        query: String,
    },
    /// [ADR-0023] Derived, read-only graph view over the verified registry.
    /// A consumer under ADR-0019 discipline: verified + published artifacts
    /// only, fail-closed, re-addressed. NEVER an input to a decision — the
    /// tree-walk engine over signed artifacts stays the only decision path.
    Graph {
        #[command(subcommand)]
        action: GraphAction,
    },
}

/// `ke graph` subactions (ADR-0023).
#[derive(Subcommand, Debug)]
pub enum GraphAction {
    /// Export the verified+published registry as an idempotent Cypher script
    /// (`graph.cypher`) plus the refusal log (`refusals.log` — what is NOT in
    /// the graph, and why).
    Export {
        /// Output directory.
        #[arg(long)]
        out: String,
        /// Policy environment for the verification context (mock-TSA-stamped
        /// test attestations verify only under `local`, R8).
        #[arg(long, default_value = "local")]
        env: String,
    },
    /// Rust-side differential oracle: blast radius of amending a document
    /// (optionally a single article). Prints sorted node ids, one per line.
    #[command(name = "oracle-blast")]
    OracleBlast {
        /// Document id (CorpusDoc node).
        #[arg(long)]
        doc: String,
        /// Restrict to citations of one article.
        #[arg(long)]
        article: Option<String>,
        /// Policy environment for the verification context.
        #[arg(long, default_value = "local")]
        env: String,
    },
    /// Rust-side differential oracle: cross-regime conflict exposure (citation
    /// closure from `--from`, one CONFLICTS_WITH hop into `--to`). Prints
    /// sorted rule node ids, one per line.
    #[command(name = "oracle-exposure")]
    OracleExposure {
        /// Source regime id.
        #[arg(long)]
        from: String,
        /// Target regime id.
        #[arg(long)]
        to: String,
        /// Policy environment for the verification context.
        #[arg(long, default_value = "local")]
        env: String,
    },
}

/// Resolve the registry root from `--registry`, else `KE_REGISTRY_DIR` (when
/// `KE_REGISTRY_BACKEND=local`), erroring if neither is set.
fn registry_root(cli: &Cli) -> Result<String> {
    if let Some(dir) = &cli.registry {
        return Ok(dir.clone());
    }
    let backend = std::env::var("KE_REGISTRY_BACKEND").unwrap_or_default();
    if backend == "local" {
        if let Ok(dir) = std::env::var("KE_REGISTRY_DIR") {
            return Ok(dir);
        }
    }
    anyhow::bail!(
        "no registry selected: pass --registry <dir> or set \
         KE_REGISTRY_BACKEND=local + KE_REGISTRY_DIR=<dir>"
    )
}

/// Resolve the clock from `--now`, else `KE_NOW`, else the system clock (this
/// CLI edge is the only place a clock is read; the registry core is clock-free).
fn now_unix(cli: &Cli) -> Result<u64> {
    if let Some(now) = cli.now {
        return Ok(now);
    }
    if let Ok(now) = std::env::var("KE_NOW") {
        return now
            .parse::<u64>()
            .with_context(|| format!("KE_NOW must be unix seconds, got {now:?}"));
    }
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Ok(secs)
}

/// Parse `YYYY-MM-DD` into a [`ke_core::ir::JurisdictionDate`].
fn parse_date(s: &str) -> Result<ke_core::ir::JurisdictionDate> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 {
        anyhow::bail!("effective date must be YYYY-MM-DD, got {s:?}");
    }
    let year: i16 = parts[0].parse().with_context(|| format!("year in {s:?}"))?;
    let month: u8 = parts[1]
        .parse()
        .with_context(|| format!("month in {s:?}"))?;
    let day: u8 = parts[2].parse().with_context(|| format!("day in {s:?}"))?;
    Ok(ke_core::ir::JurisdictionDate::new(year, month, day))
}

/// Build a [`Selector`] from the parsed `query` flags.
fn query_selector(
    hash: &Option<String>,
    tag: &Option<String>,
    regime: &Option<String>,
    effective: &Option<String>,
    env: &str,
) -> Result<Selector> {
    if let Some(hash) = hash {
        return Ok(Selector::ByHash(crate::registry::hash_from_hex(hash)?));
    }
    if let Some(tag) = tag {
        let (tenv, ttag) = tag
            .split_once('/')
            .ok_or_else(|| anyhow::anyhow!("--tag must be <env>/<tag>, got {tag:?}"))?;
        return Ok(Selector::ByTag {
            env: tenv.to_string(),
            tag: ttag.to_string(),
        });
    }
    if let Some(regime) = regime {
        let effective = effective
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("--regime requires --effective YYYY-MM-DD"))?;
        return Ok(Selector::ByRegime {
            regime_id: regime.clone(),
            effective: parse_date(effective)?,
            env: env.to_string(),
        });
    }
    anyhow::bail!("`ke query` needs one of --hash, --tag, or --regime --effective")
}

/// Parse argv, dispatch, and return a process exit code. `verify` prints a
/// deferred-surface message and exits 2; errors print to stderr and exit 1;
/// success exits 0.
pub fn run() -> i32 {
    let cli = Cli::parse();
    match dispatch(&cli) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("error: {err:#}");
            1
        }
    }
}

fn dispatch(cli: &Cli) -> Result<i32> {
    match &cli.command {
        Command::Compile { yaml, regime, env } => {
            let root = registry_root(cli)?;
            let now = now_unix(cli)?;
            let backend = LocalFsBackend::open(root)?;
            let args = compile::CompileArgs {
                yaml_path: yaml,
                regime_id: regime,
                env,
                now_unix: now,
            };
            let outcome = compile::run(&backend, &args)?;
            println!(
                "compiled: hash={} state={:?}",
                crate::registry::hash_hex(&outcome.artifact_hash),
                outcome.final_state
            );
            Ok(0)
        }
        Command::CompileIntent { json, regime, env } => {
            let root = registry_root(cli)?;
            let now = now_unix(cli)?;
            let backend = LocalFsBackend::open(root)?;
            let args = compile::IntentSpecArgs {
                json_path: json,
                regime_id: regime,
                env,
                now_unix: now,
            };
            let outcome = compile::run_intent_spec(&backend, &args)?;
            println!(
                "compiled: hash={} state={:?}",
                crate::registry::hash_hex(&outcome.artifact_hash),
                outcome.final_state
            );
            Ok(0)
        }
        Command::Query {
            hash,
            tag,
            regime,
            effective,
            env,
        } => {
            let root = registry_root(cli)?;
            let now = now_unix(cli)?;
            let backend = LocalFsBackend::open(root)?;
            let selector = query_selector(hash, tag, regime, effective, env)?;
            let record = query::run(&backend, &selector, now)?;
            print!("{}", query::render(&record));
            Ok(0)
        }
        Command::MlCheck { hash } => {
            let backend = open_backend(cli)?;
            let now = now_unix(cli)?;
            let args = ml_check::MlCheckArgs {
                artifact_hash: crate::registry::hash_from_hex(hash)?,
                now_unix: now,
            };
            let outcome = ml_check::run(&backend, &args)?;
            println!("ml-check: hash={} state={:?}", hash, outcome.final_state);
            Ok(0)
        }
        Command::Attest { hash, types } => {
            let backend = open_backend(cli)?;
            let now = now_unix(cli)?;
            let parsed: Vec<ke_core::manifest::AttestationType> = types
                .iter()
                .map(|t| attest::parse_attestation_type(t))
                .collect::<Result<_>>()?;
            let args = attest::AttestArgs {
                artifact_hash: crate::registry::hash_from_hex(hash)?,
                types: &parsed,
                now_unix: now,
            };
            let outcome = attest::run(&backend, &args)?;
            println!(
                "attest: hash={} state={:?} attestations={}",
                hash, outcome.final_state, outcome.attestation_count
            );
            Ok(0)
        }
        Command::Publish {
            hash,
            env,
            tag,
            policy,
        } => {
            let backend = open_backend(cli)?;
            let now = now_unix(cli)?;
            let args = publish::PublishArgs {
                artifact_hash: crate::registry::hash_from_hex(hash)?,
                env,
                tag,
                policy_path: policy.as_deref(),
                now_unix: now,
            };
            let outcome = publish::run(&backend, &args)?;
            println!(
                "publish: hash={} state={:?} tag={}",
                hash, outcome.final_state, outcome.tag_ref
            );
            Ok(0)
        }
        Command::Deprecate { hash } => {
            let backend = open_backend(cli)?;
            let now = now_unix(cli)?;
            let args = deprecate::DeprecateArgs {
                artifact_hash: crate::registry::hash_from_hex(hash)?,
                now_unix: now,
            };
            let outcome = deprecate::run(&backend, &args)?;
            println!("deprecate: hash={} state={:?}", hash, outcome.final_state);
            Ok(0)
        }
        Command::Revoke {
            hash,
            policy,
            reason,
        } => {
            let backend = open_backend(cli)?;
            let now = now_unix(cli)?;
            let args = revoke::RevokeArgs {
                artifact_hash: crate::registry::hash_from_hex(hash)?,
                policy: revoke::parse_revocation_policy(policy)?,
                reason: reason.as_deref(),
                now_unix: now,
            };
            let outcome = revoke::run(&backend, &args)?;
            println!(
                "revoke: hash={} state={:?} severity={}",
                hash, outcome.final_state, outcome.severity
            );
            Ok(0)
        }
        Command::Rollback { env, tag, to } => {
            let backend = open_backend(cli)?;
            let now = now_unix(cli)?;
            let args = rollback::RollbackArgs {
                env,
                tag,
                to_hash: crate::registry::hash_from_hex(to)?,
                now_unix: now,
            };
            let outcome = rollback::run(&backend, &args)?;
            println!(
                "rollback: target={} state={:?} tag={}",
                to, outcome.target_state, outcome.tag_ref
            );
            Ok(0)
        }
        Command::ExportProvenance { hash, write } => {
            let root = registry_root(cli)?;
            let now = now_unix(cli)?;
            let backend = LocalFsBackend::open(&root)?;
            let args = export_provenance::ExportProvenanceArgs {
                artifact_hash: crate::registry::hash_from_hex(hash)?,
                exported_at_unix: now,
                write_root: if *write { Some(root.as_str()) } else { None },
            };
            let outcome = export_provenance::run(&backend, &args)?;
            println!("{}", outcome.canonical_json);
            Ok(0)
        }
        Command::Verify => {
            eprintln!(
                "`ke verify` (standalone attestation-set verification) is a deferred surface. \
                 Use `ke publish --policy ...` for the publish-time policy gate."
            );
            Ok(2)
        }
        Command::Serve { host, port } => {
            // serve reads the canonical registry; it sources its clock at the
            // request edge (handlers), not once here — keep `--now`/`KE_NOW`
            // honored per-request.
            let root = registry_root(cli)?;
            crate::serve::run(root, host, *port)?;
            Ok(0)
        }
        Command::Lint { yaml, deny } => {
            // NON-AUTHORITATIVE: no registry, no backend, no clock — lint only
            // compiles the YAML and runs the T5 tier.
            let args = lint::LintArgs {
                yaml_path: yaml,
                deny: *deny,
            };
            let outcome = lint::run(&args)?;
            for finding in &outcome.findings {
                println!(
                    "[{:?}] {} ({}): {}{}",
                    finding.tier,
                    finding.rule_id,
                    finding.code,
                    finding.message,
                    if finding.blocking { " [blocking]" } else { "" }
                );
            }
            println!(
                "lint: findings={} blocking={}",
                outcome.findings.len(),
                outcome.blocking_count
            );
            Ok(if *deny && outcome.blocking_count > 0 {
                2
            } else {
                0
            })
        }
        Command::Export { hash, out } => {
            // Reads the stored .kew and writes byte-identical bytes; no clock,
            // no signing, no lifecycle change.
            let root = registry_root(cli)?;
            let backend = LocalFsBackend::open(&root)?;
            let args = export::ExportArgs {
                artifact_hash: crate::registry::hash_from_hex(hash)?,
                out_path: out,
            };
            let outcome = export::run(&backend, &args)?;
            println!(
                "export: hash={} bytes={} out={}",
                crate::registry::hash_hex(&outcome.artifact_hash),
                outcome.bytes_written,
                outcome.out_path
            );
            Ok(0)
        }
        Command::Import { in_path } => {
            // OFFLINE: no backend read. Only the CLI-edge clock is needed for
            // verify_artifact. import_kew::run hard-rejects integrity failures
            // (Err -> exit 1); the expected offline NotPublished/StaleEventHead
            // pass through as crypto-OK with a warning. Without `test-keys`,
            // `run` returns a typed "requires --features test-keys" Err.
            let now = now_unix(cli)?;
            let args = import_kew::ImportArgs {
                kew_path: in_path,
                now_unix: now,
            };
            #[cfg(any(test, feature = "test-keys"))]
            {
                let outcome = import_kew::run(&args)?;
                print_import_outcome(&outcome);
                Ok(0)
            }
            #[cfg(not(any(test, feature = "test-keys")))]
            {
                let _ = import_kew::run(&args)?;
                Ok(0)
            }
        }
        Command::Sql { query } => {
            // READ-ONLY: opens the backend and runs an in-memory DuckDB query
            // over a projection of canonical artifact contents. Without
            // `--features sql-views`, `sql::run` returns a typed error (duckdb
            // is not linked on the default windows-gnu build).
            let backend = open_backend(cli)?;
            let outcome = sql::run(&backend, &sql::SqlArgs { query })?;
            println!("{}", outcome.rows_json);
            Ok(0)
        }
        Command::Graph { action } => {
            let backend = open_backend(cli)?;
            let now = now_unix(cli)?;
            let export = |env: &str| {
                graph_export::run(
                    &backend,
                    &graph_export::GraphExportArgs { env, now_unix: now },
                )
            };
            match action {
                GraphAction::Export { out, env } => {
                    let outcome = export(env)?;
                    graph_export::write_outputs(&outcome, out)?;
                    println!(
                        "graph export: {} artifacts exported, {} refused -> {out}",
                        outcome.exported.len(),
                        outcome.refusals.len()
                    );
                    for refusal in &outcome.refusals {
                        eprintln!("refused {}: {}", refusal.artifact_hex, refusal.reason);
                    }
                    Ok(0)
                }
                GraphAction::OracleBlast { doc, article, env } => {
                    let outcome = export(env)?;
                    for id in crate::graph::blast_radius(&outcome.graph, doc, article.as_deref()) {
                        println!("{id}");
                    }
                    Ok(0)
                }
                GraphAction::OracleExposure { from, to, env } => {
                    let outcome = export(env)?;
                    for id in crate::graph::conflict_exposure(&outcome.graph, from, to) {
                        println!("{id}");
                    }
                    Ok(0)
                }
            }
        }
    }
}

/// Print the import verdict + provenance summary. Gated with `run`: the
/// no-feature build's `run` returns a typed error before this is reached.
#[cfg(any(test, feature = "test-keys"))]
fn print_import_outcome(outcome: &import_kew::ImportOutcome) {
    use ke_artifact::{RejectionReason, Verdict};
    println!(
        "import: hash={} verdict={:?} registry_state={:?} signer_key={} is_test_key={}",
        crate::registry::hash_hex(&outcome.artifact_hash),
        outcome.verdict,
        outcome.registry_state,
        outcome.provenance.signer_key_id,
        outcome.provenance.is_test_key
    );
    match &outcome.verdict {
        Verdict::Verified => {}
        Verdict::Rejected(RejectionReason::NotPublished { .. }) => {
            eprintln!(
                "warning: crypto OK but not authoritative offline (no live registry; \
                 status is not Published). This is expected for `ke import`."
            );
        }
        Verdict::Rejected(RejectionReason::StaleEventHead { .. }) => {
            eprintln!(
                "warning: crypto OK but the embedded event-head is stale against the \
                 supplied live head."
            );
        }
        // Integrity failures never reach here — `run` returns Err for them.
        _ => {}
    }
}

/// Open the local-FS backend from the resolved registry root.
fn open_backend(cli: &Cli) -> Result<LocalFsBackend> {
    let root = registry_root(cli)?;
    Ok(LocalFsBackend::open(root)?)
}
