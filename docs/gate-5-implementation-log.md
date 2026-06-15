# Gate 5 — implementation log

Phase-by-phase record of the Gate 5 implementation (surface rollout + frontend
rewire), written as each sub-phase lands. Authoritative spec:
`docs/spec/ke-workbench-rust-migration-spec-v3.1.md` §19 (Gate 5 acceptance), §6
(WASM/serve discipline), §7.4 (frontend flags), §13 (review UI), §16 (multi-surface
access). Continues the Gate 1–4 doc-each-phase convention.

**Toolchain:** Rust 1.85 (`x86_64-pc-windows-gnu`); Node 20 (frontend); wasm32
target + `wasm-bindgen` (cli pinned `=0.2.95` to the crate).

**Sequencing (ADR-0017):** `institutional-defi-platform-api` is **decoupled** from
the ATLAS artifact path — it is not the consumer; **COMPASS** is (verify-only,
in-browser, gated post-Gate-5). Gate-4 C1/C2 were redefined accordingly (verifier +
equivalence foundation MET in-repo; platform execute-parity obsolete). With no
platform PR, nothing external gates Gate 5; the frontend cutover (5d) and review UI
(5e) stay gated behind per-page parity + default-off flags.

**Acceptance map (spec §19 Gate 5):**

| Criterion | Closed by | Status |
|---|---|---|
| G5-1 each flagged surface reads the **canonical** registry/artifact view | 5a, 5b-data | 5a ✅ |
| G5-2 WASM preview vs canonical compile → differences **surfaced, never silently published** | 5b-preview | ✅ |
| G5-3 SQL over metadata **== canonical** | 5b-data | pending |
| G5-4 flat-file export → offline import → **sig + hash verify** | 5b-data | pending |
| G5-5 every previously-working page **functions against local Rust surfaces** | 5d | pending |

**Gate 5 status: 5a + 5b-preview complete & independently verified — 5b-data, 5c,
5d, 5e ahead.** Each sub-phase was built by an automode workflow (spike → contract →
scaffold → build → integration-runner), closed by a skeptic-verifier on the
load-bearing claim, and the gate was **re-run by the session** (not self-reported).
No commits made — Hossain owns the history; commit commands handed over per phase.

---

## Phase 5a — `ke-cli serve` (REST + SSE) ✅ (2026-06-15)

A thin, **non-authoritative** HTTP read/preview adapter over the existing pure
surfaces. New module `crates/ke-cli/src/serve/{mod,router,handlers,dto}.rs`, a
`Serve` CLI variant (replacing the deferred stub), and `tests/serve.rs`.

**Transport decision (ADR-0018):** synchronous **`tiny_http` 0.12 + Server-Sent
Events**, not WebSocket. A spike (isolated worktree, `cargo tree -i windows-sys` /
`-i getrandom`) proved the spec's "WebSocket" wording is not buildable on this
windows-gnu toolchain — `tungstenite → rand 0.9 → getrandom 0.3` and
`tokio → mio → windows-sys` both fail to build (the same constraint behind
`clap default-features=false`). `tiny_http` builds with **no `windows-sys`, no
`getrandom 0.3`** (deps: `ascii`, `chunked_transfer`, `httpdate`, `log`). The
WebSocket→SSE deviation was surfaced, not silently swapped.

**Endpoints** (all reuse the canonical surfaces; serve never signs/publishes):
`GET /healthz`; `GET /resolve?hash=` and `?env=&tag=` → `registry::resolve`;
`POST /verify` → `ke_artifact::verify_artifact` over canonical `RegistryEvidence`;
`POST /compile/preview` → `ke_compiler::compile_rules` + `verify::verify`;
`POST /dry-run` → `ke_runtime::evaluate`; `GET /events` → read-only SSE feed.

**Authority (CLAUDE.md §5/§10/§13):** no sign/attest/publish/assemble/revoke path
is reachable over HTTP; `Artifact::assemble` stays exclusively on `ke compile`.

**Verification (re-run by the session):** `tests/serve.rs` **13/0** (incl.
`canonical_view_is_the_serve_backend`, an independent oracle reopening the same
`LocalFsBackend` — G5-1); full workspace **155/0**. A skeptic-verifier tried to
refute "reads canonical view / never signs" and **survived** (only doc-comment
matches for sign/publish; one concrete `RegistryBackend`; verifying-keys only).

**ADR:** 0018 (Proposed). **Commit:** handed to Hossain (`migration/gate-5a-serve`).

---

## Phase 5b-preview — `ke-wasm` browser preview/dry-run + `frontend/src/wasm/` ✅ (2026-06-15)

The load-bearing browser surface (the frontend rewire and the COMPASS consumer both
depend on it). Added `compile_preview(source)` and `dry_run(source, facts)`
`#[wasm_bindgen]` fns to `crates/ke-wasm/src/lib.rs`, reusing the **same** pure fns
as native `ke-cli` (`compile_rules` / `verify` / `facts_from_json` / `evaluate` /
`normalized_json`). **No new crate deps** (ke-compiler/ke-runtime already deps),
**getrandom-free**. The shipped Gate-4 verifier (`verify_artifact`,
`read_provenance` — ADR-0016, `@platform/atlas-artifact`) is left **byte-for-byte
unchanged**.

**Frontend:** `frontend/src/wasm/` (`index.ts` typed adapter, `parity.ts` browser
mismatch banner, `index.test.ts` vitest mocking the raw bindings). The real wasm32
build + `wasm-bindgen` step replaced the Gate-0 stub in
`.github/workflows/wasm-build.yml`, with the cli↔crate `=0.2.95` pin asserted in CI.

**G5-2 parity:** because the wrappers reuse the canonical pure fns, parity holds by
construction; `crates/ke-wasm/tests/parity.rs` asserts wasm output == native
canonical output byte-for-byte and fails loudly otherwise (surfaced, never silently
published). The browser leg deep-equals the local preview against the canonical
`ke-cli serve` endpoint when reachable and surfaces any mismatch.

**Verification (re-run by the session):** wasm32 build green; `ke-wasm` **4/0**
(parity); full workspace **159/0**; frontend wasm vitest **8/0**; `tsc --noEmit`
clean; verifier suites intact (`verify_surface` 6, `attestation` 17, `golden` 8). A
skeptic-verifier **survived both claims** (non-authoritative; parity non-vacuous) —
proving non-vacuity by injecting a bug into the oracle and confirming the parity
test failed, then reverting.

**Deviations surfaced (not silently applied):** WASM binds only the inline-`source`
dry-run path (the stored-`hash` path needs the off-WASM canonical backend);
projection helpers (`project_report/finding/conflict`) are currently **duplicated**
byte-identically in `ke-cli/handlers.rs` and `ke-wasm/lib.rs` — recommended
follow-up is to lift them into a shared `ke-compiler` module (a 2-crate refactor,
Plan-Mode territory) since `parity.rs` guards ke-wasm against an inline oracle, not
against `ke-cli`.

**Commit:** handed to Hossain (`migration/gate-5b-preview`).

---

## Ahead

- **5b-data** — DuckDB SQL views over registry metadata (G5-3) + flat-file `.kew`
  export/import with offline re-verify (G5-4). Own windows-gnu spike (DuckDB C++).
- **5c** — lint integration into `ke-cli compile`.
- **5d** — frontend rewire page-by-page behind `VITE_USE_LOCAL_KE_API` /
  `VITE_USE_WASM_PREVIEW` (default-off); §21.8 visual-regression tool chosen first.
- **5e** — minimum §13 AI-provenance review UI behind `VITE_USE_REVIEW_UI`.
