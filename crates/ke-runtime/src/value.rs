//! Fact values and the CPython-faithful coercions the executor needs.
//!
//! A scenario is a JSON object of facts. A fact value is one of the JSON scalar
//! kinds plus lists. We keep **`Int` and `Float` distinct** (decoded from the
//! JSON number literal kind) because Python's `str()` distinguishes them —
//! `str(5) == "5"` but `str(5.0) == "5.0"` — and that drives the `in`/`not_in`
//! set-membership coercion (see [`python_str_fact`] and ADR 0008).
//!
//! A missing fact and an explicit `null` are the same to Python's
//! `facts.get(field)` (both `None`); [`lookup`] collapses absent → [`FactValue::Null`].

use ke_core::ir::ScalarValue;
use serde_json::Value;
use std::collections::BTreeMap;

/// A fact value, mirroring the Python types a fact takes at runtime.
#[derive(Clone, Debug, PartialEq)]
pub enum FactValue {
    /// A missing field, or an explicit JSON `null`.
    Null,
    Bool(bool),
    /// A JSON integer literal (Python `int`).
    Int(i128),
    /// A JSON non-integer number literal (Python `float`).
    Float(f64),
    Str(String),
    List(Vec<FactValue>),
}

/// A fact set: field → value. `BTreeMap` for deterministic key order in any
/// emitted JSON.
pub type Facts = BTreeMap<String, FactValue>;

impl FactValue {
    /// Convert one `serde_json::Value` to a `FactValue`, preserving the
    /// integer-vs-float distinction the way Python's `json.loads` does. A nested
    /// JSON object is outside the flat-facts contract; it maps to its compact
    /// JSON string (so `exists` is still true and it equals no scalar).
    pub fn from_json(v: &Value) -> FactValue {
        match v {
            Value::Null => FactValue::Null,
            Value::Bool(b) => FactValue::Bool(*b),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    FactValue::Int(i as i128)
                } else if let Some(u) = n.as_u64() {
                    FactValue::Int(u as i128)
                } else {
                    // Non-integer (or out of i64/u64 range): Python keeps huge
                    // ints exact; serde_json falls back to f64. Bounded away by
                    // the scenario generator (ADR 0008).
                    FactValue::Float(n.as_f64().unwrap_or(f64::NAN))
                }
            }
            Value::String(s) => FactValue::Str(s.clone()),
            Value::Array(a) => FactValue::List(a.iter().map(FactValue::from_json).collect()),
            Value::Object(_) => FactValue::Str(v.to_string()),
        }
    }

    /// `actual is not None` (the `exists` operator).
    pub fn is_present(&self) -> bool {
        !matches!(self, FactValue::Null)
    }

    /// As a Python-numeric `f64` if this value participates in the numeric tower
    /// (`int`/`float`/`bool`); `None` for `str`/`null`/`list`. Mirrors that
    /// Python's `bool` is an `int` (`True == 1`).
    pub fn as_number(&self) -> Option<f64> {
        match self {
            FactValue::Int(i) => Some(*i as f64),
            FactValue::Float(f) => Some(*f),
            FactValue::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            _ => None,
        }
    }
}

/// Build a validated `Facts` map from a JSON object. The facts payload must be a
/// JSON object (the only boundary "error class" — evaluation itself is total,
/// ADR 0008).
pub fn facts_from_json(v: &Value) -> Result<Facts, String> {
    let obj = v
        .as_object()
        .ok_or_else(|| "facts must be a JSON object".to_string())?;
    Ok(obj
        .iter()
        .map(|(k, val)| (k.clone(), FactValue::from_json(val)))
        .collect())
}

/// Look up a field, collapsing an absent key to [`FactValue::Null`] exactly as
/// Python's `facts.get(field)` returns `None`.
pub fn lookup<'a>(facts: &'a Facts, field: &str) -> &'a FactValue {
    facts.get(field).unwrap_or(&FactValue::Null)
}

// --- Python str() coercion -------------------------------------------------

/// Python `str()` of a fact value. Used to build the left side of the
/// `in`/`not_in` membership test (`str(actual) in value_set`).
pub fn python_str_fact(v: &FactValue) -> String {
    match v {
        FactValue::Null => "None".to_string(),
        FactValue::Bool(true) => "True".to_string(),
        FactValue::Bool(false) => "False".to_string(),
        FactValue::Int(i) => i.to_string(),
        FactValue::Float(f) => python_float_str(*f),
        FactValue::Str(s) => s.clone(),
        FactValue::List(items) => python_list_repr(items.iter().map(python_repr_fact)),
    }
}

/// Python `str()` of a rule operand scalar. Used to build the `value_set`
/// elements (`{str(v) for v in list}`). A `Decimal` with `scale == 0` came from
/// an integer literal (Python `int` → `"5"`); `scale > 0` came from a literal
/// with a fractional part (Python `float` → `"5.0"`, `"0.9"`). See ADR 0008.
pub fn python_str_scalar(v: &ScalarValue) -> String {
    match v {
        ScalarValue::Str(s) => s.clone(),
        ScalarValue::Bool(true) => "True".to_string(),
        ScalarValue::Bool(false) => "False".to_string(),
        ScalarValue::Decimal { mantissa, scale } => {
            if *scale <= 0 {
                decimal_to_string(*mantissa, *scale)
            } else {
                python_float_str(decimal_to_f64(*mantissa, *scale))
            }
        }
        ScalarValue::List(items) => python_list_repr(items.iter().map(python_repr_scalar)),
    }
}

/// Python `repr()` of a fact element inside a list (`str(list)` uses element
/// `repr`, which quotes strings). Defensive — the generator never puts a list on
/// an `in`/`not_in` field, so this is not exercised in parity runs.
fn python_repr_fact(v: &FactValue) -> String {
    match v {
        FactValue::Str(s) => format!("'{s}'"),
        other => python_str_fact(other),
    }
}

fn python_repr_scalar(v: &ScalarValue) -> String {
    match v {
        ScalarValue::Str(s) => format!("'{s}'"),
        other => python_str_scalar(other),
    }
}

fn python_list_repr(parts: impl Iterator<Item = String>) -> String {
    let inner: Vec<String> = parts.collect();
    format!("[{}]", inner.join(", "))
}

/// Python `str(float)`: shortest round-trip, always carrying a decimal point in
/// the fixed-notation range. Rust's `{:?}` for `f64` is the same shortest
/// round-trip and always includes a `.` or exponent, so it matches Python over
/// the non-scientific magnitude range the generator stays within (ADR 0008).
pub fn python_float_str(f: f64) -> String {
    if f == 0.0 {
        return if f.is_sign_negative() {
            "-0.0".to_string()
        } else {
            "0.0".to_string()
        };
    }
    format!("{f:?}")
}

// --- decimal reconstruction ------------------------------------------------

/// Render an exact decimal (`mantissa × 10^-scale`) as its plain decimal string.
/// `scale == 0` → integer; `scale > 0` → fractional; `scale < 0` (not produced
/// by the corpus parser) → integer with trailing zeros.
pub fn decimal_to_string(mantissa: i128, scale: i8) -> String {
    let sign = if mantissa < 0 { "-" } else { "" };
    let digits = mantissa.unsigned_abs().to_string();
    if scale <= 0 {
        let zeros = "0".repeat((-(scale as i32)).max(0) as usize);
        return format!("{sign}{digits}{zeros}");
    }
    let scale = scale as usize;
    if digits.len() <= scale {
        let pad = "0".repeat(scale - digits.len());
        format!("{sign}0.{pad}{digits}")
    } else {
        let point = digits.len() - scale;
        format!("{sign}{}.{}", &digits[..point], &digits[point..])
    }
}

/// Reconstruct an operand decimal as the `f64` Python would hold for the same
/// literal: format the shortest decimal string and parse it once
/// (correctly-rounded in both Rust and Python). NOT `mantissa as f64 *
/// 10^-scale` (two rounding steps can disagree). See ADR 0008.
pub fn decimal_to_f64(mantissa: i128, scale: i8) -> f64 {
    decimal_to_string(mantissa, scale)
        .parse::<f64>()
        .unwrap_or(f64::NAN)
}

/// As a Python-numeric `f64` if the operand participates in the numeric tower
/// (`Decimal`/`Bool`); `None` for `str`/`list`.
pub fn operand_as_number(v: &ScalarValue) -> Option<f64> {
    match v {
        ScalarValue::Decimal { mantissa, scale } => Some(decimal_to_f64(*mantissa, *scale)),
        ScalarValue::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_str_fact_matches_cpython() {
        // Golden table from an empirical CPython 3.14 probe (see gate-3 log).
        assert_eq!(python_str_fact(&FactValue::Bool(true)), "True");
        assert_eq!(python_str_fact(&FactValue::Bool(false)), "False");
        assert_eq!(python_str_fact(&FactValue::Int(5)), "5");
        assert_eq!(python_str_fact(&FactValue::Int(0)), "0");
        assert_eq!(python_str_fact(&FactValue::Int(-3)), "-3");
        assert_eq!(python_str_fact(&FactValue::Float(5.0)), "5.0");
        assert_eq!(python_str_fact(&FactValue::Float(5.5)), "5.5");
        assert_eq!(python_str_fact(&FactValue::Float(0.9)), "0.9");
        assert_eq!(python_str_fact(&FactValue::Float(-1.5)), "-1.5");
        assert_eq!(python_str_fact(&FactValue::Str("art".into())), "art");
        assert_eq!(python_str_fact(&FactValue::Null), "None");
    }

    #[test]
    fn python_str_scalar_int_vs_float() {
        // scale 0 → Python int "5"; scale>0 → Python float "5.0"/"0.9".
        assert_eq!(
            python_str_scalar(&ScalarValue::Decimal {
                mantissa: 5,
                scale: 0
            }),
            "5"
        );
        assert_eq!(
            python_str_scalar(&ScalarValue::Decimal {
                mantissa: 50,
                scale: 1
            }),
            "5.0"
        );
        assert_eq!(
            python_str_scalar(&ScalarValue::Decimal {
                mantissa: 90,
                scale: 2
            }),
            "0.9"
        );
        assert_eq!(python_str_scalar(&ScalarValue::Str("EU".into())), "EU");
        assert_eq!(python_str_scalar(&ScalarValue::Bool(true)), "True");
    }

    #[test]
    fn decimal_string_and_f64() {
        assert_eq!(decimal_to_string(5, 0), "5");
        assert_eq!(decimal_to_string(90, 2), "0.90");
        assert_eq!(decimal_to_string(50, 1), "5.0");
        assert_eq!(decimal_to_string(-15, 1), "-1.5");
        assert_eq!(decimal_to_string(123, 2), "1.23");
        assert_eq!(decimal_to_string(3, 2), "0.03");
        assert_eq!(decimal_to_f64(90, 2), 0.9);
        assert_eq!(decimal_to_f64(50, 1), 5.0);
        assert_eq!(decimal_to_f64(3000000, 0), 3_000_000.0);
    }

    #[test]
    fn json_preserves_int_vs_float() {
        let v: Value = serde_json::from_str("5").unwrap();
        assert_eq!(FactValue::from_json(&v), FactValue::Int(5));
        let v: Value = serde_json::from_str("5.0").unwrap();
        assert_eq!(FactValue::from_json(&v), FactValue::Float(5.0));
        let v: Value = serde_json::from_str("true").unwrap();
        assert_eq!(FactValue::from_json(&v), FactValue::Bool(true));
    }

    #[test]
    fn lookup_absent_is_null() {
        let facts: Facts = [("a".to_string(), FactValue::Int(1))].into_iter().collect();
        assert_eq!(lookup(&facts, "a"), &FactValue::Int(1));
        assert_eq!(lookup(&facts, "missing"), &FactValue::Null);
    }
}
