//! `ke compile <yaml> --regime <id> [--env <env>]`: compile a YAML rule
//! document, verify it, assemble a signed content-addressed artifact, store it,
//! and record the `draft` (then, if preconditions hold, `structurally_verified`)
//! lifecycle events.
//!
//! `ke compile-intent <json> --regime <id> [--env <env>]` is the sibling
//! authoring path (ADR-0021): it reads a JSON IntentSpec document (not rule
//! YAML), assembles an `ArtifactPayload::IntentSpec` artifact via
//! [`ke_artifact::Artifact::assemble_payload`] under the **identical** fixed-seed
//! `test-keys` signing path, and records the same draft / structurally_verified
//! lifecycle events. The two paths share [`store_and_record`]; only the payload
//! assembly differs.
//!
//! # Flow (plan / contract)
//!
//! 1. `ke_compiler::compile_rules(yaml)` -> `RuleIR`s.
//! 2. `ke_compiler::verify::verify(&rules)` — **abort nonzero** and print the
//!    blocking findings if `has_blocking()`.
//! 3. Build a synthetic manifest (`ke_core::examples::synthetic_manifest`) and
//!    `ke_artifact::Artifact::assemble` with the fixed-seed **test compiler
//!    key** (`key_id = "test-fixed-seed-1"`; never a production key, never
//!    `OsRng`/`getrandom`).
//! 4. `backend.put_artifact` (`.kew` + manifest.json + schema.json review
//!    views).
//! 5. Append the `draft` event (`prior_state = None`), registry-root-signed.
//! 6. Evaluate the `draft -> structurally_verified` precondition (verify clean
//!    **and** the compiler signature valid). If met, append the
//!    `structurally_verified` event.
//! 7. Print the content hash + the final lifecycle state.
//!
//! Signing (the compiler key and the registry-root event key) is gated behind
//! the `test-keys` feature; without it, `ke compile` cannot produce signed
//! artifacts and returns a typed error. This keeps `cargo build -p ke-cli`
//! (no features) clean while the smoke script / end-to-end runs use the
//! feature (mirrors `ke-artifact`'s gated golden generator).

use crate::registry::backend::RegistryBackend;
#[cfg(any(test, feature = "test-keys"))]
use crate::registry::RegistryError;
use anyhow::Result;

/// Arguments for `ke compile`.
pub struct CompileArgs<'a> {
    /// Path to the YAML rule document.
    pub yaml_path: &'a str,
    /// Regime id recorded on the synthetic manifest.
    pub regime_id: &'a str,
    /// Named environment (review view only in Phase 3a; default `local`).
    pub env: &'a str,
    /// Resolution / event clock, unix seconds (sourced at the CLI edge).
    pub now_unix: u64,
}

/// Arguments for `ke compile-intent` — the IntentSpec authoring path (ADR-0021).
pub struct IntentSpecArgs<'a> {
    /// Path to the JSON IntentSpec document (maps to [`ke_core::ir::IntentSpecIR`]).
    pub json_path: &'a str,
    /// Regime id recorded on the synthetic manifest.
    pub regime_id: &'a str,
    /// Named environment (review view only; default `local`).
    pub env: &'a str,
    /// Resolution / event clock, unix seconds (sourced at the CLI edge).
    pub now_unix: u64,
}

/// Outcome of a `ke compile` run (also used by tests).
pub struct CompileOutcome {
    pub artifact_hash: [u8; 32],
    pub final_state: crate::registry::LifecycleState,
}

#[cfg(any(test, feature = "test-keys"))]
pub fn run<B: RegistryBackend>(backend: &B, args: &CompileArgs<'_>) -> Result<CompileOutcome> {
    use crate::registry::structural_clean;
    use ke_artifact::sign::test_keys as artifact_keys;
    use ke_artifact::AuditVersions;
    use ke_compiler::verify::verify;
    use ke_core::examples::synthetic_manifest;
    use ke_core::ir::JurisdictionDate;
    use ke_core::manifest::ArtifactKind;

    // 1. Compile.
    let source = std::fs::read_to_string(args.yaml_path)
        .map_err(|e| anyhow::anyhow!("read {}: {e}", args.yaml_path))?;
    let rules = ke_compiler::compile_rules(&source)
        .map_err(|e| anyhow::anyhow!("compile {}: {e:?}", args.yaml_path))?;

    // 2. Verify — abort if blocking.
    let report = verify(&rules);
    if report.has_blocking() {
        let mut msg =
            String::from("verification produced blocking findings; refusing to assemble:\n");
        for finding in report.findings.iter().filter(|f| f.blocking) {
            msg.push_str(&format!(
                "  [{:?}] {} ({}): {}\n",
                finding.tier, finding.rule_id, finding.code, finding.message
            ));
        }
        for conflict in &report.conflicts {
            if conflict.severity == ke_compiler::verify::Severity::Blocking {
                msg.push_str(&format!("  [T4 conflict] {conflict:?}\n"));
            }
        }
        anyhow::bail!(msg);
    }

    // 3. Synthetic manifest + assemble with the test compiler key.
    //    The effective_from is taken from the first rule's window (corpus
    //    rules carry one); fall back to a floor date if absent.
    let effective_from = rules
        .first()
        .and_then(|r| r.effective_window.as_ref())
        .map(|w| w.effective_from)
        .unwrap_or_else(|| JurisdictionDate::new(1900, 1, 1));
    // The synthetic manifest hashes some bytes for its placeholder slots; the
    // exact bytes are irrelevant because `assemble` re-zeroes and re-derives
    // `artifact_hash`. Use the postcard of the rule set as the seed bytes.
    let seed_bytes = postcard::to_stdvec(&rules)
        .map_err(|e| anyhow::anyhow!("encode rules for manifest seed: {e}"))?;
    let manifest = synthetic_manifest(
        ArtifactKind::RegimePack,
        args.regime_id,
        effective_from,
        &seed_bytes,
    );
    let audit_versions = AuditVersions {
        jurisdiction_resolver_version: None,
        scenario_corpus_version: None,
    };
    let (artifact, kew) = ke_artifact::Artifact::assemble(
        manifest,
        rules.clone(),
        audit_versions,
        &artifact_keys::signing_key(),
        artifact_keys::TEST_KEY_ID,
    )
    .map_err(|e| anyhow::anyhow!("assemble artifact: {e}"))?;

    // 4-7. Store the artifact + review views, record the draft event, evaluate
    //       the structural transition, and derive the final state from the log.
    //       Shared with the IntentSpec path.
    store_and_record(
        backend,
        &artifact,
        &kew,
        structural_clean(&report),
        args.now_unix,
    )
}

/// `ke compile-intent`: author an `IntentSpec` artifact (ADR-0021) from a JSON
/// document. Same signing/lifecycle path as [`run`]; only the payload differs
/// — it assembles an [`ke_artifact::ArtifactPayload::IntentSpec`] via
/// [`ke_artifact::Artifact::assemble_payload`]. An IntentSpec carries no rule
/// verify tier, so it is structurally clean by construction (the only structural
/// gate on the `draft -> structurally_verified` edge is the compiler signature,
/// checked in [`store_and_record`]).
#[cfg(any(test, feature = "test-keys"))]
pub fn run_intent_spec<B: RegistryBackend>(
    backend: &B,
    args: &IntentSpecArgs<'_>,
) -> Result<CompileOutcome> {
    use ke_artifact::sign::test_keys as artifact_keys;
    use ke_artifact::{ArtifactPayload, AuditVersions};
    use ke_core::examples::synthetic_manifest;
    use ke_core::ir::JurisdictionDate;
    use ke_core::manifest::ArtifactKind;

    // 1. Read + parse the JSON IntentSpec document into the canonical IR.
    let source = std::fs::read_to_string(args.json_path)
        .map_err(|e| anyhow::anyhow!("read {}: {e}", args.json_path))?;
    let ir = parse_intent_spec(&source)
        .map_err(|e| anyhow::anyhow!("parse intent-spec {}: {e}", args.json_path))?;

    // 2. Synthetic manifest (kind = IntentSpec) + assemble the polymorphic
    //    payload with the same fixed-seed test compiler key `ke compile` uses.
    //    IntentSpec criteria carry no effective window; use the floor date.
    let effective_from = JurisdictionDate::new(1900, 1, 1);
    // Seed bytes are irrelevant — `assemble_payload` re-zeroes and re-derives
    // `artifact_hash`. Use the postcard of the IR.
    let seed_bytes = postcard::to_stdvec(&ir)
        .map_err(|e| anyhow::anyhow!("encode intent-spec for manifest seed: {e}"))?;
    let manifest = synthetic_manifest(
        ArtifactKind::IntentSpec,
        args.regime_id,
        effective_from,
        &seed_bytes,
    );
    let audit_versions = AuditVersions {
        jurisdiction_resolver_version: None,
        scenario_corpus_version: None,
    };
    let (artifact, kew) = ke_artifact::Artifact::assemble_payload(
        manifest,
        ArtifactPayload::IntentSpec(ir),
        audit_versions,
        &artifact_keys::signing_key(),
        artifact_keys::TEST_KEY_ID,
    )
    .map_err(|e| anyhow::anyhow!("assemble intent-spec artifact: {e}"))?;

    // 3. Store + record. Structurally clean by construction (no rule verify
    //    tier); the signature gate still applies inside `store_and_record`.
    store_and_record(backend, &artifact, &kew, true, args.now_unix)
}

/// Shared tail for both authoring paths (steps 4-7): store the `.kew` + review
/// views, append the registry-root-signed `draft` event, evaluate the
/// `draft -> structurally_verified` precondition (structural cleanliness **and**
/// a valid compiler signature over the hash-patched envelope), append the
/// transition event when met, then re-derive the final state from the event log
/// as the source of truth (ADR 0012 §2).
#[cfg(any(test, feature = "test-keys"))]
fn store_and_record<B: RegistryBackend>(
    backend: &B,
    artifact: &ke_artifact::Artifact,
    kew: &[u8],
    structural_clean_flag: bool,
    now_unix: u64,
) -> Result<CompileOutcome> {
    use crate::registry::{
        build_draft_event, build_transition_event, can_transition, current_state, LifecycleState,
        Preconditions,
    };
    use ke_artifact::sign::test_keys as artifact_keys;
    use ke_artifact::tsa::MockTsa;
    use ke_artifact::{verify_hash, SignerRole};

    let artifact_hash = artifact.manifest.artifact_hash;

    // 4. Store the artifact + review views. The schema view is the polymorphic
    //    payload (rules for a RegimePack, the IntentSpec IR for an IntentSpec).
    let manifest_json = serde_json::to_string_pretty(&artifact.manifest)
        .map_err(|e| anyhow::anyhow!("manifest json: {e}"))?;
    let schema_json = serde_json::to_string_pretty(&artifact.payload)
        .map_err(|e| anyhow::anyhow!("schema json: {e}"))?;
    backend.put_artifact(&artifact_hash, kew, &manifest_json, &schema_json)?;

    // 5. Append the draft event (registry-root-signed; authority = compiler).
    let draft_ts = MockTsa::stamp(&artifact_hash, now_unix);
    let draft = build_draft_event(
        artifact_hash,
        crate::registry::event::test_keys::REGISTRY_ROOT_KEY_ID,
        draft_ts,
    )?;
    backend.append_event(&artifact_hash, &draft)?;

    // 6. Evaluate draft -> structurally_verified. Compiler signature validity
    //    is confirmed via the re-zero hash + signature path (`verify_hash`
    //    re-zeroes; the assembled signature is over the hash-patched prefix and
    //    is checked here against the test compiler key).
    let hash_ok = verify_hash(kew).is_ok();
    let sig_ok = compiler_signature_valid(kew, &artifact_keys::verifying_key());
    let pre = Preconditions {
        structural_clean: structural_clean_flag,
        compiler_signature_valid: hash_ok && sig_ok,
        ..Preconditions::default()
    };

    let final_state = if can_transition(
        LifecycleState::Draft,
        LifecycleState::StructurallyVerified,
        &pre,
    ) {
        let sv_ts = MockTsa::stamp(&artifact_hash, now_unix);
        let sv = build_transition_event(
            &draft,
            LifecycleState::StructurallyVerified,
            // The triggering authority is the compiler/CI (§9); the event is
            // still registry-root-signed.
            artifact_keys::TEST_KEY_ID,
            SignerRole::Registry,
            sv_ts,
        )?;
        backend.append_event(&artifact_hash, &sv)?;
        LifecycleState::StructurallyVerified
    } else {
        LifecycleState::Draft
    };

    // 7. Re-derive the state from the log as the source of truth (ADR 0012 §2).
    let events = backend.read_events(&artifact_hash)?;
    let derived = current_state(&events)?.ok_or_else(|| RegistryError::NotFound {
        selector: "post-compile state".to_string(),
    })?;
    debug_assert_eq!(
        derived, final_state,
        "derived state must match appended state"
    );

    Ok(CompileOutcome {
        artifact_hash,
        final_state: derived,
    })
}

/// Without the `test-keys` feature the CLI cannot sign artifacts or events, so
/// `ke compile` is unavailable. Returns a typed error rather than silently
/// producing an unsigned artifact (CLAUDE.md: nothing here signs without an
/// authorized key).
#[cfg(not(any(test, feature = "test-keys")))]
pub fn run<B: RegistryBackend>(_backend: &B, _args: &CompileArgs<'_>) -> Result<CompileOutcome> {
    anyhow::bail!(
        "`ke compile` requires the `test-keys` feature in Phase 3a (the compiler and \
         registry-root signing keys are fixed-seed test keys). Build with \
         `--features test-keys`. Production signing keys are an infra/ADR-0009 concern \
         (out of Phase 3a)."
    )
}

/// IntentSpec authoring is likewise `test-keys`-gated (it signs with the same
/// fixed-seed compiler key). Without the feature it returns the same typed error.
#[cfg(not(any(test, feature = "test-keys")))]
pub fn run_intent_spec<B: RegistryBackend>(
    _backend: &B,
    _args: &IntentSpecArgs<'_>,
) -> Result<CompileOutcome> {
    anyhow::bail!(
        "`ke compile-intent` requires the `test-keys` feature (the compiler and \
         registry-root signing keys are fixed-seed test keys). Build with \
         `--features test-keys`. Production signing keys are an infra/ADR-0009 concern."
    )
}

/// Verify the compiler signature embedded in `.kew` over the hash-patched
/// envelope prefix (the bytes `[0, envelope_len)` as stored).
#[cfg(any(test, feature = "test-keys"))]
fn compiler_signature_valid(kew: &[u8], verifying_key: &ed25519_dalek::VerifyingKey) -> bool {
    let Ok((artifact, envelope_len)) = ke_artifact::decode_artifact(kew) else {
        return false;
    };
    ke_artifact::verify_signature(
        &kew[..envelope_len],
        &artifact.compiler_signature,
        verifying_key,
    )
    .is_ok()
}

// ---------------------------------------------------------------------------
// IntentSpec JSON authoring input (ADR-0021)
// ---------------------------------------------------------------------------
//
// The on-disk authoring shape is a small, ergonomic DTO that maps to
// `IntentSpecIR` — deliberately *not* `IntentSpecIR`'s raw serde form, so the
// threshold is a decimal **string** (ADR-0003: no floats ever touch the IR) and
// the volatility tag is a lowercase word rather than a bare enum variant.
//
//   {
//     "action_class": "wire_transfer_usd",
//     "criteria": [
//       { "name": "max_amount", "threshold": "10000.00", "volatility": "stable" }
//     ],
//     "idempotency": { "scope": "payer", "key_fields": ["payer_id", "nonce"] },
//     "source_spans": []            // optional; defaults to empty
//   }

// Reject unknown keys so a typo (e.g. `criterion` vs `criteria`) fails loud
// rather than silently dropping content from a signed artifact.
#[cfg(any(test, feature = "test-keys"))]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct IntentSpecInput {
    action_class: String,
    criteria: Vec<CriterionInput>,
    idempotency: IdempotencyInput,
    /// Source spans the criteria derive from (ADR-0021). Optional in the
    /// authoring input; when present it uses `SourceSpan`'s serde shape.
    #[serde(default)]
    source_spans: Vec<ke_core::ir::SourceSpan>,
}

#[cfg(any(test, feature = "test-keys"))]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CriterionInput {
    name: String,
    /// Exact decimal as a string (e.g. `"10000.00"`, `"-1.5"`). Parsed to a
    /// float-free `ScalarValue::Decimal` (ADR-0003).
    threshold: String,
    /// `"stable"` or `"volatile"` (case-insensitive).
    volatility: String,
}

#[cfg(any(test, feature = "test-keys"))]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct IdempotencyInput {
    scope: String,
    key_fields: Vec<String>,
}

/// Parse the JSON authoring document into a canonical [`ke_core::ir::IntentSpecIR`].
#[cfg(any(test, feature = "test-keys"))]
fn parse_intent_spec(json: &str) -> Result<ke_core::ir::IntentSpecIR> {
    use ke_core::ir::{AuthorizationCriterion, IdempotencyDef, IntentSpecIR};

    let input: IntentSpecInput =
        serde_json::from_str(json).map_err(|e| anyhow::anyhow!("invalid JSON: {e}"))?;

    if input.criteria.is_empty() {
        anyhow::bail!("intent-spec must declare at least one criterion");
    }

    let mut criteria = Vec::with_capacity(input.criteria.len());
    for c in input.criteria {
        criteria.push(AuthorizationCriterion {
            threshold: parse_decimal(&c.threshold)
                .map_err(|e| anyhow::anyhow!("criterion {:?} threshold: {e}", c.name))?,
            volatility: parse_volatility(&c.volatility)
                .map_err(|e| anyhow::anyhow!("criterion {:?} volatility: {e}", c.name))?,
            name: c.name,
        });
    }

    Ok(IntentSpecIR {
        action_class: input.action_class,
        criteria,
        idempotency: IdempotencyDef {
            key_fields: input.idempotency.key_fields,
            scope: input.idempotency.scope,
        },
        source_spans: input.source_spans,
    })
}

/// Parse a decimal string into a canonical `ScalarValue::Decimal` **without any
/// float** (ADR-0003): the string is split on the decimal point and the digits
/// are read directly into an `i128` mantissa with an integer scale, then folded
/// to canonical form (non-negative scale, no trailing zeros, `0 => scale 0`) to
/// match ke-core's `normalize_decimal` contract.
#[cfg(any(test, feature = "test-keys"))]
fn parse_decimal(s: &str) -> Result<ke_core::ir::ScalarValue> {
    let t = s.trim();
    if t.is_empty() {
        anyhow::bail!("empty decimal");
    }
    let (neg, body) = match t.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };

    let (int_part, frac_part) = match body.split_once('.') {
        Some((i, f)) => (i, f),
        None => (body, ""),
    };
    if int_part.is_empty() && frac_part.is_empty() {
        anyhow::bail!("no digits in {s:?}");
    }
    if !int_part.bytes().all(|b| b.is_ascii_digit())
        || !frac_part.bytes().all(|b| b.is_ascii_digit())
    {
        anyhow::bail!("non-digit character in {s:?}");
    }
    if frac_part.len() > i8::MAX as usize {
        anyhow::bail!("more than {} fractional digits in {s:?}", i8::MAX);
    }

    let digits: String = format!("{int_part}{frac_part}");
    let mut mantissa: i128 = digits
        .parse::<i128>()
        .map_err(|_| anyhow::anyhow!("decimal {s:?} overflows i128"))?;
    let mut scale: i8 = frac_part.len() as i8;
    if neg {
        mantissa = -mantissa;
    }

    // Fold to canonical form: strip trailing zeros, and zero has scale 0.
    while scale > 0 && mantissa % 10 == 0 {
        mantissa /= 10;
        scale -= 1;
    }
    if mantissa == 0 {
        scale = 0;
    }

    Ok(ke_core::ir::ScalarValue::Decimal { mantissa, scale })
}

/// Parse the volatility tag (case-insensitive) into the append-only enum.
#[cfg(any(test, feature = "test-keys"))]
fn parse_volatility(s: &str) -> Result<ke_core::ir::Volatility> {
    match s.trim().to_ascii_lowercase().as_str() {
        "stable" => Ok(ke_core::ir::Volatility::Stable),
        "volatile" => Ok(ke_core::ir::Volatility::Volatile),
        other => anyhow::bail!("expected \"stable\" or \"volatile\", got {other:?}"),
    }
}
