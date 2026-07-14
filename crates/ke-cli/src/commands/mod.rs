//! `ke` subcommand implementations.
//!
//! Phase 3a: [`compile`] (`draft` -> `structurally_verified`) and [`query`]
//! (resolution). Phase 3b drives the rest of the § 9 lifecycle:
//! [`ml_check`] (dev stand-in `structurally_verified` -> `ml_checked`),
//! [`attest`] (`ml_checked` -> `expert_attested`), [`publish`]
//! (`expert_attested` -> `published`, tag pointer), [`deprecate`], [`revoke`]
//! (+ revocation-policy sidecar), and [`rollback`] (ADR 0013 tag move).
//!
//! Every command that signs (events or attestations) is gated behind the
//! `test-keys` feature; a no-feature build keeps each `run` a typed
//! "requires --features test-keys" error (see [`crate::cli`]). Lifecycle events
//! stay registry-root-signed (`SignerRole::Registry`); the triggering authority
//! is recorded, never the signer.

pub mod attest;
pub mod compile;
pub mod deprecate;
pub mod export;
pub mod export_provenance;
pub mod graph_export;
pub mod import_kew;
pub mod lint;
pub mod ml_check;
pub mod publish;
pub mod query;
pub mod revoke;
pub mod rollback;
pub mod sql;
