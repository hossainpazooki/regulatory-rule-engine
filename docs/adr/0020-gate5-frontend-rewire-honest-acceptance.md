# 0020. Gate-5 frontend rewire: G5-5 redefined for the platform decoupling (artifact-path pages rewire locally; ML/analytics stay external)

**Status:** Proposed (pending sign-off by Hossain)
**Date:** 2026-06-16
**Spec references:** § 19 (Gate 5 acceptance, G5-5), § 7.4 (frontend feature flags), § 13 (review-first UI), § 16 (multi-surface access), § 6 (WASM/serve discipline)
**Relates to:** ADR-0017 (platform-api decoupled; COMPASS is the consumer), ADR-0018 (`ke serve` surface = `/healthz,/resolve,/verify,/compile/preview,/dry-run,/events`, non-authoritative), ADR-0019 (re-derive trust; non-`published` blocked; fail-closed on `unknown`)
**Amends:** the spec § 19 Gate-5 acceptance criterion G5-5 ("when every previously-working page is loaded post-rewire, then it functions against local Rust surfaces"), which predated the ADR-0017 decoupling.

## Context

G5-5 as written assumes every frontend page can be served by a *local Rust
surface*. After the ADR-0017 decoupling, that is **not achievable, by design**, for
most pages:

- `ke-cli serve` (ADR-0018) deliberately exposes only the **artifact/rule-engine**
  surface: `/healthz, /resolve, /verify, /compile/preview, /dry-run, /events`.
- The ATLAS frontend has 9 pages. Only **KEWorkbench** (compile/dry-run/verify) and
  **ProductionDemo** (health) consume the artifact/rule-engine surface. The other
  seven — analytics, embeddings, similarity search, graph, cross-border navigator,
  document ingestion, home rule-list — consume **ML / analytics / jurisdiction /
  credit** data that is *off the ATLAS artifact path*. There is no local endpoint to
  call, and adding one would re-import exactly the coupling ADR-0017 removed.

Adversarial verification of the 5d/5e build surfaced this directly: the workflow
marketed KEWorkbench and the review UI as "rewired", but only ProductionDemo
genuinely reached a local surface, and KEWorkbench's compile-preview/dry-run
affordance and the 5e `ReviewSurface` were **built but mounted on no page**. The
honest options were to (a) extend serve with off-path endpoints (rejected —
re-couples), (b) land as scaffold and defer, or (c) mount the genuinely-local
affordances and redefine G5-5. Hossain chose (c).

This mirrors how ADR-0017 redefined Gate-4 C1/C2 (`docs/gate-4-acceptance.md`):
keep the in-repo deliverable honest and mark the part that the decoupling made
inapplicable as out-of-scope-by-design, not as a failure.

## Decision

> Accepted reading of G5-5: **every page that depends on the ATLAS artifact /
> rule-engine surface functions against local Rust surfaces; pages whose data is
> off the artifact path (ML / analytics / jurisdiction / credit, per ADR-0017)
> remain on the external `VITE_API_URL` by design**, behind the same default-off
> flags with a transparent fallback.

Concretely, the genuinely-local surfaces delivered in 5d/5e:

- **KEWorkbench** — compile-preview + dry-run of inline YAML (WASM adapter when
  `USE_WASM_PREVIEW`, else serve `POST /compile/preview` // `/dry-run`), and verify
  an artifact by hash (serve `POST /verify`) feeding **canonical provenance** into
  the 5e `ReviewSurface`. Mounted via the flag-gated `LocalKePreviewPane`.
- **ProductionDemo** — health via serve `GET /healthz`.

The off-path pages keep their `*Local` variant that throws `ServeUnsupportedError`
(never fabricates data) so the hook falls back to the untouched `VITE_API_URL` path.
All `VITE_USE_*` flags stay **default-off**, so `main` is byte-unchanged.

The 5e `ReviewSurface` is fed only the two provenance classes the canonical artifact
actually carries (compiler-validity, expert-attestation); ml-evidence / ai-suggestion
are caller-supplied proposal metadata with no canonical source yet, so they render
empty rather than fabricated.

## Consequences

- G5-5 is closeable in-repo on honest terms: artifact-path pages rewired and
  verified (`npm run typecheck`/`test:run`/`build` green, flags-off ⇒ `main`
  unchanged); off-path pages explicitly scoped out, not silently "passed".
- The frontend stays decoupled from the ML/analytics backends — consistent with
  ADR-0017 — instead of growing serve endpoints that re-couple them.
- A future AI-proposal source (post-Gate-5) can feed ReviewSurface's remaining two
  classes without re-opening this decision.
- The Playwright visual harness remains experimental / non-gating; its baselines are
  Linux-CI-canonical.

## Alternatives considered

- **Extend `ke-cli serve` with the off-path endpoints (rules-list, analytics, …).**
  Rejected: it re-imports the coupling ADR-0017 removed, and the ML/analytics data is
  not on the artifact path at all — serve would become a proxy to the very backend
  the decoupling separated.
- **Land 5d/5e as scaffold and defer the mounts + redefinition.** Rejected by
  Hossain: leaves two built affordances inert and G5-5 ambiguously "pending".
- **Keep G5-5 literal and mark Gate 5 incomplete.** Rejected: the literal criterion
  is unachievable post-decoupling for off-path pages; redefining (as with C1/C2) is
  the honest closure, not lowering the bar.
