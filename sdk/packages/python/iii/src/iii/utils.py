"""Utility functions for the III SDK."""

from __future__ import annotations

import json
from typing import TYPE_CHECKING, Any, Awaitable, Callable, cast

from .types import HttpRequest, InternalHttpRequest, StreamingRequest, StreamingResponse
from .types import is_channel_ref as is_channel_ref  # noqa: F401 - re-exported from types

if TYPE_CHECKING:
    from .types import HttpResponse


def _to_internal(req: Any) -> InternalHttpRequest:
    if isinstance(req, InternalHttpRequest):
        return req
    if isinstance(req, dict):
        return InternalHttpRequest(
            path_params=req.get("path_params", {}),
            query_params=req.get("query_params", {}),
            body=req.get("body"),
            headers=req.get("headers", {}),
            method=req.get("method", "GET"),
            response=req["response"],
            request_body=req["request_body"],
        )
    return cast(InternalHttpRequest, req)


def http(
    callback: Callable[[HttpRequest[Any], StreamingResponse], Awaitable[HttpResponse[Any] | None]],
) -> Callable[[Any], Awaitable[HttpResponse[Any] | None]]:
    """Wrap a buffered HTTP handler: receives HttpRequest + StreamingResponse."""

    async def wrapper(req: Any) -> HttpResponse[Any] | None:
        internal = _to_internal(req)
        res = StreamingResponse(internal.response)
        http_request: HttpRequest[Any] = HttpRequest(
            path_params=internal.path_params,
            query_params=internal.query_params,
            body=internal.body,
            headers=internal.headers,
            method=internal.method,
        )
        return await callback(http_request, res)

    return wrapper


def http_stream(
    callback: Callable[[StreamingRequest, StreamingResponse], Awaitable[HttpResponse[Any] | None]],
) -> Callable[[Any], Awaitable[HttpResponse[Any] | None]]:
    """Wrap a streaming HTTP handler: receives StreamingRequest + StreamingResponse."""

    async def wrapper(req: Any) -> HttpResponse[Any] | None:
        internal = _to_internal(req)
        res = StreamingResponse(internal.response)
        streaming_request = StreamingRequest(
            path_params=internal.path_params,
            query_params=internal.query_params,
            headers=internal.headers,
            method=internal.method,
            request_body=internal.request_body,
        )
        return await callback(streaming_request, res)

    return wrapper


def safe_stringify(value: Any) -> str:
    """Safely stringify a value, handling circular references and non-serializable types."""
    try:
        return json.dumps(value, default=str)
    except (TypeError, ValueError):
        return "[unserializable]"
