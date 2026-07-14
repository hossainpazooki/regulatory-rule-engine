//! Handlers for `ke serve` — each a **thin adapter** over an existing pure
//! function. Nothing here reimplements a verifier, a compiler, a registry walk,
//! or an evaluator; every handler resolves its inputs and delegates to the
//! reuse signatures pinned in [`super`], then serializes the canonical result.
//!
//! Authority (CLAUDE.md §5/§10/§13 — hard constraint): NONE of these may sign,
//! attest, publish, revoke, or assemble. `resolve`/`verify`/`dry-run`-by-hash
//! read the CANONICAL registry backend in [`AppState`] (G5-1), never a vendored
//! snapshot. `compile_preview` and `dry-run`-by-source are non-authoritative
//! previews that store nothing and reach no `Artifact::assemble`.

use super::dto::{
    CompilePreviewRequest, CompilePreviewResponse, ConflictDto, DryRunRequest, DryRunResponse,
    FindingDto, HealthResponse, VerificationReportDto, VerifyRequest, VerifyResponse,
};
use super::router::{json_response, text_event_stream_headers, ServeError, ServeResult};
use super::AppState;
use crate::registry::backend::RegistryBackend;
use crate::registry::{
    current_state, hash_from_hex, head_event, resolve as registry_resolve, Selector,
};
use ke_compiler::verify::{Conflict, Finding, Tier, VerificationReport};
use std::io::Write;
use tiny_http::Request;

/// The request-edge clock for `ke serve`: `KE_NOW` (deterministic override),
/// else the system clock. Sourcing time per request (not once at bind) keeps the
/// CLI's `--now`/`KE_NOW` edge convention — the registry core stays clock-free.
fn now_unix() -> u64 {
    if let Ok(now) = std::env::var("KE_NOW") {
        if let Ok(parsed) = now.parse::<u64>() {
            return parsed;
        }
    }
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Parse a `key=value&key=value` query string into the keys the resolve handler
/// understands (`hash`, `env`, `tag`). Values are taken as-is (the only values
/// we read are hex / short tag names — no escaping needed).
fn query_param<'a>(query: &'a str, key: &str) -> Option<&'a str> {
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == key).then_some(v)
    })
}

/// `GET /healthz` → 200 `{ok:true, surface:"ke-cli serve (preview, non-authoritative)"}`.
/// The liveness probe and the loud non-authoritative banner.
pub fn health() -> ServeResult {
    json_response(&HealthResponse::default())
}

/// `GET /resolve?hash=<64hex>` or `?env=<e>&tag=<t>` →
/// 200 [`crate::registry::ResolutionRecord`] JSON.
///
/// Thin adapter: builds a [`Selector`] from the query and delegates to
/// [`crate::registry::resolve`] against the CANONICAL backend. `RegistryError`
/// maps to status via [`ServeError::from`] (NotFound→404, Ambiguous→409,
/// BadHashHex→400).
pub fn resolve(state: &AppState, query: Option<&str>) -> ServeResult {
    let query = query.unwrap_or("");
    let selector = if let Some(hash_hex) = query_param(query, "hash") {
        let hash =
            hash_from_hex(hash_hex).map_err(|e| ServeError::bad_request(format!("hash: {e}")))?;
        Selector::ByHash(hash)
    } else if let (Some(env), Some(tag)) = (query_param(query, "env"), query_param(query, "tag")) {
        Selector::ByTag {
            env: env.to_string(),
            tag: tag.to_string(),
        }
    } else {
        return Err(ServeError::bad_request(
            "resolve requires either `?hash=<64hex>` or `?env=<e>&tag=<t>`",
        ));
    };

    let (_hash, record) = registry_resolve(&*state.backend, &selector, now_unix())?;
    json_response(&record)
}

/// `POST /verify` body [`VerifyRequest`] → 200 [`VerifyResponse`].
///
/// HTTP stays 200 even for a `Rejected` verdict — a rejection is a valid answer,
/// not a transport error. Reads the CANONICAL registry view to assemble
/// [`ke_artifact::RegistryEvidence`] (G5-1) exactly as
/// `export_provenance::read_registry_evidence` does (reusing
/// [`crate::commands::export_provenance::status_for`] + [`current_state`] +
/// [`head_event`]), then delegates to [`ke_artifact::verify_artifact`].
pub fn verify(state: &AppState, body: &str) -> ServeResult {
    let req: VerifyRequest = serde_json::from_str(body)
        .map_err(|e| ServeError::bad_request(format!("parse verify body: {e}")))?;
    let hash =
        hash_from_hex(&req.hash).map_err(|e| ServeError::bad_request(format!("hash: {e}")))?;

    // Canonical artifact bytes + canonical registry evidence (G5-1).
    let kew = state.backend.read_artifact_kew(&hash)?;
    let evidence = registry_evidence(state, &hash)?;

    let outcome = run_verify(&kew, &req, evidence, now_unix())?;

    // Project the non-`Serialize` Verdict into the response's string fields.
    use ke_artifact::Verdict;
    let (verdict, rejection) = match outcome.verdict {
        Verdict::Verified => ("verified".to_string(), None),
        Verdict::Rejected(reason) => ("rejected".to_string(), Some(render_rejection(&reason))),
    };
    json_response(&VerifyResponse {
        verdict,
        rejection,
        provenance: outcome.provenance,
        registry_state: outcome.registry_state,
    })
}

/// Build [`ke_artifact::RegistryEvidence`] for `hash` from the CANONICAL backend
/// — the same construction as `export_provenance::read_registry_evidence`
/// (G5-1): current lifecycle state mapped through the single
/// [`status_for`](crate::commands::export_provenance::status_for) site, the
/// event-head chain hash (zero for an empty log), and `live_event_head: None`
/// (staleness is a consumer concern, not the read surface's).
fn registry_evidence(
    state: &AppState,
    hash: &[u8; 32],
) -> Result<ke_artifact::RegistryEvidence, ServeError> {
    use crate::commands::export_provenance::status_for;
    use ke_artifact::RegistryStatus;

    let events = state.backend.read_events(hash)?;
    let status = match current_state(&events)? {
        Some(s) => status_for(s),
        None => RegistryStatus::Unknown,
    };
    let event_head_hash = if events.is_empty() {
        [0u8; 32]
    } else {
        head_event(&*state.backend, hash)?.chain_hash()?
    };
    Ok(ke_artifact::RegistryEvidence {
        status,
        event_head_hash,
        live_event_head: None,
    })
}

/// The verify call itself. The compiler/expert verifying keys are the fixed-seed
/// `test-keys` keys (the only ones this build has — ADR-0009 production key
/// authority is an explicit Gate-4 open decision, CLAUDE.md § "Open decisions").
/// Without the `test-keys` feature there is no key directory to verify against,
/// so verify is unavailable rather than silently rejecting every artifact as
/// `CompilerSignatureInvalid` against an empty directory.
#[cfg(any(test, feature = "test-keys"))]
fn run_verify(
    kew: &[u8],
    req: &VerifyRequest,
    evidence: ke_artifact::RegistryEvidence,
    now: u64,
) -> Result<ke_artifact::VerificationOutcome, ServeError> {
    use ke_artifact::{decode_artifact, verify_artifact, PolicyContext};

    let (artifact, _envelope_len) = decode_artifact(kew).map_err(|e| {
        ServeError::from(crate::registry::RegistryError::ArtifactDecode(
            e.to_string(),
        ))
    })?;

    // Supported policy versions = exactly the versions the artifact's own
    // attestations declare (so R3 reflects the artifact under test, not a guess).
    let mut supported: Vec<String> = artifact
        .attestations
        .iter()
        .map(|a| a.attestation_policy_version.clone())
        .collect();
    supported.sort();
    supported.dedup();

    let ctx = PolicyContext {
        environment: req.env.clone().unwrap_or_else(|| "local".to_string()),
        now_unix: now,
        supported_policy_versions: supported,
        // No legal-source text store yet (open decision, spec § 21) — skip R5.
        current_legal_source_hash: None,
    };

    let keydir = test_key_directory(&artifact);
    let policy = verification_policy(req.policy.as_deref())?;

    Ok(verify_artifact(kew, &keydir, &ctx, &policy, &evidence, now))
}

/// Without `test-keys` there is no verifying-key directory wired (production key
/// authority is the Gate-4 open decision). Surface that honestly as a 500 rather
/// than verifying against nothing.
#[cfg(not(any(test, feature = "test-keys")))]
fn run_verify(
    _kew: &[u8],
    _req: &VerifyRequest,
    _evidence: ke_artifact::RegistryEvidence,
    _now: u64,
) -> Result<ke_artifact::VerificationOutcome, ServeError> {
    Err(ServeError::internal(
        "`/verify` needs a compiler/expert verifying-key directory; this build has none. \
         Production key authority is an open decision (spec § 21, Gate 4). Build `ke serve` \
         with `--features test-keys` to verify against the fixed-seed test keys.",
    ))
}

/// A key directory carrying the fixed-seed compiler verifying key (so the
/// compiler signature resolves) and the fixed-seed expert verifying key
/// authorized for exactly the attestation types the artifact carries (so the
/// attestation set can verify). Mirrors `ke-artifact`'s `verify_surface` test
/// keydir; windows are wide and `now` is supplied per request.
#[cfg(any(test, feature = "test-keys"))]
pub(crate) fn test_key_directory(artifact: &ke_artifact::Artifact) -> ke_artifact::KeyDirectory {
    use ke_artifact::sign::test_keys;
    use ke_artifact::{KeyDirectory, KeyDirectoryEntry, KeyStatus, SignerRole};

    let mut authorized_types: Vec<ke_core::manifest::AttestationType> = artifact
        .attestations
        .iter()
        .map(|a| a.attestation_type)
        .collect();
    authorized_types.sort();
    authorized_types.dedup();

    KeyDirectory {
        entries: vec![
            // Compiler key: resolves `compiler_signature.key_id` → verifying key.
            KeyDirectoryEntry {
                key_id: test_keys::TEST_KEY_ID.to_string(),
                public_key: test_keys::verifying_key().to_bytes(),
                signer_roles: vec![SignerRole::Registry],
                authorized_attestation_types: vec![],
                valid_from_unix: 0,
                valid_to_unix: u64::MAX,
                status: KeyStatus::Active,
                revoked_at_unix: None,
                revocation_reason: None,
                revocation_event_hash: None,
            },
            // Expert key: signs the artifact's attestations.
            KeyDirectoryEntry {
                key_id: test_keys::TEST_EXPERT_KEY_ID.to_string(),
                public_key: test_keys::expert_verifying_key().to_bytes(),
                signer_roles: vec![SignerRole::DomainExpert, SignerRole::PublicationApprover],
                authorized_attestation_types: authorized_types,
                valid_from_unix: 0,
                valid_to_unix: u64::MAX,
                status: KeyStatus::Active,
                revoked_at_unix: None,
                revocation_reason: None,
                revocation_event_hash: None,
            },
        ],
    }
}

/// Resolve the [`ke_core::manifest::VerificationPolicy`] from `req.policy`.
/// `"strict"` (default) reuses [`crate::policy::default_verification_policy`];
/// `"permissive"` requires no attestation types (per-attestation validity only).
/// Anything else is a 400.
#[cfg(any(test, feature = "test-keys"))]
fn verification_policy(
    policy: Option<&str>,
) -> Result<ke_core::manifest::VerificationPolicy, ServeError> {
    use ke_core::manifest::{T2T3Mode, VerificationPolicy};
    match policy.unwrap_or("strict") {
        "strict" => Ok(crate::policy::default_verification_policy()),
        "permissive" => Ok(VerificationPolicy {
            t2_t3_mode: T2T3Mode::Disabled,
            required_attestation_types: vec![],
            minimum_attestation_count_per_type: vec![],
        }),
        other => Err(ServeError::bad_request(format!(
            "unknown policy {other:?}; expected `strict` or `permissive`"
        ))),
    }
}

/// Render a non-`Serialize` [`ke_artifact::RejectionReason`] to a stable human
/// string for the verify response.
fn render_rejection(reason: &ke_artifact::RejectionReason) -> String {
    use ke_artifact::RejectionReason as R;
    match reason {
        R::Decode(e) => format!("decode: {e}"),
        R::HashMismatch => "content hash mismatch".to_string(),
        R::CompilerSignatureInvalid => "compiler signature invalid".to_string(),
        R::Attestations(rejections) => {
            let joined = rejections
                .iter()
                .map(|r| r.to_string())
                .collect::<Vec<_>>()
                .join("; ");
            format!("attestations: {joined}")
        }
        R::NotPublished { status } => format!("registry state not Published: {status:?}"),
        R::StaleEventHead { .. } => {
            "embedded event-head is stale against the live registry head".to_string()
        }
    }
}

/// `POST /compile/preview` body [`CompilePreviewRequest`] →
/// 200 [`CompilePreviewResponse`] | 422 on `CompileError`.
///
/// NON-authoritative: compiles + verifies for preview and signs/stores NOTHING
/// (never reaches `Artifact::assemble`). Thin adapter over
/// [`ke_compiler::compile_rules`] + [`ke_compiler::verify::verify`].
pub fn compile_preview(body: &str) -> ServeResult {
    let req: CompilePreviewRequest = serde_json::from_str(body)
        .map_err(|e| ServeError::bad_request(format!("parse compile/preview body: {e}")))?;

    let rules = ke_compiler::compile_rules(&req.source)
        .map_err(|e| ServeError::unprocessable("compile_error", format!("{e:?}")))?;
    let report = ke_compiler::verify::verify(&rules);

    json_response(&CompilePreviewResponse {
        rules,
        report: project_report(&report),
    })
}

/// Project a non-`Serialize` [`VerificationReport`] into the serde-friendly DTO.
fn project_report(report: &VerificationReport) -> VerificationReportDto {
    VerificationReportDto {
        has_blocking: report.has_blocking(),
        findings: report.findings.iter().map(project_finding).collect(),
        conflicts: report.conflicts.iter().map(project_conflict).collect(),
    }
}

fn project_finding(f: &Finding) -> FindingDto {
    let tier = match f.tier {
        Tier::T0 => "T0",
        Tier::T1 => "T1",
        Tier::T5 => "T5",
    };
    FindingDto {
        tier: tier.to_string(),
        rule_id: f.rule_id.clone(),
        code: f.code.to_string(),
        message: f.message.clone(),
        blocking: f.blocking,
    }
}

fn project_conflict(c: &Conflict) -> ConflictDto {
    ConflictDto {
        class: format!("{:?}", c.class),
        severity: format!("{:?}", c.severity),
        message: c.detail.clone(),
    }
}

/// `POST /dry-run` body [`DryRunRequest`] → 200 [`DryRunResponse`] | 422 on a
/// compile / facts error.
///
/// Rules come from inline `source` ([`ke_compiler::compile_rules`]) or from a
/// stored `hash` resolved against the CANONICAL backend then decoded
/// ([`resolve`] → `read_artifact_kew` → [`ke_artifact::decode_artifact`] →
/// `compiled_ir`). Facts go through [`ke_runtime::facts_from_json`] and each rule
/// through [`ke_runtime::evaluate`]; the response is the per-rule
/// `Evaluation::normalized_json()`. Non-authoritative — evaluates, stores
/// nothing.
pub fn dry_run(state: &AppState, body: &str) -> ServeResult {
    let req: DryRunRequest = serde_json::from_str(body)
        .map_err(|e| ServeError::bad_request(format!("parse dry-run body: {e}")))?;

    let rules = match (&req.source, &req.hash) {
        (Some(_), Some(_)) => {
            return Err(ServeError::bad_request(
                "dry-run takes exactly one of `source` or `hash`, not both",
            ));
        }
        (Some(source), None) => ke_compiler::compile_rules(source)
            .map_err(|e| ServeError::unprocessable("compile_error", format!("{e:?}")))?,
        (None, Some(hash_hex)) => {
            let hash = hash_from_hex(hash_hex)
                .map_err(|e| ServeError::bad_request(format!("hash: {e}")))?;
            // Resolve against the canonical view, then decode the stored bytes.
            let (resolved, _record) =
                registry_resolve(&*state.backend, &Selector::ByHash(hash), now_unix())?;
            let kew = state.backend.read_artifact_kew(&resolved)?;
            let (artifact, _envelope_len) = ke_artifact::decode_artifact(&kew).map_err(|e| {
                ServeError::from(crate::registry::RegistryError::ArtifactDecode(
                    e.to_string(),
                ))
            })?;
            match artifact.payload {
                ke_artifact::ArtifactPayload::Rules(rules) => rules,
                ke_artifact::ArtifactPayload::IntentSpec(_) => {
                    return Err(ServeError::bad_request(
                        "dry-run supports rule artifacts only, not intent-spec artifacts",
                    ));
                }
            }
        }
        (None, None) => {
            return Err(ServeError::bad_request(
                "dry-run requires either `source` (inline YAML) or `hash` (stored artifact)",
            ));
        }
    };

    let facts = ke_runtime::facts_from_json(&req.facts)
        .map_err(|e| ServeError::unprocessable("facts_error", e))?;

    let evaluations = rules
        .iter()
        .map(|rule| ke_runtime::evaluate(rule, &facts).normalized_json())
        .collect();

    json_response(&DryRunResponse { evaluations })
}

/// `GET /events` (SSE live feed): a `text/event-stream` response. One-way,
/// READ-ONLY, NON-authoritative (Deviation 1: SSE not WebSocket — see [`super`]).
///
/// The scaffold spike has no in-process event bus yet, so this emits an opening
/// `ready` frame on the canonical surface and an SSE keepalive comment, then
/// closes. The frame writer below is the exact `data: <json>\n\n` shape a richer
/// registry/compile feed appends to; wiring an event source is follow-up scope
/// (CLAUDE.md "Review UI follow-up", spec § 21). Crucially it remains read-only:
/// it never signs, attests, or mutates registry state.
pub fn events_sse(_state: &AppState, request: Request) -> std::io::Result<()> {
    let headers = text_event_stream_headers();
    let mut writer = request.into_writer();

    // Minimal HTTP/1.1 status line + the SSE headers, then the body frames.
    write!(writer, "HTTP/1.1 200 OK\r\n")?;
    for header in &headers {
        write!(
            writer,
            "{}: {}\r\n",
            header.field.as_str().as_str(),
            header.value.as_str()
        )?;
    }
    write!(writer, "\r\n")?;

    // Opening frame: name the surface as the read-only, non-authoritative feed.
    let ready = serde_json::json!({
        "event": "ready",
        "surface": "ke-cli serve (preview, non-authoritative)",
    });
    write!(writer, "data: {ready}\n\n")?;
    // SSE keepalive comment (ignored by EventSource clients).
    write!(writer, ": keepalive\n\n")?;
    writer.flush()?;
    Ok(())
}
