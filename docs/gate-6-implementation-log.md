# Gate 6 (reconciled) — implementation log

Record of the Gate-6 build (revocation runtime-decision + registry surface
completion), executed 2026-07-19 from the approved plan in
`dev/briefs/gate-6-plan-and-next-session-seed.md`. Authoritative scope:
**ADR-0024** (this gate's reconciliation ADR — spec Gate 6 "production
cutover" is unmeetable as written post-ADR-0017; ADR-0024 records what
closes, what ships, and what defers). Continues the Gate 1–5 doc-each-phase
convention.

**Toolchain:** Rust 1.85 (`x86_64-pc-windows-gnu`), Git Bash for scripts.
Built TDD (red-green per test), same discipline as the ADR-0023 build.

**Load-bearing assumption, verified before building:** the
`revocations/<hash>.json` sidecar is **outside the canonical envelope** —
`RevocationRecord` lives only in `registry/backend.rs` (plain serde JSON);
neither the artifact content hash (`ke-artifact/src/hash.rs`) nor the frozen
`LifecycleEvent` head-hash pin (`registry/event.rs`) references it. Adding
`reason_class` therefore bumps nothing. (`RevocationPolicy` itself sits inside
the canonical `PolicyBundle` — it was reused, never modified.)

## What shipped

| Piece | Where | Tests |
|---|---|---|
| `RevocationReasonClass` + `revocation_floor` + `strictness_rank` + `revocation_decision` (stricter-of floor/configured) | `crates/ke-core/src/revocation.rs` (new; pure, no deps) | `crates/ke-core/tests/revocation_decision.rs` — 6 tests: ADR-0009 §4 matrix, strictness ordering, floor-when-unconfigured, raise-OK, lower-impossible, serde round-trip |
| `ke revoke --reason-class` — records class + decided policy; `--policy` below floor rejected **before** any state transition; legacy `--policy`-only path byte-compatible (`reason_class` key absent via `skip_serializing_if`) | `commands/revoke.rs`, `registry/backend.rs` (`RevocationRecord.reason_class: Option`), `cli.rs` | `crates/ke-cli/tests/revocation_reason_class.rs` — 7 tests incl. raw-JSON byte-compat and rejected-revoke-leaves-no-sidecar |
| `GET /resolve?regime=&effective=YYYY-MM-DD[&env=]` → `Selector::ByRegime` (HTTP mirror of CLI `query --regime --effective`; shared `parse_date` grammar) | `serve/handlers.rs` | `crates/ke-cli/tests/serve.rs` — regime resolve, missing/malformed effective → 400, unknown regime → 404 |
| `revocation` block (the sidecar verbatim) on `ResolutionRecord` + `VerifyResponse`, present **exactly when** the state is `Revoked` | `registry/mod.rs`, `serve/dto.rs`, `serve/handlers.rs` | `serve.rs` — revoked resolve/verify carry the block; published resolve/verify carry **no** `revocation` key |
| Docs: contract + governance | `docs/consumer-serve-contract.md` (regime form + revocation block), ADR-0024 (new), ADR-0015 (Accepted, dated note), ADR index, `CLAUDE.md` §21 revocation row | — |

## Acceptance evidence (ADR-0024 criteria, all runs 2026-07-19, this machine)

1. **ADR-0015 Accepted** ✅ (dated acceptance note; channel 3 deferred per
   ADR-0017). **ADR-0024 Proposed** — acceptance = its PR merge (Hossain).
2. **`revocation_decision` unit-tested** ✅ — `cargo test -p ke-core --test
   revocation_decision`: **6 passed / 0 failed**. Red observed first for every
   test (module absent), then green.
3. **`revoke --reason-class`** ✅ — `cargo test -p ke-cli --test
   revocation_reason_class`: **7 passed / 0 failed**.
   `./scripts/lifecycle-smoke.sh`: **PASS** — full lifecycle twice, whole-tree
   diff (`events/ artifacts/ tags/ consistency/ revocations/`)
   **byte-identical**, proving the legacy `--policy auditonly` path unchanged.
4. **Surfacing tested** ✅ — `cargo test -p ke-cli --features test-keys --test
   serve`: **20 passed / 0 failed** (13 pre-existing + 7 Gate-6).
5. **Verify stays fail-closed** ✅ — `cargo test --workspace --features
   test-keys`: **207 passed / 0 failed** (exit 0); baseline before this build
   was 187/0 without the feature, re-verified at session pickup. Live check:
   a compile-only (`StructurallyVerified` → `Unknown`) artifact POSTed to a
   running `/verify` → `{"verdict":"rejected","registry_state":"Unknown",
   "rejection":"attestations: R6: ..."}`.
6. **CLAUDE.md §21 row updated** ✅ (decision delivered consumer-agnostically;
   orchestration enforcement deferred, ref ADR-0024).

Hygiene: `cargo fmt --all --check` clean; `cargo clippy --workspace
--all-targets --features test-keys` zero warnings.

## Live end-to-end smoke (ADR-0024's smoke, run against a real `ke serve`)

Scripted seed → serve → probe (fixed `KE_NOW=1750000000`, fixed-seed test
keys, tmp local-FS registry — non-authoritative, ADR 0012 §6):
**passed=14 failed=0**. The sequence and observed results:

- `mica_stablecoin` driven to Published; `fca_crypto` driven to Published then
  `revoke --reason-class advisory` → CLI printed `policy=AuditOnly` (floor).
- `revoke --reason-class key_compromise --policy auditonly` → **rejected**,
  error names the floor; artifact still Published, no sidecar written.
- `GET /resolve?regime=mica_2023&effective=2025-01-01&env=staging` →
  `registry_state_at_resolution:"Published"`, **no** `revocation` key.
- `effective=nope` → HTTP **400**.
- Live `revoke --hash <mica> --reason-class key_compromise` (server running)
  → `policy=HardStop`; then `GET /resolve?hash=` →
  `"Revoked"` + `"reason_class":"KeyCompromise"` + `"policy":"HardStop"`.
- `POST /verify` (same hash) → `"verdict":"rejected"` (fail-closed) **and**
  the revocation block.
- `POST /verify` (advisory artifact) → still `"rejected"`,
  `"policy":"AuditOnly"` surfaced — the § 15 audit-only wind-down is the
  consumer's decision; verify never softens.

## Honesty boundary (from ADR-0024, restated so this log can't oversell)

The decision layer is **groundwork shipped ahead of its enforcer** — no live
orchestrator consumer runs HardStop/FinishPinned/AuditOnly today. Delivered =
the pure decision + its recording + its surfacing, all tested. Deferred +
recorded, not blockers: platform Temporal pinning activity, Python KE module
removal, Rust Temporal worker (each re-opens only with a real orchestrator
consumer, via a new ADR citing ADR-0024).

**Gate 6 closes on the ADR-0024 PR merge** — per repo discipline, a gate is
not closed by this log or any local state.
