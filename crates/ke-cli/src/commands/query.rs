//! `ke query (--hash <hex> | --tag <env>/<tag> | --regime <id> --effective
//! <YYYY-MM-DD> --env <env>)`: resolve a selector to an artifact and print its
//! content hash, current lifecycle state, and the §18 [`ResolutionRecord`].
//!
//! Resolution is ungated (no signing) — it only *reads* and re-verifies the
//! content hash via the ADR 0012 §5 re-zero procedure.

use crate::registry::backend::RegistryBackend;
use crate::registry::{resolve, ResolutionRecord, Selector};
use anyhow::Result;

/// Run a query and return the resolved record (the CLI prints it; tests assert
/// its fields). `now_unix` is the resolution clock (clock-free core).
pub fn run<B: RegistryBackend>(
    backend: &B,
    selector: &Selector,
    now_unix: u64,
) -> Result<ResolutionRecord> {
    let (_, record) = resolve(backend, selector, now_unix)?;
    Ok(record)
}

/// Render a resolution record as the human-readable block `ke query` prints.
pub fn render(record: &ResolutionRecord) -> String {
    format!(
        "hash:            {}\n\
         state:           {:?}\n\
         resolving_event: {}\n\
         selector:        {}\n\
         policy_version:  {}\n\
         resolved_at:     {}\n",
        crate::registry::hash_hex(&record.artifact_hash),
        record.registry_state_at_resolution,
        record.resolving_event_key,
        record.selector_desc,
        record.attestation_policy_version,
        record.resolution_timestamp_unix,
    )
}
