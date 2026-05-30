//! Condition expressions: operators, scalar values, conditions, and the
//! `all`/`any` grouping. Ported from the platform's `ConditionSpec` /
//! `ConditionGroupSpec` (`src/rules/service.py`).

use serde::{Deserialize, Serialize};

/// The closed set of comparison operators. Declaration order is the canonical
/// discriminant order (brief § 4.6 / § 5.6); the `serde` rename gives the YAML
/// surface form used by the corpus, for round-trip and for JSON Schema.
///
/// No regex, no fuzzy operator strings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Operator {
    #[serde(rename = "==")]
    Eq,
    #[serde(rename = "!=")]
    NotEq,
    #[serde(rename = "in")]
    In,
    #[serde(rename = "not_in")]
    NotIn,
    #[serde(rename = ">")]
    Gt,
    #[serde(rename = "<")]
    Lt,
    #[serde(rename = ">=")]
    Gte,
    #[serde(rename = "<=")]
    Lte,
    #[serde(rename = "exists")]
    Exists,
}

/// A typed scalar operand. **Floats are not representable** — numbers are exact
/// decimals (`mantissa × 10^-scale`); see ADR 0003. The `List` arm carries the
/// operands of `in` / `not_in` and preserves author order (it is a sequence,
/// not the canonically-sorted `tags` set).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScalarValue {
    Str(String),
    Bool(bool),
    /// Exact decimal: value is `mantissa × 10^(-scale)`. Canonical form carries
    /// no trailing zeros (see [`crate::canonical`]).
    Decimal {
        mantissa: i128,
        scale: i8,
    },
    List(Vec<ScalarValue>),
}

impl ScalarValue {
    /// Convenience constructor for an integer (`scale = 0`).
    pub fn int(n: i128) -> Self {
        ScalarValue::Decimal {
            mantissa: n,
            scale: 0,
        }
    }
}

/// A single condition: a field, an operator, and an operand.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Condition {
    pub field: String,
    pub operator: Operator,
    pub value: ScalarValue,
    pub description: Option<String>,
}

/// Either a leaf condition or a nested group. Externally tagged so the encoding
/// is unambiguous under postcard (which is not self-describing and cannot drive
/// `serde(untagged)`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConditionOrGroup {
    Condition(Condition),
    Group(ConditionGroupSpec),
}

/// A discriminated `all` (AND) / `any` (OR) group of conditions. Exactly one of
/// the two is expected to be populated for a well-formed rule; Gate 1 does not
/// enforce that (no semantics — brief principle 7).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConditionGroupSpec {
    pub all: Option<Vec<ConditionOrGroup>>,
    pub any: Option<Vec<ConditionOrGroup>>,
}
