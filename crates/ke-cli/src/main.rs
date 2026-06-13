//! `ke`: command-line entrypoint for ke-workbench.
//!
//! Gate 4 Phase 3a lands `compile` and `query` over a local-FS registry;
//! `verify`/`attest`/`publish`/`deprecate`/`revoke`/`rollback` are declared but
//! deferred to Phase 3b (spec § 9, § 16, § 18). All logic lives in the
//! `ke_cli` library; this binary is a thin dispatcher.

fn main() {
    std::process::exit(ke_cli::cli::run());
}
