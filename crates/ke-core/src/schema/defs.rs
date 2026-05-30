//! `$defs` builders and the reference-naming / `$defs`-ordering rules
//! (brief § 5.4–5.6).
//!
//! Reference names are `PascalCase` of the Rust type. `$defs` are emitted in
//! lexicographic order by name. Enum values are listed in **declaration order**
//! (matching the canonical discriminant order), not alphabetically. Property
//! order within an object matches Rust field declaration order; this relies on
//! `serde_json`'s `preserve_order` feature so insertion order is preserved.

use serde_json::{json, Map, Value};

// --- small constructors ---------------------------------------------------

/// Build an object `Value` preserving insertion order of `pairs`.
fn ordered(pairs: Vec<(&str, Value)>) -> Value {
    let mut m = Map::new();
    for (k, v) in pairs {
        m.insert(k.to_string(), v);
    }
    Value::Object(m)
}

fn s_string() -> Value {
    json!({ "type": "string" })
}

fn s_int() -> Value {
    json!({ "type": "integer" })
}

fn s_bool() -> Value {
    json!({ "type": "boolean" })
}

fn s_ref(name: &str) -> Value {
    json!({ "$ref": format!("#/$defs/{name}") })
}

/// A nullable wrapper for `Option<T>` fields.
fn s_nullable(inner: Value) -> Value {
    json!({ "oneOf": [inner, { "type": "null" }] })
}

fn s_array(items: Value) -> Value {
    ordered(vec![("type", json!("array")), ("items", items)])
}

fn s_fixed_bytes(n: u64) -> Value {
    ordered(vec![
        ("type", json!("array")),
        ("items", s_int()),
        ("minItems", json!(n)),
        ("maxItems", json!(n)),
    ])
}

/// An object def with ordered `properties`, `required`, `additionalProperties:false`.
fn s_object(props: Vec<(&str, Value)>, required: Vec<&str>) -> Value {
    ordered(vec![
        ("type", json!("object")),
        ("properties", ordered(props)),
        ("required", json!(required)),
        ("additionalProperties", json!(false)),
    ])
}

/// A unit-only enum: `{ "enum": [...] }` in declaration order.
fn s_enum(values: &[&str]) -> Value {
    json!({ "enum": values })
}

/// One externally-tagged struct/newtype variant `{ "Name": payload }`.
fn s_variant(name: &str, payload: Value) -> Value {
    s_object(vec![(name, payload)], vec![name])
}

/// One externally-tagged unit variant, encoded as the bare string `"Name"`.
fn s_unit_variant(name: &str) -> Value {
    json!({ "const": name })
}

// --- the defs, in lexicographic order by name -----------------------------

/// Every named definition, sorted by name (brief § 5.4).
pub(crate) fn all_defs() -> Vec<(&'static str, Value)> {
    vec![
        ("ArtifactKind", artifact_kind()),
        ("AttestationCount", attestation_count()),
        ("AttestationType", attestation_type()),
        ("ByteRange", byte_range()),
        ("CompiledCheck", compiled_check()),
        ("Condition", condition()),
        ("ConditionGroupSpec", condition_group_spec()),
        ("ConditionOrGroup", condition_or_group()),
        ("DecisionEntry", decision_entry()),
        ("DecisionLeaf", decision_leaf()),
        ("DecisionNode", decision_node()),
        ("DocumentRef", document_ref()),
        ("EffectiveTimePolicy", effective_time_policy()),
        ("EffectiveWindow", effective_window()),
        ("JurisdictionDate", jurisdiction_date()),
        ("Manifest", manifest()),
        ("ObligationSpec", obligation_spec()),
        ("Operator", operator()),
        ("PolicyBundle", policy_bundle()),
        ("ProvenanceMarker", provenance_marker()),
        ("RevocationPolicy", revocation_policy()),
        ("RuleIR", rule_ir()),
        ("ScalarValue", scalar_value()),
        ("SchemaVersion", schema_version()),
        ("SemVer", sem_ver()),
        ("SourceSpan", source_span()),
        ("T2T3Mode", t2_t3_mode()),
        ("TimeZone", time_zone()),
        ("VerificationPolicy", verification_policy()),
    ]
}

/// The top-level `RuleIR` shape, reused as the root schema body.
pub(crate) fn rule_ir() -> Value {
    s_object(
        vec![
            ("rule_id", s_string()),
            ("rule_version", s_string()),
            ("description", s_nullable(s_string())),
            ("tags", s_nullable(s_array(s_string()))),
            ("applies_if", s_nullable(s_ref("ConditionGroupSpec"))),
            ("decision_tree", s_ref("DecisionEntry")),
            ("obligations", s_array(s_ref("ObligationSpec"))),
            ("source", s_ref("DocumentRef")),
            ("interpretation_notes", s_nullable(s_string())),
            ("effective_window", s_ref("EffectiveWindow")),
            ("provenance", s_ref("ProvenanceMarker")),
        ],
        vec![
            "rule_id",
            "rule_version",
            "decision_tree",
            "obligations",
            "source",
            "effective_window",
            "provenance",
        ],
    )
}

fn provenance_marker() -> Value {
    json!({
        "oneOf": [
            s_variant("Candidate", s_object(vec![("proposal_id", s_nullable(s_string()))], vec![])),
            s_unit_variant("StructurallyVerified"),
            s_variant("MlChecked", s_object(vec![("policy_version", s_string())], vec!["policy_version"])),
            s_variant("ExpertAttested", s_object(vec![("attestation_count", s_int())], vec!["attestation_count"])),
            s_variant("Published", s_object(vec![("environment", s_string())], vec!["environment"])),
            s_unit_variant("Deprecated"),
            s_unit_variant("Revoked"),
        ]
    })
}

fn condition_group_spec() -> Value {
    s_object(
        vec![
            ("all", s_nullable(s_array(s_ref("ConditionOrGroup")))),
            ("any", s_nullable(s_array(s_ref("ConditionOrGroup")))),
        ],
        vec![],
    )
}

fn condition_or_group() -> Value {
    json!({
        "oneOf": [
            s_variant("Condition", s_ref("Condition")),
            s_variant("Group", s_ref("ConditionGroupSpec")),
        ]
    })
}

fn condition() -> Value {
    s_object(
        vec![
            ("field", s_string()),
            ("operator", s_ref("Operator")),
            ("value", s_ref("ScalarValue")),
            ("description", s_nullable(s_string())),
        ],
        vec!["field", "operator", "value"],
    )
}

fn operator() -> Value {
    s_enum(&["==", "!=", "in", "not_in", ">", "<", ">=", "<=", "exists"])
}

fn scalar_value() -> Value {
    json!({
        "oneOf": [
            s_variant("Str", s_string()),
            s_variant("Bool", s_bool()),
            s_variant(
                "Decimal",
                s_object(vec![("mantissa", s_int()), ("scale", s_int())], vec!["mantissa", "scale"]),
            ),
            s_variant("List", s_array(s_ref("ScalarValue"))),
        ]
    })
}

fn decision_entry() -> Value {
    json!({
        "oneOf": [
            s_variant("Node", s_ref("DecisionNode")),
            s_variant("Leaf", s_ref("DecisionLeaf")),
        ]
    })
}

fn decision_node() -> Value {
    s_object(
        vec![
            ("node_id", s_string()),
            ("condition", s_ref("Condition")),
            ("true_branch", s_ref("DecisionEntry")),
            ("false_branch", s_ref("DecisionEntry")),
            ("source_span", s_nullable(s_ref("SourceSpan"))),
        ],
        vec!["node_id", "condition", "true_branch", "false_branch"],
    )
}

fn decision_leaf() -> Value {
    s_object(
        vec![
            ("result", s_string()),
            ("obligations", s_nullable(s_array(s_ref("ObligationSpec")))),
            ("notes", s_nullable(s_string())),
            ("source_span", s_nullable(s_ref("SourceSpan"))),
        ],
        vec!["result"],
    )
}

fn obligation_spec() -> Value {
    s_object(
        vec![
            ("id", s_string()),
            ("description", s_nullable(s_string())),
            ("deadline", s_nullable(s_string())),
            ("source_span", s_nullable(s_ref("SourceSpan"))),
        ],
        vec!["id"],
    )
}

fn document_ref() -> Value {
    s_object(
        vec![
            ("document_id", s_string()),
            ("article", s_nullable(s_string())),
            ("section", s_nullable(s_string())),
            ("paragraphs", s_array(s_string())),
            ("pages", s_array(s_int())),
            ("url", s_nullable(s_string())),
        ],
        vec!["document_id", "paragraphs", "pages"],
    )
}

fn byte_range() -> Value {
    s_object(
        vec![("start", s_int()), ("end", s_int())],
        vec!["start", "end"],
    )
}

fn source_span() -> Value {
    s_object(
        vec![
            ("document_id", s_string()),
            ("article", s_nullable(s_string())),
            ("section", s_nullable(s_string())),
            ("paragraph", s_nullable(s_string())),
            ("pages", s_nullable(s_array(s_int()))),
            ("byte_range", s_nullable(s_ref("ByteRange"))),
            ("text_hash", s_nullable(s_fixed_bytes(32))),
        ],
        vec!["document_id"],
    )
}

fn jurisdiction_date() -> Value {
    s_object(
        vec![("year", s_int()), ("month", s_int()), ("day", s_int())],
        vec!["year", "month", "day"],
    )
}

fn time_zone() -> Value {
    s_object(
        vec![("name", s_string()), ("tz_data_version", s_string())],
        vec!["name", "tz_data_version"],
    )
}

fn effective_time_policy() -> Value {
    s_enum(&["MidnightLocal"])
}

fn effective_window() -> Value {
    s_object(
        vec![
            ("effective_from", s_ref("JurisdictionDate")),
            ("effective_to", s_nullable(s_ref("JurisdictionDate"))),
            ("jurisdiction_time_zone", s_ref("TimeZone")),
            (
                "effective_time_policy",
                s_nullable(s_ref("EffectiveTimePolicy")),
            ),
        ],
        vec!["effective_from", "jurisdiction_time_zone"],
    )
}

fn compiled_check() -> Value {
    s_object(
        vec![
            ("index", s_int()),
            ("field", s_string()),
            ("operator", s_ref("Operator")),
            ("value", s_ref("ScalarValue")),
        ],
        vec!["index", "field", "operator", "value"],
    )
}

fn artifact_kind() -> Value {
    s_enum(&[
        "RegimePack",
        "EquivalenceMatrix",
        "TestCorpus",
        "PolicyBundle",
    ])
}

fn sem_ver() -> Value {
    s_object(
        vec![("major", s_int()), ("minor", s_int()), ("patch", s_int())],
        vec!["major", "minor", "patch"],
    )
}

fn schema_version() -> Value {
    s_object(
        vec![("major", s_int()), ("minor", s_int()), ("patch", s_int())],
        vec!["major", "minor", "patch"],
    )
}

fn manifest() -> Value {
    s_object(
        vec![
            ("artifact_kind", s_ref("ArtifactKind")),
            ("artifact_hash", s_fixed_bytes(32)),
            ("regime_id", s_string()),
            ("effective_from", s_ref("JurisdictionDate")),
            ("effective_to", s_nullable(s_ref("JurisdictionDate"))),
            ("compiler_version", s_ref("SemVer")),
            ("compiler_build_hash", s_fixed_bytes(32)),
            ("ir_schema_version", s_ref("SchemaVersion")),
            ("codec_version", s_string()),
            ("canonicalization_version", s_string()),
            ("corpus_root_hash", s_fixed_bytes(32)),
            ("source_corpus_hash", s_fixed_bytes(32)),
            ("attestation_policy_version", s_string()),
        ],
        vec![
            "artifact_kind",
            "artifact_hash",
            "regime_id",
            "effective_from",
            "compiler_version",
            "compiler_build_hash",
            "ir_schema_version",
            "codec_version",
            "canonicalization_version",
            "corpus_root_hash",
            "source_corpus_hash",
            "attestation_policy_version",
        ],
    )
}

fn t2_t3_mode() -> Value {
    s_enum(&["Strict", "ReviewOverride", "Advisory", "Disabled"])
}

fn attestation_type() -> Value {
    s_enum(&[
        "SourceFidelity",
        "Interpretation",
        "ScenarioCoverage",
        "EquivalenceClaim",
        "PublicationApproval",
    ])
}

fn revocation_policy() -> Value {
    s_enum(&[
        "HaltImmediately",
        "FinishPinnedThenHalt",
        "FinishPinnedNoNew",
    ])
}

fn attestation_count() -> Value {
    s_object(
        vec![
            ("attestation_type", s_ref("AttestationType")),
            ("count", s_int()),
        ],
        vec!["attestation_type", "count"],
    )
}

fn verification_policy() -> Value {
    s_object(
        vec![
            ("t2_t3_mode", s_ref("T2T3Mode")),
            (
                "required_attestation_types",
                s_array(s_ref("AttestationType")),
            ),
            (
                "minimum_attestation_count_per_type",
                s_array(s_ref("AttestationCount")),
            ),
        ],
        vec![
            "t2_t3_mode",
            "required_attestation_types",
            "minimum_attestation_count_per_type",
        ],
    )
}

fn policy_bundle() -> Value {
    s_object(
        vec![
            ("environment", s_string()),
            ("verification_policy", s_ref("VerificationPolicy")),
            ("revocation_policy", s_ref("RevocationPolicy")),
            ("effective_window", s_ref("EffectiveWindow")),
        ],
        vec![
            "environment",
            "verification_policy",
            "revocation_policy",
            "effective_window",
        ],
    )
}
