//! Operator evaluation, bit-faithful to the Python `RuleRuntime`
//! (`src/production/executor.py` `OPERATORS`) and CPython value semantics.
//!
//! The truth table this reproduces was probed empirically against CPython 3.14
//! (recorded in the gate-3 log) and pinned in ADR 0008. Highlights:
//! - `bool` is an `int` (`True == 1`, `True > 0`).
//! - `==`/`!=` never bridge str and number (`"1" == 1` is false).
//! - ordering on incompatible types is a CPython `TypeError`, which the executor
//!   catches and treats as `false`.
//! - `in`/`not_in` is `str(actual) in {str(v) for v in list}` — string membership,
//!   total, never errors. An operand that is not a list makes `in` false.
//! - `exists` is `actual is not None`.

use crate::value::{operand_as_number, python_str_fact, python_str_scalar, FactValue};
use ke_core::ir::{Operator, ScalarValue};

/// Evaluate one condition: `actual <op> operand`. Total — never panics, never
/// errors (mirrors the Python executor).
pub fn evaluate(actual: &FactValue, op: Operator, operand: &ScalarValue) -> bool {
    match op {
        Operator::Eq => eval_eq(actual, operand),
        Operator::NotEq => !eval_eq(actual, operand),
        Operator::In => eval_in(actual, operand),
        Operator::NotIn => !eval_in(actual, operand),
        Operator::Gt => eval_ord(actual, operand, Ordering::Gt),
        Operator::Lt => eval_ord(actual, operand, Ordering::Lt),
        Operator::Gte => eval_ord(actual, operand, Ordering::Gte),
        Operator::Lte => eval_ord(actual, operand, Ordering::Lte),
        Operator::Exists => actual.is_present(),
    }
}

/// Python `actual == operand`.
fn eval_eq(actual: &FactValue, operand: &ScalarValue) -> bool {
    // Numeric tower: int/float/bool on the fact side vs Decimal/bool operand.
    if let (Some(a), Some(b)) = (actual.as_number(), operand_as_number(operand)) {
        // But a plain bool fact vs a string operand must NOT be numeric — that is
        // already excluded because a Str operand has no number. The only trap is
        // bool-vs-bool / number-vs-bool, which Python *does* treat numerically.
        return a == b;
    }
    match (actual, operand) {
        (FactValue::Str(a), ScalarValue::Str(b)) => a == b,
        (FactValue::List(a), ScalarValue::List(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| eval_eq(x, y))
        }
        // str↔number, bool↔str, null↔anything, list↔scalar → not equal.
        _ => false,
    }
}

#[derive(Clone, Copy)]
enum Ordering {
    Gt,
    Lt,
    Gte,
    Lte,
}

/// Python ordering with `except TypeError: return False`. Succeeds only for
/// number-vs-number (bool as 0/1) and str-vs-str (lexicographic); every other
/// combination is a `TypeError` → `false`.
fn eval_ord(actual: &FactValue, operand: &ScalarValue, ord: Ordering) -> bool {
    if let (Some(a), Some(b)) = (actual.as_number(), operand_as_number(operand)) {
        return apply_ord(a.partial_cmp(&b), ord);
    }
    if let (FactValue::Str(a), ScalarValue::Str(b)) = (actual, operand) {
        return apply_ord(Some(a.as_str().cmp(b.as_str())), ord);
    }
    // str↔number, null↔anything, bool↔str, list↔scalar → TypeError → false.
    false
}

fn apply_ord(cmp: Option<std::cmp::Ordering>, ord: Ordering) -> bool {
    use std::cmp::Ordering as C;
    // partial_cmp is None only for NaN, which JSON cannot carry; treat as false.
    let Some(c) = cmp else { return false };
    match ord {
        Ordering::Gt => c == C::Greater,
        Ordering::Lt => c == C::Less,
        Ordering::Gte => c != C::Less,
        Ordering::Lte => c != C::Greater,
    }
}

/// Python `str(actual) in {str(v) for v in operand}`. An operand that is not a
/// list yields `false` (matching `_eval_in` when `value_set` is absent and the
/// operand is not a sequence).
fn eval_in(actual: &FactValue, operand: &ScalarValue) -> bool {
    let ScalarValue::List(items) = operand else {
        return false;
    };
    let needle = python_str_fact(actual);
    items.iter().any(|e| python_str_scalar(e) == needle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ke_core::ir::ScalarValue as S;

    fn dec(m: i128, s: i8) -> S {
        S::Decimal {
            mantissa: m,
            scale: s,
        }
    }

    #[test]
    fn eq_numeric_tower() {
        // True == 1, False == 0, 1 == 1.0, 5.0 == 5
        assert!(eval_eq(&FactValue::Bool(true), &dec(1, 0)));
        assert!(eval_eq(&FactValue::Bool(false), &dec(0, 0)));
        assert!(eval_eq(&FactValue::Int(1), &dec(10, 1))); // 1 == 1.0
        assert!(eval_eq(&FactValue::Float(5.0), &dec(5, 0))); // 5.0 == 5
        assert!(eval_eq(&FactValue::Bool(true), &S::Bool(true)));
        assert!(!eval_eq(&FactValue::Bool(true), &S::Bool(false)));
    }

    #[test]
    fn eq_never_bridges_str_and_number() {
        assert!(!eval_eq(&FactValue::Str("1".into()), &dec(1, 0))); // "1" == 1 -> False
        assert!(!eval_eq(&FactValue::Int(1), &S::Str("1".into()))); // 1 == "1" -> False
        assert!(!eval_eq(&FactValue::Bool(true), &S::Str("true".into()))); // True == "true" -> False
        assert!(eval_eq(
            &FactValue::Str("art".into()),
            &S::Str("art".into())
        ));
    }

    #[test]
    fn eq_null_equals_nothing() {
        assert!(!eval_eq(&FactValue::Null, &S::Bool(true)));
        assert!(!eval_eq(&FactValue::Null, &S::Str("x".into())));
        assert!(!eval_eq(&FactValue::Null, &dec(0, 0)));
    }

    #[test]
    fn ordering_numeric_and_bool_is_int() {
        assert!(eval_ord(&FactValue::Int(5), &dec(3, 0), Ordering::Gt));
        assert!(!eval_ord(&FactValue::Int(3), &dec(5, 0), Ordering::Gt));
        assert!(eval_ord(&FactValue::Bool(true), &dec(0, 0), Ordering::Gt)); // True > 0
        assert!(!eval_ord(&FactValue::Bool(true), &dec(0, 0), Ordering::Lt));
        assert!(eval_ord(&FactValue::Float(5.5), &dec(5, 0), Ordering::Gte));
        assert!(eval_ord(&FactValue::Int(5), &dec(5, 0), Ordering::Lte));
    }

    #[test]
    fn ordering_typeerror_is_false() {
        // str vs number, null vs number → TypeError → false (both directions of op).
        assert!(!eval_ord(
            &FactValue::Str("a".into()),
            &dec(5, 0),
            Ordering::Gt
        ));
        assert!(!eval_ord(
            &FactValue::Str("a".into()),
            &dec(5, 0),
            Ordering::Lt
        ));
        assert!(!eval_ord(&FactValue::Null, &dec(5, 0), Ordering::Gte));
        assert!(!eval_ord(
            &FactValue::Bool(true),
            &S::Str("x".into()),
            Ordering::Gt
        ));
    }

    #[test]
    fn ordering_str_str_lexicographic() {
        assert!(eval_ord(
            &FactValue::Str("b".into()),
            &S::Str("a".into()),
            Ordering::Gt
        ));
        assert!(!eval_ord(
            &FactValue::Str("a".into()),
            &S::Str("b".into()),
            Ordering::Gt
        ));
    }

    #[test]
    fn in_uses_python_str_coercion() {
        let list = S::List(vec![S::Str("art".into()), S::Str("stablecoin".into())]);
        assert!(eval_in(&FactValue::Str("art".into()), &list));
        assert!(!eval_in(&FactValue::Str("emt".into()), &list));

        // str(True) == "True" lands in a {"True","False"} set.
        let bools = S::List(vec![S::Bool(true), S::Bool(false)]);
        assert!(eval_in(&FactValue::Bool(true), &bools));

        // str(5) in {"5","6"} -> True; str(5.0) in {"5","6"} -> False.
        let nums = S::List(vec![dec(5, 0), dec(6, 0)]);
        assert!(eval_in(&FactValue::Int(5), &nums));
        assert!(!eval_in(&FactValue::Float(5.0), &nums));
    }

    #[test]
    fn in_non_list_operand_is_false() {
        assert!(!eval_in(
            &FactValue::Str("art".into()),
            &S::Str("art".into())
        ));
        // not_in of a non-list operand is therefore true.
        assert!(evaluate(
            &FactValue::Str("art".into()),
            Operator::NotIn,
            &S::Str("art".into())
        ));
    }

    #[test]
    fn exists_only_null_is_false() {
        assert!(!evaluate(
            &FactValue::Null,
            Operator::Exists,
            &S::Bool(false)
        ));
        assert!(evaluate(
            &FactValue::Bool(false),
            Operator::Exists,
            &S::Bool(false)
        ));
        assert!(evaluate(
            &FactValue::Int(0),
            Operator::Exists,
            &S::Bool(false)
        ));
        assert!(evaluate(
            &FactValue::Str(String::new()),
            Operator::Exists,
            &S::Bool(false)
        ));
    }

    #[test]
    fn ne_is_negation_of_eq() {
        assert!(evaluate(
            &FactValue::Str("a".into()),
            Operator::NotEq,
            &S::Str("b".into())
        ));
        assert!(!evaluate(
            &FactValue::Str("a".into()),
            Operator::NotEq,
            &S::Str("a".into())
        ));
        // None != "art" -> True (Python).
        assert!(evaluate(
            &FactValue::Null,
            Operator::NotEq,
            &S::Str("art".into())
        ));
    }
}
