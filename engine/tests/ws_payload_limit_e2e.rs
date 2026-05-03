//! End-to-end tests for the WebSocket payload-size limit and the
//! `invocation_failed_payload_too_large` error code contract.
//!
//! Background: the Python SDK was tearing the WS connection at ~1 MiB
//! because the `websockets` library defaults `max_size` to 2^20. The
//! engine's recv loop caught the resulting tungstenite error, dropped it
//! silently, and `cleanup_worker` halted every in-flight invocation on
//! that worker with the generic `invocation_stopped` code. Operators
//! had no way to tell oversize disconnects apart from idle disconnects,
//! shutdowns, or panics.
//!
//! These tests pin the new contract:
//!   * The engine logs WS recv errors at WARN with peer/worker_id/error.
//!   * Oversize disconnects surface as `invocation_failed_payload_too_large`
//!     on every in-flight invocation, not the legacy `invocation_stopped`.
//!   * Clean disconnects (Close frame) keep emitting `invocation_stopped`.
//!   * `WorkerManagerConfig::max_message_size` defaults to 16 MiB and is
//!     honored end-to-end via axum's `WebSocketUpgrade::max_message_size`.

use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use iii::engine::Engine;
use iii::workers::traits::Worker;
use iii::workers::worker::WorkerManager;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use uuid::Uuid;

/// Boots a `WorkerManager` on a random port with the supplied JSON config.
/// Returns (port, engine handle).
async fn spawn_engine_with_config(extra: Value) -> (u16, Arc<Engine>) {
    iii::workers::observability::metrics::ensure_default_meter();

    let probe = TcpListener::bind("127.0.0.1:0").await.expect("bind probe");
    let port = probe.local_addr().expect("local_addr").port();
    drop(probe);

    let engine = Arc::new(Engine::new());
    let mut config = json!({ "port": port, "host": "127.0.0.1" });
    if let Value::Object(extra_obj) = extra {
        let merged = config.as_object_mut().unwrap();
        for (k, v) in extra_obj {
            merged.insert(k, v);
        }
    }
    let worker = WorkerManager::create(engine.clone(), Some(config))
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

async fn spawn_engine_with_max_message_size(max_message_size: usize) -> (u16, Arc<Engine>) {
    spawn_engine_with_config(json!({ "max_message_size": max_message_size })).await
}

/// Connects a fake worker, registers a single deferred function, returns
/// the (open) socket and the function id once the engine has acknowledged
/// the registration via `WorkerRegistered` and the registration round-trip
/// has settled.
async fn connect_and_register(
    port: u16,
    function_id: &str,
) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>> {
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{}/", port))
        .await
        .expect("connect to /");

    // Drain `WorkerRegistered` so we know the registry insert happened.
    tokio::time::timeout(Duration::from_millis(500), async {
        loop {
            match ws.next().await {
                Some(Ok(WsMessage::Text(text))) => {
                    if text.contains("workerregistered") {
                        break;
                    }
                }
                Some(Ok(_)) => {}
                Some(Err(e)) => panic!("ws error before WorkerRegistered: {e:?}"),
                None => panic!("connection closed before WorkerRegistered"),
            }
        }
    })
    .await
    .expect("WorkerRegistered within 500ms");

    let register = json!({
        "type": "registerfunction",
        "id": function_id,
        "request_format": null,
        "response_format": null,
    });
    ws.send(WsMessage::Text(register.to_string().into()))
        .await
        .expect("send RegisterFunction");

    // Give the engine a moment to register the function.
    tokio::time::sleep(Duration::from_millis(100)).await;

    ws
}

#[tokio::test]
async fn test_oversize_message_emits_payload_too_large_error_code() {
    let (port, engine) = spawn_engine_with_max_message_size(64 * 1024).await;
    let mut ws = connect_and_register(port, "noop").await;

    // Spawn the invocation as a background task so we can drive the WS
    // side independently and observe the resolved error code.
    let engine_clone = engine.clone();
    let invocation_handle = tokio::spawn(async move {
        use iii::engine::EngineTrait;
        engine_clone.call("noop", json!({})).await
    });

    // Wait until the engine has dispatched InvokeFunction to the worker.
    let invoked = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            match ws.next().await {
                Some(Ok(WsMessage::Text(text))) => {
                    if text.contains("invokefunction") {
                        return text.to_string();
                    }
                }
                Some(Ok(_)) => continue,
                Some(Err(e)) => panic!("ws error before InvokeFunction: {e:?}"),
                None => panic!("ws closed before InvokeFunction"),
            }
        }
    })
    .await
    .expect("InvokeFunction within 2s");
    assert!(invoked.contains("noop"), "should target noop function");

    // Send an oversized text frame from the client to the engine. The
    // engine has `max_message_size = 64 * 1024`, so 200 KiB blows past it.
    let oversized = "x".repeat(200 * 1024);
    let _ = ws.send(WsMessage::Text(oversized.into())).await;

    let result = tokio::time::timeout(Duration::from_secs(5), invocation_handle)
        .await
        .expect("invocation resolves within 5s")
        .expect("invocation task didn't panic");

    let err = result.expect_err("oversize disconnect must fail the invocation");
    assert_eq!(
        err.code, "invocation_failed_payload_too_large",
        "expected payload-too-large code, got {err:?}"
    );
    let _ = engine.worker_registry.list_workers();
}

#[tokio::test]
async fn test_normal_disconnect_still_emits_invocation_stopped() {
    let (port, engine) = spawn_engine_with_max_message_size(64 * 1024).await;
    let mut ws = connect_and_register(port, "noop_close").await;

    let engine_clone = engine.clone();
    let invocation_handle = tokio::spawn(async move {
        use iii::engine::EngineTrait;
        engine_clone.call("noop_close", json!({})).await
    });

    // Drain InvokeFunction.
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            match ws.next().await {
                Some(Ok(WsMessage::Text(text))) => {
                    if text.contains("invokefunction") {
                        return;
                    }
                }
                Some(Ok(_)) => continue,
                Some(Err(_)) | None => return,
            }
        }
    })
    .await
    .expect("InvokeFunction within 2s");

    // Clean close — engine must keep emitting the legacy
    // `invocation_stopped` code so existing operators don't see code
    // churn for graceful shutdowns.
    let _ = ws.close(None).await;

    let result = tokio::time::timeout(Duration::from_secs(5), invocation_handle)
        .await
        .expect("invocation resolves within 5s")
        .expect("invocation task didn't panic");

    let err = result.expect_err("clean close still halts the invocation");
    assert_eq!(
        err.code, "invocation_stopped",
        "graceful close must keep emitting invocation_stopped, got {err:?}"
    );
    let _ = engine.worker_registry.list_workers();
}

#[tokio::test]
async fn test_engine_rejects_message_above_configured_limit() {
    // Phase 2 contract: the limit comes from `WorkerManagerConfig::max_message_size`.
    let (port, engine) = spawn_engine_with_max_message_size(1024).await;
    let mut ws = connect_and_register(port, "tiny_limit").await;

    let engine_clone = engine.clone();
    let invocation_handle = tokio::spawn(async move {
        use iii::engine::EngineTrait;
        engine_clone.call("tiny_limit", json!({})).await
    });

    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            match ws.next().await {
                Some(Ok(WsMessage::Text(text))) => {
                    if text.contains("invokefunction") {
                        return;
                    }
                }
                Some(Ok(_)) => continue,
                Some(Err(_)) | None => return,
            }
        }
    })
    .await
    .expect("InvokeFunction within 2s");

    // 2 KiB > 1 KiB limit.
    let oversized = "y".repeat(2048);
    let _ = ws.send(WsMessage::Text(oversized.into())).await;

    let result = tokio::time::timeout(Duration::from_secs(5), invocation_handle)
        .await
        .expect("invocation resolves within 5s")
        .expect("invocation task didn't panic");

    let err = result.expect_err("oversize disconnect must fail the invocation");
    assert_eq!(err.code, "invocation_failed_payload_too_large");
    let _ = engine.worker_registry.list_workers();
}

#[tokio::test]
async fn test_engine_accepts_message_at_limit() {
    // 1 MiB limit, 1 MiB - 1 KiB payload — must round-trip cleanly with
    // no disconnect. We send an `invocation_result` for an invocation we
    // first trigger via `engine.call`, and assert the call returns Ok.
    let (port, engine) = spawn_engine_with_max_message_size(1024 * 1024).await;
    let mut ws = connect_and_register(port, "echo").await;

    let engine_clone = engine.clone();
    let invocation_handle = tokio::spawn(async move {
        use iii::engine::EngineTrait;
        engine_clone.call("echo", json!({})).await
    });

    let invocation_id: Uuid = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Some(Ok(WsMessage::Text(text))) = ws.next().await {
                if let Ok(parsed) = serde_json::from_str::<Value>(&text) {
                    if parsed.get("type").and_then(|v| v.as_str()) == Some("invokefunction") {
                        let id = parsed
                            .get("invocation_id")
                            .and_then(|v| v.as_str())
                            .expect("invoke_function should carry invocation_id");
                        return Uuid::parse_str(id).expect("uuid");
                    }
                }
            }
        }
    })
    .await
    .expect("InvokeFunction within 2s");

    // Build a payload that sits just under the limit. Reserve ~256 bytes
    // for the JSON envelope.
    let big_blob = "z".repeat(1024 * 1024 - 256);
    let result_msg = json!({
        "type": "invocationresult",
        "invocation_id": invocation_id.to_string(),
        "function_id": "echo",
        "result": { "blob": big_blob },
    });
    ws.send(WsMessage::Text(result_msg.to_string().into()))
        .await
        .expect("send result");

    let result = tokio::time::timeout(Duration::from_secs(5), invocation_handle)
        .await
        .expect("invocation resolves within 5s")
        .expect("invocation task didn't panic")
        .expect("at-limit payload must succeed");

    assert!(
        result.is_some(),
        "at-limit payload should produce a result body"
    );
}
