# worker/start — start a configured worker

Spawn a worker that's already in `iii.config.yaml`. The engine connects to the process over its WebSocket port and waits for the ready signal.

- id: `worker::start`
- timeout: 60s
- idempotent: no (stateful — starting an already-running worker is a no-op or error depending on health)
- request: `StartOptions { name, port?, config?, wait? }`
- response: `StartOutcome { name, pid?, port? }`

- `name` — installed worker name.
- `port` — override the engine WS port (default = engine's `iii-worker-manager` port).
- `config` — YAML config file forwarded as `--config <path>`. Binary workers only; OCI ignores it.
- `wait` — block until ready. Default `true`.

`pid` and `port` may be `null` for engine builtins that don't surface a process (e.g. `iii-stream`, `iii-http`).

## Example

```json
{ "name": "image-resize", "wait": true }
```

Response:
```json
{ "name": "image-resize", "pid": 12345, "port": 49134 }
```

## Errors

- **W100** invalid name.
- **W110** worker not installed.
- **W900** spawn failure, ready-wait timeout, or port-bind failure.
