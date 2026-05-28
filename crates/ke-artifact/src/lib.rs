//! ke-artifact: canonical encoding + content addressing + signatures + attestations.
//!
//! See spec § 8, 9, 10. The PyO3 binding for the platform (`ke-artifact-py`)
//! lives behind the `pyo3` feature; see spec § 14.
//! Implementation lands in Gate 4.

#![deny(unsafe_code)]
