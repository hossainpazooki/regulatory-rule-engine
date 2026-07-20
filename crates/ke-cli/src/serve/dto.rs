//! Request / response DTOs for `ke serve` (scaffold).
//!
//! Request bodies are deserialized here; response bodies reuse the existing
//! `Serialize` types where they already exist and only wrap them where the
//! contract bundles several values. No new domain types — these are transport
//! shells over the reuse signatures in [`super`].
//!
//! Reused-as-is response payloads (already `Serialize`, do NOT re-mirror):
//! - [`crate::registry::ResolutionRecord`] — `GET /resolve`
//! - [`ke_artifact::ArtifactProvenance`] — embedded in `POST /verify`
//! - `ke_core::ir::RuleIR` — embedded in `POST /compile/preview`
//! - the per-rule `serde_json::Value` from
//!   [`ke_runtime::Evaluation::normalized_json`] — embedded in `POST /dry-run`
//!
//! NOTE for the Build phase: `ke_compiler::verify::{Finding, Conflict,
//! VerificationReport}` and `ke_artifact::{Verdict, RejectionReason}` do **not**
//! derive `Serialize`. The `/compile/preview` and `/verify` response shapes
//! below therefore project those into serde-friendly fields the handlers fill —
//! they are NON-AUTHORITATIVE preview projections, not new domain types.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Requests
// ---------------------------------------------------------------------------

/// `POST /verify` body: `{hash:<64hex>, env?:<str>, policy?:strict|permissive}`.
/// `env` defaults to `"local"`; `policy` defaults to `"strict"`. Verify reads
/// the CANONICAL registry view to build `RegistryEvidence` (G5-1).
#[derive(Clone, Debug, Deserialize)]
pub struct VerifyRequest {
    /// 64-char lowercase-hex artifact content hash.
    pub hash: String,
    /// Named environment for the [`ke_artifact::PolicyContext`] (default
    /// `"local"`). `None` → `"local"`.
    #[serde(default)]
    pub env: Option<String>,
    /// Policy selector: `"strict"` (default, the built-in
    /// [`crate::policy::default_verification_policy`]) or `"permissive"`.
    #[serde(default)]
    pub policy: Option<String>,
}

/// `POST /compile/preview` body: `{source:<yaml str>}`. NON-authoritative —
/// signs and stores NOTHING.
#[derive(Clone, Debug, Deserialize)]
pub struct CompilePreviewRequest {
    /// The YAML rule document to compile + verify for preview.
    pub source: String,
}

/// `POST /dry-run` body: `{source:<yaml str>, facts:<json obj>}` OR
/// `{hash:<64hex>, facts:<json obj>}` (resolve via registry then decode). One of
/// `source` / `hash` must be present.
#[derive(Clone, Debug, Deserialize)]
pub struct DryRunRequest {
    /// Inline YAML source to compile then evaluate. Mutually exclusive with
    /// `hash`.
    #[serde(default)]
    pub source: Option<String>,
    /// A stored artifact hash to resolve (canonical view) + decode then
    /// evaluate. Mutually exclusive with `source`.
    #[serde(default)]
    pub hash: Option<String>,
    /// The facts object, passed through [`ke_runtime::facts_from_json`].
    pub facts: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Responses
// ---------------------------------------------------------------------------

/// `GET /healthz` body.
#[derive(Clone, Debug, Serialize)]
pub struct HealthResponse {
    pub ok: bool,
    /// Loud, fixed banner naming the surface as preview / non-authoritative.
    pub surface: String,
}

impl Default for HealthResponse {
    fn default() -> Self {
        Self {
            ok: true,
            surface: "ke-cli serve (preview, non-authoritative)".to_string(),
        }
    }
}

/// `POST /verify` body: the verdict (mirrored to serde-friendly fields), the
/// always-built [`ke_artifact::ArtifactProvenance`], and the registry status the
/// verdict considered. HTTP stays 200 even for a `Rejected` verdict — a
/// rejection is a valid answer, not an HTTP error.
///
/// `verdict` is `"verified"` or `"rejected"`; `rejection` carries the
/// `RejectionReason` rendered to a string when rejected (the Build phase fills
/// the projection — `Verdict`/`RejectionReason` are not `Serialize`).
#[derive(Clone, Debug, Serialize)]
pub struct VerifyResponse {
    /// `"verified"` | `"rejected"`.
    pub verdict: String,
    /// Human-rendered rejection reason when `verdict == "rejected"`, else `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejection: Option<String>,
    /// Always present — `verify_artifact` builds provenance even on rejection.
    pub provenance: ke_artifact::ArtifactProvenance,
    /// The registry status the verdict considered
    /// (Published/Deprecated/Revoked/Unknown).
    pub registry_state: ke_artifact::RegistryStatus,
    /// The revocation sidecar, present exactly when the registry state is
    /// `Revoked` (Gate 6/ADR-0024). Informational only — the verdict above is
    /// already `rejected` for any non-Published state (fail-closed); this block
    /// gives a consumer the inputs to `revocation_decision`, it never softens
    /// the verdict.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revocation: Option<crate::registry::RevocationRecord>,
}

/// One verification finding, projected for the preview response (the
/// `ke_compiler::verify::Finding` type is not `Serialize`).
#[derive(Clone, Debug, Serialize)]
pub struct FindingDto {
    /// `"T0"` | `"T1"`.
    pub tier: String,
    pub rule_id: String,
    pub code: String,
    pub message: String,
    pub blocking: bool,
}

/// One cross-rule conflict, projected for the preview response (the
/// `ke_compiler::verify::Conflict` type is not `Serialize`).
#[derive(Clone, Debug, Serialize)]
pub struct ConflictDto {
    /// Conflict class, rendered (e.g. `Debug`-stringified `ConflictClass`).
    pub class: String,
    /// `"Blocking"` | (non-blocking severities), rendered.
    pub severity: String,
    /// Human description of the conflict.
    pub message: String,
}

/// The `report` block of `POST /compile/preview`: the
/// `ke_compiler::verify::VerificationReport` projected to serde-friendly fields.
#[derive(Clone, Debug, Serialize)]
pub struct VerificationReportDto {
    pub has_blocking: bool,
    pub findings: Vec<FindingDto>,
    pub conflicts: Vec<ConflictDto>,
}

/// `POST /compile/preview` body. NON-authoritative: `rules` is the compiled IR
/// (each `RuleIR` is already `Serialize`); `report` is the projected
/// verification report. Signs / stores NOTHING.
#[derive(Clone, Debug, Serialize)]
pub struct CompilePreviewResponse {
    pub rules: Vec<ke_core::ir::RuleIR>,
    pub report: VerificationReportDto,
}

/// `POST /dry-run` body: one normalized evaluation per rule, each the
/// `serde_json::Value` from [`ke_runtime::Evaluation::normalized_json`].
#[derive(Clone, Debug, Serialize)]
pub struct DryRunResponse {
    pub evaluations: Vec<serde_json::Value>,
}

/// A uniform error body for 4xx/5xx JSON responses (router maps typed errors to
/// status + this shape; see [`super::router`]).
#[derive(Clone, Debug, Serialize)]
pub struct ErrorResponse {
    /// A stable short kind, e.g. `"not_found"`, `"ambiguous"`, `"bad_hash_hex"`,
    /// `"compile_error"`, `"facts_error"`, `"internal"`.
    pub error: String,
    /// Human-readable detail.
    pub detail: String,
}
