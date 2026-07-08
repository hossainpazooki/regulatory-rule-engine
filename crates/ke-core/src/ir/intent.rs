//! IntentSpec IR — the canonical payload for the `IntentSpec` artifact kind
//! (ADR-0021).
//!
//! An IntentSpec declares, per action class, the authorization criteria that
//! gate an action (name + threshold + volatility tag), an idempotency-key
//! definition (what counts as a duplicate), and the source spans the criteria
//! derive from. It is a *shape* layer only: all canonicalization logic lives in
//! [`crate::canonical`], which walks these structures directly.
//!
//! Per ADR-0003 (no floats in the IR), the criterion threshold is carried as a
//! [`ScalarValue`] (exact decimal), never an `f64`.

use crate::ir::{ScalarValue, SourceSpan};
use serde::{Deserialize, Serialize};

/// Whether an authorization criterion's threshold is a fixed constant or may
/// move between evaluations. Enum variants are **append-only** (canonical
/// discriminant stability, ADR-0002).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Volatility {
    /// The threshold is fixed for the lifetime of the artifact.
    Stable,
    /// The threshold may vary (e.g. a rate or market-derived bound).
    Volatile,
}

/// A single authorization criterion: a named check, its exact-decimal
/// threshold, and a volatility tag. `threshold` is a [`ScalarValue`] so the IR
/// stays float-free (ADR-0003).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorizationCriterion {
    pub name: String,
    pub threshold: ScalarValue,
    pub volatility: Volatility,
}

/// Idempotency-key definition: the payer-scoped key fields that identify a
/// unique action and the scope over which duplicates are collapsed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdempotencyDef {
    /// Field names (payer-scoped) whose tuple forms the idempotency key.
    pub key_fields: Vec<String>,
    /// The scope over which the key is unique (e.g. an action-class scope).
    pub scope: String,
}

/// The IntentSpec payload IR (ADR-0021). Carried inside the envelope as
/// `ArtifactPayload::IntentSpec`, it flows through the identical
/// hash/sign/attest/registry lifecycle as a rule payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntentSpecIR {
    /// The action class these criteria authorize.
    pub action_class: String,
    /// Authorization criteria, in declared order.
    pub criteria: Vec<AuthorizationCriterion>,
    /// How duplicate actions are identified.
    pub idempotency: IdempotencyDef,
    /// Source spans the criteria derive from (analogous to the rule
    /// `source_span_index`, but carried on the payload itself).
    pub source_spans: Vec<SourceSpan>,
}
