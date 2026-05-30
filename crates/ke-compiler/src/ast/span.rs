//! `YamlSpan` — a **parser-local** source position (1-indexed line/column) into
//! the authoring YAML. Used for diagnostics and file-traceability ONLY. It is
//! never the same type as the legal `ke-core::ir::SourceSpan` (document/provision
//! provenance) and is dropped during lowering. See ADR 0004.
//!
//! marked-yaml reports line/column (not byte offsets), so a position is
//! `{ line, column }`.

/// A 1-indexed line/column position in the authoring YAML.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

/// A start..end range in the authoring YAML. Either end may be unknown.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct YamlSpan {
    pub start: Option<Position>,
    pub end: Option<Position>,
}

impl YamlSpan {
    /// Lift a `marked_yaml::Span` into a `YamlSpan`.
    pub fn from_marked(span: &marked_yaml::Span) -> Self {
        let to_pos = |m: &marked_yaml::Marker| Position {
            line: m.line(),
            column: m.column(),
        };
        YamlSpan {
            start: span.start().map(to_pos),
            end: span.end().map(to_pos),
        }
    }

    /// True if at least a start position is known (the parser produced a real
    /// location rather than a blank).
    pub fn is_known(&self) -> bool {
        self.start.is_some()
    }
}
