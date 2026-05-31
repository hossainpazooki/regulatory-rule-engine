//! Effective-window applicability — **preview-only, outside the equivalence
//! boundary** (ADR 0007).
//!
//! The Python `RuleRuntime` never evaluates effective dates (date filtering is a
//! separate `RuleLoader.get_applicable_rules` pre-filter). So this function is
//! NOT compared against Python in the equivalence harness; it implements spec
//! §8.4's **closed-open `[effective_from, effective_to)`** as the authoritative
//! migration semantics, deliberately diverging from the platform's legacy
//! closed-closed `[from, to]` pre-filter.
//!
//! The comparison is **date-only and zone-agnostic**: the corpus is date-only,
//! and ADR 0007 makes `jurisdiction_time_zone` optional. Full tz-aware instant
//! resolution is deferred to Gate 4 publish-time, when an artifact's effective
//! range first becomes load-bearing.

use ke_core::ir::{EffectiveWindow, JurisdictionDate};

/// Calendar-order key. `(year, month, day)` compares in calendar order because
/// the components are positional and non-overlapping.
fn key(d: &JurisdictionDate) -> (i16, u8, u8) {
    (d.year, d.month, d.day)
}

/// Is `date` within the rule's effective window under closed-open `[from, to)`
/// semantics (spec §8.4)? A `None` window (always-effective rule, ADR 0006) is
/// always effective.
pub fn effective_at(window: Option<&EffectiveWindow>, date: &JurisdictionDate) -> bool {
    let Some(w) = window else {
        return true; // no window declared → always effective
    };
    if key(date) < key(&w.effective_from) {
        return false; // before the window opens
    }
    match &w.effective_to {
        // Closed-open: `date < to` (a rule whose `effective_to == date` is NOT
        // effective — this is the deliberate divergence from the platform's
        // closed-closed pre-filter; see ADR 0007).
        Some(to) => key(date) < key(to),
        None => true, // open-ended window
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ke_core::ir::{EffectiveTimePolicy, TimeZone};

    fn d(y: i16, m: u8, day: u8) -> JurisdictionDate {
        JurisdictionDate::new(y, m, day)
    }

    fn window(from: JurisdictionDate, to: Option<JurisdictionDate>) -> EffectiveWindow {
        EffectiveWindow {
            effective_from: from,
            effective_to: to,
            jurisdiction_time_zone: Some(TimeZone {
                name: "UTC".into(),
                tz_data_version: "2025a".into(),
            }),
            effective_time_policy: Some(EffectiveTimePolicy::MidnightLocal),
        }
    }

    #[test]
    fn none_window_is_always_effective() {
        assert!(effective_at(None, &d(2024, 1, 1)));
    }

    #[test]
    fn closed_open_boundaries() {
        let w = window(d(2024, 6, 30), Some(d(2025, 1, 1)));
        assert!(!effective_at(Some(&w), &d(2024, 6, 29))); // before from
        assert!(effective_at(Some(&w), &d(2024, 6, 30))); // from is inclusive
        assert!(effective_at(Some(&w), &d(2024, 12, 31)));
        // `to` is EXCLUSIVE — the divergence from the platform's [from,to].
        assert!(!effective_at(Some(&w), &d(2025, 1, 1)));
        assert!(!effective_at(Some(&w), &d(2025, 1, 2)));
    }

    #[test]
    fn open_ended_window() {
        let w = window(d(2024, 6, 30), None);
        assert!(!effective_at(Some(&w), &d(2024, 6, 29)));
        assert!(effective_at(Some(&w), &d(2024, 6, 30)));
        assert!(effective_at(Some(&w), &d(2099, 1, 1)));
    }
}
