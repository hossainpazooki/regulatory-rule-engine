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
//! Implemented subcommands: `compile`, `query`. Declared-but-deferred (exit 2
//! with a Phase-3b message so the surface is visible): `verify`, `attest`,
//! `publish`, `deprecate`, `revoke`, `rollback`.

use crate::commands::{compile, query};
use crate::registry::backend::LocalFsBackend;
use crate::registry::Selector;
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

/// ke-workbench command-line (Gate 4 Phase 3a: registry compile/query).
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
    /// Compile a YAML rule document into a signed artifact and record its
    /// draft / structurally_verified lifecycle events.
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
    /// Resolve an artifact and print its hash, current state, and §18 record.
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
    /// Phase 3b: verify an artifact's attestation set against a policy.
    Verify,
    /// Phase 3b: sign a typed expert attestation bound to an artifact hash.
    Attest,
    /// Phase 3b: transition an expert-attested artifact to published.
    Publish,
    /// Phase 3b: deprecate a published artifact.
    Deprecate,
    /// Phase 3b: revoke a published or deprecated artifact.
    Revoke,
    /// Phase 3b: roll a tag back to a prior published artifact.
    Rollback,
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

/// Parse argv, dispatch, and return a process exit code. `verify`/`attest`/
/// `publish`/`deprecate`/`revoke`/`rollback` print a Phase-3b message and exit
/// 2; errors print to stderr and exit 1; success exits 0.
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
        Command::Verify
        | Command::Attest
        | Command::Publish
        | Command::Deprecate
        | Command::Revoke
        | Command::Rollback => {
            eprintln!(
                "`ke {}` is Phase 3b (attest/publish/deprecate/revoke/rollback + \
                 revocation-policy behavior). Not available in Phase 3a.",
                phase_3b_name(&cli.command)
            );
            Ok(2)
        }
    }
}

fn phase_3b_name(command: &Command) -> &'static str {
    match command {
        Command::Verify => "verify",
        Command::Attest => "attest",
        Command::Publish => "publish",
        Command::Deprecate => "deprecate",
        Command::Revoke => "revoke",
        Command::Rollback => "rollback",
        _ => "?",
    }
}
