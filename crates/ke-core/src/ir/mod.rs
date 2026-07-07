//! Intermediate-representation types.
//!
//! These are the **un-lowered authoring tree** — the shapes a rule has after
//! parsing YAML but before the AST→IR flattening that Gate 2's compiler
//! performs. They are ported (as *shapes*, not semantics — brief principle 7)
//! from the platform's `src/rules/service.py` (`Rule`, `ConditionGroupSpec`,
//! `ConditionSpec`, `DecisionNode`, `DecisionLeaf`, `ObligationSpec`,
//! `SourceRef`).
//!
//! Cross-language parity is by **canonical bytes**, not struct-field matching
//! (brief principle 5), so Rust-side names and casing may differ from Python.
//!
//! Every field is `pub`: the canonical encoder/decoder (in [`crate::canonical`])
//! walks these structures directly, keeping all canonicalization logic out of
//! the IR modules so the IR stays a pure shape layer.

pub mod check;
pub mod condition;
pub mod decision;
pub mod intent;
pub mod obligation;
pub mod rule;
pub mod source_span;
pub mod time;

pub use check::CompiledCheck;
pub use condition::{Condition, ConditionGroupSpec, ConditionOrGroup, Operator, ScalarValue};
pub use decision::{DecisionEntry, DecisionLeaf, DecisionNode};
pub use intent::{AuthorizationCriterion, IdempotencyDef, IntentSpecIR, Volatility};
pub use obligation::ObligationSpec;
pub use rule::{ProvenanceMarker, RuleIR};
pub use source_span::{ByteRange, DocumentRef, SourceSpan};
pub use time::{EffectiveTimePolicy, EffectiveWindow, JurisdictionDate, TimeZone};
