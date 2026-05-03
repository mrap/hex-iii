//! Tests for the payload size limit / `max_message_size` option (Phase 5).
//!
//! TDD red → green:
//! 1. Default on `InitOptions` resolves to 16 MiB.
//! 2. `InitOptions` builder accepts an override.
//! 3. `trigger()` raises `IIIError::PayloadTooLarge` before the WS round-trip
//!    when the encoded envelope exceeds `max_message_size`.
//! 4. Integration test (gated on engine URL env, `#[ignore]` until Phase 1
//!    lands the `invocation_failed_payload_too_large` error code).

use iii_sdk::{IIIError, InitOptions, register_worker};
use serde_json::json;

/// 16 MiB — the cross-SDK default ceiling.
const DEFAULT_MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024;

#[test]
fn max_message_size_default_is_16_mib() {
    let opts = InitOptions::default();
    assert_eq!(opts.max_message_size, None);
    assert_eq!(opts.resolved_max_message_size(), DEFAULT_MAX_MESSAGE_SIZE);
}

#[test]
fn init_options_accepts_max_message_size_override() {
    let opts = InitOptions {
        max_message_size: Some(8 * 1024 * 1024),
        ..InitOptions::default()
    };
    assert_eq!(opts.resolved_max_message_size(), 8 * 1024 * 1024);
}

#[tokio::test]
async fn trigger_with_oversize_payload_returns_payload_too_large_error() {
    // Configure a tiny ceiling so we don't have to allocate megabytes.
    let limit: usize = 1024;
    let iii = register_worker(
        "ws://127.0.0.1:1", // never actually connected — guard fires first
        InitOptions {
            max_message_size: Some(limit),
            ..InitOptions::default()
        },
    );

    // Build a payload whose JSON encoding will clearly exceed the limit.
    let blob = "A".repeat(limit * 4);
    let request = iii_sdk::TriggerRequest {
        function_id: "noop".to_string(),
        payload: json!({ "data": blob }),
        action: None,
        timeout_ms: Some(50),
    };

    let result = iii.trigger(request).await;
    iii.shutdown();

    match result {
        Err(IIIError::PayloadTooLarge { actual, limit: l }) => {
            assert_eq!(l, limit, "limit should match the configured value");
            assert!(
                actual > limit,
                "actual ({actual}) should exceed limit ({l})"
            );
        }
        other => panic!("expected IIIError::PayloadTooLarge, got: {other:?}"),
    }
}

#[test]
fn payload_too_large_display_matches_cross_sdk_format() {
    // The cross-SDK message format must be stable and mention "channels"
    // plus a docs URL anchor (see plan §3.2).
    let err = IIIError::PayloadTooLarge {
        actual: 20_000_000,
        limit: 16 * 1024 * 1024,
    };
    let s = err.to_string();
    assert!(
        s.contains("20000000"),
        "display should contain actual size: {s}"
    );
    assert!(
        s.contains(&format!("{}", 16 * 1024 * 1024)),
        "display should contain limit: {s}"
    );
    assert!(
        s.to_lowercase().contains("channels"),
        "display should reference channels: {s}"
    );
    assert!(
        s.contains("https://"),
        "display should include a docs URL: {s}"
    );
}

// Integration test: requires a live engine that emits the
// `invocation_failed_payload_too_large` error code (Phase 1 of the
// ws-payload-size-limit work). Skips silently when `III_URL` is unset so
// `cargo test` stays green in unit-only environments.
//
// Run with:
//   III_URL=ws://127.0.0.1:49134 cargo test --test payload_limits integration -- --include-ignored
#[tokio::test]
#[ignore = "requires a running engine reachable at III_URL"]
async fn integration_oversize_invocation_returns_payload_too_large_code() {
    let url = match std::env::var("III_URL") {
        Ok(u) => u,
        Err(_) => {
            eprintln!("III_URL not set; skipping");
            return;
        }
    };

    // Use an engine-side enforced cap. The producer guard would otherwise
    // intercept any payload sized to test the engine. We disable the guard
    // by setting `max_message_size` higher than the engine limit and rely
    // on the engine to reject the oversize message.
    let iii = register_worker(
        &url,
        InitOptions {
            max_message_size: Some(64 * 1024 * 1024),
            ..InitOptions::default()
        },
    );

    // Wait briefly for connect.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let blob = "A".repeat(20 * 1024 * 1024);
    let request = iii_sdk::TriggerRequest {
        function_id: "noop".to_string(),
        payload: json!({ "data": blob }),
        action: None,
        timeout_ms: Some(5_000),
    };

    let result = iii.trigger(request).await;
    iii.shutdown();

    match result {
        Err(IIIError::Remote { code, .. }) => {
            assert_eq!(code, "invocation_failed_payload_too_large");
        }
        other => {
            panic!("expected IIIError::Remote(invocation_failed_payload_too_large), got: {other:?}")
        }
    }
}
