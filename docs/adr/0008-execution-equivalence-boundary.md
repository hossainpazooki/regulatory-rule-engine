# 0008. Execution equivalence boundary and `FactValue` representation (Gate 3)

**Status:** Accepted
**Date:** 2026-05-31
**Spec references:** §19 (Gate 3), §20 (duplicate-runtime drift)
**Gate:** 3

## Context

Gate 3 builds `ke-runtime`, a preview interpreter that must be **observationally
equivalent** to the Python `RuleRuntime` (`src/production/executor.py`). The two
engines have different internal shapes: Rust walks the authoring **tree**
(`RuleIR.{applies_if, decision_tree}`); Python executes a **flattened decision
table** (`CompiledCheck` + `DecisionEntry.condition_mask`) compiled by
`RuleCompiler`. Spec §20 says the equivalence boundary is *observable semantics,
not incidental internal execution order*. This ADR pins exactly what "equivalent"
means and how facts are represented, so the harness compares the right things and
the operators reproduce CPython's behavior.

## Decision

### What is compared (the boundary)

For a given `(rule, facts)`, two results are equivalent iff all of:

1. **`applicable`** (bool) is equal.
2. **`decision`** (`Option<String>`, the matched leaf's `result`) is equal.
3. **Obligation id-set** is equal — obligations compared as the **set of ids**
   from the matched leaf (Python takes obligations only from the matched
   `DecisionEntry`, not rule-level `all_obligations`). Descriptions/deadlines are
   prose and are *not* compared (they are representation, not decision).
4. **Normalized trace** is equal: the ordered **evaluated-applicability prefix**
   (the checks actually evaluated under short-circuit, each as
   `{field, operator, result}`) followed by the **taken decision path** (the
   root-to-leaf branch decisions, each `{field, operator, result}`). Operators are
   normalized to the YAML surface token set (`==`,`!=`,`in`,`not_in`,`>`,`<`,
   `>=`,`<=`,`exists`) — the Python trace stores compiled tokens (`eq`,`gte`,…)
   and is mapped to the same set. Dropped from the boundary: timestamps,
   `node_id`/`entry_id` strings, `description` prose, `expected/actual` values
   (representation-dependent), `facts_used`.

The taken decision path is reconstructed on the **Python** side from the matched
entry's `condition_mask`: each non-zero `mask[i]` (in index order, which is
pre-order along the taken path) yields `{decision_checks[i].field,
normalize(op), result = mask[i] > 0}`. On the **Rust** side it is recorded
directly while descending the tree. The two are asserted byte-identical.

### What cannot error (error classes)

Python `infer` is **total**: operators catch `TypeError`, `facts.get` is total,
and `in` is a string-membership test — no evaluation path raises. Therefore Rust
`evaluate(rule, facts) -> DecisionOutcome` is **infallible** for any well-formed
facts object. "Identical error classes for invalid scenarios" (spec §20) reduces
to **input/loader-validation parity at the harness boundary**: malformed facts
(not a JSON object) and load/compile failures must fail the same way on both
sides. There are no per-evaluation error classes.

### Applicability mirrors the executor's flattening (not the semantic form)

Python `_flatten_conditions` splices a nested `all`/`any` group's members into the
parent list under the **parent** mode and **discards the nested mode**. So
`applies_if: {all: [A, {any: [B, C]}]}` executes as `A AND B AND C`. The Gate-2
`SemanticRule` instead *recurses* (it would model `A AND (B OR C)`). For
*execution* parity the runtime mirrors the **executor**: flatten to a linear
check list + a single mode, then short-circuit (`all` → false on first false;
`any` → true on first true). The empty-check early-return wins: a rule with no
applicability checks (`applies_if = None`, or a group whose members flatten to
nothing) is **applicable** regardless of mode (mirrors `executor.py`'s
`if not ir.applicability_checks: return True`). No corpus rule has a nested
`applies_if` group, so this is not exercised by the corpus harness; it is
unit-tested directly.

### `FactValue` representation

Facts arrive as a JSON object (the scenario). `FactValue` is:

```
enum FactValue { Null, Bool(bool), Int(i128), Float(f64), Str(String), List(Vec<FactValue>) }
```

- **Int vs Float are distinct arms**, decoded from the JSON number literal kind
  (`serde_json::Number::is_i64/is_u64` vs `as_f64`). This is required to
  reproduce Python `str()`: `str(5)=="5"` but `str(5.0)=="5.0"`. Both Rust
  `serde_json` and Python `json.loads` classify the same literal identically, so
  the int/float split is consistent across the boundary.
- **Absent field ≡ `Null`.** `lookup(facts, field)` returns `Null` for a missing
  key (Python `facts.get` returns `None` for both absent and explicit null).
- **Comparison semantics replicate CPython exactly:**
  - `eq`/`ne`: numeric equality across int/float/bool (`True==1`, `1==1.0` true);
    str equals only str; `Null`/`None` equals nothing else; mismatched
    non-numeric types → not equal.
  - `gt/lt/gte/lte`: numeric (bool as 0/1) or str-vs-str (lexicographic) only;
    every other combination (str↔num, `Null`↔anything, list↔scalar) is a
    CPython `TypeError` → **false**.
  - `in`/`not_in`: `python_str(actual)` ∈ `{ python_str(v) for v in operand_list }`;
    operand-not-a-list → `in` is false (`not_in` true). String-membership only;
    never errors.
  - `exists`: `actual` is not `Null`.
- **Decimal operand → f64 reconstruction.** A rule operand is an exact
  `ScalarValue::Decimal{mantissa, scale}`. For numeric comparison it is
  reconstructed to f64 by formatting the **shortest decimal string** and parsing
  it (`"0.9".parse::<f64>()`), which is correctly-rounded in both Rust and Python
  and therefore matches Python's `float("0.9")`. Naive `mantissa as f64 *
  10^-scale` is **not** used (two rounding steps can disagree).

### Generator invariant (bounds the residual divergence)

The scenario generator never feeds a **number** to a field that appears in an
`in`/`not_in` condition (every corpus `in` operand is a string list). This
removes the only realistic `python_str(float)` formatting hazard. Numbers are
generated as short decimal JSON literals so the f64 bits match cross-language.
A property test asserts the invariant holds for every generated scenario.

The generator is a **small self-contained deterministic PRNG**, not `proptest`
(which the spec §22 outline names). `proptest`→`rand`→`getrandom 0.3` cannot
build on this repo's `x86_64-pc-windows-gnu` toolchain (raw-dylib import libs via
a broken self-contained `dlltool`). This is the **accepted long-term Gate 3
solution, not a stopgap** (see § Gate 4 readiness decisions): the acceptance
property is *reproducible coverage + Rust↔Python equivalence*, not a particular
generator crate, and a seeded deterministic PRNG delivers it better — the harness
records seed + N so a run is exactly reproducible. Property/metamorphic coverage
is preserved; only the generation engine differs.

## Consequences

- Desirable: a precise, testable definition of "equivalent" that matches spec §20
  and is robust to the tree-vs-table internal difference.
- Desirable: operator semantics are pinned to a CPython truth table (unit-tested),
  including the `bool`-is-`int` and `TypeError→false` edges that are easy to miss.
- Desirable: the float/decimal hazard is contained by representation choice +
  generator invariant rather than wished away.
- Undesirable: the runtime intentionally diverges from the Gate-2 semantic form on
  nested applicability groups (it must, to match the executor). Documented and
  unit-tested; not corpus-exercised.

## Alternatives considered

- **Coerce facts to exact decimal** (like the IR operand) — rejected: more
  faithful to the IR but *less* faithful to Python, which compares IEEE-754
  floats; it would diverge from Python on non-float-representable decimals, the
  opposite of the goal.
- **Compare full traces verbatim** — rejected: Python traces every decision check
  (not just the taken path) plus a synthetic entry-match step, and carries
  timestamps/ids/prose. Verbatim comparison would fail on incidental internal
  order, contradicting spec §20.
- **Reuse `SemanticRule` as the runtime model** — rejected: it recurses nested
  applicability groups, but the Python executor flattens them; reusing it would
  make the runtime disagree with the parity target.

## Gate 4 readiness decisions

Made now; relevant to Gate 4 (artifact / signing / keygen).

1. **The deterministic generator is the long-term solution, not a stopgap.**
   `proptest` is not re-adopted. The acceptance property for scenario coverage is
   reproducible coverage + Rust↔Python equivalence, independent of the generator
   crate; the seeded PRNG satisfies it on every platform, including
   `x86_64-pc-windows-gnu`.

2. **Gate 4 signing/keygen tests use deterministic keys / fixed seeded material.**
   `ke-artifact`'s ed25519 path must not make CI depend on OS randomness or the
   `getrandom 0.3` raw-dylib path that breaks on this toolchain. ed25519 *signing*
   is already deterministic (RFC 8032); test keys come from fixed bytes
   (`SigningKey::from_bytes`) or derived seeds, and golden artifacts are signed
   with a fixed test key so hashes/signatures stay reproducible. Production key
   generation may still use secure randomness — behind the platform's key-authority
   boundary (spec §10, §21.1) and outside the deterministic test path.
