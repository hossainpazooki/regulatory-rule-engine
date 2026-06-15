//! Method + path routing for `ke serve` (scaffold): match a `tiny_http::Request`
//! onto exactly one handler, and map handler errors onto HTTP status codes.
//!
//! Error → status mapping (the single mapping site):
//! - [`RegistryError::NotFound`] → 404
//! - [`RegistryError::Ambiguous`] → 409
//! - [`RegistryError::BadHashHex`] / artifact-decode → 400
//! - `ke_compiler::CompileError` / facts-parse error → 422
//! - everything else → 500
//!
//! The `/verify` and `/dry-run`-by-hash handlers read the CANONICAL registry
//! backend in [`AppState`] (G5-1) — never a vendored snapshot. The router itself
//! is authority-free: it can only reach the read/preview handlers.

use super::dto::ErrorResponse;
use super::{handlers, AppState};
use crate::registry::RegistryError;
use serde::Serialize;
use std::io::Cursor;
use tiny_http::{Header, Request, Response};

/// The handler-facing error: a stable kind, a human detail, and the HTTP status
/// the router will emit. Handlers return this; [`route`] turns it into a JSON
/// [`ErrorResponse`] body with the right status.
#[derive(Debug)]
pub struct ServeError {
    pub status: u16,
    pub kind: &'static str,
    pub detail: String,
}

impl ServeError {
    pub fn new(status: u16, kind: &'static str, detail: impl Into<String>) -> Self {
        Self {
            status,
            kind,
            detail: detail.into(),
        }
    }

    /// 400 — malformed request (bad hash hex, undecodable body, missing field).
    pub fn bad_request(detail: impl Into<String>) -> Self {
        Self::new(400, "bad_request", detail)
    }

    /// 422 — a valid request whose payload failed compile / facts parsing.
    pub fn unprocessable(kind: &'static str, detail: impl Into<String>) -> Self {
        Self::new(422, kind, detail)
    }

    /// 500 — an unexpected internal failure.
    pub fn internal(detail: impl Into<String>) -> Self {
        Self::new(500, "internal", detail)
    }
}

impl From<RegistryError> for ServeError {
    /// The canonical registry-error → HTTP status mapping for the serve surface.
    fn from(err: RegistryError) -> Self {
        match err {
            RegistryError::NotFound { .. } => Self::new(404, "not_found", err.to_string()),
            RegistryError::Ambiguous { .. } => Self::new(409, "ambiguous", err.to_string()),
            RegistryError::BadHashHex { .. } => Self::new(400, "bad_hash_hex", err.to_string()),
            RegistryError::ArtifactDecode(_) => Self::new(400, "artifact_decode", err.to_string()),
            other => Self::internal(other.to_string()),
        }
    }
}

/// The handler result alias used throughout the serve module.
pub type ServeResult = Result<Response<Cursor<Vec<u8>>>, ServeError>;

/// Build a `200 application/json` response from any `Serialize` value. The
/// shared success helper for the non-streaming handlers.
pub fn json_response<T: Serialize>(value: &T) -> ServeResult {
    let body = serde_json::to_vec(value)
        .map_err(|e| ServeError::internal(format!("serialize response: {e}")))?;
    let header = Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
        .expect("static content-type header is valid");
    Ok(Response::from_data(body).with_header(header))
}

/// Build the SSE response header set for the live `/events` feed
/// (`Content-Type: text/event-stream`, no-cache, keep-alive). The handler writes
/// `data: <json>\n\n` frames into the long-lived response body.
///
/// Scaffold note: the actual long-lived streaming write loop is filled by the
/// Build phase in [`handlers::events_sse`]; this helper centralizes the headers
/// so the stream is unambiguously an EventSource feed.
pub fn text_event_stream_headers() -> Vec<Header> {
    vec![
        Header::from_bytes(&b"Content-Type"[..], &b"text/event-stream"[..])
            .expect("static content-type header is valid"),
        Header::from_bytes(&b"Cache-Control"[..], &b"no-cache"[..])
            .expect("static cache-control header is valid"),
        Header::from_bytes(&b"Connection"[..], &b"keep-alive"[..])
            .expect("static connection header is valid"),
    ]
}

/// Render a [`ServeError`] into a JSON [`ErrorResponse`] body at its status.
fn error_response(err: &ServeError) -> Response<Cursor<Vec<u8>>> {
    let payload = ErrorResponse {
        error: err.kind.to_string(),
        detail: err.detail.clone(),
    };
    let body = serde_json::to_vec(&payload).unwrap_or_else(|_| b"{\"error\":\"internal\"}".to_vec());
    let header = Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
        .expect("static content-type header is valid");
    Response::from_data(body)
        .with_status_code(err.status)
        .with_header(header)
}

/// Split a request URL into its path and raw query string (`/resolve?hash=ab` →
/// `("/resolve", Some("hash=ab"))`).
fn split_path_query(url: &str) -> (&str, Option<&str>) {
    match url.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (url, None),
    }
}

/// Route one request to its handler and write the response. The `/events` SSE
/// feed is dispatched here too (it consumes the request to take ownership of the
/// long-lived connection). Non-streaming handlers return a [`ServeResult`];
/// errors are rendered to a JSON [`ErrorResponse`] at the mapped status.
///
/// Build-phase TODO: each `handlers::*` call below is a thin adapter over the
/// reuse signatures pinned in [`super`]. SSE (`/events`) is dispatched specially
/// because it owns the connection rather than returning a finished response.
pub fn route(state: &AppState, mut request: Request) -> std::io::Result<()> {
    let method = request.method().clone();
    let url = request.url().to_string();
    let (path, query) = split_path_query(&url);

    use tiny_http::Method::{Get, Post};

    // `/events` is the SSE feed: hand the request to the streaming handler, which
    // takes ownership of the connection and writes frames until the client drops.
    if matches!(method, Get) && path == "/events" {
        return handlers::events_sse(state, request);
    }

    let result: ServeResult = match (&method, path) {
        (Get, "/healthz") => handlers::health(),
        (Get, "/resolve") => handlers::resolve(state, query),
        (Post, "/verify") => match read_body(&mut request) {
            Ok(body) => handlers::verify(state, &body),
            Err(e) => Err(ServeError::bad_request(format!("read body: {e}"))),
        },
        (Post, "/compile/preview") => match read_body(&mut request) {
            Ok(body) => handlers::compile_preview(&body),
            Err(e) => Err(ServeError::bad_request(format!("read body: {e}"))),
        },
        (Post, "/dry-run") => match read_body(&mut request) {
            Ok(body) => handlers::dry_run(state, &body),
            Err(e) => Err(ServeError::bad_request(format!("read body: {e}"))),
        },
        _ => Err(ServeError::new(
            404,
            "not_found",
            format!("no route for {method} {path}"),
        )),
    };

    match result {
        Ok(response) => request.respond(response),
        Err(err) => request.respond(error_response(&err)),
    }
}

/// Read a request body to a `String` in place; the request is kept so the caller
/// can still respond over it.
fn read_body(request: &mut Request) -> Result<String, std::io::Error> {
    // `as_reader()` hands back a `&mut dyn Read`, so `read_to_string` is in
    // scope without importing the trait.
    let mut body = String::new();
    request.as_reader().read_to_string(&mut body)?;
    Ok(body)
}
