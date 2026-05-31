# Gate 2 — Parser, compiler, structural verification (brief)

**Status:** implemented on `migration/gate-2-parser-compiler` — parser, lowering,
semantic form + differential harness, and T0/T1/T4 complete; see
[`docs/gate-2-implementation-log.md`](../../docs/gate-2-implementation-log.md).
Remaining for full acceptance: ADR 0005 T4 sign-off and the live differential run
(platform at `f73b940`). Pending Hossain review/merge.
**Authoritative spec sections:** §6 (crate layout), §11 (verification model),
§12 (T4 taxonomy), §19 (Gate 2), §20 (DSL ontology mismatch), §22 (Gate 2 outline).
**Predecessor:** Gate 1 (canonical IR) — committed; acceptance green.
**Successor:** Gate 3 (preview runtime + equivalence harness).

This brief is the contract for the Gate 2 implementation. It mirrors the Gate 1
brief structure and the mandatory §22 brief sections.

---

## 1. Context

Gate 1 froze `ke-core::ir::RuleIR` — the **un-lowered authoring tree** — plus
canonical encoding and the JSON Schema. Gate 2 makes that IR *producible from
YAML and checkable*:

- a span-tracking YAML parser (`marked-yaml`) producing a spanned AST,
- AST→IR lowering into `ke-core::RuleIR` (no flattening),
- T0/T1/T4 verification passes,
- a Rust↔Python differential proven at a **semantic normal form** level.

Relevant Python sources (the parity target, in `institutional-defi-platform-api`):
`src/rules/service.py` (`RuleLoader` YAML→`Rule` tree; `_parse_*`, `_parse_value`),
`src/production/compiler.py` (`_extract_premise_keys` — reused idea for T4 scope
overlap). Gate 2 compares against Python's **`Rule` tree** (post-`RuleLoader`),
not the flattened `RuleIR` (the jump-table flattening is internal execution order,
outside the equivalence boundary per spec §20).

## 2. Locked decisions

1. **Tree IR + semantic normal form.** Lower to `ke-core::RuleIR`; no flattened
   `CompiledRuleIR` in Gate 2.
2. **YAML parse spans ≠ legal source spans.** Parser-local `YamlSpan` (byte
   ranges, line/col) for diagnostics/traceability only; legal provenance stays
   in `ke-core::ir::SourceSpan`/`DocumentRef`. See ADR 0004.
3. **Differential pin = recorded SOURCE.md SHA** (`f73b9403c88a7ab5d741b351dce085b6988b6ba7`).
   Harness fails fast unless `../institutional-defi-platform-api` is at exactly
   that SHA with `src/rules/data` clean. No corpus re-bootstrap in this gate.
4. **T4 = the four accepted classes only** (ADR 0005).

## 3. Phase 1 deliverables (files)

- `crates/ke-compiler/Cargo.toml` — add `marked-yaml`; drop unused `serde_yaml`;
  declare the `ke-compile` dev bin.
- `crates/ke-compiler/src/ast/{mod,span}.rs` — spanned AST + `YamlSpan`
  (parser-local).
- `crates/ke-compiler/src/parser.rs` — `marked-yaml` Node tree → AST; positioned
  `ParseError`. Handles single-rule and list-of-rules files.
- `crates/ke-compiler/src/value.rs` — YAML scalar → `ke-core::ScalarValue`
  (bool / integer / **decimal mantissa·scale** / string / list; **no floats**).
- `crates/ke-compiler/src/lower.rs` — AST → `ke-core::RuleIR`; drops `YamlSpan`s,
  populates legal source refs from rule-level `source:`/`source_span`.
- `crates/ke-compiler/src/error.rs` — `CompileError` carrying spans.
- `crates/ke-core/src/semantic/{mod,form,diff}.rs` — `SemanticRule` normal form +
  `semantic_diff` (spec §6 "semantic diff helpers").
- `crates/ke-compiler/src/python_import.rs` — Python `Rule` JSON →
  `ke-core::RuleIR` (differential only).
- `crates/ke-compiler/src/bin/ke-compile.rs` — dev tool: `compile … --emit
  semantic-json`, `diff <yaml> <python-json>`.
- `crates/ke-compiler/src/verify/{mod,t0,t1,t4,conflict}.rs` — T0/T1/T4 + finding
  types.
- `scripts/differential-test.sh` — rewrite (SHA-pinned harness).

## 4. Phase 2 verification commands

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p ke-compiler
cargo test --workspace
# with the platform checked out at the recorded SOURCE.md SHA:
bash scripts/differential-test.sh
```

## 5. Acceptance criteria (spec §19 Gate 2)

- Given every YAML in `fixtures/rules/`, when compiled by Rust and Python, then
  the **semantic normal forms are equal** (differential harness green).
- Given known conflict fixtures (`crates/ke-compiler/tests/fixtures/conflicts/`),
  when compiled, then T4 emits the expected conflict **class and severity**.
- (The boundary-value *execution* parity bullet in §19 is Gate 3 — preview
  runtime — per the §22 Gate 2 outline. Not in this gate.)

## 6. Known risks (spec §20)

- **DSL ontology mismatch.** Standards-based provisions ("fair, clear, not
  misleading", fit-and-proper, proportionality) are externalized as boolean
  facts (e.g. `whitepaper_compliant`). The DSL needs no extension (see
  `docs/dsl-gap-review-gate-2.md`); these are interpretation points requiring
  `interpretation_notes` + expert attestation (Gate 4). **Checkpoint honored:**
  no Gate-1-frozen `ke-core` IR change is required.
- **Duplicate runtime drift.** Mitigated by the semantic-form differential over
  the full corpus, pinned to the recorded SHA.
- **Decimal/float boundary.** Python stores numbers as floats; the import
  adapter reads them via arbitrary-precision JSON (literal string → decimal),
  never `f64` (ADR 0003).

## 7. Out-of-scope clarifications (must NOT do)

Flattened/jump-table IR, optimizer, premise *index* build, preview execution,
the other four T4 classes (`source_span_divergence` needs `ke-search`), signing/
registry, WASM/frontend, corpus re-bootstrap, **any LLM/AI code** (LLM is
out-of-band authoring assistance only — never in `ke-compiler` or any
deterministic path).

## 8. Commit boundary

Hossain commits/merges manually on `migration/gate-2-parser-compiler` after
review. Claude Code makes no commits or pushes. `fixtures/` is never hand-edited.

## 9. Platform access (spec §4.5)

`differential-test.sh` resolves `${PLATFORM_REPO:-../institutional-defi-platform-api}`,
requires HEAD == the SHA recorded in `fixtures/rules/SOURCE.md`, records that SHA
in its output, and fails fast if the checkout is missing, dirty under
`src/rules/data`, or at any other commit.
