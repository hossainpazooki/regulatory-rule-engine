//! Source-span references. Every IR node that represents a decision node,
//! obligation, threshold, exception, or discretionary term carries a span so
//! Gate 2 can enforce coverage (brief principle 3). Gate 1 only makes the
//! carrier shape exist.

use serde::{Deserialize, Serialize};

/// A document citation. Ported from the platform's `SourceRef`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentRef {
    pub document_id: String,
    pub article: Option<String>,
    pub section: Option<String>,
    /// Paragraph references (legal text granularity finer than article).
    pub paragraphs: Vec<String>,
    pub pages: Vec<u32>,
    pub url: Option<String>,
}

/// A half-open byte range `[start, end)` into a referenced source segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByteRange {
    pub start: u64,
    pub end: u64,
}

/// A reference to a specific span of legal source text.
///
/// `text_hash` is the BLAKE3 of the referenced segment — populated only once
/// the legal-source-storage open decision (spec § 21.4) is resolved; it is
/// structurally optional in Gate 1.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSpan {
    pub document_id: String,
    pub article: Option<String>,
    pub section: Option<String>,
    pub paragraph: Option<String>,
    pub pages: Option<Vec<u32>>,
    pub byte_range: Option<ByteRange>,
    pub text_hash: Option<[u8; 32]>,
}
