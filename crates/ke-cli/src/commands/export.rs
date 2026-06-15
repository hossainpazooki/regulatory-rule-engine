//! `ke export --hash <h> --out <path>`: write an artifact's stored `.kew` to a
//! flat file (Gate 5, workstream 5b).
//!
//! NON-AUTHORITATIVE. This command reads the stored `.kew` bytes via the
//! existing [`RegistryBackend::read_artifact_kew`] and writes those **exact
//! bytes** to `out_path` — byte-identical, no re-encode, no re-hash, no
//! signing, no lifecycle transition, no event append. The canonical encoding
//! (envelope split + the re-zero content hash at `artifact_hash_offset`) is
//! frozen and untouched here: export is a pure copy.
//!
//! The matching [`import_kew`](crate::commands::import_kew) command re-verifies
//! the flat file offline via `ke_artifact::verify_artifact` and rejects
//! anything that does not verify — export never blesses the bytes, it just
//! transcribes them.

use crate::registry::backend::RegistryBackend;
use anyhow::Result;

/// Arguments for `ke export`.
pub struct ExportArgs<'a> {
    /// 32-byte artifact content hash (already decoded from hex at the CLI edge).
    pub artifact_hash: [u8; 32],
    /// Destination path for the flat `.kew` file.
    pub out_path: &'a str,
}

/// Outcome of an `ke export` run.
pub struct ExportOutcome {
    /// The exported artifact's content hash (echoed back, unchanged).
    pub artifact_hash: [u8; 32],
    /// Number of bytes written to `out_path`.
    pub bytes_written: usize,
    /// The destination path the bytes were written to.
    pub out_path: String,
}

/// Reads the stored `.kew` ([`RegistryBackend::read_artifact_kew`]) and writes
/// the EXACT bytes to `out_path`. No re-encode, no re-hash, no signing, no
/// lifecycle change. The registry is read-only on this path.
pub fn run<B: RegistryBackend>(backend: &B, args: &ExportArgs<'_>) -> Result<ExportOutcome> {
    // Read the authoritative stored bytes. We write them verbatim — the only
    // way to keep the artifact hash, the envelope split, and every signature
    // byte-identical is to never re-encode.
    let kew = backend.read_artifact_kew(&args.artifact_hash)?;
    std::fs::write(args.out_path, &kew)
        .map_err(|e| anyhow::anyhow!("write {}: {e}", args.out_path))?;
    Ok(ExportOutcome {
        artifact_hash: args.artifact_hash,
        bytes_written: kew.len(),
        out_path: args.out_path.to_string(),
    })
}
