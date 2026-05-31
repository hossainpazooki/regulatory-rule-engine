//! Rejection of non-canonical encodings (brief § 8.4, spec § 8.3).
//!
//! Each test crafts raw postcard bytes that *bypass* the normalizing encoder
//! (so the value is structurally decodable but violates one canonical
//! invariant) and asserts the strict decoder returns the specific
//! [`CanonicalDecodeError`] variant for that violation.

use ke_core::canonical::{decode_rule, encode_rule, CanonicalDecodeError};
use ke_core::ir::condition::{
    Condition, ConditionGroupSpec, ConditionOrGroup, Operator, ScalarValue,
};
use ke_core::ir::decision::{DecisionEntry, DecisionLeaf};
use ke_core::ir::rule::{ProvenanceMarker, RuleIR};
use ke_core::ir::source_span::DocumentRef;
use ke_core::ir::time::{EffectiveWindow, JurisdictionDate, TimeZone};

/// A clean, canonical rule. Each test mutates exactly one thing.
fn minimal_valid_rule() -> RuleIR {
    RuleIR {
        rule_id: "r".to_string(),
        rule_version: "1".to_string(),
        description: None,
        tags: None,
        applies_if: None,
        decision_tree: DecisionEntry::Leaf(Box::new(DecisionLeaf {
            result: "ok".to_string(),
            obligations: None,
            notes: None,
            source_span: None,
        })),
        obligations: Vec::new(),
        source: DocumentRef {
            document_id: "d".to_string(),
            article: None,
            section: None,
            paragraphs: Vec::new(),
            pages: Vec::new(),
            url: None,
        },
        interpretation_notes: None,
        effective_window: Some(EffectiveWindow {
            effective_from: JurisdictionDate::new(2024, 1, 1),
            effective_to: None,
            jurisdiction_time_zone: Some(TimeZone {
                name: "UTC".to_string(),
                tz_data_version: "2025a".to_string(),
            }),
            effective_time_policy: None,
        }),
        provenance: ProvenanceMarker::StructurallyVerified,
    }
}

/// Raw postcard encode, deliberately skipping canonical normalization.
fn raw(rule: &RuleIR) -> Vec<u8> {
    postcard::to_stdvec(rule).expect("postcard encode")
}

#[test]
fn baseline_is_canonical() {
    let rule = minimal_valid_rule();
    let bytes = encode_rule(&rule).expect("encode");
    assert!(decode_rule(&bytes).is_ok(), "baseline must decode");
}

#[test]
fn unsorted_tag_set_is_rejected() {
    let mut rule = minimal_valid_rule();
    rule.tags = Some(vec!["zzz".to_string(), "aaa".to_string()]);
    assert!(matches!(
        decode_rule(&raw(&rule)),
        Err(CanonicalDecodeError::UnsortedSet)
    ));
}

#[test]
fn duplicate_tag_is_rejected() {
    let mut rule = minimal_valid_rule();
    rule.tags = Some(vec!["aaa".to_string(), "aaa".to_string()]);
    assert!(matches!(
        decode_rule(&raw(&rule)),
        Err(CanonicalDecodeError::DuplicateSetElement)
    ));
}

#[test]
fn non_nfc_string_is_rejected() {
    let mut rule = minimal_valid_rule();
    // "e" + combining acute accent — NFD, not NFC.
    rule.rule_id = "e\u{0301}".to_string();
    assert!(matches!(
        decode_rule(&raw(&rule)),
        Err(CanonicalDecodeError::NonNfcString)
    ));
}

#[test]
fn out_of_range_date_is_rejected() {
    let mut rule = minimal_valid_rule();
    rule.effective_window.as_mut().unwrap().effective_from = JurisdictionDate::new(2024, 13, 1);
    assert!(matches!(
        decode_rule(&raw(&rule)),
        Err(CanonicalDecodeError::InvalidDate { month: 13, .. })
    ));
}

#[test]
fn non_canonical_decimal_is_rejected() {
    let mut rule = minimal_valid_rule();
    // 0.20 written with a trailing zero ({20, 2} is canonical; {200, 3} is not).
    rule.applies_if = Some(ConditionGroupSpec {
        all: Some(vec![ConditionOrGroup::Condition(Condition {
            field: "fee".to_string(),
            operator: Operator::Lte,
            value: ScalarValue::Decimal {
                mantissa: 200,
                scale: 3,
            },
            description: None,
        })]),
        any: None,
    });
    assert!(matches!(
        decode_rule(&raw(&rule)),
        Err(CanonicalDecodeError::NonCanonicalDecimal {
            mantissa: 200,
            scale: 3
        })
    ));
}

#[test]
fn unknown_time_zone_is_rejected() {
    let mut rule = minimal_valid_rule();
    rule.effective_window
        .as_mut()
        .unwrap()
        .jurisdiction_time_zone
        .as_mut()
        .unwrap()
        .name = "Mars/Phobos".to_string();
    assert!(matches!(
        decode_rule(&raw(&rule)),
        Err(CanonicalDecodeError::UnknownTimeZone(_))
    ));
}

#[test]
fn trailing_bytes_are_rejected() {
    let rule = minimal_valid_rule();
    let mut bytes = encode_rule(&rule).expect("encode");
    bytes.push(0x00);
    assert!(matches!(
        decode_rule(&bytes),
        Err(CanonicalDecodeError::TrailingBytes)
    ));
}
