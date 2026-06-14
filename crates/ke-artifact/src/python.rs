//! PyO3 binding for `ke-artifact-py` (spec § 14, Phase 4b; ADR 0016).
//!
//! A **thin** wrapper over the pure 4a verify surface — it adds **no crypto**
//! and **no policy**: every cryptographic check is the exact `ke-artifact`
//! function the Rust and WASM legs call, so the three-language contract test
//! can assert byte-identical verdicts and canonical provenance JSON.
//!
//! # Authority + purity (hard invariants)
//!
//! - **Verify-only.** This module exposes decoding, hashing, signature /
//!   attestation verification, the folded [`verify_artifact`] call, and
//!   provenance reading. It **never** signs, attests, publishes, or transitions
//!   registry state (spec § 5, § 6, § 13). No `SigningKey` is reachable.
//! - **RNG-free.** ed25519 *verify* is deterministic; nothing here touches
//!   `OsRng`/`getrandom` (the windows-gnu wheel can't build getrandom — every
//!   path here is verification only).
//! - **No I/O.** Bytes arrive from Python as `bytes`; the keydir / policy /
//!   registry-evidence inputs arrive as JSON strings (the shared
//!   `scripts/contract-inputs/*.json`) and are deserialized with serde. The
//!   registry read happens at the `ke-cli` edge, never here.
//!
//! Errors surface as Python `ValueError` with the underlying Rust message.

// The pyo3 0.22 `#[pymethods]` / `#[pyfunction]` macros expand each fn body
// with a result conversion that clippy reads as `PyErr -> PyErr`
// (`useless_conversion`). It is generated code, not ours, and not fixable at
// the source level — allow it module-wide so `clippy -D warnings --features
// pyo3` stays clean. (Confirmed: every flagged span is a macro-expanded return,
// never a hand-written `.into()`.)
#![allow(clippy::useless_conversion)]

use crate::artifact::{decode_artifact, Artifact};
use crate::attestation::{verify_attestation_set, PolicyContext};
use crate::hash::{content_hash, verify_hash};
use crate::keydir::KeyDirectory;
use crate::sign::{verify_signature, VerifyingKey};
use crate::verify::{verify_artifact as core_verify_artifact, RegistryEvidence};
use ke_core::manifest::VerificationPolicy;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;

/// Map any `Display` Rust error into a Python `ValueError`.
fn py_err<E: std::fmt::Display>(e: E) -> PyErr {
    PyValueError::new_err(e.to_string())
}

/// Parse a JSON string into a serde type, surfacing decode errors to Python.
fn from_json<T: serde::de::DeserializeOwned>(label: &str, json: &str) -> PyResult<T> {
    serde_json::from_str(json)
        .map_err(|e| PyValueError::new_err(format!("invalid {label} JSON: {e}")))
}

/// A decoded, content-addressed artifact (read-only view). Constructed via
/// [`from_bytes`]; carries the original `.kew` bytes so hash / signature checks
/// run over the exact envelope prefix the file holds.
#[pyclass(name = "Artifact", module = "ke_artifact_py", frozen)]
pub struct PyArtifact {
    kew: Vec<u8>,
    artifact: Artifact,
    envelope_len: usize,
}

#[pymethods]
impl PyArtifact {
    /// `regime_id` from the manifest.
    #[getter]
    fn regime_id(&self) -> &str {
        &self.artifact.manifest.regime_id
    }

    /// IR schema version, `major.minor.patch`.
    #[getter]
    fn ir_schema_version(&self) -> String {
        self.artifact.manifest.ir_schema_version.to_string()
    }

    /// Wire codec version (e.g. `postcard-1`).
    #[getter]
    fn codec_version(&self) -> String {
        self.artifact.manifest.codec_version.0.clone()
    }

    /// Canonicalization-profile version (e.g. `ke-canon-4`).
    #[getter]
    fn canonicalization_version(&self) -> String {
        self.artifact.manifest.canonicalization_version.0.clone()
    }

    /// The compiler signature's `key_id`.
    #[getter]
    fn signer_key_id(&self) -> &str {
        &self.artifact.compiler_signature.key_id
    }

    /// `manifest.artifact_hash` as lowercase hex (the content address).
    #[getter]
    fn artifact_hash(&self) -> String {
        hex32(&self.artifact.manifest.artifact_hash)
    }

    /// Rule ids carried by `compiled_ir`, in IR order (§14 `iter_rules`).
    fn iter_rules(&self) -> Vec<String> {
        self.artifact
            .compiled_ir
            .iter()
            .map(|r| r.rule_id.clone())
            .collect()
    }

    /// `consistency_block` presence (the T2/T3 evidence block is platform-owned,
    /// ADR 0011; committed goldens carry `None`). Returns `True` iff present.
    fn consistency_block(&self) -> bool {
        self.artifact.consistency_block.is_some()
    }

    /// Per-attestation summaries as a list of dicts (`attestation_type`,
    /// `signer_key_id`, `is_test_key`, `tsa_class`, `claimed_time_unix`) —
    /// the same projection the provenance carries, with no signature bytes.
    fn attestations(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        // Reuse the canonical provenance projection so this can never drift from
        // the surface the contract test compares.
        let prov = self.provenance_value(&zero_unknown_registry(), 0);
        prov.attestations
            .iter()
            .map(|a| {
                let json = serde_json::to_string(a).map_err(py_err)?;
                json_to_py(py, &json)
            })
            .collect()
    }

    /// `source_span_index`: a dict of `rule_id -> [span, ...]` as JSON-derived
    /// Python objects (the spec § 14 span-index accessor).
    fn source_span_index(&self, py: Python<'_>) -> PyResult<PyObject> {
        let json = serde_json::to_string(&self.artifact.source_span_index).map_err(py_err)?;
        json_to_py(py, &json)
    }

    /// Recompute the content hash (re-zero recompute over the envelope prefix;
    /// never `blake3(raw .kew)`) and return it as lowercase hex (§14
    /// `canonical_hash`).
    fn canonical_hash(&self) -> PyResult<String> {
        let h = content_hash(&self.kew[..self.envelope_len]).map_err(py_err)?;
        Ok(hex32(&h))
    }

    /// Verify the content address: recompute and compare against
    /// `manifest.artifact_hash`. Returns `True` on match, `False` on mismatch.
    fn verify_content_hash(&self) -> bool {
        verify_hash(&self.kew).is_ok()
    }

    /// Verify the compiler signature over the envelope prefix using the
    /// verifying key resolved from `keydir_json` by the signature's `key_id`.
    /// `True` iff the signature verifies; `False` on an unknown/malformed key or
    /// a bad signature.
    fn verify_compiler_signature(&self, keydir_json: &str) -> PyResult<bool> {
        let keydir: KeyDirectory = from_json("keydir", keydir_json)?;
        let key = keydir
            .lookup(&self.artifact.compiler_signature.key_id)
            .and_then(|entry| VerifyingKey::from_bytes(&entry.public_key).ok());
        let Some(key) = key else {
            return Ok(false);
        };
        Ok(verify_signature(
            &self.kew[..self.envelope_len],
            &self.artifact.compiler_signature,
            &key,
        )
        .is_ok())
    }

    /// Verify the attestation set (R1–R8) under a policy, keydir, and policy
    /// context (all JSON). Returns `True` if the set verifies, else a list of
    /// rejection strings (truthy/falsy split: empty list-as-`True` would be
    /// confusing, so the failure case returns the list and the caller checks
    /// emptiness).
    fn verify_attestations(
        &self,
        policy_json: &str,
        keydir_json: &str,
        ctx_json: &str,
    ) -> PyResult<Vec<String>> {
        let policy: VerificationPolicy = from_json("policy", policy_json)?;
        let keydir: KeyDirectory = from_json("keydir", keydir_json)?;
        let ctx: PolicyContext = from_json("policy context", ctx_json)?;
        match verify_attestation_set(&self.artifact, &policy, &keydir, &ctx) {
            Ok(()) => Ok(Vec::new()),
            Err(rejections) => Ok(rejections.iter().map(|r| format!("{r:?}")).collect()),
        }
    }

    /// The full folded 4a verdict + canonical provenance for this artifact, as a
    /// dict: `{verdict, registry_state, content_hash, provenance}` (provenance is
    /// the canonical-JSON object). This is the call the contract test compares
    /// across languages.
    #[pyo3(signature = (keydir_json, ctx_json, policy_json, registry_json, exported_at_unix))]
    fn verify(
        &self,
        py: Python<'_>,
        keydir_json: &str,
        ctx_json: &str,
        policy_json: &str,
        registry_json: &str,
        exported_at_unix: u64,
    ) -> PyResult<PyObject> {
        verify_to_py(
            py,
            &self.kew,
            keydir_json,
            ctx_json,
            policy_json,
            registry_json,
            exported_at_unix,
        )
    }

    /// The canonical [`ArtifactProvenance`] JSON for this artifact under the
    /// given registry evidence and export time (§14 `provenance`). Byte-stable.
    fn provenance(&self, registry_json: &str, exported_at_unix: u64) -> PyResult<String> {
        let registry: RegistryEvidence = from_json("registry evidence", registry_json)?;
        let prov = self.provenance_value(&registry, exported_at_unix);
        prov.to_canonical_json().map_err(py_err)
    }
}

impl PyArtifact {
    fn provenance_value(
        &self,
        registry: &RegistryEvidence,
        exported_at_unix: u64,
    ) -> crate::verify::ArtifactProvenance {
        crate::verify::artifact_provenance(&self.artifact, registry, exported_at_unix)
    }
}

/// Decode `.kew` bytes into an [`PyArtifact`] (§14 `from_bytes`). Raises
/// `ValueError` on non-canonical / truncated / trailing-byte input.
#[pyfunction]
fn from_bytes(kew: &Bound<'_, PyBytes>) -> PyResult<PyArtifact> {
    let bytes = kew.as_bytes().to_vec();
    let (artifact, envelope_len) = decode_artifact(&bytes).map_err(py_err)?;
    Ok(PyArtifact {
        kew: bytes,
        artifact,
        envelope_len,
    })
}

/// One-shot folded verify over raw `.kew` bytes (module-level convenience that
/// mirrors `PyArtifact.verify`). Returns the same
/// `{verdict, registry_state, content_hash, provenance}` dict.
#[pyfunction]
#[pyo3(signature = (kew, keydir_json, ctx_json, policy_json, registry_json, exported_at_unix))]
fn verify_artifact(
    py: Python<'_>,
    kew: &Bound<'_, PyBytes>,
    keydir_json: &str,
    ctx_json: &str,
    policy_json: &str,
    registry_json: &str,
    exported_at_unix: u64,
) -> PyResult<PyObject> {
    verify_to_py(
        py,
        kew.as_bytes(),
        keydir_json,
        ctx_json,
        policy_json,
        registry_json,
        exported_at_unix,
    )
}

/// Shared implementation of the folded verify, JSON in -> Python dict out.
#[allow(clippy::too_many_arguments)]
fn verify_to_py(
    py: Python<'_>,
    kew: &[u8],
    keydir_json: &str,
    ctx_json: &str,
    policy_json: &str,
    registry_json: &str,
    exported_at_unix: u64,
) -> PyResult<PyObject> {
    let keydir: KeyDirectory = from_json("keydir", keydir_json)?;
    let ctx: PolicyContext = from_json("policy context", ctx_json)?;
    let policy: VerificationPolicy = from_json("policy", policy_json)?;
    let registry: RegistryEvidence = from_json("registry evidence", registry_json)?;

    let outcome = core_verify_artifact(kew, &keydir, &ctx, &policy, &registry, exported_at_unix);

    let provenance_json = outcome.provenance.to_canonical_json().map_err(py_err)?;
    let content_hash_hex = hex32(&outcome.provenance.artifact_hash);

    // Build a stable, language-portable dict. `verdict` is "verified" or the
    // rejection reason rendered via Debug (stable enum form) so the three
    // languages compare on the same string.
    let result = serde_json::json!({
        "verdict": verdict_str(&outcome.verdict),
        "registry_state": format!("{:?}", outcome.registry_state),
        "content_hash": content_hash_hex,
        "provenance": serde_json::from_str::<serde_json::Value>(&provenance_json).map_err(py_err)?,
    });
    json_to_py(py, &result.to_string())
}

/// Stable verdict string: "verified" or "rejected:<Debug of reason>".
fn verdict_str(verdict: &crate::verify::Verdict) -> String {
    match verdict {
        crate::verify::Verdict::Verified => "verified".to_string(),
        crate::verify::Verdict::Rejected(reason) => format!("rejected:{reason:?}"),
    }
}

/// `RegistryEvidence` with `Unknown` status and a zero head — used only to
/// project attestation summaries (the registry fields are not consulted there).
fn zero_unknown_registry() -> RegistryEvidence {
    RegistryEvidence {
        status: crate::verify::RegistryStatus::Unknown,
        event_head_hash: [0u8; 32],
        live_event_head: None,
    }
}

/// Lowercase hex of a 32-byte hash.
fn hex32(bytes: &[u8; 32]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::with_capacity(64), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// Parse a JSON string into a native Python object via the `json` module.
/// Keeps the binding dependency-light (no pythonize) and guarantees the Python
/// side sees exactly the canonical JSON the Rust/WASM legs emit.
fn json_to_py(py: Python<'_>, json: &str) -> PyResult<PyObject> {
    let json_mod = py.import_bound("json")?;
    let obj = json_mod.call_method1("loads", (json,))?;
    Ok(obj.into())
}

/// The `ke_artifact_py` extension module (spec § 14). Verify-only surface.
#[pymodule]
fn ke_artifact_py(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyArtifact>()?;
    m.add_function(wrap_pyfunction!(from_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(verify_artifact, m)?)?;
    m.add("__doc__", "ke-artifact-py: verify-only bindings over the Rust artifact verify surface (spec § 14). No signing/publish.")?;
    Ok(())
}
