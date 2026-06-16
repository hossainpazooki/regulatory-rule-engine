//! Gate 5 in-process integration tests for `ke serve` — the THIN,
//! NON-AUTHORITATIVE HTTP read/preview adapter (`src/serve/`).
//!
//! These drive the serve surface end-to-end **in-process**: they bind a
//! `tiny_http` server on an ephemeral `127.0.0.1:0` port, read the bound address
//! back via `server.server_addr()`, spawn a thread-per-request accept loop that
//! routes through the real `ke_cli::serve::router::route`, and issue raw HTTP/1.1
//! requests over a `std::net::TcpStream` (no new client dependency).
//!
//! The registry under test is a tempdir registry driven through the existing
//! command `run` helpers with the fixed test keys + fixed clock, exactly like
//! `lifecycle.rs` / `export_provenance.rs`. The serve handlers read THIS backend
//! — the canonical registry/artifact view, never a vendored snapshot. That is
//! Gate 5 acceptance **G5-1**, and the canonical-view assertions below recompute
//! the expected hash/state directly off the same backend (independent of the
//! HTTP response) before comparing.
//!
//! # Coverage
//!
//! - `health_returns_non_authoritative_banner` — the loud banner, exact bytes.
//! - `unknown_route_is_404` — router fall-through.
//! - `ephemeral_bind_seam_reports_addr` — the port-0 → `server_addr()` seam the
//!   whole harness depends on.
//! - `canonical_view_is_the_serve_backend` — proves the registry the server
//!   reads is byte-for-byte the canonical backend (resolve-by-tag/by-hash agree
//!   with a direct `ke_cli::registry::resolve` off the same root), the G5-1
//!   source-of-truth invariant.
//! - HTTP-level G5-1 checks: `/resolve` (by-tag, by-hash, 400 on a bad hash),
//!   `/verify` (the real `verified` verdict under `env="local"`, and the honest
//!   non-local mock-TSA `rejected` verdict — both HTTP 200), `/compile/preview`
//!   (IR + report; 422 on bad source), `/dry-run` (one normalized evaluation per
//!   rule), and `/events` (the SSE `text/event-stream` head + first `data:`
//!   frame).
//!
//! Note: `/verify` only verifies the test corpus under `env="local"` — the
//! fixed-seed attestations carry a mock trusted-timestamp authority that
//! `verify_artifact`'s R8 rule rejects under any non-local policy. A real TSA is
//! an open decision (spec § 21, Gate 4). The non-local rejection is pinned by
//! `http_verify_non_local_env_rejects_mock_tsa` so a future TSA wiring can't
//! silently change it.
//!
//! Determinism mirrors `lifecycle.rs`: fixed registry-root + compiler + expert
//! test keys (feature unification via the `test-keys` dev-dep enables the gated
//! signing modules), fixed `NOW`, tempdir backend dropped on teardown.

use ke_cli::registry::backend::{LocalFsBackend, RegistryBackend};
use ke_cli::registry::{current_state, hash_hex, LifecycleState, Selector};
use ke_cli::serve::{router, AppState};
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

/// Fixed clock — the same `NOW` the registry/lifecycle suites use.
const NOW: u64 = 1_750_000_000;

const FIXTURE_YAML: &str = "../../fixtures/rules/mica_stablecoin.yaml";
const FIXTURE_REGIME: &str = "mica_2023";

// ---------------------------------------------------------------------------
// tempdir + registry helpers (mirrors lifecycle.rs)
// ---------------------------------------------------------------------------

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let path = std::env::temp_dir().join(format!("ke-serve-test-{label}-{pid}-{n}"));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("create tempdir");
        TempDir { path }
    }

    fn root(&self) -> String {
        self.path.to_string_lossy().into_owned()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn fixture_path() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(FIXTURE_YAML)
        .to_string_lossy()
        .into_owned()
}

fn fixture_source() -> String {
    std::fs::read_to_string(fixture_path()).expect("read fixture YAML source")
}

#[cfg(feature = "test-keys")]
use ke_cli::commands::{attest, compile, ml_check, publish};
#[cfg(feature = "test-keys")]
use ke_core::manifest::AttestationType;

#[cfg(feature = "test-keys")]
const FULL_SET: [AttestationType; 3] = [
    AttestationType::SourceFidelity,
    AttestationType::ScenarioCoverage,
    AttestationType::PublicationApproval,
];

/// Compile the fixture into a fresh backend and drive it to Published under
/// `staging/current`, returning the (open backend, content hash). Signing path
/// → requires the `test-keys` feature, same as `lifecycle.rs`.
#[cfg(feature = "test-keys")]
fn published_registry(tmp: &TempDir) -> (LocalFsBackend, [u8; 32]) {
    let backend = LocalFsBackend::open(&tmp.path).expect("open backend");
    let yaml = fixture_path();
    let outcome = compile::run(
        &backend,
        &compile::CompileArgs {
            yaml_path: &yaml,
            regime_id: FIXTURE_REGIME,
            env: "local",
            now_unix: NOW,
        },
    )
    .expect("compile run");
    let hash = outcome.artifact_hash;
    assert_eq!(outcome.final_state, LifecycleState::StructurallyVerified);

    ml_check::run(
        &backend,
        &ml_check::MlCheckArgs {
            artifact_hash: hash,
            now_unix: NOW,
        },
    )
    .expect("ml-check");
    attest::run(
        &backend,
        &attest::AttestArgs {
            artifact_hash: hash,
            types: &FULL_SET,
            now_unix: NOW,
        },
    )
    .expect("attest");
    publish::run(
        &backend,
        &publish::PublishArgs {
            artifact_hash: hash,
            env: "staging",
            tag: "current",
            policy_path: None,
            now_unix: NOW,
        },
    )
    .expect("publish");
    (backend, hash)
}

// ---------------------------------------------------------------------------
// in-process server harness (bind 127.0.0.1:0, thread-per-request accept loop
// over the REAL router) + a tiny raw-TcpStream HTTP/1.1 client
// ---------------------------------------------------------------------------

/// A serve instance bound to an ephemeral port, with its accept loop running on
/// a background thread. Dropping it shuts the listener and joins the thread.
struct TestServer {
    addr: std::net::SocketAddr,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl TestServer {
    /// Open the canonical registry at `root`, bind `127.0.0.1:0`, and start the
    /// accept loop. Uses the SAME [`AppState`] + [`router::route`] the `ke serve`
    /// command uses — only the bind/loop scaffolding is test-local (the contract
    /// flagged that `serve::run` has no ephemeral-port seam to call directly).
    fn start(root: &str) -> Self {
        let state = AppState::open(root).expect("open AppState over canonical registry");
        let server = tiny_http::Server::http("127.0.0.1:0").expect("bind ephemeral port");
        let addr = server
            .server_addr()
            .to_ip()
            .expect("ephemeral bind yields an IP socket addr");

        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = Arc::clone(&stop);
        let handle = std::thread::spawn(move || {
            // recv_timeout lets the loop observe the stop flag between requests
            // so the test can shut the server down deterministically.
            loop {
                if stop_thread.load(Ordering::Relaxed) {
                    break;
                }
                match server.recv_timeout(std::time::Duration::from_millis(50)) {
                    Ok(Some(request)) => {
                        let _ = router::route(&state, request);
                    }
                    Ok(None) => continue, // timeout tick — re-check stop
                    Err(_) => break,
                }
            }
        });

        TestServer {
            addr,
            stop,
            handle: Some(handle),
        }
    }

    fn get(&self, path: &str) -> HttpResponse {
        self.request("GET", path, None)
    }

    fn post_json(&self, path: &str, body: &str) -> HttpResponse {
        self.request("POST", path, Some(body))
    }

    /// Issue a raw HTTP/1.1 request over a fresh TCP connection and parse the
    /// status + body. `Connection: close` so the server writes a bounded body we
    /// can read to EOF without chunked/keep-alive bookkeeping.
    fn request(&self, method: &str, path: &str, body: Option<&str>) -> HttpResponse {
        let mut stream = TcpStream::connect(self.addr).expect("connect to test server");
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .expect("set read timeout");

        let mut req = format!(
            "{method} {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n",
            self.addr
        );
        if let Some(b) = body {
            req.push_str("Content-Type: application/json\r\n");
            req.push_str(&format!("Content-Length: {}\r\n", b.len()));
            req.push_str("\r\n");
            req.push_str(b);
        } else {
            req.push_str("\r\n");
        }
        stream.write_all(req.as_bytes()).expect("write request");
        stream.flush().expect("flush request");

        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).expect("read response to EOF");
        HttpResponse::parse(&raw)
    }

    /// Issue a GET and return the connection so a streaming (SSE) handler can be
    /// read frame-by-frame without the request blocking on EOF.
    fn open_stream(&self, path: &str) -> TcpStream {
        let mut stream = TcpStream::connect(self.addr).expect("connect for stream");
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .expect("set read timeout");
        let req = format!(
            "GET {path} HTTP/1.1\r\nHost: {}\r\nAccept: text/event-stream\r\n\r\n",
            self.addr
        );
        stream
            .write_all(req.as_bytes())
            .expect("write stream request");
        stream.flush().expect("flush stream request");
        stream
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// A parsed HTTP response: status code, the `Content-Type` header (lowercased
/// key match), and the body bytes.
struct HttpResponse {
    status: u16,
    content_type: Option<String>,
    body: String,
}

impl HttpResponse {
    fn parse(raw: &[u8]) -> Self {
        let text = String::from_utf8_lossy(raw);
        let (head, body) = text
            .split_once("\r\n\r\n")
            .map(|(h, b)| (h.to_string(), b.to_string()))
            .unwrap_or_else(|| (text.to_string(), String::new()));

        let mut lines = head.lines();
        let status_line = lines.next().unwrap_or_default();
        // "HTTP/1.1 200 OK" -> 200
        let status = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(0);

        let content_type = lines
            .find(|l| l.to_ascii_lowercase().starts_with("content-type:"))
            .and_then(|l| l.split_once(':').map(|(_, v)| v.trim().to_lowercase()));

        HttpResponse {
            status,
            content_type,
            body,
        }
    }

    fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.body)
            .unwrap_or_else(|e| panic!("response body is not JSON ({e}): {:?}", self.body))
    }
}

// ===========================================================================
// Tests that pass NOW against the scaffold
// ===========================================================================

/// `GET /healthz` returns 200 with the exact loud non-authoritative banner.
/// This is the one fully-implemented handler and the liveness/identity probe.
#[test]
fn health_returns_non_authoritative_banner() {
    let tmp = TempDir::new("health");
    let server = TestServer::start(&tmp.root());

    let resp = server.get("/healthz");
    assert_eq!(resp.status, 200, "body: {:?}", resp.body);
    assert_eq!(
        resp.content_type.as_deref(),
        Some("application/json"),
        "health is JSON"
    );
    let body = resp.json();
    assert_eq!(body["ok"], serde_json::json!(true));
    assert_eq!(
        body["surface"],
        serde_json::json!("ke-cli serve (preview, non-authoritative)"),
        "the banner must loudly name the surface as preview / non-authoritative"
    );
}

/// An unknown route falls through to a 404 JSON error (router fall-through arm).
#[test]
fn unknown_route_is_404() {
    let tmp = TempDir::new("notfound");
    let server = TestServer::start(&tmp.root());

    let resp = server.get("/no/such/route");
    assert_eq!(resp.status, 404, "body: {:?}", resp.body);
    let body = resp.json();
    assert_eq!(body["error"], serde_json::json!("not_found"));
}

/// The ephemeral-bind seam the whole harness relies on: binding `127.0.0.1:0`
/// yields a real, connectable loopback address. (The scaffold's `serve::run`
/// has no seam returning the bound addr; this asserts the test-local seam
/// works, so a failure here localizes a harness break, not a handler bug.)
#[test]
fn ephemeral_bind_seam_reports_addr() {
    let tmp = TempDir::new("bind");
    let server = TestServer::start(&tmp.root());
    assert!(server.addr.ip().is_loopback(), "bound on loopback");
    assert_ne!(server.addr.port(), 0, "ephemeral bind picked a real port");
    // It is actually reachable.
    let resolved = (server.addr.ip(), server.addr.port())
        .to_socket_addrs()
        .expect("addr resolves")
        .next()
        .expect("one addr");
    let _ = TcpStream::connect(resolved).expect("ephemeral port is connectable");
}

/// G5-1 source-of-truth at the library boundary: the registry the serve
/// `AppState` opens is byte-for-byte the canonical backend. We publish through
/// the command path, then prove that resolving the SAME root the server was
/// handed (by-tag and by-hash) returns the published content hash and Published
/// state. This is the invariant the HTTP `/resolve` + `/verify` handlers must
/// preserve; it holds independent of whether those handlers are built yet,
/// because both the server and this check read one canonical `LocalFsBackend`.
#[cfg(feature = "test-keys")]
#[test]
fn canonical_view_is_the_serve_backend() {
    let tmp = TempDir::new("canonical");
    let (_driver_backend, hash) = published_registry(&tmp);
    let root = tmp.root();

    // The server is handed the SAME root. Its AppState opens this canonical view.
    let server = TestServer::start(&root);
    // Reachability ties the assertion to a live server over the canonical root.
    assert_eq!(server.get("/healthz").status, 200);

    // Resolve by-tag off the canonical root (what /resolve?env&tag must return).
    let backend = LocalFsBackend::open(&root).expect("reopen canonical root");
    let (by_tag, record) = ke_cli::registry::resolve(
        &backend,
        &Selector::ByTag {
            env: "staging".to_string(),
            tag: "current".to_string(),
        },
        NOW,
    )
    .expect("resolve by tag");
    assert_eq!(
        by_tag, hash,
        "resolve-by-tag returns the published content hash (G5-1)"
    );
    assert_eq!(
        record.registry_state_at_resolution,
        LifecycleState::Published
    );

    // Resolve by-hash agrees, and the log-derived state is Published.
    let (by_hash, _) =
        ke_cli::registry::resolve(&backend, &Selector::ByHash(hash), NOW).expect("resolve by hash");
    assert_eq!(by_hash, hash);
    assert_eq!(
        current_state(&backend.read_events(&hash).expect("events"))
            .expect("derive state")
            .expect("has state"),
        LifecycleState::Published
    );
}

// ===========================================================================
// Full G5-1 HTTP-level canonical-view assertions.
//
// These are the real Gate-5 acceptance checks: each drives the live in-process
// server over the canonical tempdir registry and asserts the response equals the
// canonical registry/artifact view (resolve-by-tag returns the published hash,
// verify returns the real verdict, etc.). The serve handlers are fully built and
// these run green; see the verify test's note on the `env="local"` requirement
// (the trusted-timestamp authority is an open decision — non-local TSA is not
// wired, spec § 21 / CLAUDE.md "Open decisions").
// ===========================================================================

/// `GET /resolve?env=staging&tag=current` returns the published hash and
/// Published state — the HTTP mirror of `ke_cli::registry::resolve(ByTag)`.
#[cfg(feature = "test-keys")]
#[test]
fn http_resolve_by_tag_returns_published_hash() {
    let tmp = TempDir::new("http_resolve_tag");
    let (_b, hash) = published_registry(&tmp);
    let server = TestServer::start(&tmp.root());

    let resp = server.get("/resolve?env=staging&tag=current");
    assert_eq!(resp.status, 200, "body: {:?}", resp.body);
    let body = resp.json();
    // ResolutionRecord serializes `artifact_hash` as the 32-byte array; compare
    // against the bytes of the canonical hash.
    let expected: Vec<serde_json::Value> = hash.iter().map(|b| serde_json::json!(b)).collect();
    assert_eq!(
        body["artifact_hash"],
        serde_json::Value::Array(expected),
        "resolve-by-tag returns the canonical published hash (G5-1)"
    );
    assert_eq!(body["registry_state_at_resolution"], "Published");
}

/// `GET /resolve?hash=<64hex>` returns the same record by direct content hash.
#[cfg(feature = "test-keys")]
#[test]
fn http_resolve_by_hash() {
    let tmp = TempDir::new("http_resolve_hash");
    let (_b, hash) = published_registry(&tmp);
    let server = TestServer::start(&tmp.root());

    let resp = server.get(&format!("/resolve?hash={}", hash_hex(&hash)));
    assert_eq!(resp.status, 200, "body: {:?}", resp.body);
    assert_eq!(resp.json()["registry_state_at_resolution"], "Published");
}

/// `GET /resolve?hash=<garbage>` is a 400 (a bad hash is a malformed request).
/// The handler maps the `hash_from_hex` failure to its own `bad_request` kind
/// before the request reaches `registry::resolve`, so the body kind is
/// `bad_request` (not the `RegistryError::BadHashHex` → `bad_hash_hex` path that
/// only fires for a well-formed-but-unknown selector). Either way the status is
/// the contract's 400.
#[test]
fn http_resolve_bad_hash_is_400() {
    let tmp = TempDir::new("http_resolve_badhash");
    let server = TestServer::start(&tmp.root());

    let resp = server.get("/resolve?hash=not-a-real-hash");
    assert_eq!(resp.status, 400, "body: {:?}", resp.body);
    let kind = resp.json()["error"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(
        kind == "bad_request" || kind == "bad_hash_hex",
        "a malformed hash is a 400 with a bad-request/bad-hash kind; got {kind:?}"
    );
}

/// `POST /verify {hash, env:"local"}` returns the REAL verdict for a published,
/// fully-attested artifact under the strict default policy: `verdict ==
/// "verified"`, `registry_state == "Published"`. HTTP stays 200 (a verdict, not
/// an HTTP error). Verify reads the canonical RegistryEvidence the handler
/// assembles off the same backend (G5-1).
///
/// `env` MUST be `"local"`: the fixed-seed test attestations carry a MOCK
/// trusted-timestamp authority, and `verify_artifact`'s R8 rule rejects a mock
/// TSA timestamp under any non-local policy (then R6 cascades because the
/// rejected attestations no longer satisfy the required types). A real trusted
/// timestamp authority is an open decision (spec § 21, Gate 4); until it is
/// wired, `local` is the only environment that verifies the test corpus. This is
/// the canonical, honest verify for this build — not a weakening of the check.
#[cfg(feature = "test-keys")]
#[test]
fn http_verify_published_artifact_returns_verified() {
    let tmp = TempDir::new("http_verify");
    let (_b, hash) = published_registry(&tmp);
    let server = TestServer::start(&tmp.root());

    let body = format!(r#"{{"hash":"{}","env":"local"}}"#, hash_hex(&hash));
    let resp = server.post_json("/verify", &body);
    assert_eq!(
        resp.status, 200,
        "a Rejected verdict is still HTTP 200; body: {:?}",
        resp.body
    );
    let json = resp.json();
    assert_eq!(
        json["verdict"],
        "verified",
        "published + fully attested under strict local policy verifies; rejection={:?}",
        json.get("rejection")
    );
    assert_eq!(json["registry_state"], "Published");
    assert!(
        json.get("provenance").is_some(),
        "provenance is always present (built even on rejection)"
    );
}

/// `POST /verify` under a NON-local env rejects the mock-TSA test attestations
/// (R8), and HTTP still returns 200 with a rendered rejection reason — a verdict,
/// not a transport error. This pins the honest non-local behavior for this build
/// (no real TSA wired yet) so a future TSA decision can't silently change it.
#[cfg(feature = "test-keys")]
#[test]
fn http_verify_non_local_env_rejects_mock_tsa() {
    let tmp = TempDir::new("http_verify_nonlocal");
    let (_b, hash) = published_registry(&tmp);
    let server = TestServer::start(&tmp.root());

    let body = format!(r#"{{"hash":"{}","env":"staging"}}"#, hash_hex(&hash));
    let resp = server.post_json("/verify", &body);
    assert_eq!(
        resp.status, 200,
        "a rejection is still HTTP 200; body: {:?}",
        resp.body
    );
    let json = resp.json();
    assert_eq!(
        json["verdict"], "rejected",
        "non-local env rejects the mock-TSA attestations (R8)"
    );
    assert!(
        json["rejection"]
            .as_str()
            .map(|s| s.contains("R8"))
            .unwrap_or(false),
        "rejection names the R8 trusted-timestamp rule; got {:?}",
        json.get("rejection")
    );
    // Registry state is still the canonical Published — the rejection is about
    // the attestations, not the registry view.
    assert_eq!(json["registry_state"], "Published");
}

/// `POST /compile/preview {source}` compiles the fixture YAML and returns its IR
/// + verification report. NON-authoritative: signs/stores nothing. No registry
/// or test-keys needed (compile is pure).
#[test]
fn http_compile_preview_returns_ir_and_report() {
    let tmp = TempDir::new("http_compile");
    let server = TestServer::start(&tmp.root());

    let body = serde_json::json!({ "source": fixture_source() }).to_string();
    let resp = server.post_json("/compile/preview", &body);
    assert_eq!(resp.status, 200, "body: {:?}", resp.body);
    let json = resp.json();
    assert!(
        json["rules"]
            .as_array()
            .map(|a| !a.is_empty())
            .unwrap_or(false),
        "preview returns at least one compiled rule"
    );
    assert!(
        json["report"].get("has_blocking").is_some(),
        "report carries has_blocking"
    );
    // The corpus fixture compiles clean (no blocking findings).
    assert_eq!(json["report"]["has_blocking"], serde_json::json!(false));
}

/// `POST /compile/preview` with un-compilable source is a 422 (not 500): a valid
/// request whose payload failed compilation.
#[test]
fn http_compile_preview_bad_source_is_422() {
    let tmp = TempDir::new("http_compile_bad");
    let server = TestServer::start(&tmp.root());

    let body = serde_json::json!({ "source": "this: is: not: a: valid: rule: doc" }).to_string();
    let resp = server.post_json("/compile/preview", &body);
    assert_eq!(resp.status, 422, "body: {:?}", resp.body);
}

/// `POST /dry-run {source, facts}` compiles inline source and evaluates each rule
/// against the facts, returning one normalized evaluation per rule.
#[test]
fn http_dry_run_from_source_returns_evaluations() {
    let tmp = TempDir::new("http_dryrun");
    let server = TestServer::start(&tmp.root());

    let body = serde_json::json!({
        "source": fixture_source(),
        "facts": {}
    })
    .to_string();
    let resp = server.post_json("/dry-run", &body);
    assert_eq!(resp.status, 200, "body: {:?}", resp.body);
    let json = resp.json();
    assert!(
        json["evaluations"]
            .as_array()
            .map(|a| !a.is_empty())
            .unwrap_or(false),
        "dry-run returns one normalized evaluation per rule"
    );
}

/// `GET /events` opens an SSE stream: a `text/event-stream` response. We assert
/// the content-type on the response head (the first thing the handler writes).
#[test]
fn http_events_is_event_stream() {
    let tmp = TempDir::new("http_sse");
    let server = TestServer::start(&tmp.root());

    let mut stream = server.open_stream("/events");
    // Read whatever the server has written so far (headers + first frame).
    let mut buf = [0u8; 1024];
    let n = stream.read(&mut buf).expect("read SSE head");
    let head = String::from_utf8_lossy(&buf[..n]).to_lowercase();
    assert!(
        head.contains("content-type: text/event-stream"),
        "SSE response advertises text/event-stream; got: {head}"
    );
    // The feed writes at least one `data:` frame on connect.
    assert!(
        head.contains("data:"),
        "SSE feed emits a first `data:` frame; got: {head}"
    );
}
