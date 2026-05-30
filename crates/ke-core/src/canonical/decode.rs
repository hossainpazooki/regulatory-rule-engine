//! Strict post-decode validation. After postcard deserializes a value, these
//! walkers re-check every canonical invariant and return a specific
//! [`CanonicalDecodeError`] on the first violation, so non-canonical bytes are
//! rejected with an identifiable reason (spec § 8.3).

use super::ordering;
use super::CanonicalDecodeError;
use crate::ir::condition::{Condition, ConditionGroupSpec, ConditionOrGroup, ScalarValue};
use crate::ir::decision::{DecisionEntry, DecisionLeaf, DecisionNode};
use crate::ir::obligation::ObligationSpec;
use crate::ir::rule::{ProvenanceMarker, RuleIR};
use crate::ir::source_span::{DocumentRef, SourceSpan};
use crate::ir::time::{EffectiveWindow, TimeZone};
use crate::manifest::{Manifest, PolicyBundle, VerificationPolicy};

// --- RuleIR ---------------------------------------------------------------

pub fn validate_rule(r: &RuleIR) -> Result<(), CanonicalDecodeError> {
    ordering::check_nfc(&r.rule_id)?;
    ordering::check_nfc(&r.rule_version)?;
    if let Some(d) = &r.description {
        ordering::check_nfc(d)?;
    }
    if let Some(n) = &r.interpretation_notes {
        ordering::check_nfc(n)?;
    }
    if let Some(tags) = &r.tags {
        for t in tags {
            ordering::check_nfc(t)?;
        }
        ordering::check_set_canonical(tags)?;
    }
    if let Some(group) = &r.applies_if {
        validate_group(group)?;
    }
    validate_entry(&r.decision_tree)?;
    for o in &r.obligations {
        validate_obligation(o)?;
    }
    validate_doc_ref(&r.source)?;
    validate_window(&r.effective_window)?;
    validate_provenance(&r.provenance)?;
    Ok(())
}

fn validate_group(g: &ConditionGroupSpec) -> Result<(), CanonicalDecodeError> {
    if let Some(items) = &g.all {
        for it in items {
            validate_cond_or_group(it)?;
        }
    }
    if let Some(items) = &g.any {
        for it in items {
            validate_cond_or_group(it)?;
        }
    }
    Ok(())
}

fn validate_cond_or_group(c: &ConditionOrGroup) -> Result<(), CanonicalDecodeError> {
    match c {
        ConditionOrGroup::Condition(cond) => validate_condition(cond),
        ConditionOrGroup::Group(group) => validate_group(group),
    }
}

fn validate_condition(c: &Condition) -> Result<(), CanonicalDecodeError> {
    ordering::check_nfc(&c.field)?;
    if let Some(d) = &c.description {
        ordering::check_nfc(d)?;
    }
    validate_scalar(&c.value)
}

fn validate_scalar(v: &ScalarValue) -> Result<(), CanonicalDecodeError> {
    match v {
        ScalarValue::Str(s) => ordering::check_nfc(s)?,
        ScalarValue::Bool(_) => {}
        ScalarValue::Decimal { mantissa, scale } => {
            ordering::check_decimal_canonical(*mantissa, *scale)?
        }
        ScalarValue::List(items) => {
            for it in items {
                validate_scalar(it)?;
            }
        }
    }
    Ok(())
}

fn validate_entry(e: &DecisionEntry) -> Result<(), CanonicalDecodeError> {
    match e {
        DecisionEntry::Node(node) => validate_node(node),
        DecisionEntry::Leaf(leaf) => validate_leaf(leaf),
    }
}

fn validate_node(n: &DecisionNode) -> Result<(), CanonicalDecodeError> {
    ordering::check_nfc(&n.node_id)?;
    validate_condition(&n.condition)?;
    validate_entry(&n.true_branch)?;
    validate_entry(&n.false_branch)?;
    if let Some(span) = &n.source_span {
        validate_source_span(span)?;
    }
    Ok(())
}

fn validate_leaf(l: &DecisionLeaf) -> Result<(), CanonicalDecodeError> {
    ordering::check_nfc(&l.result)?;
    if let Some(obls) = &l.obligations {
        for o in obls {
            validate_obligation(o)?;
        }
    }
    if let Some(notes) = &l.notes {
        ordering::check_nfc(notes)?;
    }
    if let Some(span) = &l.source_span {
        validate_source_span(span)?;
    }
    Ok(())
}

fn validate_obligation(o: &ObligationSpec) -> Result<(), CanonicalDecodeError> {
    ordering::check_nfc(&o.id)?;
    if let Some(d) = &o.description {
        ordering::check_nfc(d)?;
    }
    if let Some(d) = &o.deadline {
        ordering::check_nfc(d)?;
    }
    if let Some(span) = &o.source_span {
        validate_source_span(span)?;
    }
    Ok(())
}

fn validate_doc_ref(d: &DocumentRef) -> Result<(), CanonicalDecodeError> {
    ordering::check_nfc(&d.document_id)?;
    if let Some(a) = &d.article {
        ordering::check_nfc(a)?;
    }
    if let Some(s) = &d.section {
        ordering::check_nfc(s)?;
    }
    for p in &d.paragraphs {
        ordering::check_nfc(p)?;
    }
    if let Some(u) = &d.url {
        ordering::check_nfc(u)?;
    }
    Ok(())
}

fn validate_source_span(s: &SourceSpan) -> Result<(), CanonicalDecodeError> {
    ordering::check_nfc(&s.document_id)?;
    if let Some(a) = &s.article {
        ordering::check_nfc(a)?;
    }
    if let Some(sec) = &s.section {
        ordering::check_nfc(sec)?;
    }
    if let Some(p) = &s.paragraph {
        ordering::check_nfc(p)?;
    }
    Ok(())
}

fn validate_window(w: &EffectiveWindow) -> Result<(), CanonicalDecodeError> {
    ordering::check_date(
        w.effective_from.year,
        w.effective_from.month,
        w.effective_from.day,
    )?;
    if let Some(to) = &w.effective_to {
        ordering::check_date(to.year, to.month, to.day)?;
    }
    validate_time_zone(&w.jurisdiction_time_zone)
}

fn validate_time_zone(tz: &TimeZone) -> Result<(), CanonicalDecodeError> {
    ordering::check_known_tz(&tz.name)?;
    ordering::check_nfc(&tz.tz_data_version)
}

fn validate_provenance(p: &ProvenanceMarker) -> Result<(), CanonicalDecodeError> {
    match p {
        ProvenanceMarker::Candidate {
            proposal_id: Some(id),
        } => ordering::check_nfc(id)?,
        ProvenanceMarker::MlChecked { policy_version } => ordering::check_nfc(policy_version)?,
        ProvenanceMarker::Published { environment } => ordering::check_nfc(environment)?,
        _ => {}
    }
    Ok(())
}

// --- PolicyBundle ---------------------------------------------------------

pub fn validate_policy(p: &PolicyBundle) -> Result<(), CanonicalDecodeError> {
    ordering::check_nfc(&p.environment)?;
    validate_verification_policy(&p.verification_policy)?;
    validate_window(&p.effective_window)
}

fn validate_verification_policy(v: &VerificationPolicy) -> Result<(), CanonicalDecodeError> {
    ordering::check_set_canonical(&v.required_attestation_types)?;
    ordering::check_set_canonical(&v.minimum_attestation_count_per_type)?;
    Ok(())
}

// --- Manifest -------------------------------------------------------------

pub fn validate_manifest(m: &Manifest) -> Result<(), CanonicalDecodeError> {
    ordering::check_nfc(&m.regime_id)?;
    ordering::check_nfc(&m.codec_version.0)?;
    ordering::check_nfc(&m.canonicalization_version.0)?;
    ordering::check_nfc(&m.attestation_policy_version)?;
    ordering::check_date(
        m.effective_from.year,
        m.effective_from.month,
        m.effective_from.day,
    )?;
    if let Some(to) = &m.effective_to {
        ordering::check_date(to.year, to.month, to.day)?;
    }
    Ok(())
}
