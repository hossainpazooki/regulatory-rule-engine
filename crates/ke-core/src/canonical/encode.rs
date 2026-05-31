//! In-place normalization into canonical form, run before postcard
//! serialization. Walks the IR / artifact shapes and applies the ordering and
//! validation rules from [`super::ordering`].

use super::ordering;
use super::CanonicalError;
use crate::ir::condition::{Condition, ConditionGroupSpec, ConditionOrGroup, ScalarValue};
use crate::ir::decision::{DecisionEntry, DecisionLeaf, DecisionNode};
use crate::ir::obligation::ObligationSpec;
use crate::ir::rule::{ProvenanceMarker, RuleIR};
use crate::ir::source_span::{DocumentRef, SourceSpan};
use crate::ir::time::{EffectiveWindow, TimeZone};
use crate::manifest::{Manifest, PolicyBundle, VerificationPolicy};

// --- RuleIR ---------------------------------------------------------------

pub fn canonicalize_rule(r: &mut RuleIR) -> Result<(), CanonicalError> {
    ordering::ensure_nfc(&r.rule_id)?;
    ordering::ensure_nfc(&r.rule_version)?;
    if let Some(d) = &r.description {
        ordering::ensure_nfc(d)?;
    }
    if let Some(n) = &r.interpretation_notes {
        ordering::ensure_nfc(n)?;
    }
    if let Some(tags) = &mut r.tags {
        for t in tags.iter() {
            ordering::ensure_nfc(t)?;
        }
        ordering::canonicalize_set(tags)?;
    }
    if let Some(group) = &mut r.applies_if {
        canonicalize_group(group)?;
    }
    canonicalize_entry(&mut r.decision_tree)?;
    for o in &mut r.obligations {
        canonicalize_obligation(o)?;
    }
    canonicalize_doc_ref(&mut r.source)?;
    if let Some(window) = &mut r.effective_window {
        canonicalize_window(window)?;
    }
    canonicalize_provenance(&mut r.provenance)?;
    Ok(())
}

fn canonicalize_group(g: &mut ConditionGroupSpec) -> Result<(), CanonicalError> {
    if let Some(items) = &mut g.all {
        for it in items.iter_mut() {
            canonicalize_cond_or_group(it)?;
        }
    }
    if let Some(items) = &mut g.any {
        for it in items.iter_mut() {
            canonicalize_cond_or_group(it)?;
        }
    }
    Ok(())
}

fn canonicalize_cond_or_group(c: &mut ConditionOrGroup) -> Result<(), CanonicalError> {
    match c {
        ConditionOrGroup::Condition(cond) => canonicalize_condition(cond),
        ConditionOrGroup::Group(group) => canonicalize_group(group),
    }
}

fn canonicalize_condition(c: &mut Condition) -> Result<(), CanonicalError> {
    ordering::ensure_nfc(&c.field)?;
    if let Some(d) = &c.description {
        ordering::ensure_nfc(d)?;
    }
    canonicalize_scalar(&mut c.value)
}

fn canonicalize_scalar(v: &mut ScalarValue) -> Result<(), CanonicalError> {
    match v {
        ScalarValue::Str(s) => ordering::ensure_nfc(s)?,
        ScalarValue::Bool(_) => {}
        ScalarValue::Decimal { mantissa, scale } => {
            let (m, s) = ordering::normalize_decimal(*mantissa, *scale)?;
            *mantissa = m;
            *scale = s;
        }
        // `in` / `not_in` operands are ordered sequences, not sets — preserve
        // author order, only normalize each element.
        ScalarValue::List(items) => {
            for it in items.iter_mut() {
                canonicalize_scalar(it)?;
            }
        }
    }
    Ok(())
}

fn canonicalize_entry(e: &mut DecisionEntry) -> Result<(), CanonicalError> {
    match e {
        DecisionEntry::Node(node) => canonicalize_node(node),
        DecisionEntry::Leaf(leaf) => canonicalize_leaf(leaf),
    }
}

fn canonicalize_node(n: &mut DecisionNode) -> Result<(), CanonicalError> {
    ordering::ensure_nfc(&n.node_id)?;
    canonicalize_condition(&mut n.condition)?;
    canonicalize_entry(&mut n.true_branch)?;
    canonicalize_entry(&mut n.false_branch)?;
    if let Some(span) = &mut n.source_span {
        canonicalize_source_span(span)?;
    }
    Ok(())
}

fn canonicalize_leaf(l: &mut DecisionLeaf) -> Result<(), CanonicalError> {
    ordering::ensure_nfc(&l.result)?;
    if let Some(obls) = &mut l.obligations {
        for o in obls.iter_mut() {
            canonicalize_obligation(o)?;
        }
    }
    if let Some(notes) = &l.notes {
        ordering::ensure_nfc(notes)?;
    }
    if let Some(span) = &mut l.source_span {
        canonicalize_source_span(span)?;
    }
    Ok(())
}

fn canonicalize_obligation(o: &mut ObligationSpec) -> Result<(), CanonicalError> {
    ordering::ensure_nfc(&o.id)?;
    if let Some(d) = &o.description {
        ordering::ensure_nfc(d)?;
    }
    if let Some(d) = &o.deadline {
        ordering::ensure_nfc(d)?;
    }
    if let Some(span) = &mut o.source_span {
        canonicalize_source_span(span)?;
    }
    Ok(())
}

fn canonicalize_doc_ref(d: &mut DocumentRef) -> Result<(), CanonicalError> {
    ordering::ensure_nfc(&d.document_id)?;
    if let Some(a) = &d.article {
        ordering::ensure_nfc(a)?;
    }
    if let Some(s) = &d.section {
        ordering::ensure_nfc(s)?;
    }
    for p in &d.paragraphs {
        ordering::ensure_nfc(p)?;
    }
    if let Some(u) = &d.url {
        ordering::ensure_nfc(u)?;
    }
    Ok(())
}

fn canonicalize_source_span(s: &mut SourceSpan) -> Result<(), CanonicalError> {
    ordering::ensure_nfc(&s.document_id)?;
    if let Some(a) = &s.article {
        ordering::ensure_nfc(a)?;
    }
    if let Some(sec) = &s.section {
        ordering::ensure_nfc(sec)?;
    }
    if let Some(p) = &s.paragraph {
        ordering::ensure_nfc(p)?;
    }
    Ok(())
}

fn canonicalize_window(w: &mut EffectiveWindow) -> Result<(), CanonicalError> {
    ordering::ensure_date(
        w.effective_from.year,
        w.effective_from.month,
        w.effective_from.day,
    )?;
    if let Some(to) = &w.effective_to {
        ordering::ensure_date(to.year, to.month, to.day)?;
    }
    // Optional zone (ADR 0007): a date-only rule carries `None`.
    if let Some(tz) = &mut w.jurisdiction_time_zone {
        canonicalize_time_zone(tz)?;
    }
    Ok(())
}

fn canonicalize_time_zone(tz: &mut TimeZone) -> Result<(), CanonicalError> {
    ordering::ensure_known_tz(&tz.name)?;
    ordering::ensure_nfc(&tz.tz_data_version)
}

fn canonicalize_provenance(p: &mut ProvenanceMarker) -> Result<(), CanonicalError> {
    match p {
        ProvenanceMarker::Candidate {
            proposal_id: Some(id),
        } => ordering::ensure_nfc(id)?,
        ProvenanceMarker::MlChecked { policy_version } => ordering::ensure_nfc(policy_version)?,
        ProvenanceMarker::Published { environment } => ordering::ensure_nfc(environment)?,
        _ => {}
    }
    Ok(())
}

// --- PolicyBundle ---------------------------------------------------------

pub fn canonicalize_policy(p: &mut PolicyBundle) -> Result<(), CanonicalError> {
    ordering::ensure_nfc(&p.environment)?;
    canonicalize_verification_policy(&mut p.verification_policy)?;
    canonicalize_window(&mut p.effective_window)
}

fn canonicalize_verification_policy(v: &mut VerificationPolicy) -> Result<(), CanonicalError> {
    // Required-attestation-types and the per-type-count map are both sets/maps:
    // canonically ordered, duplicate-free.
    ordering::canonicalize_set(&mut v.required_attestation_types)?;
    ordering::canonicalize_set(&mut v.minimum_attestation_count_per_type)?;
    Ok(())
}

// --- Manifest -------------------------------------------------------------

pub fn canonicalize_manifest(m: &mut Manifest) -> Result<(), CanonicalError> {
    ordering::ensure_nfc(&m.regime_id)?;
    ordering::ensure_nfc(&m.codec_version.0)?;
    ordering::ensure_nfc(&m.canonicalization_version.0)?;
    ordering::ensure_nfc(&m.attestation_policy_version)?;
    ordering::ensure_date(
        m.effective_from.year,
        m.effective_from.month,
        m.effective_from.day,
    )?;
    if let Some(to) = &m.effective_to {
        ordering::ensure_date(to.year, to.month, to.day)?;
    }
    Ok(())
}
