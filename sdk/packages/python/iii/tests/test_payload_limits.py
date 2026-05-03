"""Tests for the WebSocket invocation payload-size limit (Phase 3)."""

from __future__ import annotations

import asyncio
import os
from typing import Any

import pytest

from iii import InitOptions, TriggerAction
from iii.errors import IIIPayloadTooLarge
from iii.iii import III
from iii.iii_constants import DEFAULT_MAX_MESSAGE_SIZE


class _FakeWebSocket:
    """Minimal stand-in for `websockets.asyncio.client.ClientConnection`.

    Captures sent frames and exposes the `state.name == "OPEN"` shape the SDK
    checks before sending.
    """

    def __init__(self) -> None:
        self.sent: list[str] = []

        class _State:
            name = "OPEN"

        self.state = _State()

    async def send(self, payload: str) -> None:
        self.sent.append(payload)

    async def close(self) -> None:
        self.state.name = "CLOSED"

    def __aiter__(self) -> "_FakeWebSocket":
        return self

    async def __anext__(self) -> str:
        await asyncio.sleep(3600)
        raise StopAsyncIteration


@pytest.fixture
def captured_connect(monkeypatch: pytest.MonkeyPatch) -> dict[str, Any]:
    """Patch `websockets.connect` so tests can observe its kwargs without I/O."""
    captured: dict[str, Any] = {}

    async def fake_connect(*args: Any, **kwargs: Any) -> _FakeWebSocket:
        captured["args"] = args
        captured["kwargs"] = kwargs
        return _FakeWebSocket()

    import iii.iii as iii_mod

    monkeypatch.setattr(iii_mod.websockets, "connect", fake_connect)
    return captured


def _make_client(options: InitOptions | None = None) -> III:
    client = III("ws://localhost:1", options=options)
    client._wait_until_connected()
    return client


def test_default_max_message_size_constant_is_16_mib() -> None:
    assert DEFAULT_MAX_MESSAGE_SIZE == 16 * 1024 * 1024


def test_init_options_max_message_size_default_is_16_mib() -> None:
    assert InitOptions().max_message_size == 16 * 1024 * 1024


def test_init_options_max_message_size_override() -> None:
    opts = InitOptions(max_message_size=8 * 1024 * 1024)
    assert opts.max_message_size == 8 * 1024 * 1024


def test_init_options_max_message_size_is_plumbed_to_websockets(
    captured_connect: dict[str, Any],
) -> None:
    client = _make_client(InitOptions(max_message_size=8 * 1024 * 1024))
    try:
        assert captured_connect["kwargs"].get("max_size") == 8 * 1024 * 1024
    finally:
        client.shutdown()


def test_init_options_default_max_message_size_is_plumbed_to_websockets(
    captured_connect: dict[str, Any],
) -> None:
    client = _make_client()
    try:
        assert captured_connect["kwargs"].get("max_size") == 16 * 1024 * 1024
    finally:
        client.shutdown()


def test_trigger_with_oversize_payload_raises_specific_error(
    captured_connect: dict[str, Any],
) -> None:
    client = _make_client(InitOptions(max_message_size=1 * 1024 * 1024))
    try:
        with pytest.raises(IIIPayloadTooLarge) as excinfo:
            client.trigger(
                {
                    "function_id": "noop",
                    "payload": {"data_b64": "A" * (4 * 1024 * 1024)},
                }
            )
        msg = str(excinfo.value)
        assert "channels" in msg
        assert "https://iii.dev/docs/how-to/use-channels" in msg
        assert "exceeds invocation limit" in msg
    finally:
        client.shutdown()


def test_trigger_void_with_oversize_payload_raises_specific_error(
    captured_connect: dict[str, Any],
) -> None:
    client = _make_client(InitOptions(max_message_size=1 * 1024 * 1024))
    try:
        with pytest.raises(IIIPayloadTooLarge):
            client.trigger(
                {
                    "function_id": "noop",
                    "payload": {"data_b64": "A" * (4 * 1024 * 1024)},
                    "action": TriggerAction.Void(),
                }
            )
    finally:
        client.shutdown()


def test_trigger_below_limit_does_not_raise(
    captured_connect: dict[str, Any],
) -> None:
    """A payload comfortably below the limit must reach the WS send path."""
    client = _make_client(InitOptions(max_message_size=1 * 1024 * 1024))
    try:
        # Replace _send so we can confirm it was reached without awaiting
        # an engine response (default trigger waits for invocation_result).
        async def fake_send(msg: Any) -> None:
            return None

        client._send = fake_send  # type: ignore[assignment]

        async def call() -> None:
            await asyncio.wait_for(
                client.trigger_async(
                    {
                        "function_id": "noop",
                        "payload": {"data_b64": "A" * 1024},
                        "action": TriggerAction.Void(),
                    }
                ),
                timeout=2,
            )

        asyncio.run_coroutine_threadsafe(call(), client._loop).result(timeout=5)
    finally:
        client.shutdown()


def test_payload_too_large_error_message_format(
    captured_connect: dict[str, Any],
) -> None:
    """Cross-SDK consistent format: exact wording plus payload bytes & limit."""
    limit = 1024
    client = _make_client(InitOptions(max_message_size=limit))
    try:
        oversize_payload = {"data": "x" * (limit * 4)}
        with pytest.raises(IIIPayloadTooLarge) as excinfo:
            client.trigger({"function_id": "noop", "payload": oversize_payload})
        msg = str(excinfo.value)
        assert f"limit {limit} bytes" in msg
        assert "Payload " in msg
        assert " bytes exceeds invocation limit " in msg
        assert "For binary blobs use channels" in msg
    finally:
        client.shutdown()


def test_payload_too_large_subclasses_value_error() -> None:
    """The new exception should be catchable as a generic argument-size error."""
    err = IIIPayloadTooLarge(payload_bytes=2048, limit_bytes=1024)
    assert isinstance(err, ValueError)
    assert err.payload_bytes == 2048
    assert err.limit_bytes == 1024


@pytest.mark.integration
@pytest.mark.skipif(
    "III_URL" not in os.environ,
    reason="requires a live III engine; set III_URL to run",
)
def test_oversize_invocation_returns_payload_too_large_code() -> None:
    """End-to-end: engine rejects oversize WS message with the new error code.

    Engine default ceiling is 16 MiB. The SDK client is constructed with a
    larger ``max_message_size`` so the producer-side guard does not preempt
    the engine — we want to confirm the engine's `invocation_failed_payload_too_large`
    code surfaces back to the caller via the wire `ErrorBody`.
    """
    from iii.errors import IIIInvocationError

    engine_ws_url = os.environ["III_URL"]
    # 64 MiB SDK ceiling so the producer guard lets a 24 MiB payload through;
    # 24 MiB raw exceeds the engine's 16 MiB recv limit.
    client = III(
        engine_ws_url,
        options=InitOptions(max_message_size=64 * 1024 * 1024),
    )
    client._wait_until_connected()
    try:
        payload = {"data_b64": "A" * (24 * 1024 * 1024)}
        with pytest.raises(IIIInvocationError) as excinfo:
            client.trigger({"function_id": "noop", "payload": payload})
        assert excinfo.value.code == "invocation_failed_payload_too_large"
    finally:
        client.shutdown()
