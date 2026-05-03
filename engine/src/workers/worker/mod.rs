// Copyright Motia LLC and/or licensed to Motia LLC under one or more
// contributor license agreements. Licensed under the Elastic License 2.0;
// you may not use this file except in compliance with the Elastic License 2.0.
// This software is patent protected. We welcome discussions - reach out at support@motia.dev
// See LICENSE and PATENTS files for details.

pub mod channels;
pub mod rbac_config;
pub mod rbac_session;
pub mod ws_handler;

use std::{net::SocketAddr, sync::Arc};

use axum::{
    Router,
    extract::{ConnectInfo, State, ws::WebSocketUpgrade},
    http::{HeaderMap, Uri},
    response::IntoResponse,
    routing::get,
};
use colored::Colorize;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::net::TcpListener;

use crate::{
    engine::Engine,
    protocol::StreamChannelRef,
    workers::{traits::Worker, worker::ws_handler::channel_ws_upgrade},
};

pub const DEFAULT_PORT: u16 = 49134;

/// Default ceiling for inbound WebSocket messages from workers, in bytes.
///
/// 16 MiB is large enough that "small blob" use cases (single images,
/// JSON-with-embedded-data) ride the direct invocation path without
/// users needing to switch to channels, but small enough that a runaway
/// producer can't pin engine memory. Anything larger should use
/// channels. The engine is the source of truth; SDKs default to the
/// same value so they don't quietly underflow.
pub const DEFAULT_MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize, Serialize, Default, JsonSchema)]
pub struct CreateChannelInput {
    #[serde(default)]
    pub buffer_size: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default, JsonSchema)]
pub struct CreateChannelOutput {
    pub writer: StreamChannelRef,
    pub reader: StreamChannelRef,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerManagerConfig {
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default)]
    pub middleware_function_id: Option<String>,
    pub rbac: Option<rbac_config::RbacConfig>,
    /// Maximum size in bytes for a single inbound WebSocket message from
    /// a worker. Workers exceeding this limit are disconnected and any
    /// in-flight invocation on that connection resolves with the
    /// `invocation_failed_payload_too_large` error code. Defaults to 16
    /// MiB; for larger or streamable payloads, use channels.
    #[serde(default = "default_max_message_size")]
    pub max_message_size: usize,
}

fn default_port() -> u16 {
    DEFAULT_PORT
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_max_message_size() -> usize {
    DEFAULT_MAX_MESSAGE_SIZE
}

impl Default for WorkerManagerConfig {
    fn default() -> Self {
        Self {
            port: default_port(),
            host: default_host(),
            middleware_function_id: None,
            rbac: None,
            max_message_size: default_max_message_size(),
        }
    }
}

#[derive(Clone)]
pub struct WorkerManager {
    engine: Arc<Engine>,
    config: WorkerManagerConfig,
}

#[async_trait::async_trait]
impl Worker for WorkerManager {
    fn name(&self) -> &'static str {
        "WorkerManager"
    }

    async fn create(engine: Arc<Engine>, config: Option<Value>) -> anyhow::Result<Box<dyn Worker>> {
        let config: WorkerManagerConfig = config
            .map(serde_json::from_value)
            .transpose()?
            .unwrap_or_default();

        Ok(Box::new(WorkerManager { engine, config }))
    }

    async fn start_background_tasks(
        &self,
        shutdown_rx: tokio::sync::watch::Receiver<bool>,
        shutdown_tx: tokio::sync::watch::Sender<bool>,
    ) -> anyhow::Result<()> {
        let config = Arc::new(self.config.clone());
        let state = AppState {
            engine: self.engine.clone(),
            config: config.clone(),
            shutdown_rx: shutdown_rx.clone(),
        };

        tokio::spawn(async move {
            // Setup router
            let app = Router::new()
                .route("/", get(ws_handler))
                .route("/otel", get(otel_ws_handler))
                .route("/ws/channels/{channel_id}", get(channel_ws_upgrade))
                .with_state(state);

            // Bind and serve
            let addr = format!("{}:{}", config.host, config.port);
            let listener = TcpListener::bind(&addr).await.unwrap();
            tracing::info!("Engine listening on address: {}", addr.purple());

            let shutdown = async move {
                let _ = shutdown_signal().await;
                let _ = shutdown_tx.send(true);
            };

            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .with_graceful_shutdown(shutdown)
            .await
            .unwrap();
        });

        Ok(())
    }

    async fn initialize(&self) -> anyhow::Result<()> {
        tracing::info!("Initializing WorkerManager");
        Ok(())
    }

    async fn destroy(&self) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Clone)]
pub struct AppState {
    pub engine: Arc<Engine>,
    pub config: Arc<WorkerManagerConfig>,
    pub(crate) shutdown_rx: tokio::sync::watch::Receiver<bool>,
}

async fn shutdown_signal() -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        let mut sigterm = signal(SignalKind::terminate())?;
        let mut sigint = signal(SignalKind::interrupt())?;

        tokio::select! {
            _ = sigterm.recv() => {},
            _ = sigint.recv() => {},
            _ = tokio::signal::ctrl_c() => {},
        }
    }

    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await?;
    }

    Ok(())
}

/// Applies the configured per-message ceiling to a `WebSocketUpgrade`.
///
/// Both `ws_handler` and `otel_ws_handler` pass through here so the worker
/// path and the OTEL path share the same limit and there is one place to
/// adjust the policy.
fn apply_message_size_limit(
    ws: WebSocketUpgrade,
    config: &WorkerManagerConfig,
) -> WebSocketUpgrade {
    ws.max_message_size(config.max_message_size)
        .max_frame_size(config.max_message_size)
}

async fn ws_handler(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
    uri: Uri,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    let engine = state.engine.clone();
    let config = state.config.clone();
    let ws = apply_message_size_limit(ws, &config);

    ws.on_upgrade(move |socket| async move {
        if let Err(err) = engine
            .handle_worker(socket, addr, uri, headers, config, state.shutdown_rx)
            .await
        {
            tracing::error!(addr = %addr, error = ?err, "worker error");
        }
    })
}

/// WS upgrade handler for the OTEL-only endpoint (`/otel`).
///
/// Keeps telemetry traffic off the worker registry. See
/// `Engine::handle_otel` for the rationale.
async fn otel_ws_handler(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
    uri: Uri,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    let engine = state.engine.clone();
    let config = state.config.clone();
    let ws = apply_message_size_limit(ws, &config);

    ws.on_upgrade(move |socket| async move {
        if let Err(err) = engine
            .handle_otel(socket, addr, uri, headers, config, state.shutdown_rx)
            .await
        {
            tracing::error!(addr = %addr, error = ?err, "otel connection error");
        }
    })
}

crate::register_worker!("iii-worker-manager", WorkerManager, mandatory);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_config_default_values() {
        let config = WorkerManagerConfig::default();

        assert_eq!(config.port, DEFAULT_PORT);
        assert_eq!(config.host, "0.0.0.0");
        assert!(config.middleware_function_id.is_none());
        assert!(config.rbac.is_none());
    }

    #[test]
    fn worker_config_deserialize_empty_json_uses_defaults() {
        let config: WorkerManagerConfig = serde_json::from_str("{}").unwrap();

        assert_eq!(config.port, DEFAULT_PORT);
        assert_eq!(config.host, "0.0.0.0");
        assert!(config.middleware_function_id.is_none());
        assert!(config.rbac.is_none());
    }

    #[test]
    fn worker_config_default_max_message_size_is_16_mib() {
        let config: WorkerManagerConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(config.max_message_size, 16 * 1024 * 1024);
    }

    #[test]
    fn worker_config_max_message_size_override() {
        let config: WorkerManagerConfig =
            serde_json::from_str(r#"{"max_message_size": 32}"#).unwrap();
        assert_eq!(config.max_message_size, 32);
    }
}
