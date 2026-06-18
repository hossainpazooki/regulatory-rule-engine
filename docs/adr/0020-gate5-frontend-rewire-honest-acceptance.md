# 0020. Gate-5: defer the ATLAS frontend rewire — COMPASS is the consumer

**Status:** Accepted (sign-off by Hossain, 2026-06-18)
**Date:** 2026-06-17 (accepted 2026-06-18)
**Spec references:** § 19 (Gate 5 acceptance, G5-5), § 7.4 (frontend feature flags), § 13 (review-first UI), § 16 (multi-surface access), § 6 (WASM/serve discipline)
**Relates to:** ADR-0017 (platform-api decoupled; COMPASS is the consumer), ADR-0018 (`ke serve` surface = `/healthz,/resolve,/verify,/compile/preview,/dry-run,/events`, non-authoritative), ADR-0019 (re-derive trust; non-`published` blocked; fail-closed on `unknown`)
**Amends:** the spec § 19 Gate-5 acceptance criterion G5-5 ("when every previously-working page is loaded post-rewire, then it functions against local Rust surfaces"), which predated the ADR-0017 decoupling.

## Context

G5-5 as written assumes the ATLAS frontend's pages should be rewired off
`VITE_API_URL` onto **local Rust surfaces** (`ke serve` REST), page by page. After
the ADR-0017 decoupling, that rewire is **low-value**, and for most pages **not
achievable by design**:

- The **real consumer of the ATLAS artifact path is COMPASS**
  (`cross-border-compliance-navigator`). COMPASS verifies hash, signatures,
  registry state, and typed expert attestations **in-browser** via the
  `@platform/atlas-artifact` WASM verifier (`ke-wasm`). It is **consumer-only**: it
  does **not** sign, publish, or execute the rule engine. The COMPASS integration is
  gated **post-Gate-5** (ADR-0017, ADR-0019).
- **ATLAS's own React frontend is producer-side authoring/review tooling, not the
  consumer.** Rewiring those pages to talk to a local `ke serve` does not advance the
  producer→consumer story — the consumer is a separate repo (COMPASS), and the
  binding it consumes is the WASM verifier, not the ATLAS frontend's data fetches.
- `ke-cli serve` (ADR-0018) deliberately exposes only the **artifact/rule-engine**
  surface: `/healthz, /resolve, /verify, /compile/preview, /dry-run, /events`. Of the
  ATLAS frontend's 9 pages, **8 are off the artifact path** — analytics, embeddings,
  similarity search, graph, cross-border navigator, document ingestion, home
  rule-list, plus the off-path data on others draw **ML / analytics / jurisdiction /
  credit** data. There is no local endpoint to call, and adding one would re-import
  exactly the coupling ADR-0017 removed.

Adversarial verification of the 5d/5e build surfaced this directly: the workflow
**over-built** and marketed KEWorkbench and the review UI as "rewired", but only
ProductionDemo genuinely reached a local surface, and KEWorkbench's
compile-preview/dry-run affordance and the 5e `ReviewSurface` were **built but
mounted on no page**. The honest reading is that the ATLAS frontend does not need
the local surfaces today — the consumer that does is COMPASS, and it consumes the
WASM verifier, not these pages. So the rewire is **deferred, not delivered**.

This mirrors how ADR-0017 redefined Gate-4 C1/C2 (`docs/gate-4-acceptance.md`):
keep the in-repo deliverable honest, and mark the part the decoupling made
inapplicable as deferred-by-design, not as delivered.

## Decision

> **Defer the ATLAS frontend rewire.** The 5d page-by-page move of the ATLAS
> frontend from `VITE_API_URL` to `ke serve` REST is **DEFERRED**, not delivered.
> G5-5 → **DEFERRED**. Revisit only if/when the ATLAS frontend genuinely needs the
> local surfaces.

**Why low-value:** COMPASS is the main / real consumer of the ATLAS artifact path
and verifies in-browser via the `@platform/atlas-artifact` WASM verifier; ATLAS's own
frontend is producer-side authoring/review tooling, not the consumer. Most ATLAS
pages are off the artifact path (ML / analytics / jurisdiction / credit) and cannot
be rewired at all.

**What stays — engine surfaces, with independent value (NOT deferred):**

- **5a `ke serve`** — the non-authoritative HTTP/SSE surface (ADR-0018).
- **5b-preview WASM bindings** — the `ke-wasm` binding **COMPASS consumes**.
- **5b-data `.kew` export/import** — closes **G5-4 (MET)**.
- **5c lint** — the linting surface.

**What stays inert (kept in-tree, NOT claimed as a delivered rewire):** the
KEWorkbench in-browser WASM compile/dry-run **preview pane** (`LocalKePreviewPane`)
and the **5e review components** (`ReviewSurface`) remain in the tree behind
**default-off** `VITE_USE_*` flags as **inert optional affordances**. No code is
deleted (Option 1). With every `VITE_USE_*` flag default-off, `main` is byte-unchanged.

**G5-1** is closed by **5a (`ke serve`) + 5b-data**, per ADR-0018 (every response
reads the canonical on-disk registry/artifact view). The 5d frontend panes do **not**
close G5-1 — drop any claim that they do.

## Consequences

- **G5-5 is DEFERRED, honestly:** the ATLAS frontend rewire is not delivered;
  COMPASS is the consumer and its integration is the post-Gate-5 work that actually
  exercises the artifact path in a browser. The deferral does not lower the bar — it
  records that the rewire was the wrong target post-ADR-0017.
- **The frontend stays decoupled** from the ML/analytics backends — consistent with
  ADR-0017 — instead of growing serve endpoints that re-couple them.
- **The inert scaffolding costs nothing on `main`** (flags default-off) and is
  available if the ATLAS frontend later needs a genuine local affordance, without
  re-opening this decision.
- **The over-build is on the record, not erased.** The 5d/5e workflow built more than
  the artifact path warranted; this ADR defers it rather than pretending it was never
  built or dressing the inert scaffolding as delivered.
- The Playwright visual harness remains experimental / non-gating; its baselines are
  Linux-CI-canonical.

## Alternatives considered

- **Redefine G5-5 as "met" against the artifact-path pages (the prior draft of this
  ADR).** Rejected: it dresses inert, default-off scaffolding (an unmounted-then-mounted
  preview pane and review surface) as a delivered rewire. The honest status is
  *deferred*, because the consumer that needs the local/WASM surface is COMPASS, not
  these pages.
- **Extend `ke-cli serve` with the off-path endpoints (rules-list, analytics, …).**
  Rejected: it re-imports the coupling ADR-0017 removed, and the ML/analytics data is
  not on the artifact path at all — serve would become a proxy to the very backend the
  decoupling separated (re-couples).
- **Pare back the inert scaffolding (delete `LocalKePreviewPane` / `ReviewSurface`).**
  Optional later. Option 1 keeps them in-tree behind default-off flags so a future
  genuine need can adopt them without rebuilding; removal can be a follow-up if the
  scaffolding proves to be dead weight.
