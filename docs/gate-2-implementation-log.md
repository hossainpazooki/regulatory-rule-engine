# Gate 2 — implementation log

Phase-by-phase record of the Gate 2 implementation (parser, lowering,
verification), written as each phase lands. Authoritative contract:
`dev/briefs/gate-2-parser-compiler-verification.md`. Continues the Gate 1
doc-each-phase convention so Hossain's manual gate review is fast and auditable.

**Branch:** `migration/gate-2-parser-compiler` (off committed Gate 1).
**Toolchain:** Rust 1.85.0 (`x86_64-pc-windows-gnu`).
**Locked decisions:** tree IR + semantic normal form (no flattened IR);
differential pinned to the recorded SOURCE.md SHA
(`f73b9403c88a7ab5d741b351dce085b6988b6ba7`), fail-fast; `marked-yaml` spans kept
separate from legal `SourceSpan`; T4 = four accepted classes.

---

## Phase 0 — brief, ADRs, DSL gap review ✅

- `dev/briefs/gate-2-parser-compiler-verification.md` — the gate brief
  (mandatory §22 sections).
- `docs/adr/0004-source-span-coverage-policy.md` (**Accepted**) — distinguishes
  YAML parse spans (`YamlSpan`, parser-local) from legal `SourceSpan`, and sets
  the T1 coverage-by-inheritance rule + the interpretation-notes requirement.
- `docs/adr/0005-t4-conflict-classes-gate-2.md` (**Proposed — needs domain
  sign-off**) — the four T4 classes + severities (`contradictory_outcome`
  Blocking, `overlapping_scope` / `temporal_overlap` Review-required,
  `duplicate_rule` Advisory).
- `docs/dsl-gap-review-gate-2.md` — walked MiCA / GENIUS / FCA / FINMA / MAS /
  RWA: **no DSL/IR extension required**; the §20 ontology mismatch is handled by
  externalizing standards as boolean facts + interpretation notes. §20 checkpoint
  cleared without touching Gate-1-frozen `ke-core`.
- `docs/adr/README.md` index updated (0004, 0005).

**Sign-off needed from Hossain before Phase 3 hardens:** ADR 0005 T4 classes +
severities (domain-reviewer acceptance, per spec §23).

**Verification:** docs only; no code yet. `cargo check --workspace` unchanged
from Gate 1 (green).

---

## Gate-1 amendment — `effective_window` optional (ADR 0006) ✅

Discovered during Phase 1: `fca_crypto.yaml`'s rules carry no effective window,
which the mandatory Gate-1 `effective_window` could not represent. **Surfaced to
Hossain** (the §20 checkpoint), who chose to amend the IR rather than synthesize
a sentinel date.

- `ke-core`: `RuleIR.effective_window` → `Option<EffectiveWindow>`; canonical
  encode/decode skip `None`; schema field nullable + dropped from `required`.
- Version triplet bumped: `ir_schema_version 0.1.0 → 0.2.0`,
  `canonicalization_version ke-canon-1 → ke-canon-2` (`postcard-1` unchanged).
- Gate 1 golden fixtures + `ir.schema.json` regenerated; `docs/canonical-encoding.md`
  version table, `dsl-gap-review-gate-2.md` conclusion, and `adr/README.md`
  updated. ADR 0006 written.

**Verification:** `cargo test -p ke-core` = 19 passed under the new triplet;
fixtures idempotent.

## Phase 1 — span-tracking parser + lowering ✅

- `ke-compiler/Cargo.toml`: added `marked-yaml` 0.7 + `serde_json`; dropped
  `serde_yaml`; declared the `ke-compile` dev bin.
- `ast/{span,mod}.rs` — `YamlSpan` (parser-local line/col; never legal
  `SourceSpan`) + the spanned AST.
- `parser.rs` — `marked-yaml` → AST. Files may be a top-level mapping (single
  rule) or sequence (list); marked-yaml's strict top-level mode is handled by
  trying mapping then falling back to `toplevel_sequence()` on the specific
  `TopLevelMustBeMapping` error.
- `value.rs` — YAML scalar → `ScalarValue`, decimals as mantissa/scale (no
  floats), quoted scalars stay strings (`may_coerce`), matching `yaml.safe_load`.
- `lower.rs` — AST → `ke_core::ir::RuleIR`; operator map mirrors the platform
  `OPERATOR_MAP`; ISO-date parsing; `None` window for date-less rules; UTC tz
  placeholder.
- `error.rs` — `CompileError` with optional `(line, column)`.
- `bin/ke-compile.rs` — `compile <file>` dev command (Phase 2 adds `diff` /
  `--emit semantic-json`).

**Verification:** `ke-compile compile` lowers **all 7 corpus files (34 rules)**
with no errors — no further IR gaps beyond the effective-window one. `cargo fmt`
+ `cargo clippy --workspace --all-targets -D warnings` clean; `cargo test
--workspace` green (incl. 2 `value.rs` unit tests).

---
<!-- Phases 2–4 appended below as they land. -->
