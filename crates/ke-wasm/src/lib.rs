//! ke-wasm: browser bindings for preview compile + dry-run, **plus** the
//! Gate 4 Phase 4b verify-only artifact surface (ADR 0016).
//!
//! # WASM discipline (spec § 6, § 16) — VERIFY-ONLY
//!
//! WASM is preview / consumer only. It **MUST NOT** sign, attest, publish, or
//! transition registry state. The verify surface exposed here wraps the pure,
//! RNG-free 4a `ke-artifact` functions verbatim: it decodes a `.kew`, checks
//! the content hash / compiler signature / attestation set, folds in the
//! caller-supplied registry evidence, and emits canonical provenance. No
//! `SigningKey` and no producer path is reachable from this module.
//!
//! The COMPASS browser consumer loads the generated package
//! (`@platform/atlas-artifact`) and calls [`verify_artifact`] to turn its
//! "surfaced, not verified" packs into in-browser-verified ones (revoked /
//! stale / non-Published packs reject).
//!
//! # Purity
//!
//! ed25519 *verify* is deterministic; nothing here touches `OsRng`/`getrandom`.
//! The registry read happens off-WASM (at the `ke-cli` edge); the browser
//! passes registry state in as JSON data.
//!
//! The preview compile + dry-run bindings (the original Gate 5 scope) are not
//! implemented here yet; this file currently ships the verify-only surface.

#![deny(unsafe_code)]
// wasm-bindgen 0.2.95's `#[wasm_bindgen]` macro emits a `cfg(wasm_bindgen_unstable_test_coverage)`
// that the host `check-cfg` lint flags. It is internal to the macro, harmless,
// and not under our control; allow it so `-D warnings` stays clean. (The
// wasm-bindgen CRATE pin is `=0.2.95`; the wasm-bindgen-cli MUST match exactly.)
#![allow(unexpected_cfgs)]

use wasm_bindgen::prelude::*;

/// Verify a `.kew` artifact end-to-end in the browser.
///
/// Inputs:
/// - `kew` — the raw artifact bytes (`Uint8Array` from JS).
/// - `keydir_json`, `context_json`, `policy_json`, `registry_json` — the shared
///   verifier inputs as JSON strings (the same `scripts/contract-inputs/*.json`
///   the Rust and Python legs load).
/// - `exported_at_unix` — the export timestamp the provenance records (supplied
///   by the caller; never read from any clock here).
///
/// Returns a JSON string:
/// ```json
/// {"verdict":"verified","registry_state":"Published","content_hash":"<hex>","provenance":{...}}
/// ```
/// `verdict` is `"verified"` or `"rejected:<reason>"`; `provenance` is the
/// canonical, byte-stable provenance object. Throws a JS error (rejected
/// `Promise` / exception) only on malformed JSON inputs — a *verification*
/// failure is a normal `rejected:` verdict, never a throw.
#[wasm_bindgen]
pub fn verify_artifact(
    kew: &[u8],
    keydir_json: &str,
    context_json: &str,
    policy_json: &str,
    registry_json: &str,
    exported_at_unix: u64,
) -> Result<String, JsError> {
    use ke_artifact::{KeyDirectory, PolicyContext, RegistryEvidence, Verdict};
    use ke_core::manifest::VerificationPolicy;

    let keydir: KeyDirectory = parse_json("keydir", keydir_json)?;
    let ctx: PolicyContext = parse_json("context", context_json)?;
    let policy: VerificationPolicy = parse_json("policy", policy_json)?;
    let registry: RegistryEvidence = parse_json("registry", registry_json)?;

    let outcome =
        ke_artifact::verify_artifact(kew, &keydir, &ctx, &policy, &registry, exported_at_unix);

    let verdict = match &outcome.verdict {
        Verdict::Verified => "verified".to_string(),
        Verdict::Rejected(reason) => format!("rejected:{reason:?}"),
    };
    let provenance_json = outcome
        .provenance
        .to_canonical_json()
        .map_err(|e| JsError::new(&format!("provenance json: {e}")))?;
    let provenance: serde_json::Value = serde_json::from_str(&provenance_json)
        .map_err(|e| JsError::new(&format!("reparse provenance: {e}")))?;

    let result = serde_json::json!({
        "verdict": verdict,
        "registry_state": format!("{:?}", outcome.registry_state),
        "content_hash": hex32(&outcome.provenance.artifact_hash),
        "provenance": provenance,
    });
    Ok(result.to_string())
}

/// Read provenance from a `.kew` without a full verify (the COMPASS provenance
/// reader). Decodes the artifact and projects the canonical
/// [`ArtifactProvenance`] under the supplied registry evidence + export time.
///
/// This performs **no** cryptographic verification — it is a read of the
/// already-decoded fields. Use [`verify_artifact`] when a verdict is needed.
/// Returns the canonical provenance JSON string; throws on undecodable bytes or
/// malformed registry JSON.
#[wasm_bindgen]
pub fn read_provenance(
    kew: &[u8],
    registry_json: &str,
    exported_at_unix: u64,
) -> Result<String, JsError> {
    use ke_artifact::{artifact_provenance, decode_artifact, RegistryEvidence};

    let registry: RegistryEvidence = parse_json("registry", registry_json)?;
    let (artifact, _) =
        decode_artifact(kew).map_err(|e| JsError::new(&format!("decode .kew: {e}")))?;
    artifact_provenance(&artifact, &registry, exported_at_unix)
        .to_canonical_json()
        .map_err(|e| JsError::new(&format!("provenance json: {e}")))
}

/// Parse a JSON string into a serde type, surfacing decode errors as JS errors.
fn parse_json<T: serde::de::DeserializeOwned>(label: &str, json: &str) -> Result<T, JsError> {
    serde_json::from_str(json).map_err(|e| JsError::new(&format!("invalid {label} JSON: {e}")))
}

/// Lowercase hex of a 32-byte hash.
fn hex32(bytes: &[u8; 32]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::with_capacity(64), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}
