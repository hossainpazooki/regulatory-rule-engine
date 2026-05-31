# 0007. Effective windows in the preview runtime (tz optional; `[from,to)` is preview-only)

**Status:** Accepted
**Date:** 2026-05-31
**Spec references:** §8.4 (effective dates and jurisdiction time), §19 (Gate 3), §20
**Amends:** Gate 1 IR (ADR 0001), ADR 0006 (effective-window optional)
**Gate:** 3

## Context

Two effective-window loose ends were carried into Gate 3:

1. **The `UTC` placeholder.** Gate 2 lowering, finding the corpus YAML is
   date-only (no authored time zone), synthesized
   `jurisdiction_time_zone = "UTC"` for any date-bearing rule (ADR 0006 §Decision;
   `ke-compiler/src/lower.rs`, `python_import.rs`). The gate-2 log flagged this as
   a Gate-3 prerequisite: "before any artifact is published (Gate 4), the
   placeholder must be resolved — derive the zone from jurisdiction, **or** make
   `jurisdiction_time_zone` optional." ADR 0001 had assumed every window carries a
   meaningful zone for date-window resolution; the corpus reality is that none is
   authored.

2. **Where date-window evaluation lives.** ADR 0001 stated "Gate 3 (`ke-runtime`)
   implements the actual date-window resolution." But Gate 3's runtime is built to
   be **observationally equivalent to the Python `RuleRuntime`**, and that runtime
   (`src/production/executor.py`) is **entirely date-agnostic** — it never reads
   `effective_from`/`effective_to`. Date filtering in the platform is a *separate*
   pre-filter, `RuleLoader.get_applicable_rules` (`src/rules/service.py`), which
   uses a **closed-closed** `[from, to]` interval (`effective_to < today`
   excludes, so `== today` is included). Spec §8.4 mandates a **closed-open**
   `[from, to)` interval. These disagree.

So Gate 3 must (a) stop emitting an invented zone into canonical bytes, and
(b) decide whether/where to evaluate date windows without polluting the
Rust↔Python equivalence boundary.

## Decision

**(1) `EffectiveWindow.jurisdiction_time_zone` becomes optional:
`Option<TimeZone>`.** `None` means the rule authored no zone (the corpus case);
date-only rules then carry **no invented zone** in canonical bytes. Lowering
(`lower.rs`) and the Python import adapter (`python_import.rs`) emit `None` instead
of the `UTC`/`2025a` placeholder. This refines ADR 0001 (the zone is *present when
authored*, not *always present*) and continues the ADR 0006 precedent (faithful
representation over synthesized data). It re-touches the Gate-1-frozen IR, so the
pinned version triplet bumps:

- `ir_schema_version`: `0.2.0` → `0.3.0` (a nested field becomes nullable, dropped
  from its object's `required`).
- `canonicalization_version`: `ke-canon-2` → `ke-canon-3` (the canonical byte
  layout changes — the zone now carries an `Option` presence byte).
- `codec_version`: unchanged (`postcard-1`).

Gate 1's committed JSON Schema and golden fixtures are regenerated under the new
triplet (atomically, via their generators — never hand-edited). The semantic
normal form already drops the zone (`ke-core::semantic::form`), so the Gate 2
differential is unaffected; it is re-run at the recorded SHA to prove no
regression.

**(2) Date-window evaluation is preview-only and outside the equivalence
boundary.** `ke-runtime` provides a standalone `effective_at(window, date)`
applicability filter implementing spec §8.4's **closed-open `[from, to)`**. It is
**not** part of the `RuleRuntime`-equivalent decision path (which stays
date-agnostic) and is **not** compared against Python in `equivalence-harness.sh`.
It is unit-tested against spec-defined boundary dates. This satisfies the Gate 3
"date-window" coverage target as a preview capability while keeping the
observable-semantics equivalence with Python clean.

**(3) The interval divergence is a deliberate, recorded correction.** The
authoritative migration semantics are the spec's `[from, to)`. The platform's
`get_applicable_rules` `[from, to]` is a legacy pre-filter that the migration
replaces; it is out of the equivalence boundary. The full jurisdiction→zone
resolution and any tz database (ADR 0001's deferred item) are **not** needed for
the date-agnostic runtime; their publish-time policy is **decided** below (see
§ Gate 4 readiness decisions). In short: `jurisdiction_time_zone = None` is a
first-class publishable value (zone-independent civil-date semantics) — `UTC` is
never silently re-introduced.

## Consequences

- Desirable: no invented time zone enters canonical bytes / the future content
  hash; date-only rules are represented faithfully (Rust `None` ≡ Python `None`).
- Desirable: the Rust↔Python equivalence boundary stays exactly the observable
  semantics of `RuleRuntime.infer` — no date logic muddies it.
- Desirable: spec §8.4's `[from, to)` is implemented and tested now, as preview.
- Undesirable: a third version-triplet bump on the Gate-1-frozen shape. Mitigated
  as in ADR 0006 — any consumer on `0.2.0`/`ke-canon-2` rejects rather than
  silently misreads, and all golden vectors regenerate atomically.
- Decided for Gate 4 (see § Gate 4 readiness decisions): `None` is a first-class
  publishable value (never normalized to `UTC`); publish validation accepts a
  date-only window with or without a zone; the platform pre-filter migrates to the
  spec's `[from, to)`.

## Alternatives considered

- **Derive the zone from jurisdiction** (e.g. `EU → Europe/Brussels`) — rejected:
  it invents a real zone for a date-only rule (replacing one synthesized value
  with another), needs a jurisdiction→IANA map + a widened tz allow-list, and is
  unnecessary because the runtime is date-agnostic. ADR 0006's "no synthesized
  source data" reasoning applies.
- **Defer the placeholder entirely to Gate 4** — acceptable for equivalence (the
  zone is outside the boundary) but leaves invented `UTC` in canonical bytes,
  which the gate-2 log says must be resolved, and which would bake into the Gate 4
  content hash. Doing the small optional-field amendment now is the clean moment.
- **Implement date-window evaluation inside the equivalence-bound runtime** —
  rejected: Python's `RuleRuntime` has no date logic to compare against, so this
  would have no oracle and could only diverge.
- **Match the platform's `[from, to]` for bug-for-bug parity** — rejected: the
  pre-filter is legacy and out of the boundary; the spec's `[from, to)` is
  authoritative. Recorded here so the choice is reviewable, not silent.

## Gate 4 readiness decisions

These resolve the items left open above. The **decisions** are made now; their
**implementation** is Gate 4 (artifact / registry / attestation), and the
platform-side piece lands via the separate platform-repo brief (spec §14, §22).

1. **`jurisdiction_time_zone = None` is a first-class publishable value.** It
   denotes **zone-independent civil-date semantics** for a date-only effective
   window — explicitly *not* an implicit `UTC`. The registry must **not**
   normalize, default, or otherwise mutate it: the artifact bytes, BLAKE3 hash,
   compiler signature, and expert attestations bind to `None` exactly. Mutating
   it would break the hash/signature/attestation chain (spec §8, §9, §10).

2. **Publish-time effective-window validation.** Publication accepts:
   - a date-only effective window with `jurisdiction_time_zone = None`, and
   - a date-only effective window with `jurisdiction_time_zone = Some(..)`.

   It **fails closed** for any future *instant/datetime-precision* effective
   window that carries no zone. Note this is a forward-guard: the current IR has
   only date-precision windows (`JurisdictionDate`), so the reject branch is
   unreachable today — it takes effect only if a datetime-precision window
   variant is later introduced, at which point a zone becomes mandatory for it.

3. **`[from, to)` is the authoritative artifact/window semantics; the platform
   pre-filter migrates to match.** Gate 4 migrates the platform
   `RuleLoader.get_applicable_rules` pre-filter from the legacy closed-closed
   `[from, to]` to closed-open `[from, to)` (a platform-repo change). This is a
   real production behavior change — a rule effective exactly on its
   `effective_to` date flips from applicable → not-applicable — so it needs
   domain-reviewer awareness on boundary-date semantics. If transitional
   compatibility is required, closed-closed may be retained only as a temporary
   **platform-loader compatibility mode**, never as the artifact contract. The
   Rust preview filter (`ke_runtime::effective::effective_at`) already implements
   the authoritative `[from, to)`.
