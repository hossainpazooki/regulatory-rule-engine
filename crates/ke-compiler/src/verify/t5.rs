//! T5 — lint-beyond-compiler (Gate 5). Advisory + (opt-in) blocking style /
//! quality findings on a single `RuleIR`.
//!
//! T5 is a **non-authoritative quality gate**. It runs on the un-lowered
//! `RuleIR` *before* artifact assembly and therefore never alters canonical
//! bytes (the postcard encoding, the envelope split, the re-zero hash are all
//! downstream and untouched). It does not read the registry, sign, attest, or
//! publish anything.
//!
//! # Advisory-by-default contract
//!
//! Every finding T5 emits by default is `blocking: false` (advisory). That
//! keeps [`crate::verify::verify`] / [`crate::verify::VerificationReport::has_blocking`]
//! behavior on the current clean fixture corpus unchanged — the existing
//! compile/contract gate stays green even though T5 findings now flow into the
//! same `findings` vector.
//!
//! The single **blocking** T5 check (`T5-rule-id-whitespace`) is deliberately
//! scoped to a degenerate shape the corpus never produces: a `rule_id` that is
//! non-empty (so T0 accepts it) yet contains internal whitespace. It exists to
//! prove the blocking path is wired without firing on any real corpus rule.

use super::{Finding, Tier};
use ke_core::ir::decision::DecisionEntry;

/// T5 (lint-beyond-compiler): advisory + blocking style/quality findings on a
/// single rule. Non-authoritative quality gate; does not alter canonical bytes.
pub fn check(rule: &ke_core::ir::rule::RuleIR) -> Vec<crate::verify::Finding> {
    let mut out = Vec::new();

    let advisory = |code: &'static str, message: &str| Finding {
        tier: Tier::T5,
        rule_id: rule.rule_id.clone(),
        code,
        message: message.to_string(),
        blocking: false,
    };

    // --- advisory: authoring quality ------------------------------------
    if rule
        .description
        .as_ref()
        .map(|d| d.trim().is_empty())
        .unwrap_or(true)
    {
        out.push(advisory(
            "T5-missing-description",
            "rule has no human-readable `description` (advisory)",
        ));
    }

    if rule.tags.as_ref().map(|t| t.is_empty()).unwrap_or(true) {
        out.push(advisory(
            "T5-missing-tags",
            "rule carries no classification `tags` (advisory)",
        ));
    }

    // A leaf whose result is a bare decision with neither obligations nor a
    // note explaining it is harder to review. Advisory only.
    if has_unannotated_terminal_leaf(&rule.decision_tree) {
        out.push(advisory(
            "T5-unannotated-leaf",
            "a decision leaf has neither obligations nor explanatory `notes` (advisory)",
        ));
    }

    // --- blocking (opt-in shape; never fires on the corpus) -------------
    // T0 only rejects a rule_id that is empty after trim; an id like "a b"
    // survives T0 but is a malformed identifier. Treat internal whitespace as
    // a blocking lint so `--deny` has something real to gate on, while keeping
    // the clean corpus (all snake_case ids) at blocking_count == 0.
    if !rule.rule_id.trim().is_empty() && rule.rule_id.split_whitespace().count() > 1 {
        out.push(Finding {
            tier: Tier::T5,
            rule_id: rule.rule_id.clone(),
            code: "T5-rule-id-whitespace",
            message: "rule_id contains internal whitespace; use a single dotted/snake identifier"
                .to_string(),
            blocking: true,
        });
    }

    out
}

/// True if any leaf in the tree has neither obligations nor notes.
fn has_unannotated_terminal_leaf(entry: &DecisionEntry) -> bool {
    match entry {
        DecisionEntry::Leaf(leaf) => {
            let no_obligations = leaf
                .obligations
                .as_ref()
                .map(|o| o.is_empty())
                .unwrap_or(true);
            let no_notes = leaf
                .notes
                .as_ref()
                .map(|n| n.trim().is_empty())
                .unwrap_or(true);
            no_obligations && no_notes
        }
        DecisionEntry::Node(node) => {
            has_unannotated_terminal_leaf(&node.true_branch)
                || has_unannotated_terminal_leaf(&node.false_branch)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ke_core::ir::decision::{DecisionEntry, DecisionLeaf};
    use ke_core::ir::rule::{ProvenanceMarker, RuleIR};
    use ke_core::ir::source_span::DocumentRef;

    fn leaf(result: &str) -> DecisionEntry {
        DecisionEntry::Leaf(Box::new(DecisionLeaf {
            result: result.to_string(),
            obligations: None,
            notes: Some("annotated".to_string()),
            source_span: None,
        }))
    }

    fn base_rule(rule_id: &str) -> RuleIR {
        RuleIR {
            rule_id: rule_id.to_string(),
            rule_version: "1.0".to_string(),
            description: Some("a description".to_string()),
            tags: Some(vec!["tag".to_string()]),
            applies_if: None,
            decision_tree: leaf("compliant"),
            obligations: Vec::new(),
            source: DocumentRef {
                document_id: "doc".to_string(),
                article: None,
                section: None,
                paragraphs: Vec::new(),
                pages: Vec::new(),
                url: None,
            },
            interpretation_notes: None,
            effective_window: None,
            provenance: ProvenanceMarker::StructurallyVerified,
        }
    }

    #[test]
    fn well_formed_rule_yields_no_blocking_findings() {
        let findings = check(&base_rule("well_formed_rule"));
        assert!(findings.iter().all(|f| f.tier == Tier::T5));
        assert!(
            findings.iter().all(|f| !f.blocking),
            "a well-formed rule produces only advisory T5 findings"
        );
    }

    #[test]
    fn whitespace_rule_id_is_blocking() {
        // Non-empty after trim (so T0 would accept it) but has internal space.
        let findings = check(&base_rule("bad id with spaces"));
        let blocking: Vec<_> = findings.iter().filter(|f| f.blocking).collect();
        assert_eq!(blocking.len(), 1, "exactly one blocking T5 finding");
        assert_eq!(blocking[0].code, "T5-rule-id-whitespace");
        assert_eq!(blocking[0].tier, Tier::T5);
    }

    #[test]
    fn missing_description_and_tags_are_advisory() {
        let mut rule = base_rule("sparse_rule");
        rule.description = None;
        rule.tags = None;
        let findings = check(&rule);
        let codes: Vec<&str> = findings.iter().map(|f| f.code).collect();
        assert!(codes.contains(&"T5-missing-description"));
        assert!(codes.contains(&"T5-missing-tags"));
        assert!(findings.iter().all(|f| !f.blocking));
    }
}
