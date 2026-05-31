//! Human-readable differences between two semantic normal forms. Equality is the
//! authoritative check (`SemanticRule: Eq`); `semantic_diff` explains a
//! mismatch field-by-field for the differential harness's output.

use super::form::SemanticRule;

/// One difference between two semantic rules.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Difference {
    pub field: String,
    pub detail: String,
}

impl Difference {
    fn new(field: &str, detail: impl Into<String>) -> Self {
        Self {
            field: field.to_string(),
            detail: detail.into(),
        }
    }
}

/// Compare two semantic rules. Empty result ⇒ semantically equivalent.
pub fn semantic_diff(a: &SemanticRule, b: &SemanticRule) -> Vec<Difference> {
    let mut diffs = Vec::new();

    if a.rule_id != b.rule_id {
        diffs.push(Difference::new(
            "rule_id",
            format!("{:?} != {:?}", a.rule_id, b.rule_id),
        ));
    }
    if a.applicability != b.applicability {
        diffs.push(Difference::new(
            "applicability",
            "applicability predicates differ",
        ));
    }
    if a.source_document != b.source_document {
        diffs.push(Difference::new(
            "source_document",
            format!("{:?} != {:?}", a.source_document, b.source_document),
        ));
    }
    if a.effective != b.effective {
        diffs.push(Difference::new(
            "effective",
            format!("{:?} != {:?}", a.effective, b.effective),
        ));
    }
    if a.tags != b.tags {
        diffs.push(Difference::new(
            "tags",
            format!("{:?} != {:?}", a.tags, b.tags),
        ));
    }
    if a.decision_paths != b.decision_paths {
        diffs.push(Difference::new(
            "decision_paths",
            format!(
                "{} path(s) vs {} path(s); content differs",
                a.decision_paths.len(),
                b.decision_paths.len()
            ),
        ));
    }

    diffs
}
