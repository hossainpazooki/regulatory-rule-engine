//! Import a platform `Rule` (as `Rule.model_dump(mode="json")`) into
//! `ke_core::ir::RuleIR`. **Differential-testing only** — it lets the harness
//! reduce the Python side to the same semantic normal form as the Rust side.
//!
//! Numbers are taken from their JSON literal and parsed to exact decimals (never
//! `f64`, ADR 0003). The shape mirrors the platform `Rule`/`ConditionGroupSpec`/
//! `DecisionNode`/`DecisionLeaf` (`src/rules/service.py`).

use crate::error::CompileError;
use crate::lower::operator_from_str;
use crate::value::parse_decimal;
use ke_core::ir::{
    Condition, ConditionGroupSpec, ConditionOrGroup, DecisionEntry, DecisionLeaf, DecisionNode,
    DocumentRef, EffectiveWindow, JurisdictionDate, ObligationSpec, ProvenanceMarker, RuleIR,
    ScalarValue,
};
use serde_json::Value;

/// Import one rule, or a JSON array of rules.
pub fn import_rules(v: &Value) -> Result<Vec<RuleIR>, CompileError> {
    match v {
        Value::Array(items) => items.iter().map(import_rule).collect(),
        _ => Ok(vec![import_rule(v)?]),
    }
}

pub fn import_rule(v: &Value) -> Result<RuleIR, CompileError> {
    let obj = v
        .as_object()
        .ok_or_else(|| CompileError::unlocated("python rule must be a JSON object"))?;

    let rule_id = req_str(obj, "rule_id")?;
    let decision_tree = match obj.get("decision_tree") {
        Some(Value::Null) | None => {
            return Err(CompileError::unlocated(format!(
                "python rule `{rule_id}` has no decision_tree"
            )))
        }
        Some(dt) => import_decision(dt)?,
    };
    let source = match obj.get("source") {
        Some(s) if !s.is_null() => import_source(s)?,
        _ => {
            return Err(CompileError::unlocated(format!(
                "python rule `{rule_id}` has no source"
            )))
        }
    };

    let tags = obj
        .get("tags")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(RuleIR {
        rule_id,
        rule_version: opt_str(obj, "version").unwrap_or_else(|| "1.0".to_string()),
        description: opt_str(obj, "description"),
        tags: if tags.is_empty() { None } else { Some(tags) },
        applies_if: match obj.get("applies_if") {
            Some(g) if !g.is_null() => Some(import_group(g)?),
            _ => None,
        },
        decision_tree,
        obligations: Vec::new(),
        source,
        interpretation_notes: opt_str(obj, "interpretation_notes"),
        effective_window: import_window(obj)?,
        provenance: ProvenanceMarker::Candidate { proposal_id: None },
    })
}

fn import_group(v: &Value) -> Result<ConditionGroupSpec, CompileError> {
    let obj = v
        .as_object()
        .ok_or_else(|| CompileError::unlocated("condition group must be an object"))?;
    let import_items = |arr: &Value| -> Result<Vec<ConditionOrGroup>, CompileError> {
        arr.as_array()
            .ok_or_else(|| CompileError::unlocated("`all`/`any` must be an array"))?
            .iter()
            .map(import_item)
            .collect()
    };
    if let Some(all) = obj.get("all").filter(|x| !x.is_null()) {
        Ok(ConditionGroupSpec {
            all: Some(import_items(all)?),
            any: None,
        })
    } else if let Some(any) = obj.get("any").filter(|x| !x.is_null()) {
        Ok(ConditionGroupSpec {
            all: None,
            any: Some(import_items(any)?),
        })
    } else {
        Ok(ConditionGroupSpec {
            all: None,
            any: None,
        })
    }
}

fn import_item(v: &Value) -> Result<ConditionOrGroup, CompileError> {
    let obj = v
        .as_object()
        .ok_or_else(|| CompileError::unlocated("group item must be an object"))?;
    if obj.contains_key("field") {
        Ok(ConditionOrGroup::Condition(import_condition(v)?))
    } else {
        Ok(ConditionOrGroup::Group(import_group(v)?))
    }
}

fn import_condition(v: &Value) -> Result<Condition, CompileError> {
    let obj = v.as_object().unwrap();
    let field = req_str(obj, "field")?;
    let op_str = opt_str(obj, "operator").unwrap_or_else(|| "==".to_string());
    let operator = operator_from_str(&op_str)
        .ok_or_else(|| CompileError::unlocated(format!("unknown operator `{op_str}`")))?;
    let value = import_value(obj.get("value").unwrap_or(&Value::Null));
    Ok(Condition {
        field,
        operator,
        value,
        description: opt_str(obj, "description"),
    })
}

fn import_value(v: &Value) -> ScalarValue {
    match v {
        Value::Bool(b) => ScalarValue::Bool(*b),
        Value::Number(n) => {
            parse_decimal(&n.to_string()).unwrap_or_else(|| ScalarValue::Str(n.to_string()))
        }
        Value::String(s) => ScalarValue::Str(s.clone()),
        Value::Array(a) => ScalarValue::List(a.iter().map(import_value).collect()),
        // `null` (e.g. an `exists` condition with no operand) → empty string.
        Value::Null => ScalarValue::Str(String::new()),
        Value::Object(_) => ScalarValue::Str(String::new()),
    }
}

fn import_decision(v: &Value) -> Result<DecisionEntry, CompileError> {
    let obj = v
        .as_object()
        .ok_or_else(|| CompileError::unlocated("decision entry must be an object"))?;
    if obj.contains_key("result") {
        let mut obligations = Vec::new();
        if let Some(arr) = obj.get("obligations").and_then(Value::as_array) {
            for o in arr {
                obligations.push(import_obligation(o)?);
            }
        }
        Ok(DecisionEntry::Leaf(Box::new(DecisionLeaf {
            result: req_str(obj, "result")?,
            obligations: if obligations.is_empty() {
                None
            } else {
                Some(obligations)
            },
            notes: opt_str(obj, "notes"),
            source_span: None,
        })))
    } else {
        let condition = match obj.get("condition") {
            Some(c) if !c.is_null() => import_condition(c)?,
            _ => return Err(CompileError::unlocated("decision node has no condition")),
        };
        let true_branch = import_branch(obj.get("true_branch"), "true_branch")?;
        let false_branch = import_branch(obj.get("false_branch"), "false_branch")?;
        Ok(DecisionEntry::Node(Box::new(DecisionNode {
            node_id: opt_str(obj, "node_id").unwrap_or_else(|| "unnamed".to_string()),
            condition,
            true_branch,
            false_branch,
            source_span: None,
        })))
    }
}

fn import_branch(v: Option<&Value>, which: &str) -> Result<DecisionEntry, CompileError> {
    match v {
        Some(b) if !b.is_null() => import_decision(b),
        _ => Err(CompileError::unlocated(format!(
            "decision node has no `{which}`"
        ))),
    }
}

fn import_obligation(v: &Value) -> Result<ObligationSpec, CompileError> {
    let obj = v
        .as_object()
        .ok_or_else(|| CompileError::unlocated("obligation must be an object"))?;
    Ok(ObligationSpec {
        id: req_str(obj, "id")?,
        description: opt_str(obj, "description"),
        deadline: opt_str(obj, "deadline"),
        source_span: None,
    })
}

fn import_source(v: &Value) -> Result<DocumentRef, CompileError> {
    let obj = v
        .as_object()
        .ok_or_else(|| CompileError::unlocated("source must be an object"))?;
    let pages = obj
        .get("pages")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_u64().map(|n| n as u32))
                .collect()
        })
        .unwrap_or_default();
    let paragraphs = obj
        .get("paragraphs")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    Ok(DocumentRef {
        document_id: req_str(obj, "document_id")?,
        article: opt_str(obj, "article"),
        section: opt_str(obj, "section"),
        paragraphs,
        pages,
        url: opt_str(obj, "url"),
    })
}

fn import_window(
    obj: &serde_json::Map<String, Value>,
) -> Result<Option<EffectiveWindow>, CompileError> {
    let from = match opt_str(obj, "effective_from") {
        Some(s) => s,
        None => return Ok(None),
    };
    Ok(Some(EffectiveWindow {
        effective_from: parse_date(&from)?,
        effective_to: match opt_str(obj, "effective_to") {
            Some(s) => Some(parse_date(&s)?),
            None => None,
        },
        // ADR 0007: the platform rule carries no authored zone → `None`.
        jurisdiction_time_zone: None,
        effective_time_policy: None,
    }))
}

fn parse_date(s: &str) -> Result<JurisdictionDate, CompileError> {
    let parts: Vec<&str> = s.trim().split('-').collect();
    let bad = || CompileError::unlocated(format!("invalid ISO date `{s}`"));
    if parts.len() != 3 {
        return Err(bad());
    }
    Ok(JurisdictionDate::new(
        parts[0].parse().map_err(|_| bad())?,
        parts[1].parse().map_err(|_| bad())?,
        parts[2].parse().map_err(|_| bad())?,
    ))
}

// --- helpers --------------------------------------------------------------

fn opt_str(obj: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    obj.get(key).and_then(Value::as_str).map(str::to_string)
}

fn req_str(obj: &serde_json::Map<String, Value>, key: &str) -> Result<String, CompileError> {
    opt_str(obj, key).ok_or_else(|| CompileError::unlocated(format!("missing `{key}`")))
}
