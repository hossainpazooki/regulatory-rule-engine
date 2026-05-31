//! Jurisdiction-time model. Effective windows are *legal-date* concepts, not
//! UTC instants (spec § 8.4). Gate 1 encodes the fields and structurally
//! validates them; the closed-open `[effective_from, effective_to)` resolution
//! and time-zone conversion land in Gate 3 (`ke-runtime`). See ADR 0001.

use serde::{Deserialize, Serialize};

/// A jurisdiction-local calendar date. No timestamps live in the IR.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JurisdictionDate {
    pub year: i16,
    pub month: u8,
    pub day: u8,
}

/// Earliest year accepted by the structural date validator.
pub const MIN_YEAR: i16 = 1900;

impl JurisdictionDate {
    pub const fn new(year: i16, month: u8, day: u8) -> Self {
        Self { year, month, day }
    }

    /// Structural validity only (month 1–12, day 1–31, year ≥ [`MIN_YEAR`]).
    /// This does **not** check calendar correctness (e.g. Feb 30) — that is a
    /// Gate 3 runtime concern.
    pub fn is_structurally_valid(&self) -> bool {
        self.year >= MIN_YEAR && (1..=12).contains(&self.month) && (1..=31).contains(&self.day)
    }
}

/// An IANA time-zone name plus the pinned tz-data version it must be resolved
/// against (ADR 0001). Gate 1 stores the name + version only; the tz database
/// and resolution arrive in Gate 3.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeZone {
    /// IANA zone name, e.g. `"Europe/Brussels"`.
    pub name: String,
    pub tz_data_version: String,
}

/// Escape hatch for regimes with a non-standard legal-effective-time convention
/// (spec § 8.4). Typed key + version, not free-form, so future variants do not
/// break canonicalization. Gate 1 starts with a single default variant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectiveTimePolicy {
    /// The window boundary falls at local midnight in the rule's zone.
    MidnightLocal,
}

/// A legal effective window. Closed-open semantics `[from, to)` are enforced in
/// the Gate 3 preview runtime (`ke_runtime::effective`).
///
/// `jurisdiction_time_zone` is **optional** (ADR 0007): a date-only rule (the
/// corpus case) carries `None` — no invented zone enters canonical bytes. When a
/// zone is authored/derived (Gate 4 publish-time), `Some(tz)` is unchanged. This
/// refines ADR 0001 (zone present *when authored*, not *always present*).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectiveWindow {
    pub effective_from: JurisdictionDate,
    pub effective_to: Option<JurisdictionDate>,
    pub jurisdiction_time_zone: Option<TimeZone>,
    pub effective_time_policy: Option<EffectiveTimePolicy>,
}
