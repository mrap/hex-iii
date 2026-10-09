//! Refuse browser-originated requests on the engine's local listeners.
//!
//! The worker manager, stream, and REST listeners bind to loopback and have no
//! credential by default. A web page open in any browser on the same host can
//! still reach loopback, so without this guard a page could open
//! `ws://127.0.0.1:49134`, invoke any function, and register functions over
//! existing ids (hex enginews1009, 2026-10-09).
//!
//! Browsers always send `Origin` on a WebSocket handshake and on cross-origin
//! or non-GET requests, and send `Sec-Fetch-*` metadata on requests. The SDK
//! clients send neither. So any request carrying one of these headers is
//! refused with 403 before routing, which is before any WebSocket upgrade,
//! `workerregistered` frame, or `workers_available` trigger.

use axum::{
    body::Body,
    http::{HeaderMap, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};

const BROWSER_HEADERS: &[&str] = &[
    "origin",
    "sec-fetch-site",
    "sec-fetch-mode",
    "sec-fetch-dest",
];

/// True when the request carries a header only browsers send.
pub fn is_browser_request(headers: &HeaderMap) -> bool {
    BROWSER_HEADERS.iter().any(|h| headers.contains_key(*h))
}

/// The 403 response for a refused browser request. Logged at warn with the
/// path only; header values are not logged.
pub fn refusal(path: &str) -> Response {
    tracing::warn!(path = %path, "refused browser-originated request on engine listener");
    (StatusCode::FORBIDDEN, "browser requests are not accepted").into_response()
}

/// Axum middleware: refuse browser requests, pass everything else through.
pub async fn refuse_browser_origin(req: Request<Body>, next: Next) -> Response {
    if is_browser_request(req.headers()) {
        return refusal(req.uri().path());
    }
    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_each_browser_header() {
        for h in BROWSER_HEADERS {
            let mut headers = HeaderMap::new();
            headers.insert(*h, "x".parse().unwrap());
            assert!(is_browser_request(&headers), "{h}");
        }
    }

    #[test]
    fn sdk_shaped_headers_pass() {
        let mut headers = HeaderMap::new();
        headers.insert("host", "127.0.0.1:49134".parse().unwrap());
        headers.insert("upgrade", "websocket".parse().unwrap());
        headers.insert("sec-websocket-key", "abc".parse().unwrap());
        headers.insert("user-agent", "iii-sdk".parse().unwrap());
        assert!(!is_browser_request(&headers));
    }
}
