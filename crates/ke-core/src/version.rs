//! Version triplet pinning the IR schema, wire codec, and canonicalization
//! profile. All three travel in the artifact manifest (spec § 8.1, § 4.9 of the
//! brief) and are reproduced in any decode error so a mismatch is immediately
//! diagnosable.
//!
//! A change to any of these three is a **breaking** change to the artifact
//! format. In particular, reordering a struct field — which changes the
//! canonical byte layout — requires bumping [`CANONICALIZATION_VERSION`]. See
//! `docs/canonical-encoding.md`.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Semver-ish IR schema version (`major.minor.patch`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl SchemaVersion {
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

impl fmt::Display for SchemaVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Opaque codec version string (e.g. `"postcard-1"`). See ADR 0002.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodecVersion(pub String);

/// Opaque canonicalization-profile version string. Bumped whenever the byte
/// layout changes (field reorder, ordering-rule change, etc.).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalizationVersion(pub String);

/// Pinned IR schema version for Gate 1.
pub const IR_SCHEMA_VERSION: SchemaVersion = SchemaVersion::new(0, 1, 0);

/// Pinned codec version. See ADR 0002 (`docs/adr/0002-canonical-codec-postcard.md`).
pub const CODEC_VERSION: &str = "postcard-1";

/// Pinned canonicalization-profile version. See `docs/canonical-encoding.md`.
pub const CANONICALIZATION_VERSION: &str = "ke-canon-1";
