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

**Consumer trust boundary (ADR-0019, Accepted):** COMPASS
(`../cross-border-compliance-navigator`, HEAD `e192cf5`, branch
`feat/next15-phase-c1`) is a **federated consumer** — its post-Gate-5 rewire must
**re-derive** trust (issuer via the ADR-0009 key directory/registry root, tenant via
`regime_id`, registration via live registry state, revocation via signed directory
`status`), treat any non-`published` state as **blocked even with valid crypto**, and
fail **closed** on `unknown`. The 5d cutover of ATLAS's own `frontend/` honors the
same three principles (lifecycle-as-control-point, credential ≠ authority, re-derive
don't import). Contract + acceptance: `dev/briefs/compass-consumer-state-and-gate5-rewire.md` §3.

**Acceptance map (spec §19 Gate 5):**

| Criterion | Closed by | Status |
|---|---|---|
| G5-1 each flagged surface reads the **canonical** registry/artifact view | 5a, 5b-data | 5a ✅ |
| G5-2 WASM preview vs canonical compile → differences **surfaced, never silently published** | 5b-preview | ✅ |
| G5-3 SQL over metadata **== canonical** | 5b-data | ✅ (Linux CI leg; feature-gated-defer) |
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

**ADR:** 0018 (Accepted). **Commit:** handed to Hossain (`migration/gate-5a-serve`).

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

## Phase 5b-data SQL views — `ke sql` READ-ONLY DuckDB over registry metadata (G5-3), feature-gated-defer (2026-06-15)

Read-only SQL over a **projection of canonical artifact contents**. New module
`crates/ke-cli/src/commands/sql.rs`, an always-present `Sql` CLI variant, and
`crates/ke-cli/tests/sql_views.rs` (`#![cfg(feature = "sql-views")]`, inert on the
default build). NON-AUTHORITATIVE and read-only: no sign/attest/publish/revoke/
transition, no `append_event`, no re-encode. The in-memory DuckDB connection
reads the on-disk registry (every `.kew` decoded via `ke_artifact::decode_artifact`,
lifecycle state derived from the event log via the existing `current_state` /
`head_event` — the same read path `export-provenance` uses) and never mutates it.
The projected columns are exactly the RECON `metadataFields` set, sourced from
`Manifest` + the event-log-derived state + `Artifact::compiler_signature` /
`attestations`.

**Probe verdict → SHAPE = feature-gated-defer (authoritative):** on this
`1.85.0-x86_64-pc-windows-gnu` toolchain DuckDB cannot build. Two independent
blockers, both confirmed this session:

- **No C/C++ compiler on PATH** — `which g++` returns "no g++ in …". The
  `libduckdb-sys` `cc-rs` build script fails (`ToolNotFound`, `EXITCODE=101`) —
  the probe's documented failure.
- **MSRV mismatch** — `duckdb = "=1.1.1"` resolves `libduckdb-sys 1.10503.1`,
  whose MSRV is **rustc 1.85.1**; the local toolchain is 1.85.0, so
  `cargo check -p ke-cli --features sql-views` fails at resolution *before* even
  reaching the C++ step (`rustc 1.85.0 is not supported … requires rustc 1.85.1`).

Both point to the same conclusion: this surface is built only where a modern C++
toolchain and a current stable rustc exist. So it ships **default-off** behind a
`sql-views` cargo feature (`duckdb = { version = "=1.1.1", features = ["bundled"],
optional = true }`, gated `dep:duckdb`; `default = []` links no duckdb), with:

- the `Sql` CLI variant **always present** (so `ke --help` lists it); only
  `sql::run` is gated — on the default windows-gnu build it returns the typed
  `"ke sql requires --features sql-views (blocked on windows-gnu …)"` error, the
  same pattern `compile`/`import` use for `test-keys`;
- a dedicated **Linux CI leg** — job `sql-views` in
  `.github/workflows/rust-ci.yml` (ubuntu-latest, `cargo test -p ke-cli --features
  sql-views`) — the **only** place the SQL views compile + run, where a C++
  toolchain and a `@stable` rustc (>1.85.1) exist.

**G5-3 spot-check (`tests/sql_views.rs`, runs only on the CI leg):** compile a
corpus artifact into a tempdir registry, run
`SELECT artifact_hash, regime_id, lifecycle_state, signer_key_id FROM artifacts`,
and assert each projected value equals an **independent oracle** read straight
through the registry path (hex hash == compiled hash; `regime_id` == manifest;
`lifecycle_state` == event-log `current_state().event_kind()`; `signer_key_id` ==
the decoded `.kew` compiler-signature key id, loudly `test-`). Plus a
`COUNT(*)`-shaped read-only query. SQL rows == canonical artifact contents.

**Verification (re-run by the session, default build):** `cargo build -p ke-cli`
(no features) clean, **no duckdb in `target/debug/deps`**; `cargo test --workspace`
**166/0**; the `sql.rs`/`sql_views.rs`/`cli.rs` edits are `cargo fmt`-clean and
produce no clippy warnings. The CI leg cannot be exercised locally (that is the
whole point of the defer); it is asserted by the new ubuntu-latest job.

**Deviation surfaced (not silently applied):** `=1.1.1` is the contract's pin but
its `libduckdb-sys 1.10503.1` carries MSRV 1.85.1, one patch above the local
toolchain — recorded above so the next maintainer knows the local
`--features sql-views` check fails at *resolution*, not only at the C++ step.

**Commit:** handed to Hossain (current Gate-5 closeout branch).

---

## Ahead

- **5c** — lint integration into `ke-cli compile`.
- **5d** — frontend rewire page-by-page behind `VITE_USE_LOCAL_KE_API` /
  `VITE_USE_WASM_PREVIEW` (default-off); §21.8 visual-regression tool chosen first.
  Each cutover page reads the **canonical** registry/artifact view and honors the
  ADR-0019 governance framing (non-`published` blocked even with valid crypto;
  fail-closed on `unknown`) — the same boundary the COMPASS consumer enforces.
- **5e** — minimum §13 AI-provenance review UI behind `VITE_USE_REVIEW_UI`.
