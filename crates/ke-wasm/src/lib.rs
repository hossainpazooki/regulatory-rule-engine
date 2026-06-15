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
//! # Preview compile + dry-run (Gate 5, G5-2) — NON-AUTHORITATIVE
//!
//! [`compile_preview`] and [`dry_run`] are thin browser adapters over the SAME
//! pure functions the native `ke-cli serve` handlers call
//! ([`ke_compiler::compile_rules`], [`ke_compiler::verify::verify`],
//! [`ke_runtime::facts_from_json`], [`ke_runtime::evaluate`],
//! [`ke_runtime::Evaluation::normalized_json`]) and emit the SAME JSON shapes
//! via the SAME projection. They COMPUTE + RETURN only: no `Artifact::assemble`,
//! no signing, no registry mutation is reachable. Parity with the canonical
//! native compute therefore holds by construction (one algorithm, not two) and
//! is asserted in `tests/parity.rs`; any difference the browser leg observes
//! against a canonical `ke-cli serve` endpoint is SURFACED, never silently used.
//!
//! WASM binds only the inline-`source` dry-run path. The native by-`hash` path
//! resolves the CANONICAL registry backend (off-WASM, G5-1) and is intentionally
//! NOT bound here (authority boundary): the browser uses the canonical endpoint
//! for stored artifacts.

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

/// Compile + verify a YAML rule document for in-browser PREVIEW.
///
/// NON-AUTHORITATIVE (spec § 6, § 16; CLAUDE.md authority boundary): this
/// computes and returns only. It never reaches `Artifact::assemble`, never
/// signs, attests, publishes, or mutates a registry. It is the byte-identical
/// twin of the native `POST /compile/preview` handler — same
/// [`ke_compiler::compile_rules`] + [`ke_compiler::verify::verify`] calls, same
/// projection, same JSON shape.
///
/// Returns a JSON string mirroring the native `CompilePreviewResponse`:
/// ```json
/// {"rules": RuleIR[], "report": {"has_blocking": bool,
///   "findings": [{"tier":"T0"|"T1","rule_id":..,"code":..,"message":..,"blocking":..}],
///   "conflicts": [{"class":..,"severity":..,"message":..}]}}
/// ```
/// On a [`ke_compiler::CompileError`], throws a `JsError` whose message is the
/// SAME `{"error":"compile_error","detail":"<{e:?}>"}` body the native 422
/// returns (encoded as a thrown error rather than an HTTP status).
#[wasm_bindgen]
pub fn compile_preview(source: &str) -> Result<String, JsError> {
    compile_preview_impl(source).map_err(|msg| JsError::new(&msg))
}

/// The pure compute body behind [`compile_preview`], free of any
/// `wasm-bindgen`/`JsError` types so it is callable on the NATIVE target (the
/// `tests/parity.rs` equality assertion runs this directly — `JsValue`
/// operations abort off-wasm). On error returns the SAME JSON body the wrapper
/// throws and the native handler returns as a 422: a `String` holding
/// `{"error":"compile_error","detail":"<{e:?}>"}`.
pub fn compile_preview_impl(source: &str) -> Result<String, String> {
    let rules = ke_compiler::compile_rules(source).map_err(|e| compile_error_body(&e))?;
    let report = ke_compiler::verify::verify(&rules);

    let response = CompilePreviewResponse {
        rules,
        report: project_report(&report),
    };
    serde_json::to_string(&response).map_err(|e| format!("serialize compile/preview: {e}"))
}

/// Evaluate inline YAML `source` against `facts_json` for in-browser PREVIEW.
///
/// NON-AUTHORITATIVE: computes + returns only; stores/signs nothing. Byte-
/// identical twin of the native `POST /dry-run` handler's `source` path —
/// [`ke_compiler::compile_rules`] → [`ke_runtime::facts_from_json`] →
/// [`ke_runtime::evaluate`] → [`ke_runtime::Evaluation::normalized_json`]. The
/// native handler receives `facts` already-deserialized from the request body;
/// the WASM edge deserializes the `facts_json` string first
/// ([`serde_json::from_str`]) to reach the SAME `facts_from_json` call.
///
/// WASM binds ONLY this inline-`source` path. The native by-`hash` path needs
/// the canonical registry backend (off-WASM, G5-1) and is intentionally not
/// bound here.
///
/// Returns a JSON string mirroring the native `DryRunResponse`:
/// `{"evaluations": Value[]}` where each element is
/// `Evaluation::normalized_json()`. On a compile error, throws a `JsError`
/// carrying `{"error":"compile_error",...}`; on a facts error, throws a
/// `JsError` carrying `{"error":"facts_error","detail":<from facts_from_json>}`.
#[wasm_bindgen]
pub fn dry_run(source: &str, facts_json: &str) -> Result<String, JsError> {
    dry_run_impl(source, facts_json).map_err(|msg| JsError::new(&msg))
}

/// The pure compute body behind [`dry_run`], free of `wasm-bindgen`/`JsError`
/// types so it is callable on the NATIVE target (see [`compile_preview_impl`]).
/// On a compile error returns the `{"error":"compile_error",...}` body; on a
/// facts error returns `{"error":"facts_error","detail":<from facts_from_json>}`.
pub fn dry_run_impl(source: &str, facts_json: &str) -> Result<String, String> {
    let rules = ke_compiler::compile_rules(source).map_err(|e| compile_error_body(&e))?;

    // Deserialize the facts string at the JS edge, then hand the SAME
    // serde_json::Value to facts_from_json the native handler receives.
    let facts_value: serde_json::Value = serde_json::from_str(facts_json)
        .map_err(|e| facts_error_body(&format!("parse facts JSON: {e}")))?;
    let facts = ke_runtime::facts_from_json(&facts_value).map_err(|e| facts_error_body(&e))?;

    let evaluations: Vec<serde_json::Value> = rules
        .iter()
        .map(|rule| ke_runtime::evaluate(rule, &facts).normalized_json())
        .collect();

    let response = DryRunResponse { evaluations };
    serde_json::to_string(&response).map_err(|e| format!("serialize dry-run: {e}"))
}

/// The `{"error":"compile_error","detail":"<{e:?}>"}` body — the SAME body the
/// native handler returns as a 422 (`ServeError::unprocessable("compile_error",
/// format!("{e:?}"))`), returned here as a `String` the wrapper wraps in a
/// `JsError`.
fn compile_error_body(e: &ke_compiler::CompileError) -> String {
    serde_json::json!({ "error": "compile_error", "detail": format!("{e:?}") }).to_string()
}

/// The `{"error":"facts_error","detail":<String>}` body — the SAME body the
/// native handler returns as a 422 (`ServeError::unprocessable("facts_error",
/// e)`), returned as a `String` the wrapper wraps in a `JsError`.
fn facts_error_body(detail: &str) -> String {
    serde_json::json!({ "error": "facts_error", "detail": detail }).to_string()
}

// ---------------------------------------------------------------------------
// Preview response DTOs + projection helpers.
//
// These mirror `ke-cli`'s `serve::dto` + `serve::handlers` byte-for-byte: the
// non-`Serialize` verify types (`Finding`/`Conflict`/`VerificationReport`) are
// projected with the EXACT same match arms the native `project_report` /
// `project_finding` / `project_conflict` use (tier T0/T1; class/severity via
// `Debug`). Duplicated here (not lifted to a shared crate — that crate-boundary
// refactor is Plan-Mode territory, flagged to Hossain) and pinned by the
// `tests/parity.rs` equality assertion so the two copies cannot drift.
// ---------------------------------------------------------------------------

use serde::Serialize;

/// Mirrors `ke-cli` `serve::dto::CompilePreviewResponse`.
#[derive(Serialize)]
struct CompilePreviewResponse {
    rules: Vec<ke_core::ir::RuleIR>,
    report: VerificationReportDto,
}

/// Mirrors `ke-cli` `serve::dto::DryRunResponse`.
#[derive(Serialize)]
struct DryRunResponse {
    evaluations: Vec<serde_json::Value>,
}

/// Mirrors `ke-cli` `serve::dto::VerificationReportDto`.
#[derive(Serialize)]
struct VerificationReportDto {
    has_blocking: bool,
    findings: Vec<FindingDto>,
    conflicts: Vec<ConflictDto>,
}

/// Mirrors `ke-cli` `serve::dto::FindingDto`.
#[derive(Serialize)]
struct FindingDto {
    tier: String,
    rule_id: String,
    code: String,
    message: String,
    blocking: bool,
}

/// Mirrors `ke-cli` `serve::dto::ConflictDto`.
#[derive(Serialize)]
struct ConflictDto {
    class: String,
    severity: String,
    message: String,
}

/// Byte-identical to `ke-cli` `serve::handlers::project_report`.
fn project_report(report: &ke_compiler::verify::VerificationReport) -> VerificationReportDto {
    VerificationReportDto {
        has_blocking: report.has_blocking(),
        findings: report.findings.iter().map(project_finding).collect(),
        conflicts: report.conflicts.iter().map(project_conflict).collect(),
    }
}

/// Byte-identical to `ke-cli` `serve::handlers::project_finding`.
fn project_finding(f: &ke_compiler::verify::Finding) -> FindingDto {
    use ke_compiler::verify::Tier;
    let tier = match f.tier {
        Tier::T0 => "T0",
        Tier::T1 => "T1",
    };
    FindingDto {
        tier: tier.to_string(),
        rule_id: f.rule_id.clone(),
        code: f.code.to_string(),
        message: f.message.clone(),
        blocking: f.blocking,
    }
}

/// Byte-identical to `ke-cli` `serve::handlers::project_conflict`.
fn project_conflict(c: &ke_compiler::verify::Conflict) -> ConflictDto {
    ConflictDto {
        class: format!("{:?}", c.class),
        severity: format!("{:?}", c.severity),
        message: c.detail.clone(),
    }
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
