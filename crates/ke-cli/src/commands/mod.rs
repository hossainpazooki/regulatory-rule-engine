//! `ke` subcommand implementations (Gate 4 Phase 3a): `compile` and `query`.
//!
//! `verify`/`attest`/`publish`/`deprecate`/`revoke`/`rollback` are declared in
//! the CLI surface (so the command set is visible) but exit with a Phase-3b
//! message — see [`crate::cli`]. Phase 3a executes only the `draft` and
//! `structurally_verified` lifecycle edges.

pub mod compile;
pub mod query;
