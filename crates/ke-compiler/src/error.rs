//! Compiler errors, carrying a `YamlSpan` where one is available so diagnostics
//! can point back into the authoring YAML.

use crate::ast::YamlSpan;
use thiserror::Error;

/// An error from parsing or lowering, with an optional source location.
#[derive(Debug, Error)]
#[error("{message}{}", location_suffix(.span))]
pub struct CompileError {
    pub message: String,
    pub span: YamlSpan,
}

impl CompileError {
    pub fn new(message: impl Into<String>, span: YamlSpan) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }

    /// An error with no known location (e.g. a whole-document problem).
    pub fn unlocated(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span: YamlSpan::default(),
        }
    }
}

fn location_suffix(span: &YamlSpan) -> String {
    match span.start {
        Some(p) => format!(" (line {}, column {})", p.line, p.column),
        None => String::new(),
    }
}
