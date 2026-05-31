#!/usr/bin/env python3
"""Batched Python reference runtime for the Gate 3 equivalence harness.

Reads scenario JSONL on stdin (produced by `gen-scenarios`), each line:

    {"rule_id","label","facts":{...},"rust":{normalized}}

Loads + compiles every corpus rule ONCE (so per-scenario cost is just
`RuleRuntime.infer`), recomputes the SAME normalized form on the Python side,
and compares it to the embedded `rust` object. Exits non-zero if any scenario
diverges. This is the runtime half of spec §20's duplicate-runtime-drift
mitigation; the boundary is pinned in ADR 0008.

The platform code is the parity target — run with the platform repo on
PYTHONPATH (the harness sets this). Reuses the platform's RuleLoader,
RuleCompiler, and RuleRuntime exactly as production would.

Usage (driven by scripts/equivalence-harness.sh):
    python py_reference_runtime.py <corpus_dir> [--trace-rules id1,id2,...]
                                                [--emit-traces <out.json>]
                                   < scenarios.jsonl
"""

from __future__ import annotations

import glob
import json
import os
import sys

from src.production.compiler import RuleCompiler
from src.production.executor import RuleRuntime
from src.rules.service import RuleLoader

# Compiled-op token -> canonical YAML token (the set ke-runtime's op_token emits).
OP = {
    "eq": "==",
    "ne": "!=",
    "gt": ">",
    "lt": "<",
    "gte": ">=",
    "lte": "<=",
    "in": "in",
    "not_in": "not_in",
    "exists": "exists",
}


def normalize(ir, result) -> dict:
    """Reduce a Python DecisionResult to the normalized comparison form."""
    appl_steps = []
    decision_path = []
    if result.trace is not None:
        for s in result.trace.applicability_steps:
            appl_steps.append(
                {"field": s.field, "operator": OP.get(s.operator, s.operator), "result": bool(s.result)}
            )
        # Reconstruct the TAKEN decision path from the matched entry's mask.
        if result.applicable and result.trace.entry_matched is not None:
            entry = next(
                (e for e in ir.decision_table if e.entry_id == result.trace.entry_matched),
                None,
            )
            if entry is not None:
                for i, m in enumerate(entry.condition_mask):
                    if m == 0:
                        continue
                    chk = ir.decision_checks[i]
                    decision_path.append(
                        {"field": chk.field, "operator": OP.get(chk.op, chk.op), "result": m > 0}
                    )
    obligations = sorted({o["id"] for o in result.obligations})
    return {
        "applicable": bool(result.applicable),
        "decision": result.decision,
        "obligations": obligations,
        "applicability_steps": appl_steps,
        "decision_path": decision_path,
    }


def load_corpus(corpus_dir: str) -> dict:
    """rule_id -> compiled RuleIR, over every YAML in the corpus (skip schema)."""
    loader = RuleLoader()
    compiler = RuleCompiler()
    by_id = {}
    for path in sorted(glob.glob(os.path.join(corpus_dir, "*.yaml"))):
        if os.path.basename(path) == "schema.yaml":
            continue
        for rule in loader.load_file(path):
            by_id[rule.rule_id] = compiler.compile(rule)
    return by_id


def main() -> int:
    args = sys.argv[1:]
    if not args:
        print("usage: py_reference_runtime.py <corpus_dir> [--trace-rules ...] [--emit-traces f]", file=sys.stderr)
        return 64

    corpus_dir = args[0]
    trace_rules: set[str] = set()
    emit_traces: str | None = None
    i = 1
    while i < len(args):
        if args[i] == "--trace-rules":
            trace_rules = set(filter(None, args[i + 1].split(",")))
            i += 2
        elif args[i] == "--emit-traces":
            emit_traces = args[i + 1]
            i += 2
        else:
            print(f"unknown arg {args[i]}", file=sys.stderr)
            return 64

    irs = load_corpus(corpus_dir)
    runtime = RuleRuntime()

    total = 0
    diverged = 0
    rules_seen: set[str] = set()
    traces: list[dict] = []
    first_failures: list[str] = []

    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        obj = json.loads(line)
        rid = obj["rule_id"]
        label = obj.get("label", "")
        facts = obj["facts"]
        rust = obj["rust"]

        ir = irs.get(rid)
        if ir is None:
            print(f"FATAL: rule_id {rid!r} not found in corpus {corpus_dir}", file=sys.stderr)
            return 1

        result = runtime.infer(ir, facts, include_trace=True)
        py = normalize(ir, result)
        total += 1
        rules_seen.add(rid)

        diffs = {k: {"rust": rust.get(k), "python": py.get(k)} for k in py if rust.get(k) != py.get(k)}
        if diffs:
            diverged += 1
            if len(first_failures) < 20:
                first_failures.append(
                    f"  DIVERGE {rid} [{label}] facts={json.dumps(facts)}\n"
                    f"          {json.dumps(diffs)}"
                )

        if emit_traces and rid in trace_rules and not label.startswith("fuzz"):
            traces.append({"rule_id": rid, "label": label, "facts": facts, "normalized": py})

    if emit_traces:
        # Only (re)write the golden traces when the run is clean, so a committed
        # fixture is always a verified Rust≡Python agreement.
        if diverged == 0:
            traces.sort(key=lambda t: (t["rule_id"], t["label"]))
            with open(emit_traces, "w", encoding="utf-8") as f:
                json.dump(traces, f, indent=2, sort_keys=True)
                f.write("\n")
            print(f"emitted {len(traces)} trace fixtures -> {emit_traces}", file=sys.stderr)
        else:
            print("NOT emitting trace fixtures: run diverged", file=sys.stderr)

    print("----", file=sys.stderr)
    print(f"scenarios checked: {total} over {len(rules_seen)} rules", file=sys.stderr)
    if diverged:
        print(f"DIVERGENCES: {diverged}", file=sys.stderr)
        print("\n".join(first_failures), file=sys.stderr)
        return 1
    print("PASS: Rust ≡ Python over all scenarios", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
