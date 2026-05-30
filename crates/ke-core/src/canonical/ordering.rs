//! Field-, map-, and set-ordering rules plus the shared scalar validators
//! (NFC, decimal canonical form, date structure, time-zone allow-list).
//!
//! Set/map ordering is by the **lexicographic byte order of each element's (or
//! key's) canonical postcard encoding** (brief § 4.3–4.4). Encoding is
//! deterministic, so this gives a stable total order without relying on any
//! container's internal ordering.

use super::{CanonicalDecodeError, CanonicalError};
use crate::ir::time::MIN_YEAR;
use serde::Serialize;
use std::cmp::Ordering;
use unicode_normalization::is_nfc;

/// Time zones accepted by Gate 1's encoder. Seeded from the corpus' regimes;
/// Gate 3 widens this against a pinned IANA tz-data snapshot (ADR 0001).
pub const KNOWN_TIME_ZONES: &[&str] = &[
    "UTC",
    "Europe/Brussels",
    "Europe/Berlin",
    "Europe/London",
    "Europe/Zurich",
    "Asia/Singapore",
    "America/New_York",
];

// --- NFC ------------------------------------------------------------------

pub(crate) fn ensure_nfc(s: &str) -> Result<(), CanonicalError> {
    if is_nfc(s) {
        Ok(())
    } else {
        Err(CanonicalError::NonNfcString {
            value: s.to_string(),
        })
    }
}

pub(crate) fn check_nfc(s: &str) -> Result<(), CanonicalDecodeError> {
    if is_nfc(s) {
        Ok(())
    } else {
        Err(CanonicalDecodeError::NonNfcString)
    }
}

// --- Decimals -------------------------------------------------------------

/// Fold a decimal into canonical form: non-negative scale, no trailing zeros,
/// and `mantissa == 0 ⇒ scale == 0`. Returns `(mantissa, scale)`.
pub(crate) fn normalize_decimal(mantissa: i128, scale: i8) -> Result<(i128, i8), CanonicalError> {
    let mut m = mantissa;
    let mut s = scale;
    // Fold any negative scale into the mantissa.
    while s < 0 {
        m = m
            .checked_mul(10)
            .ok_or(CanonicalError::DecimalOverflow { mantissa, scale })?;
        s += 1;
    }
    // Strip trailing zeros.
    while s > 0 && m != 0 && m % 10 == 0 {
        m /= 10;
        s -= 1;
    }
    if m == 0 {
        s = 0;
    }
    Ok((m, s))
}

pub(crate) fn check_decimal_canonical(
    mantissa: i128,
    scale: i8,
) -> Result<(), CanonicalDecodeError> {
    let canonical = scale >= 0
        && !(scale > 0 && mantissa != 0 && mantissa % 10 == 0)
        && !(mantissa == 0 && scale != 0);
    if canonical {
        Ok(())
    } else {
        Err(CanonicalDecodeError::NonCanonicalDecimal { mantissa, scale })
    }
}

// --- Dates ----------------------------------------------------------------

pub(crate) fn ensure_date(year: i16, month: u8, day: u8) -> Result<(), CanonicalError> {
    if year >= MIN_YEAR && (1..=12).contains(&month) && (1..=31).contains(&day) {
        Ok(())
    } else {
        Err(CanonicalError::InvalidDate { year, month, day })
    }
}

pub(crate) fn check_date(year: i16, month: u8, day: u8) -> Result<(), CanonicalDecodeError> {
    if year >= MIN_YEAR && (1..=12).contains(&month) && (1..=31).contains(&day) {
        Ok(())
    } else {
        Err(CanonicalDecodeError::InvalidDate { year, month, day })
    }
}

// --- Time zones -----------------------------------------------------------

pub(crate) fn ensure_known_tz(name: &str) -> Result<(), CanonicalError> {
    if KNOWN_TIME_ZONES.contains(&name) {
        Ok(())
    } else {
        Err(CanonicalError::UnknownTimeZone(name.to_string()))
    }
}

pub(crate) fn check_known_tz(name: &str) -> Result<(), CanonicalDecodeError> {
    if KNOWN_TIME_ZONES.contains(&name) {
        Ok(())
    } else {
        Err(CanonicalDecodeError::UnknownTimeZone(name.to_string()))
    }
}

// --- Sets / maps ----------------------------------------------------------

/// Sort a sequence-that-represents-a-set in place by canonical-encoded element
/// bytes, rejecting duplicates. Used for `tags` and policy attestation sets.
pub(crate) fn canonicalize_set<T: Serialize>(items: &mut Vec<T>) -> Result<(), CanonicalError> {
    // Key each item by its canonical encoding.
    let mut keys: Vec<Vec<u8>> = Vec::with_capacity(items.len());
    for it in items.iter() {
        keys.push(postcard::to_stdvec(it).map_err(CanonicalError::Codec)?);
    }
    // Argsort indices by key.
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by(|&a, &b| keys[a].cmp(&keys[b]));
    // Reject duplicates (equal encodings).
    for w in order.windows(2) {
        if keys[w[0]] == keys[w[1]] {
            return Err(CanonicalError::DuplicateSetElement);
        }
    }
    // Apply the permutation without requiring `T: Clone`.
    let mut slots: Vec<Option<T>> = items.drain(..).map(Some).collect();
    let mut out: Vec<T> = Vec::with_capacity(order.len());
    for i in order {
        out.push(slots[i].take().expect("each index taken once"));
    }
    *items = out;
    Ok(())
}

/// Verify a decoded sequence-that-represents-a-set is in canonical order with
/// no duplicates.
pub(crate) fn check_set_canonical<T: Serialize>(items: &[T]) -> Result<(), CanonicalDecodeError> {
    let mut prev: Option<Vec<u8>> = None;
    for it in items {
        let key = postcard::to_stdvec(it).map_err(CanonicalDecodeError::Codec)?;
        if let Some(p) = &prev {
            match key.cmp(p) {
                Ordering::Less => return Err(CanonicalDecodeError::UnsortedSet),
                Ordering::Equal => return Err(CanonicalDecodeError::DuplicateSetElement),
                Ordering::Greater => {}
            }
        }
        prev = Some(key);
    }
    Ok(())
}
