//! `ke export-provenance --hash <h> [--write]`: the registry-reading
//! **producer** for the consumer-agnostic provenance export (Gate 4 Phase 4a,
//! ADR 0016).
//!
//! This is the **only** backend-touching part of the verification surface: it
//! reads the artifact `.kew` ([`decode_artifact`]) and the event log
//! ([`current_state`] + [`head_event`]), maps the registry [`LifecycleState`]
//! to the `ke-artifact`-local [`RegistryStatus`] mirror **at this boundary**
//! (`ke-cli` depends on `ke-artifact`, never the reverse), builds
//! [`RegistryEvidence`] (with `live_event_head = None`: staleness is a consumer
//! concern, not the producer's), calls the pure
//! [`artifact_provenance`](ke_artifact::artifact_provenance), and prints one
//! canonical JSON line. With a write root set it also writes
//! `<registry-root>/artifacts/<hash>/provenance.json`.
//!
//! `exported_at` is the CLI clock (`--now` / `KE_NOW` / system clock at this
//! edge). No signing happens here, so this command needs **no** `test-keys`
//! feature.

use crate::registry::backend::{hex32, RegistryBackend};
use crate::registry::{current_state, head_event, LifecycleState, RegistryError};
use anyhow::Result;
use ke_artifact::{
    artifact_provenance, decode_artifact, ArtifactProvenance, RegistryEvidence, RegistryStatus,
};
use std::path::PathBuf;

/// Arguments for `ke export-provenance`.
pub struct ExportProvenanceArgs<'a> {
    /// 32-byte artifact content hash (already decoded from hex).
    pub artifact_hash: [u8; 32],
    /// Export time, unix seconds (sourced at the CLI edge).
    pub exported_at_unix: u64,
    /// If `Some`, write `<root>/artifacts/<hash>/provenance.json` under this
    /// registry root. `None` prints only.
    pub write_root: Option<&'a str>,
}

/// Outcome of an `ke export-provenance` run: the built provenance and its
/// canonical JSON rendering.
pub struct ExportProvenanceOutcome {
    pub provenance: ArtifactProvenance,
    /// The canonical JSON string (printed, and written to the sidecar when a
    /// write root was given).
    pub canonical_json: String,
}

/// Map the `ke-cli` registry [`LifecycleState`] to the `ke-artifact`-local
/// [`RegistryStatus`] mirror (ADR 0016). `Published`/`Deprecated`/`Revoked` map
/// directly; every pre-`Published` state (draft / structurally_verified /
/// ml_checked / expert_attested) is `Unknown` to an execution consumer (not yet
/// authoritative). This is the **single** mapping site for the mirror.
pub fn status_for(state: LifecycleState) -> RegistryStatus {
    match state {
        LifecycleState::Published => RegistryStatus::Published,
        LifecycleState::Deprecated => RegistryStatus::Deprecated,
        LifecycleState::Revoked => RegistryStatus::Revoked,
        LifecycleState::Draft
        | LifecycleState::StructurallyVerified
        | LifecycleState::MlChecked
        | LifecycleState::ExpertAttested => RegistryStatus::Unknown,
    }
}

/// Read the registry evidence for an artifact: the current lifecycle state
/// (mapped to [`RegistryStatus`]) and the event-head chain hash. An empty event
/// log is `RegistryStatus::Unknown` with a zero head. `live_event_head` is
/// `None`: the producer records the head as-of-export; a consumer supplies a
/// fresh live head for staleness detection.
fn read_registry_evidence<B: RegistryBackend>(
    backend: &B,
    hash: &[u8; 32],
) -> Result<RegistryEvidence, RegistryError> {
    let events = backend.read_events(hash)?;
    let status = match current_state(&events)? {
        Some(state) => status_for(state),
        None => RegistryStatus::Unknown,
    };
    let event_head_hash = if events.is_empty() {
        [0u8; 32]
    } else {
        head_event(backend, hash)?.chain_hash()?
    };
    Ok(RegistryEvidence {
        status,
        event_head_hash,
        live_event_head: None,
    })
}

/// Run `ke export-provenance`. Reads the `.kew` + event log, builds the
/// provenance, returns the canonical JSON (and writes the sidecar when a write
/// root was given).
pub fn run<B: RegistryBackend>(
    backend: &B,
    args: &ExportProvenanceArgs<'_>,
) -> Result<ExportProvenanceOutcome> {
    let hash = args.artifact_hash;

    // Decode the stored artifact (the .kew is the source of truth).
    let kew = backend.read_artifact_kew(&hash)?;
    let (artifact, _envelope_len) =
        decode_artifact(&kew).map_err(|e| RegistryError::ArtifactDecode(e.to_string()))?;

    // Read registry state + event-head at the backend boundary.
    let evidence = read_registry_evidence(backend, &hash)?;

    // Build the pure provenance projection and serialize canonically.
    let provenance = artifact_provenance(&artifact, &evidence, args.exported_at_unix);
    let canonical_json = provenance
        .to_canonical_json()
        .map_err(|e| anyhow::anyhow!("serialize provenance: {e}"))?;

    if let Some(root) = args.write_root {
        let dir = PathBuf::from(root).join("artifacts").join(hex32(&hash));
        std::fs::create_dir_all(&dir)
            .map_err(|e| RegistryError::io("create artifact dir for provenance", e))?;
        std::fs::write(dir.join("provenance.json"), &canonical_json)
            .map_err(|e| RegistryError::io("write provenance.json", e))?;
    }

    Ok(ExportProvenanceOutcome {
        provenance,
        canonical_json,
    })
}
