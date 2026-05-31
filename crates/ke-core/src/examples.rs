//! Synthetic example artifacts used by the golden-fixture generator
//! (`bin/gen-fixtures`) and the round-trip / non-canonical tests.
//!
//! These are **Rust-authored** sample IRs, not platform-derived ones: Gate 1
//! runs fixture generation in synthetic mode (the platform-driven cross-corpus
//! bytes are deferred until the recorded `fixtures/rules/SOURCE.md` SHA is
//! reconciled with the platform `HEAD`). They are deliberately small but
//! exercise nested `all`/`any` groups, decimal scalars, tags (set ordering),
//! obligations, source spans, the effective window, and several provenance
//! markers.

use crate::ir::condition::{
    Condition, ConditionGroupSpec, ConditionOrGroup, Operator, ScalarValue,
};
use crate::ir::decision::{DecisionEntry, DecisionLeaf, DecisionNode};
use crate::ir::obligation::ObligationSpec;
use crate::ir::rule::{ProvenanceMarker, RuleIR};
use crate::ir::source_span::DocumentRef;
use crate::ir::time::{EffectiveWindow, JurisdictionDate, TimeZone};
use crate::manifest::{
    ArtifactKind, AttestationCount, AttestationType, Manifest, PolicyBundle, RevocationPolicy,
    SemVer, T2T3Mode, VerificationPolicy,
};
use crate::version::{
    CanonicalizationVersion, CodecVersion, CANONICALIZATION_VERSION, CODEC_VERSION,
    IR_SCHEMA_VERSION,
};

// --- tiny builders --------------------------------------------------------

fn tz() -> TimeZone {
    TimeZone {
        name: "Europe/Brussels".to_string(),
        tz_data_version: "2025a".to_string(),
    }
}

fn cond(field: &str, operator: Operator, value: ScalarValue) -> Condition {
    Condition {
        field: field.to_string(),
        operator,
        value,
        description: None,
    }
}

fn str_list(values: &[&str]) -> ScalarValue {
    ScalarValue::List(
        values
            .iter()
            .map(|v| ScalarValue::Str(v.to_string()))
            .collect(),
    )
}

fn leaf(result: &str, obligations: Vec<ObligationSpec>, notes: Option<&str>) -> DecisionEntry {
    DecisionEntry::Leaf(Box::new(DecisionLeaf {
        result: result.to_string(),
        obligations: if obligations.is_empty() {
            None
        } else {
            Some(obligations)
        },
        notes: notes.map(|s| s.to_string()),
        source_span: None,
    }))
}

fn node(node_id: &str, condition: Condition, t: DecisionEntry, f: DecisionEntry) -> DecisionEntry {
    DecisionEntry::Node(Box::new(DecisionNode {
        node_id: node_id.to_string(),
        condition,
        true_branch: t,
        false_branch: f,
        source_span: None,
    }))
}

fn obl(id: &str, description: &str) -> ObligationSpec {
    ObligationSpec {
        id: id.to_string(),
        description: Some(description.to_string()),
        deadline: None,
        source_span: None,
    }
}

fn doc(document_id: &str, article: &str, pages: &[u32]) -> DocumentRef {
    DocumentRef {
        document_id: document_id.to_string(),
        article: Some(article.to_string()),
        section: None,
        paragraphs: Vec::new(),
        pages: pages.to_vec(),
        url: None,
    }
}

fn window(from: JurisdictionDate) -> EffectiveWindow {
    EffectiveWindow {
        effective_from: from,
        effective_to: None,
        // Example keeps a zone (ADR 0007 made it optional); the corpus None path
        // is exercised by the ke-compiler lowering/round-trip tests.
        jurisdiction_time_zone: Some(tz()),
        effective_time_policy: None,
    }
}

// --- the example rules ----------------------------------------------------

/// MiCA Art. 38 reserve-asset style rule — nested `all` group with `in`
/// operators, a two-level decision tree, obligations, and a tag set provided
/// out of order to exercise canonical set sorting.
fn reserve_assets() -> RuleIR {
    RuleIR {
        rule_id: "mica_art38_reserve_assets".to_string(),
        rule_version: "1.0".to_string(),
        description: Some("Reserve asset requirements for ARTs under MiCA Article 38.".to_string()),
        // Intentionally unsorted; the encoder canonicalizes.
        tags: Some(vec![
            "stablecoin".to_string(),
            "mica".to_string(),
            "reserves".to_string(),
            "art".to_string(),
        ]),
        applies_if: Some(ConditionGroupSpec {
            all: Some(vec![
                ConditionOrGroup::Condition(cond(
                    "instrument_type",
                    Operator::In,
                    str_list(&["art", "stablecoin"]),
                )),
                ConditionOrGroup::Condition(cond(
                    "jurisdiction",
                    Operator::Eq,
                    ScalarValue::Str("EU".to_string()),
                )),
            ]),
            any: None,
        }),
        decision_tree: node(
            "check_reserve_exists",
            cond("has_reserve", Operator::Eq, ScalarValue::Bool(true)),
            node(
                "check_reserve_custody",
                cond(
                    "reserve_custodian_authorized",
                    Operator::Eq,
                    ScalarValue::Bool(true),
                ),
                leaf("compliant", vec![], Some("Reserve requirements satisfied")),
                leaf(
                    "non_compliant",
                    vec![obl(
                        "appoint_custodian_art37",
                        "Appoint authorized custodian for reserve assets per Article 37",
                    )],
                    None,
                ),
            ),
            leaf(
                "non_compliant",
                vec![obl(
                    "establish_reserve_art38",
                    "Establish reserve of assets per Article 38",
                )],
                None,
            ),
        ),
        obligations: Vec::new(),
        source: doc("mica_2023", "38", &[67, 68]),
        interpretation_notes: Some(
            "Article 38 requires ART issuers to maintain a reserve of assets at all times."
                .to_string(),
        ),
        effective_window: Some(window(JurisdictionDate::new(2024, 6, 30))),
        provenance: ProvenanceMarker::StructurallyVerified,
    }
}

/// A thresholds rule exercising decimal scalars (exact integer and fractional)
/// and a nested `any` group, plus a `Candidate` provenance marker.
fn significant_thresholds() -> RuleIR {
    RuleIR {
        rule_id: "mica_art45_significant_thresholds".to_string(),
        rule_version: "1.0".to_string(),
        description: Some("Significant-ART thresholds under MiCA Article 45.".to_string()),
        tags: Some(vec!["mica".to_string(), "significant".to_string()]),
        applies_if: Some(ConditionGroupSpec {
            all: None,
            any: Some(vec![
                // EUR 5,000,000,000 market cap — exact integer decimal.
                ConditionOrGroup::Condition(cond(
                    "market_cap_eur",
                    Operator::Gt,
                    ScalarValue::int(5_000_000_000),
                )),
                // > 10,000,000 holders.
                ConditionOrGroup::Condition(cond(
                    "holders",
                    Operator::Gt,
                    ScalarValue::int(10_000_000),
                )),
            ]),
        }),
        decision_tree: node(
            "check_fee",
            // redemption fee <= 0.20% — fractional decimal {20, 2}.
            cond(
                "redemption_fee_percentage",
                Operator::Lte,
                ScalarValue::Decimal {
                    mantissa: 20,
                    scale: 2,
                },
            ),
            leaf("compliant", vec![], Some("Fee within bound")),
            leaf(
                "non_compliant",
                vec![obl(
                    "higher_own_funds_art45",
                    "Maintain higher own funds per Article 45(5)",
                )],
                None,
            ),
        ),
        obligations: Vec::new(),
        source: doc("mica_2023", "45", &[72, 73]),
        interpretation_notes: None,
        effective_window: Some(window(JurisdictionDate::new(2024, 6, 30))),
        provenance: ProvenanceMarker::Candidate {
            proposal_id: Some("prop-0001".to_string()),
        },
    }
}

/// All example rules as `(artifact_id, RuleIR)` pairs.
pub fn rules() -> Vec<(String, RuleIR)> {
    vec![
        ("rule_reserve_assets".to_string(), reserve_assets()),
        (
            "rule_significant_thresholds".to_string(),
            significant_thresholds(),
        ),
    ]
}

/// An example policy bundle, exercising the attestation set/map ordering.
pub fn policy() -> (String, PolicyBundle) {
    let bundle = PolicyBundle {
        environment: "production-eu".to_string(),
        verification_policy: VerificationPolicy {
            t2_t3_mode: T2T3Mode::ReviewOverride,
            // Provided out of order; the encoder canonicalizes.
            required_attestation_types: vec![
                AttestationType::PublicationApproval,
                AttestationType::SourceFidelity,
            ],
            minimum_attestation_count_per_type: vec![
                AttestationCount {
                    attestation_type: AttestationType::SourceFidelity,
                    count: 1,
                },
                AttestationCount {
                    attestation_type: AttestationType::PublicationApproval,
                    count: 1,
                },
            ],
        },
        revocation_policy: RevocationPolicy::FinishPinnedNoNew,
        effective_window: window(JurisdictionDate::new(2024, 6, 30)),
    };
    ("policy_production_eu".to_string(), bundle)
}

/// Build a synthetic manifest for an artifact, recording its BLAKE3 as the
/// `artifact_hash` and the source/corpus hashes. The compiler build hash is a
/// fixed placeholder (real provenance is Gate 4).
pub fn synthetic_manifest(
    artifact_kind: ArtifactKind,
    regime_id: &str,
    effective_from: JurisdictionDate,
    artifact_bytes: &[u8],
) -> Manifest {
    let hash: [u8; 32] = *blake3::hash(artifact_bytes).as_bytes();
    Manifest {
        artifact_kind,
        artifact_hash: hash,
        regime_id: regime_id.to_string(),
        effective_from,
        effective_to: None,
        compiler_version: SemVer {
            major: 0,
            minor: 1,
            patch: 0,
        },
        compiler_build_hash: [0u8; 32],
        ir_schema_version: IR_SCHEMA_VERSION,
        codec_version: CodecVersion(CODEC_VERSION.to_string()),
        canonicalization_version: CanonicalizationVersion(CANONICALIZATION_VERSION.to_string()),
        corpus_root_hash: hash,
        source_corpus_hash: hash,
        attestation_policy_version: "ap-1".to_string(),
    }
}
