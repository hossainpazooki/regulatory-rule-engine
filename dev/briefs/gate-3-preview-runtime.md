# Gate 3 — Preview runtime + equivalence harness (brief)

**Status:** complete on `migration/gate-3-preview-runtime` (off accepted Gate 2) —
ready for Hossain review/merge. Live Rust↔Python equivalence PASS over 1326
scenarios; Gate 2 differential still 7/7 after the ADR-0007 IR amendment. See
[`docs/gate-3-implementation-log.md`](../../docs/gate-3-implementation-log.md).
**Authoritative spec sections:** §6 (crate layout — `ke-runtime`), §17 (authoring/
review workflow), §19 (Gate 3 acceptance), §20 (duplicate-runtime drift — the
equivalence boundary), §22 (Gate 3 outline).
**Predecessor:** Gate 2 (parser/compiler/T0-T1-T4) — accepted 2026-05-30.
**Successor:** Gate 4 (artifact, registry, attestation).

This brief is the contract for the Gate 3 implementation. It mirrors the Gate 1/2
brief structure and the mandatory §22 brief sections.

---

## 1. Context

Gate 1 froze `ke-core::ir::RuleIR` (the **un-lowered authoring tree**). Gate 2 made
it producible from YAML (`ke-compiler`) and proved Rust↔Python parity at a
**semantic-normal-form** level. Gate 3 makes the IR *executable in preview*:

- a Rust interpreter that walks the tree IR against a fact set,
- scenario scaffolding + property/metamorphic tests (deterministic generator —
  see decision 7),
- a fuzzed Rust↔Python equivalence harness over the corpus.

The parity target (Python, in `institutional-defi-platform-api`):
`src/production/executor.py` (`RuleRuntime.infer` — applicability + decision-table
lookup), `src/production/trace.py` (`ExecutionTrace`/`DecisionResult`),
`src/production/compiler.py` (`RuleCompiler` — the tree→jump-table flattening the
runtime executes), `src/rules/service.py` (`RuleLoader` YAML→`Rule`).

**`ke-runtime` is preview-only.** Production execution stays in the Python
`RuleRuntime`. The Rust runtime exists for fast in-browser/CLI dry-run, scenario
tracing, and as the differential oracle that keeps the two engines from drifting
(spec §2 non-goals, §20).

### The two-level IR reality (load-bearing)

The Rust runtime walks the **tree** (`RuleIR.decision_tree`, `applies_if`). The
Python runtime executes a **flattened decision table** (`CompiledCheck` +
`DecisionEntry.condition_mask`) that `RuleCompiler` derives from the same tree. A
tree-walk is observationally equivalent to the flattened-table lookup:
`RuleCompiler` assigns a **unique check index per node** (no dedup), `_walk_tree`
emits table entries in pre-order (true-branch first), and `_matches_mask` makes
**only the taken path's constraints decisive** (untaken-branch checks are
wildcards in the matched entry's mask). So the first matching entry is exactly the
leaf the tree-walk reaches. Gate 3 proves this by execution, not by inspection.

## 2. Locked decisions

1. **Walk the tree, not a flattened IR.** Gate 2 deliberately did not build a
   flattened `CompiledRuleIR`; Gate 3 does not either. The runtime interprets
   `RuleIR.{applies_if, decision_tree}` directly.
2. **Equivalence boundary = observable semantics (spec §20).** Identical final
   outcomes, identical obligation **id-sets**, identical rule/branch decisions
   after trace normalization, identical error classes. Internal step order and
   representation (float vs decimal, node ids, prose) are *outside* the boundary.
   See ADR 0008.
3. **Mirror the executor's flattening, not the Gate-2 semantic form.** Python's
   `_flatten_conditions` splices nested `all`/`any` group members under the
   **parent** mode and **discards the nested mode**. The Gate-2 `SemanticRule`
   *recurses* (preserves nested mode). For *execution* parity the runtime must
   follow the **executor** (flatten). No corpus rule has a nested `applies_if`
   group, so this is not exercised by the corpus harness, but it is implemented
   faithfully and unit-tested.
4. **The executor is total.** Python `infer` never raises (operators catch
   `TypeError`; `facts.get` is total; `in` is a string-membership test). Rust
   `evaluate` is therefore infallible for any well-formed facts object; "error
   class" parity is input/loader-validation parity at the harness boundary, not
   per-evaluation errors.
5. **`ke-runtime` is date-agnostic in the equivalence path.** Python's
   `RuleRuntime` never evaluates `effective_from`/`effective_to` (date filtering
   is a separate `RuleLoader.get_applicable_rules` pre-filter). The runtime's
   decision path therefore ignores effective windows. A separate, **preview-only**
   `effective_at(date)` filter (spec §8.4 `[from,to)`) is provided for the
   date-window coverage target and is explicitly **out** of the Rust↔Python
   boundary. See ADR 0007.
6. **Equivalence pin = recorded SOURCE.md SHA** (`f73b9403…6ba7`), same as Gate 2.
   `equivalence-harness.sh` fails fast unless the platform checkout is at exactly
   that SHA with `src/rules/data` clean. No corpus re-bootstrap in this gate.
7. **Deterministic generator, not `proptest`.** The spec §22 outline names
   `proptest`, but it cannot build on this repo's `x86_64-pc-windows-gnu`
   toolchain (`proptest`→`rand`→`getrandom 0.3` needs raw-dylib import libs and
   the self-contained `dlltool` is broken — `CreateProcess` failure). The
   scenario generator and property/metamorphic tests use a small self-contained
   deterministic PRNG instead — no external crate, no raw-dylib — which also
   gives the harness reproducible, recorded seeds. Coverage is preserved; only
   the generation engine differs. See ADR 0008.

## 3. Phase 1 deliverables (files)

Executor core + the two prerequisite docs (Phase 0 lands first):

- Phase 0: this brief, `docs/adr/0007-effective-window-preview-runtime.md`,
  `docs/adr/0008-execution-equivalence-boundary.md`, `docs/adr/README.md` index,
  `docs/gate-3-implementation-log.md`.
- `crates/ke-runtime/src/value.rs` — `FactValue` (`Null|Bool|Int(i128)|
  Float(f64)|Str|List`); `serde_json::Value → FactValue` preserving int-vs-float;
  `python_str` (Python `str()` faithful); `lookup(facts, field)` collapsing
  absent → `Null`.
- `crates/ke-runtime/src/compare.rs` — the nine operators bit-faithful to
  `executor.py`: `eq/ne` (numeric across int/float/bool, never str↔num, `None`
  equals nothing), `gt/lt/gte/lte` (numeric/str-str only, else `TypeError→false`),
  `in/not_in` (`python_str(actual)` in the `python_str`-coerced operand set;
  operand-not-a-list → `in=false`), `exists` (`actual is not None`).
- `crates/ke-runtime/src/exec.rs` — `flatten_conditions` (mirrors
  `_flatten_conditions`), applicability (`all`/`any`, short-circuit, empty-group
  early-return), tree-walk to a leaf, `DecisionOutcome` (`applicable`,
  `decision: Option<String>`, obligations from the matched leaf only).
- `crates/ke-runtime/src/lib.rs` — module wiring; `#![deny(unsafe_code)]`.
- `crates/ke-runtime/Cargo.toml` — add `ke-compiler` (dev), `proptest`, `anyhow`
  (bins). `Cargo.toml` workspace — add `proptest = "1"`.

## 4. Phase 2–4 deliverables (files)

- `crates/ke-runtime/src/trace.rs` — `NormalizedTrace` (evaluated-applicability
  prefix + taken decision path; obligation id-set) + operator-token normalizer.
- `crates/ke-runtime/src/scenario.rs` — `Scenario`, rule field-catalog
  extraction, a self-contained deterministic PRNG + generation strategies,
  coverage classes (operator / boundary / obligation / missing-field /
  wrong-type / irrelevant-facts / date-window).
- `crates/ke-runtime/src/bin/ke-eval.rs` — dev: evaluate one `(rule, facts)` →
  normalized JSON.
- `crates/ke-runtime/src/bin/gen-scenarios.rs` — deterministic JSONL scenario
  emitter (fixed seed, N configurable) carrying the Rust normalized result.
- `crates/ke-runtime/tests/{property,metamorphic,trace_fixtures}.rs`.
- `scripts/equivalence-harness.sh` — SHA-gated, batched Rust↔Python parity.
- `scripts/py_reference_runtime.py` — batched Python driver (`RuleLoader` +
  `RuleCompiler` + `RuleRuntime.infer`, normalized to the same form).
- `fixtures/traces/*.json` — harness-generated golden normalized traces.
- Phase 4 (IR amendment, isolated): `crates/ke-core/src/ir/time.rs`,
  `canonical/{encode,decode}.rs`, `schema/defs.rs`, `version.rs`,
  `ke-compiler/src/{lower,python_import}.rs`; regenerated `ir.schema.json` +
  `fixtures/artifacts/`; `docs/canonical-encoding.md` version table.

## 5. Phase verification commands

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p ke-runtime
cargo test --workspace
cargo check -p ke-core --target wasm32-unknown-unknown   # ke-core stays wasm-clean
# with the platform checked out at the recorded SOURCE.md SHA:
bash scripts/equivalence-harness.sh        # N=1000, Rust ≡ Python
bash scripts/differential-test.sh          # re-run after the Phase-4 IR amendment
```

## 6. Acceptance criteria (spec §19 Gate 3)

- Given existing trace fixtures (`fixtures/traces/`), when executed by the Rust
  preview runtime, then normalized public trace events match Python output.
- Given generated scenarios (N=1000), when evaluated by both runtimes, then
  outputs, obligation sets, error classes, and normalized traces are equivalent.
- Given metamorphic transformations that should preserve semantics, then outputs
  remain stable; transformations that should flip an outcome flip it as predicted.
- Coverage targets exercised: operator, boundary-value, obligation, date-window,
  jurisdiction/scope, historical regression.

## 7. Known risks (spec §20)

- **Duplicate runtime drift** — the gate's whole purpose. Mitigated by the
  property/metamorphic tests + the SHA-pinned fuzzed equivalence harness over the
  full corpus.
- **Decimal/float boundary.** `FactValue` keeps `Int`/`Float` distinct (so Python
  `str()` is reproducible) and reconstructs decimal operands to f64 via
  shortest-decimal-string→parse (matches Python's correctly-rounded `float()`).
  Residual risk confined by a generator invariant: numbers never feed
  `in`/`not_in` (every corpus `in` operand is a string list). See ADR 0008.
- **Applicability flattening vs semantic recursion** — the runtime mirrors the
  executor's flattening (decision 3); a divergence here would be a silent
  wrong-applicability. Unit-tested directly; not exercised by the corpus.
- **`str()` coercion** — `str(True)=="True"`, `str(5.0)=="5.0"`; the `python_str`
  helper has a dedicated golden unit table.

## 8. Out-of-scope clarifications (must NOT do)

Production use of `ke-runtime`; a flattened/jump-table Rust IR; premise-index
acceleration; the other T4 conflict classes; signing/registry/attestation
(Gate 4); WASM/frontend (Gate 5); corpus re-bootstrap; embedding a full IANA tz
database (ADR 0007 makes the zone optional instead); **any LLM/AI code** — the LLM
is out-of-band authoring assistance only, never in `ke-runtime` or any
deterministic path.

## 9. Commit boundary

Hossain commits/merges manually on `migration/gate-3-preview-runtime` after
review. Claude Code makes no commits or pushes. `fixtures/` is never hand-edited;
`fixtures/traces/` and `fixtures/artifacts/` are regenerated by their scripts.

## 10. Platform access (spec §4.5)

`equivalence-harness.sh` resolves `${PLATFORM_REPO:-../institutional-defi-platform-api}`,
requires HEAD == the SHA recorded in `fixtures/rules/SOURCE.md`, records that SHA
(plus the generator seed and N) in its output, and fails fast if the checkout is
missing, dirty under `src/rules/data`, or at any other commit. The batched Python
driver runs with the platform on `PYTHONPATH`; native `python.exe` paths are
translated with `cygpath` (mirrors `differential-test.sh`).

## 11. Gate 4 readiness decisions (resolved, not deferred)

The forward-looking items this gate surfaced are **decided** — decisions in
ADR 0007 § *Gate 4 readiness decisions* and ADR 0008 § *Gate 4 readiness
decisions*; implementation is Gate 4 (artifact/registry/attestation), with the
platform piece via the separate platform-repo brief (spec §14):

- `jurisdiction_time_zone = None` is a **first-class publishable value**
  (zone-independent civil-date semantics, never `UTC`); the registry must not
  normalize/mutate it. Publish validation accepts a date-only window with zone
  `None` or `Some(..)`, and fails closed for a future datetime-precision window
  with no zone (forward-guard).
- `[from, to)` is the **authoritative** window semantics; Gate 4 migrates the
  platform `get_applicable_rules` pre-filter from `[from, to]` to `[from, to)`
  (real boundary-date behavior change; compatibility only as a temporary
  platform-loader mode).
- The deterministic generator is the **accepted long-term** approach (not a
  proptest stopgap); Gate 4 signing/keygen tests use deterministic keys / fixed
  seeds (no OS-randomness / `getrandom 0.3` dependence in CI).
