//! Browser-shaped requests are refused on every engine listener.
//!
//! Incident (hex, 2026-10-09, enginews1009): the worker manager upgraded any
//! WebSocket with no check, so a web page open in a browser on the same host
//! could connect to ws://127.0.0.1:49134, list and invoke every function,
//! and register functions over existing ids. A live probe with
//! `Origin: https://evil.example` got `workerregistered` and a full function
//! list. Browsers always send `Origin` on a WebSocket handshake and
//! `Sec-Fetch-*` on other requests; the engine's own SDK clients send neither.
//!
//! These tests boot each listener on a random port and assert:
//!   1. A request with `Origin` or `Sec-Fetch-Site` gets 403 before any
//!      upgrade, so no `workerregistered` frame and no registry entry.
//!   2. A plain SDK-shaped client still connects (no regression).

use std::sync::Arc;
use std::time::Duration;

use iii::engine::Engine;
use iii::workers::traits::Worker;
use iii::workers::worker::WorkerManager;
use serde_json::json;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::{self, client::IntoClientRequest, http::StatusCode};

async fn free_port() -> u16 {
    let probe = TcpListener::bind("127.0.0.1:0").await.expect("bind probe");
    let port = probe.local_addr().expect("local_addr").port();
    drop(probe);
    port
}

async fn spawn_worker_manager() -> (u16, Arc<Engine>) {
    let port = free_port().await;
    let engine = Arc::new(Engine::new());
    let worker = WorkerManager::create(
        engine.clone(),
        Some(json!({ "port": port, "host": "127.0.0.1" })),
    )
    .await
    .expect("create WorkerManager");
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    worker
        .start_background_tasks(shutdown_rx, shutdown_tx)
        .await
        .expect("start WorkerManager");
    tokio::time::sleep(Duration::from_millis(150)).await;
    (port, engine)
}

/// Opens a WebSocket with extra headers; returns the HTTP status on refusal.
async fn ws_status(url: &str, headers: &[(&str, &str)]) -> Result<(), StatusCode> {
    let mut req = url.into_client_request().expect("request");
    for (k, v) in headers {
        req.headers_mut().insert(
            tungstenite::http::HeaderName::from_bytes(k.as_bytes()).unwrap(),
            v.parse().unwrap(),
        );
    }
    match tokio_tungstenite::connect_async(req).await {
        Ok(_) => Ok(()),
        Err(tungstenite::Error::Http(resp)) => Err(resp.status()),
        Err(e) => panic!("unexpected connect error: {e}"),
    }
}

#[tokio::test]
async fn worker_manager_refuses_origin_before_registration() {
    let (port, engine) = spawn_worker_manager().await;
    for path in ["/", "/otel"] {
        let url = format!("ws://127.0.0.1:{port}{path}");
        assert_eq!(
            ws_status(&url, &[("origin", "https://evil.example")]).await,
            Err(StatusCode::FORBIDDEN),
            "{path} must refuse a browser Origin"
        );
        assert_eq!(
            ws_status(&url, &[("sec-fetch-site", "cross-site")]).await,
            Err(StatusCode::FORBIDDEN),
            "{path} must refuse a Sec-Fetch request"
        );
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        engine.worker_registry.list_workers().len(),
        0,
        "a refused browser must not register a worker"
    );
}

#[tokio::test]
async fn worker_manager_refuses_origin_on_channel_route() {
    let (port, _engine) = spawn_worker_manager().await;
    let url = format!("ws://127.0.0.1:{port}/ws/channels/any");
    assert_eq!(
        ws_status(&url, &[("origin", "null")]).await,
        Err(StatusCode::FORBIDDEN)
    );
}

#[tokio::test]
async fn worker_manager_still_accepts_sdk_client() {
    let (port, engine) = spawn_worker_manager().await;
    let url = format!("ws://127.0.0.1:{port}/");
    let (_ws, _) = tokio_tungstenite::connect_async(url.as_str())
        .await
        .expect("an SDK-shaped client (no Origin) must still connect");
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(engine.worker_registry.list_workers().len(), 1);
}

async fn start(worker: Box<dyn Worker>) {
    worker.initialize().await.expect("initialize");
    // Keep the sender that feeds `shutdown_rx` alive for the whole test: a
    // dropped sender reads as shutdown and the server stops at once.
    let (keep_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let (unused_tx, _unused_rx) = tokio::sync::watch::channel(false);
    worker
        .start_background_tasks(shutdown_rx, unused_tx)
        .await
        .expect("start");
    tokio::time::sleep(Duration::from_millis(200)).await;
    std::mem::forget(keep_tx);
    std::mem::forget(worker);
}

#[tokio::test]
async fn stream_refuses_origin_and_accepts_plain_client() {
    let port = free_port().await;
    let engine = Arc::new(Engine::new());
    let worker = iii::workers::stream::StreamWorker::create(
        engine,
        Some(json!({ "port": port, "host": "127.0.0.1" })),
    )
    .await
    .expect("create StreamWorker");
    start(worker).await;
    let url = format!("ws://127.0.0.1:{port}/");
    assert_eq!(
        ws_status(&url, &[("origin", "https://evil.example")]).await,
        Err(StatusCode::FORBIDDEN)
    );
    assert_eq!(
        ws_status(&url, &[]).await,
        Ok(()),
        "plain client still connects"
    );
}

#[tokio::test]
async fn rest_api_refuses_browser_requests() {
    let port = free_port().await;
    let engine = Arc::new(Engine::new());
    let worker = iii::workers::rest_api::HttpWorker::create(
        engine,
        Some(json!({ "port": port, "host": "127.0.0.1" })),
    )
    .await
    .expect("create HttpWorker");
    start(worker).await;
    let client = reqwest::Client::new();
    for (k, v) in [
        ("origin", "https://evil.example"),
        ("sec-fetch-site", "cross-site"),
    ] {
        let resp = client
            .get(format!("http://127.0.0.1:{port}/anything"))
            .header(k, v)
            .send()
            .await
            .expect("request");
        assert_eq!(resp.status().as_u16(), 403, "{k} must be refused");
        assert!(
            resp.headers().get("access-control-allow-origin").is_none(),
            "no CORS grant on a refused request"
        );
    }
    let plain = client
        .get(format!("http://127.0.0.1:{port}/anything"))
        .send()
        .await
        .expect("request");
    assert_ne!(plain.status().as_u16(), 403, "plain client is not refused");
}
