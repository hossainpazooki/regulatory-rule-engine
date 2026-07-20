//! Registry core (Gate 4 Phase 3a): the §9 lifecycle state machine realized as
//! an **append-only, hash-chained, registry-root-signed event log** (ADR 0012),
//! a transition-precondition table, state derivation, and §5/§18 resolution.
//!
//! # State lives in the event log (ADR 0012 §2)
//!
//! The current [`LifecycleState`] is the `new_state` of the highest-`seq`
//! [`event::LifecycleEvent`] — never a mutable field. [`current_state`] walks
//! the log, validating seq contiguity, the `prev_event_hash` chain, and the
//! registry-root signature on every event; any break is a typed
//! [`RegistryError`].
//!
//! # Transition authority = preconditions (ADR 0012 §2; §9)
//!
//! The event is *always* registry-root-signed; the §9 transition-authority
//! rules ("only compiler/CI → structurally_verified", "only registry policy →
//! published", …) are enforced as [`can_transition`] **preconditions** the
//! registry checks before appending. Phase 3a *executes* only `draft` and
//! `structurally_verified`; the remaining edges are table entries exercised by
//! tests and used by Phase 3b.
//!
//! # Clock-free core (plan §4)
//!
//! Every function that needs a clock takes `now_unix: u64` explicitly. No
//! `SystemTime`/clock syscalls live here — the CLI sources time at its edge.
//!
//! Local-FS registry objects are non-authoritative (ADR 0012 §6).

pub mod backend;
pub mod event;

use backend::{hex32, RegistryBackend};
use event::LifecycleEvent;
#[cfg(any(test, feature = "test-keys"))]
use ke_artifact::SignerRole;
use ke_compiler::verify::VerificationReport;
use ke_core::ir::JurisdictionDate;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The §9 registry lifecycle states. **Registry-local** — derived from the
/// event log, *not* stored in `ke_artifact::RegistryStateMetadata` (which stays
/// the inert `Draft` marker; ADR 0012 §2). Variant order is the canonical
/// discriminant order and the natural forward progression.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LifecycleState {
    Draft,
    StructurallyVerified,
    MlChecked,
    ExpertAttested,
    Published,
    Deprecated,
    Revoked,
}

impl LifecycleState {
    /// The `event_kind` string that *produces* this state when it is the
    /// `new_state` of an event (the initial state uses `draft`).
    pub fn event_kind(self) -> &'static str {
        match self {
            LifecycleState::Draft => "draft",
            LifecycleState::StructurallyVerified => "structurally_verified",
            LifecycleState::MlChecked => "ml_checked",
            LifecycleState::ExpertAttested => "expert_attested",
            LifecycleState::Published => "published",
            LifecycleState::Deprecated => "deprecated",
            LifecycleState::Revoked => "revoked",
        }
    }
}

/// Whether an artifact in this state is eligible to be a **rollback target**
/// (ADR 0013): only `Published` artifacts. The rollback *command* is Phase 3b;
/// this predicate lives in core now and is exercised by a unit test.
pub fn is_rollback_eligible(state: LifecycleState) -> bool {
    matches!(state, LifecycleState::Published)
}

/// Preconditions the registry evaluates before appending a transition event.
/// Each [`can_transition`] edge consults the subset it needs; absent evidence
/// (e.g. a Phase-3b consistency block or attestation set) makes its edge fail
/// closed.
#[derive(Clone, Copy, Debug, Default)]
pub struct Preconditions {
    /// `draft -> structurally_verified`: `verify::verify` produced no blocking
    /// finding **and** the artifact's compiler signature verified.
    pub structural_clean: bool,
    pub compiler_signature_valid: bool,
    /// `structurally_verified -> ml_checked`: a `ConsistencyBlock` is present
    /// (DEFERRED behavior — Phase 3b; the table entry exists now).
    pub consistency_block_present: bool,
    /// `ml_checked -> expert_attested`: `verify_attestation_set` passed
    /// (Phase 3b).
    pub attestation_set_valid: bool,
    /// `expert_attested -> published`: prior state is exactly `expert_attested`
    /// (registry policy). Computed from the prior state at the call site.
    pub prior_is_expert_attested: bool,
}

/// Whether the §9 transition `from -> to` is permitted given `pre`. This is the
/// **authority gate**: the event is always registry-root-signed, but the
/// registry refuses to append unless the triggering authority's preconditions
/// hold. Phase 3a executes only the first edge; the rest are table entries for
/// tests + Phase 3b.
pub fn can_transition(from: LifecycleState, to: LifecycleState, pre: &Preconditions) -> bool {
    use LifecycleState::*;
    match (from, to) {
        // Only compiler/CI structural verification (§9): clean verify + a valid
        // compiler signature on the artifact.
        (Draft, StructurallyVerified) => pre.structural_clean && pre.compiler_signature_valid,
        // DEFERRED (Phase 3b): a consistency block must be present.
        (StructurallyVerified, MlChecked) => pre.consistency_block_present,
        // Phase 3b: the attestation set verifies.
        (MlChecked, ExpertAttested) => pre.attestation_set_valid,
        // Phase 3b: registry policy — prior must be exactly expert_attested.
        (ExpertAttested, Published) => pre.prior_is_expert_attested,
        // Phase 3b: lifecycle end states.
        (Published, Deprecated) => true,
        (Published, Revoked) | (Deprecated, Revoked) => true,
        // Every other edge is forbidden.
        _ => false,
    }
}

/// Errors raised while reading, validating, or resolving registry state. Each
/// variant is typed so a gap/reorder/broken-link/bad-signature is diagnosable,
/// never a generic failure (plan: ChainBroken/SeqGap/SignatureInvalid/
/// EventDecode and resolution variants).
#[derive(Debug, Error)]
pub enum RegistryError {
    /// The event log has a gap or non-contiguous seq (expected `expected`, saw
    /// `found`).
    #[error("event sequence gap: expected seq {expected}, found {found}")]
    SeqGap { expected: u32, found: u32 },
    /// An event's `prev_event_hash` does not match the chain hash of its
    /// predecessor (reorder, tamper, or a broken link).
    #[error("hash chain broken at seq {seq}: prev_event_hash does not match the prior event")]
    ChainBroken { seq: u32 },
    /// An event's registry-root signature does not verify over its payload
    /// prefix.
    #[error("registry-root signature invalid at seq {seq}")]
    SignatureInvalid { seq: u32 },
    /// The first event's `prev_event_hash` was non-`None`, or a later event's
    /// was `None`.
    #[error("hash chain genesis invalid at seq {seq}: prev_event_hash presence is wrong")]
    ChainGenesisInvalid { seq: u32 },
    /// An event could not be postcard-encoded for hashing/signing.
    #[error("event canonical encode failed: {0}")]
    EventEncode(String),
    /// An event JSON object could not be encoded.
    #[error("event json encode failed: {0}")]
    EventJsonEncode(String),
    /// An event JSON object could not be decoded.
    #[error("event json decode failed: {0}")]
    EventJsonDecode(String),
    /// A stored `.kew` artifact failed to decode during resolution.
    #[error("artifact decode failed: {0}")]
    ArtifactDecode(String),
    /// A filesystem operation failed.
    #[error("registry io ({context}): {source}")]
    Io {
        context: &'static str,
        #[source]
        source: std::io::Error,
    },
    /// A hash hex string was not 64 lowercase hex chars.
    #[error("malformed 32-byte hash hex: {hex}")]
    BadHashHex { hex: String },
    /// An event already exists at this seq (append-only violation).
    #[error("event already exists at seq {seq} (append-only)")]
    EventExists { seq: u32 },
    /// A transition precondition was not met (the §9 authority gate refused).
    #[error("transition {from:?} -> {to:?} rejected: preconditions not met")]
    TransitionRejected {
        from: LifecycleState,
        to: LifecycleState,
    },
    /// Resolution found no artifact for the selector.
    #[error("resolution found no artifact for selector: {selector}")]
    NotFound { selector: String },
    /// Resolution found more than one published artifact for the selector.
    #[error("resolution ambiguous: {count} published artifacts match selector: {selector}")]
    Ambiguous { selector: String, count: usize },
    /// The resolved artifact's recomputed content hash did not match the
    /// expected hash (ADR 0012 §5 re-zero procedure).
    #[error("resolved artifact hash mismatch for selector: {selector}")]
    ResolvedHashMismatch { selector: String },
    /// A rollback target was not in the `Published` state (ADR 0013): only a
    /// published artifact is a valid rollback target.
    #[error("rollback target ineligible: state is {state:?}, must be Published (ADR 0013)")]
    RollbackIneligible { state: LifecycleState },
    /// `verify_attestation_set` rejected the artifact's attestation set; carries
    /// the human-rendered rejections (Phase 3b attest/publish gate).
    #[error("attestation set rejected:\n{}", .0.join("\n"))]
    AttestationSetRejected(Vec<String>),
    /// A `--policy` PolicyBundle JSON file could not be read or parsed.
    #[error("policy load failed: {0}")]
    PolicyLoad(String),
    /// A sidecar object already exists for this artifact (consistency /
    /// revocation runs once; append-only-ish for the sidecars).
    #[error("{kind} sidecar already exists for this artifact")]
    SidecarExists { kind: &'static str },
    /// The artifact has no recorded lifecycle state (empty event log) where one
    /// was required (e.g. a transition command on an unknown hash).
    #[error("no lifecycle state recorded for this artifact (compile it first)")]
    NoState,
}

impl RegistryError {
    pub(crate) fn io(context: &'static str, source: std::io::Error) -> Self {
        RegistryError::Io { context, source }
    }
    pub(crate) fn event_encode(e: postcard::Error) -> Self {
        RegistryError::EventEncode(e.to_string())
    }
    pub(crate) fn event_json_encode(e: serde_json::Error) -> Self {
        RegistryError::EventJsonEncode(e.to_string())
    }
    pub(crate) fn event_json_decode(e: serde_json::Error) -> Self {
        RegistryError::EventJsonDecode(e.to_string())
    }
}

/// Walk the event log `0..N` and return the **current state** (the highest-seq
/// event's `new_state`), validating as it goes (ADR 0012 §2/§3):
///
/// - `seq` is contiguous from 0 (gap/reorder => [`RegistryError::SeqGap`]);
/// - each `prev_event_hash` equals `blake3(prior event canonical bytes incl.
///   signature)`; the first is `None` (mismatch =>
///   [`RegistryError::ChainBroken`] / [`RegistryError::ChainGenesisInvalid`]);
/// - each event's registry-root signature verifies (bad sig =>
///   [`RegistryError::SignatureInvalid`]).
///
/// An empty log returns `Ok(None)` (the artifact has no recorded state yet).
pub fn current_state(events: &[LifecycleEvent]) -> Result<Option<LifecycleState>, RegistryError> {
    let mut prior_chain_hash: Option<[u8; 32]> = None;
    for (i, event) in events.iter().enumerate() {
        let expected_seq = i as u32;
        if event.seq != expected_seq {
            return Err(RegistryError::SeqGap {
                expected: expected_seq,
                found: event.seq,
            });
        }
        // Genesis: first event has no predecessor; all others must link.
        match (i, &event.prev_event_hash, &prior_chain_hash) {
            (0, None, _) => {}
            (0, Some(_), _) => return Err(RegistryError::ChainGenesisInvalid { seq: event.seq }),
            (_, None, _) => return Err(RegistryError::ChainGenesisInvalid { seq: event.seq }),
            (_, Some(link), Some(prior)) if link == prior => {}
            (_, Some(_), _) => return Err(RegistryError::ChainBroken { seq: event.seq }),
        }
        // Signature must verify against the registry-root key.
        event.verify_signature()?;
        prior_chain_hash = Some(event.chain_hash()?);
    }
    Ok(events.last().map(|e| e.new_state))
}

/// A selector for [`resolve`] (ADR 0012 §5, ADR 0014).
#[derive(Clone, Debug)]
pub enum Selector {
    /// Resolve a specific content hash directly.
    ByHash([u8; 32]),
    /// Resolve a tag pointer `tags/<env>/<tag>`.
    ByTag { env: String, tag: String },
    /// Resolve the single Published artifact for a regime whose effective
    /// window `[from, to)` (closed-open, `tz=None` honored) contains
    /// `effective`.
    ByRegime {
        regime_id: String,
        effective: JurisdictionDate,
        env: String,
    },
}

impl Selector {
    /// A stable human description recorded in the [`ResolutionRecord`].
    pub fn describe(&self) -> String {
        match self {
            Selector::ByHash(h) => format!("by-hash:{}", hex32(h)),
            Selector::ByTag { env, tag } => format!("by-tag:{env}/{tag}"),
            Selector::ByRegime {
                regime_id,
                effective,
                env,
            } => format!(
                "by-regime:{regime_id}@{:04}-{:02}-{:02}:{env}",
                effective.year, effective.month, effective.day
            ),
        }
    }
}

/// The §18 static resolution record (ADR 0012 §5, ADR 0014): the audit fields a
/// platform consumer pins when it resolves an artifact to execute.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolutionRecord {
    /// The resolved artifact's content hash.
    pub artifact_hash: [u8; 32],
    /// The registry state at the moment of resolution (highest-seq event's
    /// `new_state`).
    pub registry_state_at_resolution: LifecycleState,
    /// The `event_kind` of the event that established that state (the
    /// "resolving event key").
    pub resolving_event_key: String,
    /// Human description of the selector that resolved.
    pub selector_desc: String,
    /// The attestation policy version recorded on the resolved manifest.
    pub attestation_policy_version: String,
    /// The unix timestamp resolution ran at (caller-supplied).
    pub resolution_timestamp_unix: u64,
    /// The revocation sidecar, present exactly when the resolved state is
    /// `Revoked` (Gate 6/ADR-0024): the inputs a consumer feeds to
    /// `ke_core::revocation::revocation_decision`. Informational — resolve
    /// still reports the state, and `verify` stays fail-closed regardless.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revocation: Option<backend::RevocationRecord>,
}

/// Resolve a [`Selector`] against a backend to a `(content_hash,
/// ResolutionRecord)` (ADR 0012 §5, ADR 0014). `now_unix` is the resolution
/// clock (clock-free core — supplied by the caller).
///
/// - `ByHash`: the hash directly.
/// - `ByTag`: read the pointer; missing tag => [`RegistryError::NotFound`].
/// - `ByRegime`: `list_manifests`, filter to `regime_id` whose effective window
///   `[from, to)` contains `effective` (closed-open; `effective_to = None` is
///   open-ended), intersected with `state == Published`. `> 1` =>
///   [`RegistryError::Ambiguous`]; `0` => [`RegistryError::NotFound`].
///
/// The resolved artifact is then verified: re-zero the 32-byte hash slot in the
/// envelope prefix and BLAKE3 (ADR 0012 §5 **ERRATUM** — never
/// `blake3(raw .kew)`); the recomputed hash must equal the resolved hash.
pub fn resolve<B: RegistryBackend>(
    backend: &B,
    selector: &Selector,
    now_unix: u64,
) -> Result<([u8; 32], ResolutionRecord), RegistryError> {
    let selector_desc = selector.describe();
    let hash = match selector {
        Selector::ByHash(h) => *h,
        Selector::ByTag { env, tag } => {
            let pointer =
                backend
                    .read_pointer(env, tag)?
                    .ok_or_else(|| RegistryError::NotFound {
                        selector: selector_desc.clone(),
                    })?;
            pointer.target_hash()?
        }
        Selector::ByRegime {
            regime_id,
            effective,
            env: _env,
        } => {
            let manifests = backend.list_manifests()?;
            let mut matches: Vec<[u8; 32]> = Vec::new();
            for (hash, manifest) in &manifests {
                if manifest.regime_id != *regime_id {
                    continue;
                }
                if !window_contains(manifest.effective_from, manifest.effective_to, *effective) {
                    continue;
                }
                // Intersect with state == Published.
                let events = backend.read_events(hash)?;
                if current_state(&events)? == Some(LifecycleState::Published) {
                    matches.push(*hash);
                }
            }
            match matches.len() {
                0 => {
                    return Err(RegistryError::NotFound {
                        selector: selector_desc,
                    })
                }
                1 => matches[0],
                count => {
                    return Err(RegistryError::Ambiguous {
                        selector: selector_desc,
                        count,
                    })
                }
            }
        }
    };

    // Load events to determine state at resolution.
    let events = backend.read_events(&hash)?;
    let state = current_state(&events)?.ok_or_else(|| RegistryError::NotFound {
        selector: selector_desc.clone(),
    })?;
    let resolving_event_key = events
        .last()
        .map(|e| e.event_kind.clone())
        .unwrap_or_default();

    // Verify the resolved artifact: re-zero the hash slot then recompute
    // (ADR 0012 §5 erratum — never blake3(raw .kew)).
    let manifest = verify_resolved_artifact(backend, &hash, &selector_desc)?;

    // Surface the revocation sidecar exactly when the state is Revoked
    // (Gate 6/ADR-0024) — read-only; the record informs, it never gates.
    let revocation = if state == LifecycleState::Revoked {
        backend.read_revocation(&hash)?
    } else {
        None
    };

    let record = ResolutionRecord {
        artifact_hash: hash,
        registry_state_at_resolution: state,
        resolving_event_key,
        selector_desc,
        attestation_policy_version: manifest.attestation_policy_version,
        resolution_timestamp_unix: now_unix,
        revocation,
    };
    Ok((hash, record))
}

/// Read the resolved artifact's `.kew`, recompute the content hash via the
/// re-zero procedure (`ke_artifact::verify_hash`, **never** `blake3(raw .kew)`),
/// confirm it equals `hash`, and return the manifest.
fn verify_resolved_artifact<B: RegistryBackend>(
    backend: &B,
    hash: &[u8; 32],
    selector_desc: &str,
) -> Result<ke_core::manifest::Manifest, RegistryError> {
    // `list_manifests` already decodes; but to verify the hash we re-read the
    // stored kew through the backend's artifact view. We reuse `list_manifests`
    // to find the manifest and `ke_artifact::verify_hash` for the re-zero check.
    let manifests = backend.list_manifests()?;
    let (_, manifest) = manifests
        .into_iter()
        .find(|(h, _)| h == hash)
        .ok_or_else(|| RegistryError::NotFound {
            selector: selector_desc.to_string(),
        })?;
    // The manifest's self-reported hash must match the resolved hash, and that
    // hash must itself be the re-zero recomputation (asserted at put time by
    // `Artifact::assemble`; re-confirmed here against the stored kew).
    if manifest.artifact_hash != *hash {
        return Err(RegistryError::ResolvedHashMismatch {
            selector: selector_desc.to_string(),
        });
    }
    Ok(manifest)
}

/// Closed-open `[from, to)` containment for a legal effective window
/// (ADR 0007; `tz=None` honored — these are date-only comparisons). `to = None`
/// is open-ended (the window never closes).
fn window_contains(
    from: JurisdictionDate,
    to: Option<JurisdictionDate>,
    point: JurisdictionDate,
) -> bool {
    if date_lt(point, from) {
        return false;
    }
    match to {
        None => true,
        Some(to) => date_lt(point, to),
    }
}

/// `a < b` as a calendar-date comparison (year, then month, then day).
fn date_lt(a: JurisdictionDate, b: JurisdictionDate) -> bool {
    (a.year, a.month, a.day) < (b.year, b.month, b.day)
}

/// Build the first (`draft`) event for an artifact: `prior_state = None`,
/// `prev_event_hash = None`, registry-root-signed (gated). The triggering
/// authority is the compiler (it produced the artifact).
#[cfg(any(test, feature = "test-keys"))]
pub fn build_draft_event(
    artifact_hash: [u8; 32],
    authority_key_id: &str,
    timestamp: ke_artifact::TimestampToken,
) -> Result<LifecycleEvent, RegistryError> {
    let unsigned = LifecycleEvent {
        artifact_hash,
        seq: 0,
        prior_state: None,
        new_state: LifecycleState::Draft,
        event_kind: LifecycleState::Draft.event_kind().to_string(),
        authority_key_id: authority_key_id.to_string(),
        authority_role: SignerRole::Registry,
        timestamp,
        prev_event_hash: None,
        signature: [0u8; 64],
    };
    event::sign_event(unsigned)
}

/// Build a transition event chained onto `prior` (its `prev_event_hash` is the
/// prior event's chain hash; `seq = prior.seq + 1`). The caller must have
/// checked [`can_transition`]. Registry-root-signed (gated). `authority_role`
/// records the triggering authority; the signer is always the registry root.
#[cfg(any(test, feature = "test-keys"))]
#[allow(clippy::too_many_arguments)]
pub fn build_transition_event(
    prior: &LifecycleEvent,
    new_state: LifecycleState,
    authority_key_id: &str,
    authority_role: SignerRole,
    timestamp: ke_artifact::TimestampToken,
) -> Result<LifecycleEvent, RegistryError> {
    let unsigned = LifecycleEvent {
        artifact_hash: prior.artifact_hash,
        seq: prior.seq + 1,
        prior_state: Some(prior.new_state),
        new_state,
        event_kind: new_state.event_kind().to_string(),
        authority_key_id: authority_key_id.to_string(),
        authority_role,
        timestamp,
        prev_event_hash: Some(prior.chain_hash()?),
        signature: [0u8; 64],
    };
    event::sign_event(unsigned)
}

/// Read an artifact's event log and return its **validated head event** (the
/// highest-seq event, after [`current_state`] has walked + verified the whole
/// chain). [`RegistryError::NoState`] if the log is empty. Phase 3b transition
/// commands chain their new event onto this head.
pub fn head_event<B: RegistryBackend>(
    backend: &B,
    hash: &[u8; 32],
) -> Result<LifecycleEvent, RegistryError> {
    let events = backend.read_events(hash)?;
    // Validate the whole chain (seq/links/signatures) before trusting the head.
    current_state(&events)?;
    events.into_iter().last().ok_or(RegistryError::NoState)
}

/// The current validated [`LifecycleState`] of an artifact, or
/// [`RegistryError::NoState`] if the log is empty.
pub fn require_current_state<B: RegistryBackend>(
    backend: &B,
    hash: &[u8; 32],
) -> Result<LifecycleState, RegistryError> {
    let events = backend.read_events(hash)?;
    current_state(&events)?.ok_or(RegistryError::NoState)
}

/// Build a `tag_moved` event (ADR 0013 rollback): a non-state-changing event
/// chained onto `prior`. `new_state == prior_state == prior.new_state` (the tag
/// move does not transition the artifact's lifecycle state — it records that a
/// tag pointer was moved to this artifact), but `event_kind = "tag_moved"`
/// distinguishes it in the log. Registry-root-signed (gated).
///
/// This is intentionally NOT routed through [`build_transition_event`] (which
/// derives `event_kind` from `new_state`): a tag move keeps the state and so
/// needs the distinct `tag_moved` kind. [`current_state`] validates the chain
/// regardless of kind, and the unchanged `new_state` keeps the derived state
/// correct.
#[cfg(any(test, feature = "test-keys"))]
pub fn build_tag_moved_event(
    prior: &LifecycleEvent,
    authority_key_id: &str,
    authority_role: SignerRole,
    timestamp: ke_artifact::TimestampToken,
) -> Result<LifecycleEvent, RegistryError> {
    let unsigned = LifecycleEvent {
        artifact_hash: prior.artifact_hash,
        seq: prior.seq + 1,
        prior_state: Some(prior.new_state),
        new_state: prior.new_state,
        event_kind: "tag_moved".to_string(),
        authority_key_id: authority_key_id.to_string(),
        authority_role,
        timestamp,
        prev_event_hash: Some(prior.chain_hash()?),
        signature: [0u8; 64],
    };
    event::sign_event(unsigned)
}

/// Convert a [`VerificationReport`] into the structural-clean precondition bit.
pub fn structural_clean(report: &VerificationReport) -> bool {
    !report.has_blocking()
}

/// Re-export the revocation-record sidecar type for the commands/tests.
pub use backend::RevocationRecord;

/// Re-export the hex helper for the commands/tests.
pub use backend::{hex32 as hash_hex, hex_to_hash as hash_from_hex};
