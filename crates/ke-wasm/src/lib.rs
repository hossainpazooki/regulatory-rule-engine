//! ke-wasm: browser bindings for preview compile + dry-run.
//!
//! WASM is preview-only. It MUST NOT sign, attest, or publish.
//! See spec § 6 "WASM discipline" and § 16 (multi-surface access).
//! Implementation lands in Gate 5.

#![deny(unsafe_code)]
