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
//! verification is a later surface).

use crate::commands::{
    attest, compile, deprecate, export_provenance, ml_check, publish, query, revoke, rollback,
};
use crate::registry::backend::LocalFsBackend;
use crate::registry::Selector;
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

/// ke-workbench command-line — Gate 4 registry + verification.
///
/// Implemented phases (shown per command in `[Gate 4 · Phase N]` tags below):
/// 3a = compile/query; 3b = the lifecycle commands (ml-check/attest/publish/
/// deprecate/revoke/rollback); 4a = export-provenance. `serve` is Gate 5.
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
    }
}

/// Open the local-FS backend from the resolved registry root.
fn open_backend(cli: &Cli) -> Result<LocalFsBackend> {
    let root = registry_root(cli)?;
    Ok(LocalFsBackend::open(root)?)
}
