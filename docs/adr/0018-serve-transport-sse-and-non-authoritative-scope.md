# 0018. `ke serve` uses SSE (not WebSocket) and is strictly non-authoritative

**Status:** Proposed (pending sign-off by Hossain)
**Date:** 2026-06-15
**Spec references:** § 16 (multi-surface access), § 7.4 (frontend feature flags / live feed), § 6 (WASM/serve preview discipline), § 5 / § 10 / § 13 (authority boundaries)
**Gate:** 5 (Phase 5a — `ke-cli serve`)

## Context

Spec § 16 / § 7.4 describe the Gate-5 server surface as "REST + **WebSocket**".
Building `ke serve` on the primary **windows-gnu** dev toolchain surfaced a hard
constraint (memory `toolchain-windows-gnu-getrandom-dlltool`, ADR rationale also
behind `clap default-features=false`):

- A WebSocket crate (`tungstenite`) pulls `rand 0.9` → **`getrandom 0.3`**, which
  **cannot build** on this toolchain (broken self-contained dlltool).
- The async alternative (`tokio`/`axum`/`hyper`) pulls `tokio → mio →
  **windows-sys**`, which also cannot build here (the exact reason clap is pinned
  `default-features=false`).

A dependency-build spike (isolated worktree, verified with `cargo build` +
`cargo tree -i windows-sys` + `cargo tree -i getrandom`) found a synchronous,
std-net stack that builds cleanly with **zero** of those transitive deps:
`tiny_http 0.12` (deps: `ascii`, `chunked_transfer`, `httpdate`, `log` only).

Separately, `serve` exposes the registry/artifact surfaces over HTTP, which
raises the authority question: an HTTP surface must not become a back door around
the CLAUDE.md § 5/§ 10/§ 13 authority boundaries.

## Decision

1. **Transport: synchronous `tiny_http` + Server-Sent Events.** `ke serve` is a
   blocking, thread-per-request HTTP/1.1 server on `tiny_http`. The live feed is
   **SSE** (a long-lived `text/event-stream` response), **not WebSocket**. SSE is
   plain HTTP, needs no new dependency, and fits a one-way registry/compile event
   feed; `EventSource` clients consume it directly. If a future need requires
   bidirectional WebSocket, it must be gated behind a **Linux-CI-only cargo
   feature** via a follow-up ADR — never by swapping the default stack to one that
   breaks the windows-gnu toolchain.

2. **Scope: strictly non-authoritative.** `serve` exposes
   read / resolve / verify / compile-preview / dry-run + a read-only event feed
   **only**. It MUST NEVER sign, attest, publish, revoke, or assemble artifacts
   over HTTP. `Artifact::assemble` and signing stay exclusively on the
   `ke compile` CLI path (`ke_cli::commands::compile::run`). Every response reads
   the **canonical** registry/artifact view (the on-disk `LocalFsBackend`), never
   a vendored or in-memory snapshot — this is Gate-5 acceptance **G5-1**.

## Consequences

- **Desirable:** `serve` builds and tests on the primary dev toolchain (workspace
  155/0, `tests/serve.rs` 13/0 incl. a `canonical_view_is_the_serve_backend`
  oracle). No async runtime, dependency-light, deterministic — matching the repo
  posture. The authority boundary is enforced by construction (a skeptic pass
  confirmed serve/ references no signing key and no write/publish/assemble call).
- **Undesirable / managed:** SSE is one-way, so a future interactive/bidirectional
  use case is not served today; it is deferred to the Linux-CI-feature path above.
  `tiny_http` is blocking/thread-per-connection — fine for a local dev/preview
  surface, not a high-concurrency production endpoint (which `serve` is not).

## Alternatives considered

- **WebSocket via `tungstenite` (spec wording):** rejected — `getrandom 0.3` does
  not build on windows-gnu.
- **`axum`/`tokio`:** rejected — `windows-sys` does not build on windows-gnu;
  adopting it would also reverse the `clap default-features=false` discipline.
- **Polling-only (no live feed):** rejected — SSE delivers the live feed with zero
  added dependencies, so there is no reason to drop the capability.
