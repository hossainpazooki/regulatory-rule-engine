//! `ke-cli` library: the registry core (Gate 4 Phase 3a) plus the `compile` /
//! `query` command implementations and the clap CLI surface.
//!
//! The `ke` binary ([`crate::cli::run`]) is a thin dispatcher over this
//! library; the integration tests under `tests/` drive [`registry`] and
//! [`commands`] directly. See `docs/spec/ke-workbench-rust-migration-spec-v3.1.md`
//! § 9, § 16, § 18 and ADR 0012/0013/0014.
//!
//! Authority boundaries (CLAUDE.md): the compiler signs structural validity
//! only; the registry-root key signs lifecycle events; no AI/LLM code
//! participates in any path here. Local-FS registry objects are
//! non-authoritative (ADR 0012 §6). No async, no tokio, no AWS SDK.

#![deny(unsafe_code)]

pub mod cli;
pub mod commands;
pub mod registry;
