//! AST → `ke_core::ir::RuleIR`.
//!
//! Lowering validates and reshapes the spanned AST into the canonical IR,
//! dropping `YamlSpan`s (legal provenance comes from the rule's `source:`, not
//! YAML positions — ADR 0004). Rules with no effective dates lower to
//! `effective_window: None` (ADR 0006); when dates are present the
//! YAML-absent time zone defaults to `UTC` as a Gate-2 placeholder
//! (jurisdiction→zone resolution is Gate 3).

use crate::ast::{
    AstCondition, AstDecision, AstGroup, AstGroupItem, AstLeaf, AstNode, AstObligation, AstRule,
    AstSource, GroupKind, Spanned, YamlSpan,
};
use crate::error::CompileError;
use ke_core::ir::{
    Condition, ConditionGroupSpec, ConditionOrGroup, DecisionEntry, DecisionLeaf, DecisionNode,
    DocumentRef, EffectiveWindow, JurisdictionDate, ObligationSpec, Operator, ProvenanceMarker,
    RuleIR, TimeZone,
};

/// Placeholder tz-data version for the Gate-2 default `UTC` zone (ADR 0006).
const DEFAULT_TZ_DATA_VERSION: &str = "2025a";

/// Lower one parsed rule into the canonical IR.
pub fn lower_rule(ast: &AstRule) -> Result<RuleIR, CompileError> {
    Ok(RuleIR {
        rule_id: ast.rule_id.value.clone(),
        rule_version: ast.version.clone().unwrap_or_else(|| "1.0".to_string()),
        description: ast.description.clone(),
        tags: if ast.tags.is_empty() {
            None
        } else {
            Some(ast.tags.clone())
        },
        applies_if: match &ast.applies_if {
            Some(g) => Some(lower_group(&g.value)?),
            None => None,
        },
        decision_tree: lower_decision(&ast.decision_tree)?,
        obligations: ast
            .obligations
            .iter()
            .map(lower_obligation)
            .collect::<Result<_, _>>()?,
        source: lower_source(&ast.source.value),
        interpretation_notes: ast.interpretation_notes.clone(),
        effective_window: lower_window(ast)?,
        provenance: ProvenanceMarker::Candidate { proposal_id: None },
    })
}

fn lower_group(group: &AstGroup) -> Result<ConditionGroupSpec, CompileError> {
    let items = group
        .items
        .iter()
        .map(|it| lower_group_item(&it.value))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(match group.kind {
        GroupKind::All => ConditionGroupSpec {
            all: Some(items),
            any: None,
        },
        GroupKind::Any => ConditionGroupSpec {
            all: None,
            any: Some(items),
        },
    })
}

fn lower_group_item(item: &AstGroupItem) -> Result<ConditionOrGroup, CompileError> {
    match item {
        AstGroupItem::Condition(c) => Ok(ConditionOrGroup::Condition(lower_condition(c)?)),
        AstGroupItem::Group(g) => Ok(ConditionOrGroup::Group(lower_group(g)?)),
    }
}

fn lower_condition(c: &AstCondition) -> Result<Condition, CompileError> {
    Ok(Condition {
        field: c.field.value.clone(),
        operator: lower_operator(&c.operator)?,
        value: c.value.clone(),
        description: c.description.clone(),
    })
}

/// Map a YAML operator string to the closed `Operator` enum. Mirrors the
/// platform `OPERATOR_MAP`. Shared with `python_import`.
pub(crate) fn operator_from_str(s: &str) -> Option<Operator> {
    Some(match s {
        "==" | "=" => Operator::Eq,
        "!=" | "<>" => Operator::NotEq,
        "in" => Operator::In,
        "not in" | "not_in" => Operator::NotIn,
        ">" => Operator::Gt,
        "<" => Operator::Lt,
        ">=" => Operator::Gte,
        "<=" => Operator::Lte,
        "exists" => Operator::Exists,
        _ => return None,
    })
}

fn lower_operator(op: &Spanned<String>) -> Result<Operator, CompileError> {
    operator_from_str(&op.value)
        .ok_or_else(|| CompileError::new(format!("unknown operator `{}`", op.value), op.span))
}

fn lower_decision(d: &Spanned<AstDecision>) -> Result<DecisionEntry, CompileError> {
    match &d.value {
        AstDecision::Leaf(leaf) => Ok(DecisionEntry::Leaf(Box::new(lower_leaf(leaf)?))),
        AstDecision::Node(node) => Ok(DecisionEntry::Node(Box::new(lower_node(node, d.span)?))),
    }
}

fn lower_node(node: &AstNode, span: YamlSpan) -> Result<DecisionNode, CompileError> {
    let condition = node
        .condition
        .as_ref()
        .ok_or_else(|| CompileError::new("decision node has no `condition`", span))?;
    let true_branch = node
        .true_branch
        .as_ref()
        .ok_or_else(|| CompileError::new("decision node has no `true_branch`", span))?;
    let false_branch = node
        .false_branch
        .as_ref()
        .ok_or_else(|| CompileError::new("decision node has no `false_branch`", span))?;
    Ok(DecisionNode {
        node_id: node
            .node_id
            .clone()
            .unwrap_or_else(|| "unnamed".to_string()),
        condition: lower_condition(&condition.value)?,
        true_branch: lower_decision(true_branch)?,
        false_branch: lower_decision(false_branch)?,
        source_span: None,
    })
}

fn lower_leaf(leaf: &AstLeaf) -> Result<DecisionLeaf, CompileError> {
    let obligations = leaf
        .obligations
        .iter()
        .map(lower_obligation)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(DecisionLeaf {
        result: leaf.result.value.clone(),
        obligations: if obligations.is_empty() {
            None
        } else {
            Some(obligations)
        },
        notes: leaf.notes.clone(),
        source_span: None,
    })
}

fn lower_obligation(o: &Spanned<AstObligation>) -> Result<ObligationSpec, CompileError> {
    Ok(ObligationSpec {
        id: o.value.id.value.clone(),
        description: o.value.description.clone(),
        deadline: o.value.deadline.clone(),
        source_span: None,
    })
}

fn lower_source(s: &AstSource) -> DocumentRef {
    DocumentRef {
        document_id: s.document_id.clone(),
        article: s.article.clone(),
        section: s.section.clone(),
        paragraphs: s.paragraphs.clone(),
        pages: s.pages.clone(),
        url: None,
    }
}

fn lower_window(ast: &AstRule) -> Result<Option<EffectiveWindow>, CompileError> {
    let from = match &ast.effective_from {
        Some(s) => s,
        // No effective dates → always-effective rule (ADR 0006).
        None => return Ok(None),
    };
    let effective_from = parse_date(from, ast.span)?;
    let effective_to = match &ast.effective_to {
        Some(s) => Some(parse_date(s, ast.span)?),
        None => None,
    };
    Ok(Some(EffectiveWindow {
        effective_from,
        effective_to,
        jurisdiction_time_zone: TimeZone {
            name: "UTC".to_string(),
            tz_data_version: DEFAULT_TZ_DATA_VERSION.to_string(),
        },
        effective_time_policy: None,
    }))
}

fn parse_date(s: &str, span: YamlSpan) -> Result<JurisdictionDate, CompileError> {
    let parts: Vec<&str> = s.trim().split('-').collect();
    let bad = || CompileError::new(format!("invalid ISO date `{s}` (want YYYY-MM-DD)"), span);
    if parts.len() != 3 {
        return Err(bad());
    }
    let year: i16 = parts[0].parse().map_err(|_| bad())?;
    let month: u8 = parts[1].parse().map_err(|_| bad())?;
    let day: u8 = parts[2].parse().map_err(|_| bad())?;
    let date = JurisdictionDate::new(year, month, day);
    if !date.is_structurally_valid() {
        return Err(bad());
    }
    Ok(date)
}
