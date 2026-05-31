//! YAML → spanned AST, via `marked-yaml` (so every node keeps a `YamlSpan`).
//!
//! Mirrors the platform `RuleLoader` (`src/rules/service.py`): a file is either
//! a single rule mapping or a sequence of them; a decision entry is a leaf iff
//! it has a `result` key, otherwise a branch node.

use crate::ast::{
    AstCondition, AstDecision, AstGroup, AstGroupItem, AstLeaf, AstNode, AstObligation, AstRule,
    AstSource, GroupKind, Spanned, YamlSpan,
};
use crate::error::CompileError;
use crate::value;
use ke_core::ir::ScalarValue;
use marked_yaml::types::{MarkedMappingNode, Node};
use marked_yaml::{LoadError, LoaderOptions};

/// Parse a YAML document into one or more rules.
pub fn parse_rules(source: &str) -> Result<Vec<AstRule>, CompileError> {
    let root = parse_root(source)?;
    match &root {
        Node::Mapping(map) => Ok(vec![parse_rule(map)?]),
        Node::Sequence(seq) => {
            let mut rules = Vec::with_capacity(seq.len());
            for el in seq.iter() {
                let map = el.as_mapping().ok_or_else(|| {
                    CompileError::new(
                        "each rule in a list must be a mapping",
                        YamlSpan::from_marked(el.span()),
                    )
                })?;
                rules.push(parse_rule(map)?);
            }
            Ok(rules)
        }
        Node::Scalar(s) => Err(CompileError::new(
            "expected a rule mapping or a list of rules",
            YamlSpan::from_marked(s.span()),
        )),
    }
}

/// A rule file is either a single rule (top-level mapping) or a list of rules
/// (top-level sequence). marked-yaml's top-level mode is strict, so try mapping
/// first and fall back to sequence on the specific mismatch error.
fn parse_root(source: &str) -> Result<Node, CompileError> {
    match marked_yaml::parse_yaml(0, source) {
        Ok(node) => Ok(node),
        Err(LoadError::TopLevelMustBeMapping(_)) => marked_yaml::parse_yaml_with_options(
            0,
            source,
            LoaderOptions::default().toplevel_sequence(),
        )
        .map_err(|e| CompileError::unlocated(format!("YAML parse error: {e}"))),
        Err(e) => Err(CompileError::unlocated(format!("YAML parse error: {e}"))),
    }
}

fn parse_rule(map: &MarkedMappingNode) -> Result<AstRule, CompileError> {
    let span = YamlSpan::from_marked(map.span());

    let rule_id = req_scalar(map, "rule_id", span)?;
    let decision_tree_map = map
        .get_mapping("decision_tree")
        .ok_or_else(|| CompileError::new("rule is missing a `decision_tree`", span))?;
    let decision_tree = Spanned::new(
        parse_decision(decision_tree_map)?,
        YamlSpan::from_marked(decision_tree_map.span()),
    );
    let source_map = map
        .get_mapping("source")
        .ok_or_else(|| CompileError::new("rule is missing a `source`", span))?;
    let source = Spanned::new(
        parse_source(source_map)?,
        YamlSpan::from_marked(source_map.span()),
    );

    let applies_if = match map.get_mapping("applies_if") {
        Some(g) => Some(Spanned::new(
            parse_group(g)?,
            YamlSpan::from_marked(g.span()),
        )),
        None => None,
    };

    let obligations = match map.get_sequence("obligations") {
        Some(seq) => parse_obligations(seq)?,
        None => Vec::new(),
    };

    let tags = match map.get_sequence("tags") {
        Some(seq) => seq
            .iter()
            .filter_map(|n| n.as_scalar().map(|s| s.as_str().to_string()))
            .collect(),
        None => Vec::new(),
    };

    Ok(AstRule {
        span,
        rule_id,
        version: opt_scalar(map, "version"),
        description: opt_scalar(map, "description"),
        tags,
        effective_from: opt_scalar(map, "effective_from"),
        effective_to: opt_scalar(map, "effective_to"),
        jurisdiction: opt_scalar(map, "jurisdiction"),
        applies_if,
        decision_tree,
        obligations,
        source,
        interpretation_notes: opt_scalar(map, "interpretation_notes"),
    })
}

fn parse_group(map: &MarkedMappingNode) -> Result<AstGroup, CompileError> {
    let (kind, seq) = if let Some(seq) = map.get_sequence("all") {
        (GroupKind::All, seq)
    } else if let Some(seq) = map.get_sequence("any") {
        (GroupKind::Any, seq)
    } else {
        return Err(CompileError::new(
            "a condition group must have an `all` or `any` list",
            YamlSpan::from_marked(map.span()),
        ));
    };

    let mut items = Vec::with_capacity(seq.len());
    for el in seq.iter() {
        let item_map = el.as_mapping().ok_or_else(|| {
            CompileError::new(
                "each `all`/`any` entry must be a mapping",
                YamlSpan::from_marked(el.span()),
            )
        })?;
        let span = YamlSpan::from_marked(item_map.span());
        let item = if item_map.get_scalar("field").is_some() {
            AstGroupItem::Condition(parse_condition(item_map)?)
        } else {
            AstGroupItem::Group(parse_group(item_map)?)
        };
        items.push(Spanned::new(item, span));
    }
    Ok(AstGroup { kind, items })
}

fn parse_condition(map: &MarkedMappingNode) -> Result<AstCondition, CompileError> {
    let span = YamlSpan::from_marked(map.span());
    let field = req_scalar(map, "field", span)?;
    let operator = match map.get_scalar("operator") {
        Some(op) => Spanned::new(op.as_str().to_string(), YamlSpan::from_marked(op.span())),
        // Platform default operator is `==`.
        None => Spanned::new("==".to_string(), span),
    };
    let value = parse_value(map, "value", span)?;
    Ok(AstCondition {
        field,
        operator,
        value,
        description: opt_scalar(map, "description"),
    })
}

fn parse_value(
    map: &MarkedMappingNode,
    key: &str,
    ctx: YamlSpan,
) -> Result<ScalarValue, CompileError> {
    if let Some(s) = map.get_scalar(key) {
        return Ok(value::scalar_to_value(s));
    }
    if let Some(seq) = map.get_sequence(key) {
        let mut items = Vec::with_capacity(seq.len());
        for el in seq.iter() {
            items.push(value::node_to_value(el)?);
        }
        return Ok(ScalarValue::List(items));
    }
    Err(CompileError::new(
        format!("condition is missing a `{key}`"),
        ctx,
    ))
}

fn parse_decision(map: &MarkedMappingNode) -> Result<AstDecision, CompileError> {
    if map.get_scalar("result").is_some() {
        return Ok(AstDecision::Leaf(parse_leaf(map)?));
    }
    Ok(AstDecision::Node(Box::new(parse_node(map)?)))
}

fn parse_node(map: &MarkedMappingNode) -> Result<AstNode, CompileError> {
    let condition = match map.get_mapping("condition") {
        Some(c) => Some(Spanned::new(
            parse_condition(c)?,
            YamlSpan::from_marked(c.span()),
        )),
        None => None,
    };
    let true_branch = parse_branch(map, "true_branch")?;
    let false_branch = parse_branch(map, "false_branch")?;
    Ok(AstNode {
        node_id: opt_scalar(map, "node_id"),
        condition,
        true_branch,
        false_branch,
    })
}

fn parse_branch(
    map: &MarkedMappingNode,
    key: &str,
) -> Result<Option<Spanned<AstDecision>>, CompileError> {
    match map.get_mapping(key) {
        Some(b) => Ok(Some(Spanned::new(
            parse_decision(b)?,
            YamlSpan::from_marked(b.span()),
        ))),
        None => Ok(None),
    }
}

fn parse_leaf(map: &MarkedMappingNode) -> Result<AstLeaf, CompileError> {
    let span = YamlSpan::from_marked(map.span());
    let result = req_scalar(map, "result", span)?;
    let obligations = match map.get_sequence("obligations") {
        Some(seq) => parse_obligations(seq)?,
        None => Vec::new(),
    };
    Ok(AstLeaf {
        result,
        obligations,
        notes: opt_scalar(map, "notes"),
    })
}

fn parse_obligations(
    seq: &marked_yaml::types::MarkedSequenceNode,
) -> Result<Vec<Spanned<AstObligation>>, CompileError> {
    let mut out = Vec::with_capacity(seq.len());
    for el in seq.iter() {
        let map = el.as_mapping().ok_or_else(|| {
            CompileError::new(
                "each obligation must be a mapping",
                YamlSpan::from_marked(el.span()),
            )
        })?;
        let span = YamlSpan::from_marked(map.span());
        let id = req_scalar(map, "id", span)?;
        out.push(Spanned::new(
            AstObligation {
                id,
                description: opt_scalar(map, "description"),
                deadline: opt_scalar(map, "deadline"),
            },
            span,
        ));
    }
    Ok(out)
}

fn parse_source(map: &MarkedMappingNode) -> Result<AstSource, CompileError> {
    let span = YamlSpan::from_marked(map.span());
    let document_id = map
        .get_scalar("document_id")
        .map(|s| s.as_str().to_string())
        .ok_or_else(|| CompileError::new("`source` is missing a `document_id`", span))?;
    let pages = match map.get_sequence("pages") {
        Some(seq) => parse_pages(seq)?,
        None => Vec::new(),
    };
    let paragraphs = match map.get_sequence("paragraphs") {
        Some(seq) => seq
            .iter()
            .filter_map(|n| n.as_scalar().map(|s| s.as_str().to_string()))
            .collect(),
        None => Vec::new(),
    };
    Ok(AstSource {
        document_id,
        article: opt_scalar(map, "article"),
        section: opt_scalar(map, "section"),
        paragraphs,
        pages,
    })
}

fn parse_pages(seq: &marked_yaml::types::MarkedSequenceNode) -> Result<Vec<u32>, CompileError> {
    let mut pages = Vec::with_capacity(seq.len());
    for el in seq.iter() {
        let s = el.as_scalar().ok_or_else(|| {
            CompileError::new(
                "`pages` entries must be integers",
                YamlSpan::from_marked(el.span()),
            )
        })?;
        let n: u32 = s.as_str().trim().parse().map_err(|_| {
            CompileError::new(
                format!("page `{}` is not an integer", s.as_str()),
                YamlSpan::from_marked(s.span()),
            )
        })?;
        pages.push(n);
    }
    Ok(pages)
}

// --- small helpers --------------------------------------------------------

fn opt_scalar(map: &MarkedMappingNode, key: &str) -> Option<String> {
    map.get_scalar(key).map(|s| s.as_str().to_string())
}

fn req_scalar(
    map: &MarkedMappingNode,
    key: &str,
    ctx: YamlSpan,
) -> Result<Spanned<String>, CompileError> {
    match map.get_scalar(key) {
        Some(s) => Ok(Spanned::new(
            s.as_str().to_string(),
            YamlSpan::from_marked(s.span()),
        )),
        None => Err(CompileError::new(format!("missing required `{key}`"), ctx)),
    }
}
