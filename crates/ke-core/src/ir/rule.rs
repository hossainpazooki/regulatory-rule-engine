//! The top-level rule IR and its provenance marker.

use super::condition::ConditionGroupSpec;
use super::decision::DecisionEntry;
use super::obligation::ObligationSpec;
use super::source_span::DocumentRef;
use super::time::EffectiveWindow;
use serde::{Deserialize, Serialize};

/// A typed marker that makes the candidate-vs-attested distinction explicit in
/// the type system rather than via a flippable string field (brief
/// principle 2). Gate 1 defines the enum; Gate 4 enforces the state-machine
/// transitions (spec § 9) and binds attestations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProvenanceMarker {
    Candidate { proposal_id: Option<String> },
    StructurallyVerified,
    MlChecked { policy_version: String },
    ExpertAttested { attestation_count: u16 },
    Published { environment: String },
    Deprecated,
    Revoked,
}

/// The complete intermediate representation of a rule — the un-lowered
/// authoring tree. Field **declaration order is part of the canonical-encoding
/// contract** (see `docs/canonical-encoding.md`); reordering requires bumping
/// [`crate::version::CANONICALIZATION_VERSION`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleIR {
    pub rule_id: String,
    pub rule_version: String,
    pub description: Option<String>,
    /// Classification tags. Encoded as a canonically-sorted, duplicate-free set
    /// (brief § 4.4).
    pub tags: Option<Vec<String>>,
    pub applies_if: Option<ConditionGroupSpec>,
    pub decision_tree: DecisionEntry,
    pub obligations: Vec<ObligationSpec>,
    pub source: DocumentRef,
    pub interpretation_notes: Option<String>,
    pub effective_window: EffectiveWindow,
    pub provenance: ProvenanceMarker,
}
