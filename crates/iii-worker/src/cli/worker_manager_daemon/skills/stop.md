# worker/stop — stop a running worker

Send a graceful shutdown signal. Destructive — requires explicit `yes: true` consent on the trigger surface. The CLI prompts interactively or accepts `-y`.

- id: `worker::stop`
- timeout: 30s
- idempotent: no (stateful)
- request: `StopOptions { name, yes }`
- response: `StopOutcome { name, stopped }`

`yes` must be exactly `true` — not `false`, not omitted, not the string `"true"`, not the number `1`. A slip in caller code should not silently kill a worker.

`stopped: false` means the stop didn't take effect within the daemon's grace window. Retry, or verify with [`worker/list`](iii://worker/list).

## Example

```json
{ "name": "image-resize", "yes": true }
```

Response:
```json
{ "name": "image-resize", "stopped": true }
```

## Errors

- **W100** invalid worker name (shell metacharacters, empty, > 64 chars).
- **W104** `yes` not `true`.
- **W110** worker name unknown.
- **W900** signal failure or grace-window timeout.
