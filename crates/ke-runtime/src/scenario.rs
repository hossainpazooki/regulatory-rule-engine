//! Structure-aware scenario generation for the equivalence harness and the
//! property/metamorphic tests.
//!
//! A `Scenario` is a `(rule_id, label, facts)` triple; `facts` is a JSON object
//! emitted as one JSONL line and parsed identically by Rust (`serde_json`) and
//! Python (`json.loads`), so number bits match across the boundary.
//!
//! Generation is **deterministic** — a small self-contained PRNG (no `proptest`,
//! see ADR 0008 / the gate-3 log) seeded per run so the harness records an exact
//! seed. It walks a rule's conditions to produce facts that exercise each branch,
//! threshold boundaries, missing fields, wrong types, and irrelevant facts, then
//! tops up with seeded fuzz. Equivalence is checked on whatever facts result, so
//! imperfect branch-reaching never invalidates a run — it only affects coverage.
//!
//! **Generator invariant (ADR 0008):** a field that appears in any `in`/`not_in`
//! condition is only ever given a string/bool/absent value, never a number —
//! removing the only realistic `str(float)` cross-language hazard.

use crate::exec::{flattened, Mode};
use crate::value::{decimal_to_f64, facts_from_json, Facts};
use ke_core::ir::{Condition, DecisionEntry, Operator, RuleIR, ScalarValue};
use serde_json::{Map, Number, Value};
use std::collections::BTreeSet;

/// A generated scenario.
#[derive(Clone, Debug)]
pub struct Scenario {
    pub rule_id: String,
    pub label: String,
    pub facts: Value,
}

impl Scenario {
    /// The facts as the executor's `Facts` map.
    pub fn facts_map(&self) -> Facts {
        // Generated facts are always a JSON object, so this never errors.
        facts_from_json(&self.facts).unwrap_or_default()
    }
}

/// A tiny deterministic PRNG (SplitMix64). Reproducible from a seed; no external
/// crate, no `getrandom`/raw-dylib (which the windows-gnu toolchain can't build).
#[derive(Clone)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng { state: seed }
    }
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in `[0, n)` (n > 0).
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

/// Stable per-rule seed derived from a global seed + the rule id (so each rule's
/// fuzz is independent yet reproducible).
fn rule_seed(global: u64, rule_id: &str) -> u64 {
    let mut h = global ^ 0xD1B5_4A32_D192_ED03;
    for b in rule_id.bytes() {
        h = (h ^ b as u64).wrapping_mul(0x0100_0000_01B3); // FNV-ish
    }
    h
}

// --- operand → JSON + branch-driving values --------------------------------

fn operand_to_json(v: &ScalarValue) -> Value {
    match v {
        ScalarValue::Str(s) => Value::String(s.clone()),
        ScalarValue::Bool(b) => Value::Bool(*b),
        ScalarValue::Decimal { mantissa, scale } => decimal_json(*mantissa, *scale),
        ScalarValue::List(items) => Value::Array(items.iter().map(operand_to_json).collect()),
    }
}

/// JSON for an exact decimal: integer literal when `scale <= 0` (Python `int`),
/// float literal otherwise (Python `float`). Matches the operand's Python type.
fn decimal_json(mantissa: i128, scale: i8) -> Value {
    if scale <= 0 {
        match i64::try_from(mantissa) {
            Ok(i) => Value::Number(Number::from(i)),
            Err(_) => float_json(mantissa as f64),
        }
    } else {
        float_json(decimal_to_f64(mantissa, scale))
    }
}

fn float_json(f: f64) -> Value {
    Number::from_f64(f)
        .map(Value::Number)
        .unwrap_or(Value::Null)
}

/// A value the field can take, or its deliberate absence.
#[derive(Clone, Debug)]
enum Gen {
    Set(Value),
    Absent,
}

/// Shift a decimal operand by `delta` units in its last place (for threshold
/// boundaries); non-decimal operands are returned unshifted.
fn shift(operand: &ScalarValue, delta: i128) -> Value {
    match operand {
        ScalarValue::Decimal { mantissa, scale } => decimal_json(mantissa + delta, *scale),
        other => operand_to_json(other),
    }
}

/// A value distinct from `operand` (to falsify `==` / satisfy `!=`).
fn different(operand: &ScalarValue) -> Value {
    match operand {
        ScalarValue::Str(s) => Value::String(format!("{s}__NOMATCH")),
        ScalarValue::Bool(b) => Value::Bool(!b),
        ScalarValue::Decimal { mantissa, scale } => decimal_json(mantissa + 1, *scale),
        ScalarValue::List(_) => Value::String("__NOMATCH__".into()),
    }
}

const NON_MEMBER: &str = "__NOT_IN_SET__";

fn first_member(operand: &ScalarValue) -> Value {
    match operand {
        ScalarValue::List(items) if !items.is_empty() => operand_to_json(&items[0]),
        _ => Value::String(NON_MEMBER.into()),
    }
}

/// A value that makes `actual <op> operand` true.
fn value_true(op: Operator, operand: &ScalarValue) -> Gen {
    match op {
        Operator::Eq => Gen::Set(operand_to_json(operand)),
        Operator::NotEq => Gen::Set(different(operand)),
        Operator::In => Gen::Set(first_member(operand)),
        Operator::NotIn => Gen::Set(Value::String(NON_MEMBER.into())),
        Operator::Gt => Gen::Set(shift(operand, 1)),
        Operator::Lt => Gen::Set(shift(operand, -1)),
        Operator::Gte => Gen::Set(operand_to_json(operand)),
        Operator::Lte => Gen::Set(operand_to_json(operand)),
        Operator::Exists => Gen::Set(Value::String("present".into())),
    }
}

/// A value that makes `actual <op> operand` false.
fn value_false(op: Operator, operand: &ScalarValue) -> Gen {
    match op {
        Operator::Eq => Gen::Set(different(operand)),
        Operator::NotEq => Gen::Set(operand_to_json(operand)),
        Operator::In => Gen::Set(Value::String(NON_MEMBER.into())),
        Operator::NotIn => Gen::Set(first_member(operand)),
        Operator::Gt => Gen::Set(operand_to_json(operand)),
        Operator::Lt => Gen::Set(operand_to_json(operand)),
        Operator::Gte => Gen::Set(shift(operand, -1)),
        Operator::Lte => Gen::Set(shift(operand, 1)),
        Operator::Exists => Gen::Absent,
    }
}

// --- rule catalog ----------------------------------------------------------

/// Every condition observed in a rule (applicability flattened + each decision
/// node), plus which fields are used with `in`/`not_in`.
pub struct Catalog<'a> {
    pub conditions: Vec<&'a Condition>,
    pub in_fields: BTreeSet<String>,
}

impl<'a> Catalog<'a> {
    pub fn from_rule(rule: &'a RuleIR) -> Self {
        let mut conditions = Vec::new();
        if let Some(g) = &rule.applies_if {
            let (checks, _) = flattened(g);
            conditions.extend(checks);
        }
        collect_tree(&rule.decision_tree, &mut conditions);
        let in_fields = conditions
            .iter()
            .filter(|c| matches!(c.operator, Operator::In | Operator::NotIn))
            .map(|c| c.field.clone())
            .collect();
        Catalog {
            conditions,
            in_fields,
        }
    }
}

fn collect_tree<'a>(entry: &'a DecisionEntry, out: &mut Vec<&'a Condition>) {
    if let DecisionEntry::Node(n) = entry {
        out.push(&n.condition);
        collect_tree(&n.true_branch, out);
        collect_tree(&n.false_branch, out);
    }
}

// --- applicability + path facts --------------------------------------------

/// Facts that make `applies_if` hold (or fail), mirroring the executor's
/// flatten+mode evaluation.
fn applicability_pairs(rule: &RuleIR, satisfy: bool) -> Vec<(String, Gen)> {
    let Some(group) = &rule.applies_if else {
        return Vec::new(); // no group → always applicable; nothing to set
    };
    let (checks, mode) = flattened(group);
    if checks.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    match (satisfy, mode) {
        // applicable: every check true (sufficient for both `all` and `any`).
        (true, _) => {
            for c in &checks {
                out.push((c.field.clone(), value_true(c.operator, &c.value)));
            }
        }
        // not applicable, `all`: first check false is enough.
        (false, Mode::All) => {
            let c = checks[0];
            out.push((c.field.clone(), value_false(c.operator, &c.value)));
        }
        // not applicable, `any`: every check must be false.
        (false, Mode::Any) => {
            for c in &checks {
                out.push((c.field.clone(), value_false(c.operator, &c.value)));
            }
        }
    }
    out
}

/// Enumerate every root-to-leaf path as the list of branch decisions taken to
/// reach the leaf.
fn enumerate_paths(tree: &DecisionEntry) -> Vec<Vec<(String, Gen)>> {
    let mut out = Vec::new();
    walk_paths(tree, Vec::new(), &mut out);
    out
}

fn walk_paths(tree: &DecisionEntry, prefix: Vec<(String, Gen)>, out: &mut Vec<Vec<(String, Gen)>>) {
    match tree {
        DecisionEntry::Leaf(_) => out.push(prefix),
        DecisionEntry::Node(n) => {
            let f = n.condition.field.clone();
            let mut t = prefix.clone();
            t.push((
                f.clone(),
                value_true(n.condition.operator, &n.condition.value),
            ));
            walk_paths(&n.true_branch, t, out);

            let mut fa = prefix;
            fa.push((f, value_false(n.condition.operator, &n.condition.value)));
            walk_paths(&n.false_branch, fa, out);
        }
    }
}

/// Build a facts object from layered `(field, Gen)` pairs: later pairs override
/// earlier ones; `Absent` removes the key.
fn build_facts(layers: &[&[(String, Gen)]]) -> Value {
    let mut map = Map::new();
    for layer in layers {
        for (k, g) in *layer {
            match g {
                Gen::Set(v) => {
                    map.insert(k.clone(), v.clone());
                }
                Gen::Absent => {
                    map.remove(k);
                }
            }
        }
    }
    Value::Object(map)
}

// --- the generator ----------------------------------------------------------

/// Generate scenarios for one rule: every leaf path (applicable), a
/// not-applicable case, threshold boundaries, a missing-field and a wrong-type
/// case, irrelevant facts, then `fuzz_extra` seeded-random scenarios.
pub fn generate_for_rule(rule: &RuleIR, global_seed: u64, fuzz_extra: usize) -> Vec<Scenario> {
    let mut out = Vec::new();
    let appl_true = applicability_pairs(rule, true);
    let appl_false = applicability_pairs(rule, false);

    // 1. Branch coverage: one applicable scenario per leaf path.
    for (i, path) in enumerate_paths(&rule.decision_tree).into_iter().enumerate() {
        let facts = build_facts(&[&appl_true, &path]);
        out.push(Scenario {
            rule_id: rule.rule_id.clone(),
            label: format!("path#{i}"),
            facts,
        });
    }

    // 2. Not applicable.
    if rule.applies_if.is_some() {
        out.push(Scenario {
            rule_id: rule.rule_id.clone(),
            label: "not_applicable".into(),
            facts: build_facts(&[&appl_false]),
        });
    }

    // A default "all-true" decision assignment to anchor boundary/missing cases.
    let cat = Catalog::from_rule(rule);
    let all_true: Vec<(String, Gen)> = cat
        .conditions
        .iter()
        .map(|c| (c.field.clone(), value_true(c.operator, &c.value)))
        .collect();

    // 3. Threshold boundaries: vary each ordering field across {T-1, T, T+1}.
    for c in &cat.conditions {
        if matches!(
            c.operator,
            Operator::Gt | Operator::Lt | Operator::Gte | Operator::Lte
        ) {
            for (tag, delta) in [("below", -1i128), ("at", 0), ("above", 1)] {
                let override_pair = vec![(c.field.clone(), Gen::Set(shift(&c.value, delta)))];
                out.push(Scenario {
                    rule_id: rule.rule_id.clone(),
                    label: format!("boundary:{}:{}", c.field, tag),
                    facts: build_facts(&[&appl_true, &all_true, &override_pair]),
                });
            }
        }
    }

    // 4. Missing field + wrong type, from the all-true anchor.
    if let Some(c) = cat.conditions.first() {
        let drop = vec![(c.field.clone(), Gen::Absent)];
        out.push(Scenario {
            rule_id: rule.rule_id.clone(),
            label: format!("missing:{}", c.field),
            facts: build_facts(&[&appl_true, &all_true, &drop]),
        });
        let wrong = vec![(
            c.field.clone(),
            Gen::Set(wrong_type(&c.value, cat.in_fields.contains(&c.field))),
        )];
        out.push(Scenario {
            rule_id: rule.rule_id.clone(),
            label: format!("wrong_type:{}", c.field),
            facts: build_facts(&[&appl_true, &all_true, &wrong]),
        });
    }

    // 5. Irrelevant facts (metamorphic: must not change the outcome).
    {
        let extra = vec![
            ("__irrelevant_a".to_string(), Gen::Set(Value::Bool(true))),
            (
                "__irrelevant_b".to_string(),
                Gen::Set(Value::String("noise".into())),
            ),
        ];
        out.push(Scenario {
            rule_id: rule.rule_id.clone(),
            label: "irrelevant_facts".into(),
            facts: build_facts(&[&appl_true, &all_true, &extra]),
        });
    }

    // 6. Seeded fuzz: each cataloged field drawn from its candidate values,
    //    honoring the generator invariant (numbers never on in/not_in fields).
    let mut rng = Rng::new(rule_seed(global_seed, &rule.rule_id));
    for n in 0..fuzz_extra {
        let mut pairs: Vec<(String, Gen)> = Vec::new();
        for c in &cat.conditions {
            let is_in_field = cat.in_fields.contains(&c.field);
            pairs.push((c.field.clone(), fuzz_value(&mut rng, c, is_in_field)));
        }
        out.push(Scenario {
            rule_id: rule.rule_id.clone(),
            label: format!("fuzz#{n}"),
            facts: build_facts(&[&pairs]),
        });
    }

    out
}

/// A wrong-typed value for a field. Numeric fields get a string (str-vs-number);
/// other fields get a bool — never a number on an `in`/`not_in` field (the
/// generator invariant).
fn wrong_type(operand: &ScalarValue, _is_in_field: bool) -> Value {
    match operand {
        ScalarValue::Decimal { .. } => Value::String("not_a_number".into()),
        _ => Value::Bool(true),
    }
}

/// Draw one fuzz value for a condition: true-making, false-making, wrong-typed,
/// or absent, chosen by the PRNG.
fn fuzz_value(rng: &mut Rng, c: &Condition, is_in_field: bool) -> Gen {
    match rng.below(5) {
        0 => value_true(c.operator, &c.value),
        1 => value_false(c.operator, &c.value),
        2 => Gen::Absent,
        3 => Gen::Set(wrong_type(&c.value, is_in_field)),
        _ => value_true(c.operator, &c.value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::FactValue;

    #[test]
    fn rng_is_deterministic() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn decimal_json_int_vs_float() {
        // scale 0 → JSON integer (Python int); scale>0 → JSON float (Python float).
        assert_eq!(decimal_json(5, 0), Value::Number(Number::from(5)));
        assert_eq!(FactValue::from_json(&decimal_json(5, 0)), FactValue::Int(5));
        assert_eq!(
            FactValue::from_json(&decimal_json(90, 2)),
            FactValue::Float(0.9)
        );
    }
}
