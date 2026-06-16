//! `ke serve` (Gate 5): a **thin, NON-AUTHORITATIVE** HTTP read/preview adapter
//! over the existing pure functions. It REUSES — never reimplements —
//! `ke_artifact::verify_artifact` (verify), `ke_cli::registry::resolve`
//! (resolve), `ke_compiler::compile_rules` + `verify::verify`
//! (compile/preview), and `ke_runtime::evaluate` (dry-run).
//!
//! # Authority boundary (CLAUDE.md §5/§10/§13 — a hard constraint)
//!
//! `serve` MUST NEVER sign, attest, publish, revoke, or assemble artifacts over
//! HTTP. It exposes read/preview/dry-run/resolve/verify + a read-only event feed
//! ONLY. The authoritative compile-and-sign path stays exclusively on the
//! existing `ke compile` CLI command (`ke_cli::commands::compile::run` →
//! `Artifact::assemble`); `serve` MUST NOT call `Artifact::assemble`. Every
//! response reads the **canonical** registry/artifact view (the
//! [`LocalFsBackend`] opened here), never a vendored snapshot — this is Gate 5
//! acceptance **G5-1**.
//!
//! # Spec deviations surfaced for Hossain's approval (NOT applied silently)
//!
//! **Deviation 1 — SSE, not WebSocket.** Spec §16/§7.4 wording says
//! "REST + WebSocket" for the live feed. This scaffold implements the live feed
//! as **Server-Sent Events (SSE)** over a long-lived `tiny_http`
//! `text/event-stream` response, NOT WebSocket. Rationale (verified against this
//! repo's toolchain memory `toolchain-windows-gnu-getrandom-dlltool`): a
//! WebSocket crate (`tungstenite`) pulls `rand 0.9` → `getrandom 0.3`, which
//! cannot build on this windows-gnu toolchain (the same constraint that pins
//! `clap default-features=false` and blocks ed25519 keygen via `getrandom 0.3`).
//! `tokio`/`axum` is also out (`tokio` → `mio` → `windows-sys` breaks the
//! toolchain). SSE is plain HTTP, needs ZERO new deps, is a natural fit for a
//! one-way registry/compile event feed, and `EventSource` clients parse it
//! directly. If a future need requires bidirectional WS, gate it behind a
//! Linux-CI-only cargo feature in an ADR rather than swapping the default.
//!
//! **Deviation 2 — serve's authority scope.** Per CLAUDE.md §5/§10/§13 the
//! serve subcommand is read/preview/dry-run/resolve/verify + a read-only event
//! feed ONLY (no sign/attest/publish/revoke/assemble over HTTP). Recommend a
//! short ADR pinning serve's non-authoritative scope before the Build phase
//! fills the handler bodies.
//!
//! Both deviations are surfaced here, never silently swapped.
//!
//! # Runtime shape
//!
//! Sync, dependency-light: `tiny_http` is a thread-per-request HTTP/1.1 server.
//! No async, no tokio, no AWS SDK (CLAUDE.md hard rule). [`run`] binds the
//! server (port `0` picks an ephemeral port so integration tests can read the
//! bound address), opens the [`LocalFsBackend`] once into the shared
//! [`AppState`], and loops handing each request to [`router::route`].

pub mod dto;
pub mod handlers;
pub mod router;

use crate::registry::backend::LocalFsBackend;
use anyhow::{Context, Result};
use std::sync::Arc;

/// Shared, read-only server state handed to every handler. Holds the canonical
/// registry backend (opened once) and the bound base URL for logging. Cloneable
/// (cheap `Arc`) so the thread-per-request loop can hand each request its own
/// handle.
///
/// **Authority:** this state grants READ access to the canonical registry only.
/// Handlers reachable through it MUST NOT sign, attest, publish, revoke, or
/// assemble. There is deliberately no signing key, no `test-keys` path, and no
/// `Artifact::assemble` reachable from here.
#[derive(Clone)]
pub struct AppState {
    /// The canonical local-FS registry backend (G5-1: responses read this, never
    /// a vendored snapshot). `Arc` so request threads share one open backend.
    pub backend: Arc<LocalFsBackend>,
}

impl AppState {
    /// Open the canonical registry at `registry_root` into shared state.
    pub fn open(registry_root: &str) -> Result<Self> {
        let backend = LocalFsBackend::open(registry_root)
            .with_context(|| format!("open registry at {registry_root:?} for `ke serve`"))?;
        Ok(Self {
            backend: Arc::new(backend),
        })
    }
}

/// Bind a `tiny_http` server on `(host, port)` and serve the non-authoritative
/// read/preview surface until the process is killed. `port = 0` picks an
/// ephemeral port; the actual bound address is logged (and is what integration
/// tests read via `server.server_addr()` after binding `0`).
///
/// Thread-per-request: each incoming request is dispatched to
/// [`router::route`], which maps it onto exactly one reuse signature (or an SSE
/// stream) and back to an HTTP response. The opened [`AppState`] is shared
/// read-only across requests.
pub fn run(registry_root: String, host: &str, port: u16) -> Result<()> {
    let state = AppState::open(&registry_root)?;
    let server = tiny_http::Server::http((host, port))
        .map_err(|e| anyhow::anyhow!("bind ke serve on {host}:{port}: {e}"))?;
    let addr = server
        .server_addr()
        .to_ip()
        .map(|a| a.to_string())
        .unwrap_or_else(|| format!("{host}:{port}"));
    tracing::info!(
        %addr,
        "ke serve listening (NON-AUTHORITATIVE: read/preview/dry-run/resolve/verify + read-only SSE feed)"
    );
    eprintln!("ke serve listening on http://{addr} (non-authoritative preview surface)");

    for request in server.incoming_requests() {
        // Errors writing a response are logged, not fatal — one bad client
        // connection must not take the server down.
        if let Err(err) = router::route(&state, request) {
            tracing::warn!(error = %err, "ke serve request failed");
        }
    }
    Ok(())
}
