//! YAML scalar → `ke_core::ir::ScalarValue`.
//!
//! Numbers become **exact decimals** (`mantissa·10^-scale`), never `f64`
//! (ADR 0003). Type inference mirrors `yaml.safe_load` / the platform's
//! `RuleLoader._parse_value`: an unquoted `true`/`false` is a bool, an unquoted
//! numeric literal is a decimal, everything else is a string. A **quoted**
//! scalar (`may_coerce == false`) is always a string, even if it looks like a
//! number or bool — matching YAML semantics and the platform.

use crate::ast::YamlSpan;
use crate::error::CompileError;
use ke_core::ir::ScalarValue;
use marked_yaml::types::{MarkedScalarNode, Node};

/// Convert a YAML node (scalar or sequence) into a `ScalarValue`. Sequences
/// become `List`; mappings are rejected (a condition operand is never a map).
pub fn node_to_value(node: &Node) -> Result<ScalarValue, CompileError> {
    match node {
        Node::Scalar(s) => Ok(scalar_to_value(s)),
        Node::Sequence(seq) => {
            let mut items = Vec::with_capacity(seq.len());
            for el in seq.iter() {
                items.push(node_to_value(el)?);
            }
            Ok(ScalarValue::List(items))
        }
        Node::Mapping(m) => Err(CompileError::new(
            "a condition value must be a scalar or list, not a mapping",
            YamlSpan::from_marked(m.span()),
        )),
    }
}

/// Convert a scalar node into a `ScalarValue` with YAML-faithful type inference.
pub fn scalar_to_value(node: &MarkedScalarNode) -> ScalarValue {
    // Quoted scalars never coerce → always a string.
    if !node.may_coerce() {
        return ScalarValue::Str(node.as_str().to_string());
    }
    if let Some(b) = node.as_bool() {
        return ScalarValue::Bool(b);
    }
    let raw = node.as_str();
    if let Some(d) = parse_decimal(raw) {
        return d;
    }
    ScalarValue::Str(raw.to_string())
}

/// Parse an integer or fixed-point decimal literal into `ScalarValue::Decimal`.
/// Returns `None` for anything that is not a plain `[-]?digits(.digits)?`
/// (e.g. scientific notation, hex, or non-numeric strings — those stay strings).
/// Shared with `python_import` (which feeds it the JSON literal of a number).
pub(crate) fn parse_decimal(s: &str) -> Option<ScalarValue> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    let (neg, body) = match t.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };

    let (int_part, frac_part) = match body.split_once('.') {
        Some((i, f)) => (i, f),
        None => (body, ""),
    };

    // Both parts must be ASCII digits; at least one digit total.
    if int_part.is_empty() && frac_part.is_empty() {
        return None;
    }
    if !int_part.chars().all(|c| c.is_ascii_digit())
        || !frac_part.chars().all(|c| c.is_ascii_digit())
    {
        return None;
    }

    let digits: String = format!("{int_part}{frac_part}");
    let mantissa_abs: i128 = digits.parse().ok()?;
    let mantissa = if neg { -mantissa_abs } else { mantissa_abs };
    let scale: i8 = i8::try_from(frac_part.len()).ok()?;

    Some(ScalarValue::Decimal { mantissa, scale })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integers_and_decimals() {
        assert_eq!(
            parse_decimal("5"),
            Some(ScalarValue::Decimal {
                mantissa: 5,
                scale: 0
            })
        );
        assert_eq!(
            parse_decimal("0.90"),
            Some(ScalarValue::Decimal {
                mantissa: 90,
                scale: 2
            })
        );
        assert_eq!(
            parse_decimal("-1.5"),
            Some(ScalarValue::Decimal {
                mantissa: -15,
                scale: 1
            })
        );
        assert_eq!(
            parse_decimal("1000000"),
            Some(ScalarValue::Decimal {
                mantissa: 1_000_000,
                scale: 0
            })
        );
    }

    #[test]
    fn non_numeric_stays_none() {
        assert_eq!(parse_decimal("EU"), None);
        assert_eq!(parse_decimal("1e5"), None);
        assert_eq!(parse_decimal("0x10"), None);
        assert_eq!(parse_decimal(""), None);
    }
}
