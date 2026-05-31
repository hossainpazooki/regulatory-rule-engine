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
separate from legal `SourceSpan`; T4 = four **provisional implemented** classes
(ADR 0005 still **Proposed**, pending Hossain sign-off).

**Gate 2 status:** implementation and tests are **green** (fmt/clippy clean, full
workspace test suite passing). Full Gate 2 **acceptance is not yet met** — it
depends on the **live Rust↔Python differential run** against
`../institutional-defi-platform-api` at the recorded SOURCE.md SHA
`f73b9403c88a7ab5d741b351dce085b6988b6ba7` (and on the ADR 0005 sign-off for the
T4 severity policy). The harness is complete and SHA-gated; only the cross-repo
run remains.

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
  RWA. Conclusion (as later corrected): **no operator/DSL-syntax extension is
  required** — the §20 ontology mismatch is handled by externalizing standards as
  boolean facts + interpretation notes. The §20 *operator* checkpoint is clear
  without new DSL constructs. **However, one IR-contract amendment was discovered
  later** during Phase-1 lowering (`effective_window` must be optional — ADR
  0006); that touched Gate-1-frozen `ke-core`. So: no DSL/operator extension, but
  one IR-shape amendment. (The gap-review doc carries this same correction.)
- `docs/adr/README.md` index updated (0004, 0005; 0006 added with the amendment).

**Sign-off still required from Hossain:** ADR 0005's T4 classes + severities
(domain-reviewer acceptance, per spec §23). The Phase-3 T4 verifier is already
implemented against these classes, but the class→severity policy is
**provisional** until that sign-off (see C3 and Phase 3).

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
  `OPERATOR_MAP`; ISO-date parsing; `None` window for date-less rules; for
  date-bearing rules the YAML-absent time zone defaults to `UTC` (see the
  Timezone note below).
- `error.rs` — `CompileError` with optional `(line, column)`.
- `bin/ke-compile.rs` — `compile <file>` dev command (Phase 2 adds `diff` /
  `--emit semantic-json`).

**Verification:** `ke-compile compile` lowers **all 7 corpus files (34 rules)**
with no errors — no further IR gaps beyond the effective-window one. `cargo fmt`
+ `cargo clippy --workspace --all-targets -D warnings` clean; `cargo test
--workspace` green (incl. 2 `value.rs` unit tests).

### Timezone handling — non-authoritative compatibility metadata (follow-up flagged)

The corpus YAML is **date-only**: no rule carries a time zone. But the Gate-1
`EffectiveWindow` requires a `jurisdiction_time_zone`. So for a rule that *does*
have effective dates, lowering synthesizes `jurisdiction_time_zone = "UTC"`.

Honest statement of where this lands:

- It **is not** derived from source and **does not** invent regulatory meaning:
  the semantic normal form **drops the zone entirely**, so it never participates
  in Rust↔Python equivalence or in T4 reasoning. No decision, applicability, or
  conflict depends on it.
- It **does** enter the **canonical IR bytes** (and therefore a future content
  hash) for date-bearing rules, as **non-authoritative compatibility metadata**
  required only to satisfy the current `EffectiveWindow` shape. `UTC` is a
  placeholder, not a claim that an EU rule is UTC-effective.
- **Follow-up (flagged, not silently accepted):** Gate 3 owns real
  jurisdiction→zone resolution (ADR 0001). Before any artifact is *published*
  (Gate 4), the placeholder must be resolved one of two ways — derive the zone
  from jurisdiction, **or** make `jurisdiction_time_zone` optional (a small
  follow-up amendment, mirroring ADR 0006's `effective_window` change) so a
  date-only rule carries no invented zone in canonical bytes. Tracked as a Gate-3
  prerequisite; not changed in Gate 2 to avoid an unrequested second IR amendment.

---

## Tracked acceptance constraints (review — must stay green through Gate 2)

These three review constraints are **acceptance criteria**, not just notes. Each
phase that touches the relevant area must keep them satisfied; Phase 4 final
verification re-checks all three.

### C1 — `effective_window` optionality is consistent everywhere

`RuleIR.effective_window: Option<EffectiveWindow>` must be represented
consistently across **all** of:

- Rust IR types — `crates/ke-core/src/ir/rule.rs` (field is `Option`). ✅ Phase 1.
- canonical encode/decode — `crates/ke-core/src/canonical/{encode,decode}.rs`
  skip `None`, walk `Some`. ✅ Phase 1.
- JSON schema — `crates/ke-core/src/schema/defs.rs` (`s_nullable`, dropped from
  `required`); committed `ir.schema.json` regenerated. ✅ Phase 1.
- golden fixtures — `fixtures/artifacts/` regenerated under `0.2.0`/`ke-canon-2`. ✅ Phase 1.
- docs / ADR — ADR 0006 + `docs/canonical-encoding.md` version table. ✅ Phase 1.
- **semantic normal form + differential** — `ke-core::semantic` must model the
  window as optional and treat Rust `None` ≡ Python `Rule.effective_from = None`.
  → **Phase 2 obligation.**

### C2 — `YamlSpan` vs legal `SourceSpan` separation (ADR 0004)

The boundary is hard and must not erode:

- `YamlSpan` is **parser-local diagnostic metadata only**
  (`crates/ke-compiler/src/ast/span.rs`); it lives on the AST, not the IR.
- It is **never serialized as canonical IR** — lowering drops it; it is absent
  from `ke-core` entirely.
- It is **never treated as legal `SourceSpan`** — legal provenance comes from the
  rule's `source:` / `DocumentRef`.
- T1 legal-`SourceSpan` **inheritance is provenance-based** (nearest ancestor up
  to the mandatory rule-level `source:`), **never YAML-line-based**.
  → **Phase 3 (T1) obligation;** Phase 2 semantic form keys provenance off
  `DocumentRef`, not `YamlSpan`.

### C3 — ADR 0005 T4 severities are PROVISIONAL (pending Hossain sign-off)

ADR 0005's four T4 classes and their severities are **provisional** until the
domain reviewer (Hossain) signs off. The Phase 3 T4 verifier must:

- keep the class→severity mapping in **one easy-to-review place** (a single
  `default_severity(class)` function / table), not scattered or inlined;
- carry a doc comment flagging the mapping as provisional + ADR-0005-pending;
- make the severities trivially adjustable (and, later, overridable by a
  `PolicyBundle` per environment — Gate 4).
  → **Phase 3 (T4) obligation.**

---

## Phase 2 — semantic normal form + differential harness ✅

- `ke-core::semantic::{form,diff}` — `SemanticRule` reduces a `RuleIR` to its
  *meaning* (rule id, applicability predicate with sorted `all`/`any`, the **set
  of root-to-leaf decision paths**, source **document** id, optional effective
  window, sorted tags). Abstracts away node ids, member order, decimal
  representation (`0.90 ≡ 0.9`), `in`/`not_in` order, and prose. `semantic_diff`
  reports field-level differences. **Computed from `RuleIR`**, so both Rust and
  Python sides reduce through the same code path.
  - **C1:** `effective` is `Option`; Rust `None` ≡ Python `None`.
  - **C2:** provenance keyed off `DocumentRef.document_id`, never a `YamlSpan`.
- `ke-compiler::python_import` — Python `Rule.model_dump(mode="json")` →
  `RuleIR`; numbers parsed from their JSON literal to exact decimals (no `f64`).
- `ke-compile` bin extended: `compile --emit semantic-json` and
  `diff <yaml> <python-json>` (exit non-zero on divergence).
- `scripts/differential-test.sh` rewritten: SHA-gated (requires the platform at
  the recorded SOURCE.md SHA, `src/rules/data` clean), runs the platform
  `RuleLoader` per file, and `ke-compile diff`s each.

**Verification:**
- Diff *mechanics* validated locally with hand-crafted files: an equivalent
  Python-shaped JSON (with swapped `all` order, `0.9` vs `0.90`, different
  `node_id`, different obligation prose) → **OK / exit 0**; a changed result →
  **DIVERGENCE / exit 1**.
- `differential-test.sh` correctly **fails fast** (platform at `224dcab` ≠
  recorded `f73b940`) with the exact `git checkout` instruction.
- `cargo fmt`/`clippy -D warnings` clean; `cargo test --workspace` green.
- **Pending (user environment):** the live Rust↔Python parity run needs
  `../institutional-defi-platform-api` checked out at `f73b940` with its Python
  deps installed.

---

## Phase 3 — T0/T1/T4 verification ✅

`ke-compiler::verify` — `Finding`, `VerificationReport`, `verify(rules)`:

- **T0** (`t0.rs`, blocking): structural invariants a lowered `RuleIR` could
  still violate — empty `rule_id`/`version`/`source.document_id`, empty
  `applies_if` group, empty leaf `result`.
- **T1** (`t1.rs`, blocking): source-span coverage **by inheritance** from the
  mandatory rule-level `source:` (provenance-based, never YAML-line-based — C2);
  plus required `interpretation_notes` when a rule carries a numeric threshold
  (`>`,`<`,`>=`,`<=` on a decimal) or a `not_in` exception (spec §17).
- **T4** (`t4.rs` + `conflict.rs`, severity-dependent): the four ADR-0005
  classes. Scope overlap via top-level `all` `==`/`in` premises (disjoint value
  sets ⇒ no overlap); contradiction via a consistent decision-path pair with
  different results; duplicate via semantic-form logic equality; temporal via
  overlapping `[from,to)` windows (both present) + scope overlap.
  - **C3:** the class→severity policy lives in the single
    `conflict::default_severity()` function, doc-flagged **PROVISIONAL pending
    ADR-0005 sign-off**, and is the only place to adjust.

## Phase 4 — tests + docs ✅

- `tests/parser_spans.rs` (3) — `YamlSpan`s present on rule id / applicability /
  condition / decision / obligation; positioned errors for missing fields and
  malformed YAML.
- `tests/lowering.rs` (2) — the whole corpus (34 rules) lowers, canonically
  encodes, and round-trips; **5 rules have no effective window** (C1 / fca_crypto).
- `tests/verify_t0_t1.rs` (4) — T0 empty-id, T1 threshold-without-notes, T1
  satisfied-with-notes, clean-rule-no-findings.
- `tests/t4_conflicts.rs` (4) + `tests/fixtures/conflicts/*.yaml` — each class
  asserts the expected class + severity.
- `tests/python_import.rs` (1) — a Python-`model_dump`-shaped JSON (swapped
  order, `0.9` vs `0.90`, different ids/prose) reduces to the **same** semantic
  form as the Rust lowering.

### Final verification

| Check | Result |
| ----- | ------ |
| `cargo fmt --all -- --check` | clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `cargo test --workspace` | all pass (23 suites, 0 failures) |
| `cargo check -p ke-core --target wasm32` | green |
| `ke-compile` on the corpus | 34 rules, 0 errors |
| `differential-test.sh` (wrong-SHA) | fails fast with checkout instruction |

### Review-constraint confirmation (C1–C3)

- **C1** — `effective_window: Option` verified consistent across IR types,
  canonical encode/decode, schema (`s_nullable`), regenerated fixtures, ADR 0006,
  **and the semantic form** (`SemanticRule.effective: Option<SemWindow>`, tz
  dropped). `tests/lowering.rs` asserts 5 window-less corpus rules; the
  `python_import` test proves Rust `None` ≡ Python `None`.
- **C2** — `YamlSpan` lives only on the AST (`ast/span.rs`), is dropped in
  lowering, and is absent from `ke-core`/canonical IR. Legal coverage (T1) and
  semantic provenance key off `DocumentRef`, never YAML lines.
- **C3** — T4 severities are in the single `conflict::default_severity()` table,
  doc-flagged provisional (ADR 0005 still **Proposed**, awaiting Hossain's
  sign-off); `t4_conflicts.rs` asserts the provisional mapping.

### Still pending (user environment)

The live Rust↔Python differential (`scripts/differential-test.sh`) needs
`../institutional-defi-platform-api` checked out at `f73b940` with Python deps.
The harness, adapter, and semantic form are complete and unit-validated; only the
cross-repo run is gated on that checkout.
