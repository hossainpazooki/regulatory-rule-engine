//! Obligations triggered by a decision. Ported from the platform's
//! `ObligationSpec` (`src/rules/service.py`).

use super::source_span::SourceSpan;
use serde::{Deserialize, Serialize};

/// A single obligation a decision leaf may impose.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObligationSpec {
    pub id: String,
    pub description: Option<String>,
    pub deadline: Option<String>,
    pub source_span: Option<SourceSpan>,
}
