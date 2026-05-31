# Gate 3 — implementation log

Phase-by-phase record of the Gate 3 implementation (preview runtime + scenario
scaffolding + property/metamorphic tests + fuzzed Rust↔Python equivalence
harness), written as each phase lands. Authoritative contract:
`dev/briefs/gate-3-preview-runtime.md`. Continues the Gate 1/2 doc-each-phase
convention so Hossain's manual gate review is fast and auditable.

**Branch:** `migration/gate-3-preview-runtime` (off accepted Gate 2).
**Toolchain:** Rust 1.85 (`x86_64-pc-windows-gnu`).
**Equivalence pin:** platform `../institutional-defi-platform-api` at the recorded
SOURCE.md SHA `f73b9403c88a7ab5d741b351dce085b6988b6ba7`, `src/rules/data` clean
(same pin as Gate 2). Reference interpreter: system Python 3.14.2 + pydantic
2.12.5 (no venv required; matches the Gate 2 differential fallback).

**Locked decisions** (ADRs 0007, 0008): walk the tree (no flattened Rust IR);
equivalence boundary = observable semantics (outcomes, obligation id-sets,
normalized trace, error classes); mirror the executor's applicability *flattening*
(not the Gate-2 semantic form's recursion); the executor is **total**; the
runtime is **date-agnostic** in the equivalence path (Python `RuleRuntime` never
evaluates effective dates); `effective_at()` `[from,to)` is preview-only and out
of the boundary; `jurisdiction_time_zone` becomes optional (Phase 4 IR amendment).

**Gate 3 status: ✅ COMPLETE — ready for Hossain review/merge (2026-05-31).**
All spec §19 Gate 3 acceptance criteria are met (see Phase 5). Live Rust↔Python
equivalence is green over 1326 scenarios at the recorded platform SHA; Gate 2's
differential stays 7/7 after the tz-optional IR amendment. No commits made —
Hossain owns the merge.

---

## Phase 0 — brief + ADRs ✅

- `dev/briefs/gate-3-preview-runtime.md` — the gate brief (mandatory §22
  sections), mirroring the Gate 1/2 briefs.
- `docs/adr/0007-effective-window-preview-runtime.md` (**Accepted**) — makes
  `jurisdiction_time_zone` optional (amends ADR 0006, refines ADR 0001) so
  date-only corpus rules carry no invented `UTC`; defines `effective_at()`
  `[from,to)` as a preview-only filter **outside** the Rust↔Python boundary;
  records the deliberate divergence from the platform's legacy closed-closed
  `[from,to]` `get_applicable_rules` pre-filter.
- `docs/adr/0008-execution-equivalence-boundary.md` (**Accepted**) — pins the
  execution equivalence boundary (outcomes, obligation id-sets, normalized trace =
  evaluated-applicability prefix + taken decision path, total executor / no
  per-eval error classes), the executor-faithful applicability flattening, and the
  `FactValue` representation (Int/Float distinct; decimal→shortest-string→f64;
  generator keeps numbers off `in`/`not_in`).
- `docs/adr/README.md` — index updated (Gate 3 section: 0007, 0008).

**Key findings that shaped the design (verified against platform code + CPython):**

- The Python `RuleRuntime` (`src/production/executor.py`) is **date-agnostic** —
  it never reads `effective_from`/`effective_to`. Effective-date filtering is a
  separate `RuleLoader.get_applicable_rules` pre-filter using a **closed-closed**
  `[from,to]` interval, which diverges from spec §8.4's `[from,to)`. This scopes
  the equivalence boundary to the date-agnostic executor and resolves the Gate-2
  timezone follow-up cleanly (ADR 0007).
- `RuleCompiler._flatten_conditions` **discards nested `all`/`any` modes** (splices
  members under the parent mode). The Gate-2 semantic form recurses; for execution
  parity the runtime must follow the executor (flatten). No corpus rule has a
  nested `applies_if` group (verified), so this is unit-tested, not corpus-tested.
- CPython operator truth table (probed empirically) drives `compare.rs`:
  `True==1`/`1==1.0` true; str↔num never equal; ordering on str↔num/`None` →
  `TypeError`→false; `in` uses `str()`-coercion (`str(True)=="True"`,
  `str(5.0)=="5.0"`); `exists` = not-`None`.
- Obligations in the decision result come **only from the matched leaf**.

**Verification:** docs only; no code yet. `cargo check --workspace` unchanged from
Gate 2 (green).

---

## Phase 1 — executor core (value/compare/exec) ✅

- `crates/ke-runtime/src/value.rs` — `FactValue` (`Null|Bool|Int(i128)|Float(f64)|
  Str|List`); `FactValue::from_json` preserving the JSON int-vs-float kind;
  `python_str_fact`/`python_str_scalar` (CPython `str()` faithful, incl. the
  `Decimal` scale==0→int / scale>0→float rule); `decimal_to_string`/`_f64`
  (shortest-string→parse, matching Python `float()`); `lookup` collapsing
  absent→`Null`; `facts_from_json` (the one boundary validation).
- `crates/ke-runtime/src/compare.rs` — the nine operators, bit-faithful to
  `executor.py` + the empirical CPython truth table (bool-is-int, str↔num never
  equal, ordering `TypeError`→false, `in` str()-coercion, `exists`).
- `crates/ke-runtime/src/exec.rs` — `Mode`, `flatten` (mirrors
  `_flatten_conditions`, discards nested mode), `applicability` (short-circuit +
  empty-checks early-return), tree `walk`, `Evaluation` (outcome + normalized
  trace; obligations from the matched leaf only; `obligation_ids()`).
- `crates/ke-runtime/src/trace.rs` — `NormStep` + `op_token` (canonical YAML
  token set).
- `crates/ke-runtime/src/lib.rs` — module wiring + re-exports; `#![deny(unsafe_code)]`.
- `crates/ke-runtime/Cargo.toml` — lib depends only on `ke-core` (stays
  wasm-clean for Gate 5); native tooling (`ke-compiler`, `anyhow`) behind a
  default `tools` feature.

### proptest dropped (toolchain blocker) — deterministic generator instead

The spec §22 Gate-3 outline names `proptest`. It cannot be used here: this
repo's `x86_64-pc-windows-gnu` toolchain cannot build `getrandom 0.3` (which
`proptest`→`rand` pulls in), because `getrandom` uses **raw-dylib** import libs
for `bcryptprimitives.dll`/`kernel32.dll`, and the toolchain's self-contained
`dlltool.exe` fails with `CreateProcess` (it can't spawn its assembler). This is
an environment limitation, not a code issue. **Resolution:** Phase 2's
property/metamorphic tests and the scenario generator use a small self-contained
deterministic PRNG (no external crate, no raw-dylib), which also gives the
equivalence harness reproducible, recorded seeds. Coverage is preserved; only the
generator engine differs. Recorded in the brief (§3/§7), ADR 0008, and here.

**Verification:** `cargo test -p ke-runtime` = **22 passed**; `cargo clippy
--workspace --all-targets -- -D warnings` clean; `cargo fmt --all -- --check`
clean; `cargo test --workspace` green (all Gate 1/2 suites + ke-runtime 22).

## Phase 2 — normalized trace + scenarios + property/metamorphic tests ✅

- `crates/ke-runtime/src/effective.rs` — preview-only `effective_at(window,
  date)` with spec §8.4 closed-open `[from,to)` (date-only, zone-agnostic;
  **outside** the equivalence boundary — ADR 0007). Unit-tested at the boundaries
  (`to` is exclusive — the deliberate divergence from the platform's `[from,to]`).
- `crates/ke-runtime/src/scenario.rs` — `Scenario`, a SplitMix64 `Rng`
  (deterministic, no `getrandom`), `Catalog::from_rule` (conditions + `in_fields`),
  per-condition `value_true`/`value_false` (incl. decimal-LSB threshold shifts),
  `generate_for_rule` (branch-coverage paths, not-applicable, threshold
  boundaries {below/at/above}, missing-field, wrong-type, irrelevant-facts,
  seeded fuzz). Emits facts as JSON literals so number bits match cross-language;
  honors the in/not_in number invariant.
- `crates/ke-runtime/src/bin/ke-eval.rs` — `ke-eval <rule.yaml> <facts.json>` →
  normalized JSON (debug tool).
- `crates/ke-runtime/src/bin/gen-scenarios.rs` — `gen-scenarios [--seed N]
  [--fuzz K] <yaml>…` → JSONL `{rule_id,label,facts,rust:{normalized}}` with the
  seed/N echoed to stderr (reproducible).
- `Evaluation::normalized_json` (in `exec.rs`) — the compact compared shape
  (obligations as a sorted id set; representation-dependent fields dropped).
- `exec::flattened` exposed so the generator satisfies/violates applicability the
  same way the executor evaluates it.
- `crates/ke-runtime/tests/property.rs` (3) — evaluation determinism + totality
  (applicable ⇒ decision), canonical trace tokens, and the **generator
  invariant** (no numeric fact on any `in`/`not_in` field) over the corpus.
- `crates/ke-runtime/tests/metamorphic.rs` (5) — threshold/boolean negation flip
  the decision; removing a required `all` fact flips applicability; reordering an
  `all` group preserves outcome+obligations; adding irrelevant facts preserves
  the full normalized result over every corpus scenario.

`gen-scenarios --fuzz 5` over the corpus emits 476 scenarios (so `--fuzz 25`
clears the N≥1000 target); the harness (Phase 3) sets fuzz to reach N.

**Verification:** `cargo test -p ke-runtime` = **35 passed** (27 lib + 5
metamorphic + 3 property); `cargo clippy --workspace --all-targets -- -D warnings`
clean; `cargo fmt --all -- --check` clean.

## Phase 3 — equivalence harness + trace fixtures + live parity ✅

- `scripts/py_reference_runtime.py` — batched Python oracle: loads + compiles the
  corpus once (`RuleLoader` → `RuleCompiler`), runs `RuleRuntime.infer` per
  scenario, normalizes to the same shape (reconstructing the taken decision path
  from the matched entry's `condition_mask` + `decision_checks`), and compares to
  the embedded Rust result. Single process, no per-scenario spawn. Optionally
  emits the golden trace fixtures (only on a clean run).
- `scripts/equivalence-harness.sh` — SHA-gated (recorded SOURCE.md SHA, clean
  `src/rules/data`), cygpath/PYTHONPATH/cargo-on-PATH robust (mirrors
  `differential-test.sh`); builds `gen-scenarios`, generates N scenarios, pipes
  them through the Python oracle, records platform SHA + seed + N, fails fast on
  any divergence, and refreshes `fixtures/traces/golden.json`.
- `fixtures/traces/golden.json` — 35 harness-generated golden traces (Python
  oracle; paths, threshold boundaries incl. `redemption_fee_percentage <= 0`,
  missing-field, not-applicable, irrelevant-facts across 4 varied rules). Never
  hand-edited.
- `crates/ke-runtime/tests/trace_fixtures.rs` — asserts the Rust runtime
  reproduces every golden trace.

### Live parity — ✅ PASS (2026-05-31)

`bash scripts/equivalence-harness.sh` against the platform at SOURCE.md SHA
`f73b9403c88a7ab5d741b351dce085b6988b6ba7` (system Python 3.14.2):
**1326 scenarios over 34 rules, 0 divergences — Rust ≡ Python** (seed
`7242087531`, fuzz/rule `30`). This is the spec §19 Gate 3 N≥1000 acceptance
criterion. Re-run reproducibly via `KE_SEED` / `KE_FUZZ`.

**Verification:** `cargo test --workspace` green (ke-runtime: 27 lib + 5
metamorphic + 3 property + 1 trace_fixtures; all Gate 1/2 suites unchanged);
`cargo fmt --all -- --check` clean; `cargo clippy --workspace --all-targets --
-D warnings` clean; `equivalence-harness.sh` PASS over 1326 scenarios.

## Phase 4 — tz-optional IR amendment (ADR 0007) ✅

`EffectiveWindow.jurisdiction_time_zone` → `Option<TimeZone>`, so a date-only
corpus rule carries **no invented `UTC`** in canonical bytes (resolves the Gate-2
timezone follow-up). Consistent across every site:

- `crates/ke-core/src/ir/time.rs` — field is `Option<TimeZone>`.
- `crates/ke-core/src/canonical/{encode,decode}.rs` — `canonicalize_window` /
  `validate_window` walk the zone only when `Some`.
- `crates/ke-core/src/schema/defs.rs` — `jurisdiction_time_zone` is `s_nullable`
  and dropped from `effective_window`'s `required` (now just `["effective_from"]`).
- `crates/ke-core/src/version.rs` — triplet bumped: `ir_schema_version 0.2.0 →
  0.3.0`, `canonicalization_version ke-canon-2 → ke-canon-3` (`postcard-1`
  unchanged).
- `crates/ke-core/src/examples.rs` — example keeps `Some(tz())` (exercises the
  `Some` path); `tests/non_canonical.rs` updated (`Some(..)` + `.as_mut()`).
- `crates/ke-compiler/src/{lower,python_import}.rs` — emit `None` (placeholder
  `UTC`/`DEFAULT_TZ_DATA_VERSION` removed).
- Regenerated `crates/ke-core/schema/ir.schema.json` + `fixtures/artifacts/`
  (via `emit-schema`/`gen-fixtures` — idempotent; never hand-edited). Updated
  `docs/canonical-encoding.md` version table + dates/tz note.

### No-regression proof

- `cargo test -p ke-core` = 19 passed under the new triplet; fixtures idempotent
  (second regen → no diff).
- **`differential-test.sh` (Gate 2) — ✅ 7/7 files, 0 divergences** at the
  recorded SHA `f73b940` (the semantic form already drops the zone, so Gate 2 is
  unaffected by the amendment).
- **`equivalence-harness.sh` (Gate 3) — ✅ 1326/0** and `fixtures/traces/golden.json`
  **byte-unchanged** (the runtime is date-agnostic — the amendment touches only
  canonical bytes, never a decision).
- `cargo check -p ke-core --target wasm32-unknown-unknown` green; full workspace
  `clippy -D warnings` + `fmt --check` clean.

### Gate 4 readiness decisions (decided — implemented in Gate 4)

The two items ADR 0007 left open are now **decided** (decisions live in ADR 0007
§ Gate 4 readiness decisions and ADR 0008 § Gate 4 readiness decisions; the
implementation is Gate 4, with the platform piece via the separate platform-repo
brief):

- **`jurisdiction_time_zone = None` is a first-class publishable value** —
  zone-independent civil-date semantics, **not** `UTC`. The registry must not
  normalize/mutate it (bytes, hash, signature, attestations bind to `None`
  exactly). Publish validation accepts a date-only window with zone `None` *or*
  `Some(..)`, and fails closed for any future datetime-precision window with no
  zone (forward-guard — no datetime window exists in the IR today).
- **`[from, to)` is the authoritative window semantics**; Gate 4 migrates the
  platform `RuleLoader.get_applicable_rules` pre-filter from legacy `[from, to]`
  to `[from, to)` (a platform-repo change; real boundary-date behavior shift —
  needs domain-reviewer awareness). Closed-closed may survive only as a temporary
  platform-loader compatibility mode, never as the artifact contract.
- **The deterministic generator is the long-term Gate-3 solution**, not a
  proptest stopgap (ADR 0008). **Gate 4 signing/keygen tests use deterministic
  test keys / fixed seeds** so CI never depends on OS randomness or the
  `getrandom 0.3` raw-dylib path.

## Phase 5 — acceptance ✅

### Final verification snapshot (2026-05-31)

| Check | Result |
| ----- | ------ |
| `cargo fmt --all -- --check` | clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `cargo test --workspace` | **71 tests across 28 suites, 0 failures** |
| `cargo check -p ke-core --target wasm32` | green |
| `cargo check -p ke-runtime --lib --no-default-features --target wasm32` | green (executor lib is wasm-ready for Gate 5) |
| `differential-test.sh` (Gate 2 regression) | **7/7 files, 0 divergences** @ `f73b940` |
| `equivalence-harness.sh` (Gate 3) | **PASS — 1326 scenarios, 0 divergences** @ `f73b940` |

### Acceptance criteria (spec §19 Gate 3)

- **Trace-fixture parity** — `fixtures/traces/golden.json` (35 Python-oracle
  traces) is reproduced by the Rust runtime (`tests/trace_fixtures.rs`). ✅
- **Generated-scenario equivalence (N≥1000)** — 1326 scenarios over 34 rules:
  outcomes, decisions, obligation id-sets, and normalized traces match Python;
  error classes reduce to input-validation parity (the executor is total,
  ADR 0008). ✅
- **Metamorphic invariants** — outcome-preserving (reorder `all`, irrelevant
  facts) and outcome-flipping (threshold/boolean negation, dropped `all` fact)
  hold (`tests/metamorphic.rs`). ✅
- **Coverage targets** — operator (`compare` truth-table tests + all 8 corpus
  operators), boundary-value (threshold `{below,at,above}`, incl. `<= 0`),
  obligation (matched-leaf obligation sets), date-window (`effective.rs`
  `[from,to)` preview filter, out-of-boundary per ADR 0007),
  jurisdiction/scope (applicability over the corpus), historical-regression
  (committed golden traces). ✅

### Reproducibility

`equivalence-harness.sh` records platform SHA + seed (`7242087531`) + N; override
via `KE_SEED` / `KE_FUZZ`. Generation is a self-contained deterministic PRNG (no
`proptest` — toolchain blocker, Phase 1 note / ADR 0008).

### Out-of-scope (honored)

`ke-runtime` is preview-only (no production use); no flattened Rust IR; no
premise-index; no signing/registry/attestation (Gate 4); no WASM bindings or
frontend (Gate 5); no corpus re-bootstrap; **no LLM/AI code** in any
deterministic path.

Gate 3 is **ready to merge** on `migration/gate-3-preview-runtime`.
