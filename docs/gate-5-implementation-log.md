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
| G5-1 each flagged surface reads the **canonical** registry/artifact view | 5a, 5b-data | ✅ (serve 5a; SQL 5b-data) |
| G5-2 WASM preview vs canonical compile → differences **surfaced, never silently published** | 5b-preview | ✅ |
| G5-3 SQL over metadata **== canonical** | 5b-data | 🟡 **not yet green on CI** — never actually executed until the toolchain fix; first real run panicked because `sql.rs` read `column_names()` *before* executing the query (duckdb populates the result schema only post-execution). Fixed by reordering — read column names from the executed statement via `Rows::as_ref` (2026-06-18); pending first green CI run. |
| G5-4 flat-file export → offline import → **sig + hash verify** | 5b-data | ✅ MET (in-repo) |
| G5-5 every page **functions against local Rust surfaces** | 5d/5e | 🟡 **DEFERRED** (ADR-0020): the ATLAS frontend rewire onto local Rust surfaces is deferred, not delivered — **COMPASS** is the real consumer of the ATLAS artifact path (verify-only, in-browser, ADR-0017/0019); ATLAS's own frontend is producer-side authoring/review tooling, and most pages are off the artifact path (ML/analytics/jurisdiction/credit) and cannot be rewired. Revisit only if/when the ATLAS frontend genuinely needs the local surfaces. |

**Gate 5 status: 5a + 5b-preview + 5b-data + 5c complete & independently verified;
5d/5e (frontend rewire + review UI) DEFERRED (ADR-0020).** Each delivered sub-phase
was built by an automode workflow (spike → contract → scaffold → build →
integration-runner), closed by a skeptic-verifier on the load-bearing claim, and the
gate was **re-run by the session** (not self-reported).
For 5d/5e the skeptics **refuted** the workflow's "rewired" claim — only 1 of 9
pages genuinely hit a local surface and two affordances were built-but-unmounted; the
session then **DEFERRED the ATLAS frontend rewire (G5-5, ADR-0020)** rather than
accept the overclaim. No commits made by Claude — Hossain owns the history; commit
commands handed over per phase.

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

## Phase 5b-data export/import — `ke export` / `ke import` (G5-4) ✅ MET (in-repo) (2026-06-15)

Flat-file `.kew` round-trip. `ke export --hash --out` reads the stored `.kew` via
`RegistryBackend::read_artifact_kew` and writes **byte-identical** bytes (no
re-encode, no re-hash, no sign, no lifecycle transition). `ke import --in` reads the
flat file **offline** (`std::fs::read`, no backend/network) and re-verifies via
`ke_artifact::verify_artifact` **verbatim** — the re-zero content-hash recompute is
that function's internal step 2; import never calls `blake3` directly. New
`crates/ke-cli/src/commands/{export,import_kew}.rs` + `tests/export_import.rs`.

**Skeptic (G5-4) SURVIVED:** an exhaustive single-byte tamper sweep — **14,720
tampers** across the whole 7,360-byte artifact (envelope + the 84-byte
out-of-envelope tail), each at two deltas — was accepted by import **zero** times;
envelope tampers fail `HashMismatch`/`CompilerSignatureInvalid`, tail tampers fail
strict postcard decode. Confirmed re-zero (not naive whole-file): `blake3(whole)` and
`blake3(envelope prefix)` both differ from the stored hash while `verify_hash`
passes clean. **Commit:** `cbd904d`.

---

## Phase 5c — lint integration: advisory `T5` tier + `ke lint` ✅ (2026-06-15)

Lint-beyond-the-compiler as a new **advisory** `T5` tier in `ke-compiler`
(`src/verify/t5.rs`): three advisory findings (missing description / missing tags /
unannotated leaf, all `blocking:false`) + one opt-in blocking finding
(`T5-rule-id-whitespace`) deliberately scoped so it never fires on the corpus.
Wired into `verify/mod.rs` after `t1::check`; `has_blocking()` unchanged so the
compile gate is not tightened. New `ke lint` CLI command
(`src/commands/lint.rs`) runs **only** `t5::check` — reads no registry, opens no
backend, signs nothing. Canonical encoding (postcard, envelope split, re-zero hash)
untouched: T5 runs on `RuleIR` before assembly. **Commit:** `cbd904d`.

---

## Phase 5d/5e — frontend rewire over-built, then DEFERRED (flag-gated scaffolding kept) 🟡 (2026-06-16 → deferred 2026-06-17, ADR-0020)

Built by the `gate5-5d-5e` workflow, then **deferred by the session** after
adversarial verification — see the honest verdict below. The workflow over-built a
per-page rewire that ADR-0017 made low-value; the rewire is now **DEFERRED**, with
the genuine engine-surface affordances kept in the tree behind **default-off** flags
as inert optional scaffolding.

**What the workflow delivered (verified green):** a clean flag scaffold —
`frontend/src/config/flags.ts` (`USE_LOCAL_KE_API` / `USE_WASM_PREVIEW` /
`USE_REVIEW_UI`, all **default-off**) — plus, across all 9 pages, a per-page
local-variant + transparent `VITE_API_URL` fallback, the 5e review components
(`src/components/review/`, four provenance classes, authority-boundary-correct), and
a Playwright self-hosted visual harness (**experimental / non-gating**; baselines are
Linux-CI-canonical — windows-local pixels are not authoritative).

**Skeptics REFUTED the "rewired" claim (and the session confirmed from code):**
only **1 of 9 pages** genuinely hit a local Rust surface — **ProductionDemo**
(`useHealth → healthLocal → serve GET /healthz`). The other 8 are honest
**scaffold-only** fallbacks: `ke-cli serve` (ADR-0018) exposes only
`/healthz,/resolve,/verify,/compile/preview,/dry-run,/events`, and the
analytics/embeddings/similarity/graph/jurisdiction/credit pages need ML/analytics
data that is **off the ATLAS artifact path by ADR-0017** — no endpoint exists by
design. Worse, **KEWorkbench's compile-preview/dry-run affordance and the 5e
`ReviewSurface` were built but mounted on no page** (referenced only in a comment +
tests), so flag-on they did nothing in the real app.

**Session disposition — DEFERRED, not delivered (ADR-0020):** the per-page ATLAS
frontend rewire (the 5d move from `VITE_API_URL` to `ke serve` REST) is **deferred**.
It is low-value post-ADR-0017 because **COMPASS** (`cross-border-compliance-navigator`)
is the **MAIN / real CONSUMER** of the ATLAS artifact path: COMPASS verifies hash,
signatures, registry state, and typed expert attestations **in-browser** via the
`@platform/atlas-artifact` WASM verifier; it is **consumer-only** (it does not sign,
publish, or execute the rule engine) and is **gated post-Gate-5** (ADR-0017/0019).
ATLAS's own React frontend is **producer-side authoring/review tooling, NOT the
consumer** — so rewiring it onto local Rust surfaces buys little, and most ATLAS pages
are **off the artifact path** (ML/analytics/jurisdiction/credit) and cannot be rewired
at all.

**What was built and is KEPT (inert, default-off — not claimed as a delivered rewire):**
- A flag-gated `LocalKePreviewPane`
  (`frontend/src/components/workbench/LocalKePreviewPane.tsx`) mounted in
  `KEWorkbench.tsx`, hidden unless `USE_WASM_PREVIEW`/`USE_LOCAL_KE_API` is on — so
  KEWorkbench is byte-unchanged with flags off. It is an **optional KEWorkbench
  in-browser WASM compile/dry-run preview pane**: compile-preview/dry-run of inline
  YAML (WASM adapter or serve `POST /compile/preview` // `/dry-run`), and verify an
  artifact by hash (serve `POST /verify`) → feeds canonical provenance into the
  `ReviewSurface`. Wired via new hooks (`useCompilePreview`/`useDryRun`/`useVerify` in
  `hooks/useRules.ts` + `serveVerify` in `api/serve/serveClient.ts`).
- The **5e review components** (`src/components/review/`) remain in the tree behind
  the default-off `USE_REVIEW_UI` flag — kept as inert optional affordances, **not**
  claimed as a delivered rewire.
- Honesty kept: this is the genuine record that the 5d/5e workflow **was built and is
  now deferred** — the scaffolding is not removed (Option 1), it is left inert and
  default-off. Only the 2 canonical provenance classes (compiler-validity,
  expert-attestation) are fed from `/verify`; the ml-evidence/ai-suggestion classes
  remain wired-but-unfed (no AI-proposal source yet).
- **G5-5 disposition (ADR-0020):** 🟡 **DEFERRED** — the ATLAS frontend rewire is
  deferred; COMPASS is the consumer; revisit only if/when the ATLAS frontend genuinely
  needs the local surfaces. ADR-0020 stays **Status: Proposed** pending Hossain's
  sign-off.

**Verification (re-run by the session):** `cd frontend && npm run typecheck` → 0;
`npm run test:run` → **111 passed / 111** (default flags OFF — `main` unchanged);
`npm run build` → 0. Skeptic re-check by grep: `ReviewSurface` + `LocalKePreviewPane`
now imported by `src/pages/KEWorkbench.tsx`; the preview/verify hooks are
page-consumed (no longer comment/test-only). **Commit:** handed to Hossain.

---

## Ahead / follow-ups

- **5b-data SQL baseline + Playwright baselines** bootstrap on the Linux CI legs
  (cross-OS; not exercisable on windows-local). Playwright stays experimental /
  non-gating.
- **ML/analytics pages** (8 of 9) remain on the external `VITE_API_URL` by design
  (off the ATLAS artifact path, ADR-0017) — not a gap, a boundary (ADR-0020).
- **A real AI-proposal source** to feed ReviewSurface's ml-evidence/ai-suggestion
  classes (post-Gate-5).

---

## Live verifier unblock (2026-06-23) — loop is now demonstrably green

Earlier status: the live-verifier loop was blocked on three ATLAS-side items the
COMPASS consumer needs — the `@platform/atlas-artifact` browser build did not
exist, the npm package was unpublished, and no `ke serve` instance held
**Published** artifacts (so `/verify` correctly returned `Unknown`). All three are
now resolved in-repo, on the windows-gnu toolchain, against **fixed-seed TEST
keys** (`is_test_key:true`; production-key authority remains the open ADR-0009
decision — this makes the loop *real*, not *production-trusted*).

**Browser build.** `cargo build -p ke-wasm --target wasm32-unknown-unknown
--release` + `wasm-bindgen --target bundler` produces `crates/ke-wasm/pkg/`
(generated, gitignored). The wasm32 path is RNG-free (no `getrandom`, no `cc`
linking — the dlltool issue does not apply). `cargo test -p ke-wasm --test parity`
green (WASM ≡ native). The four exports (`verify_artifact`, `read_provenance`,
`compile_preview`, `dry_run`) are present in `pkg/ke_wasm.d.ts`.

**Publish-ready package.** `crates/ke-wasm/package.json` bumped to `0.1.0`,
`private:false`, `publishConfig` → GitHub Packages, `repository` set, and the
previously-missing `pkg/ke_wasm_bg.js` added to `files` (the bundler `ke_wasm.js`
imports it — the package would otherwise publish broken). `npm pack --dry-run` →
6 files, ~380 kB. Publish runbook + GitHub-Packages scope caveat in
`docs/publish-atlas-artifact.md`. `npm publish` is Hossain's (credentialed).

**Running Published registry.** `scripts/serve-published-registry.sh` (reuses the
`lifecycle-smoke.sh` pattern) builds `ke-cli --features test-keys`, seeds a
registry to `Published` (mica_stablecoin) plus a compile-only `Unknown` artifact
(fca_crypto), and execs `ke serve`. Consumer contract documented in
`docs/consumer-serve-contract.md`.

**End-to-end proof (captured live, not asserted):**
- Published → `{"verdict":"verified", "registry_state":"Published"}`.
- Fully attested but NOT published → `{"verdict":"rejected", "rejection":"registry
  state not Published: Unknown", "registry_state":"Unknown"}` — **valid crypto +
  complete attestations still blocked**, the core ADR-0019 fail-closed guarantee.
- Unattested → rejected (`attestations: R6 ... missing`).
- Unknown hash → **HTTP 404**.
- `/resolve?hash=` of the published artifact → `registry_state_at_resolution:
  "Published"`, `resolving_event_key:"published"`.

**Verification (re-run by the session):** both `cargo build` targets clean; `cargo
test -p ke-wasm --test parity` → 4/4; `cargo test --workspace --features
test-keys` → exit 0, zero failures. **Commit + `npm publish`: handed to Hossain.**

**Scope note:** ATLAS-only change (no COMPASS-repo edits; consumer wiring is the
separate post-Gate-5 task). New files: `scripts/serve-published-registry.sh`,
`docs/consumer-serve-contract.md`, `docs/publish-atlas-artifact.md`. Edited:
`crates/ke-wasm/package.json`. Generated (gitignored): `crates/ke-wasm/pkg/`.
