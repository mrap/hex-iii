# sandbox/stop — tear down a sandbox

Signal a sandbox to shut down and reap its resources. Already-stopped sandboxes return success with no work done. **Unknown ids return S002** — stop is idempotent against already-stopped state, but not against never-existed state.

- id: `sandbox::stop`
- timeout: 30s
- idempotent: against already-stopped only
- request: `StopRequest { sandbox_id, wait? }`
- response: `StopResponse { sandbox_id, stopped }`

`wait: true` blocks until the VM has fully exited. Default `false` is fire-and-forget within the daemon's grace window.

`stopped: false` means the stop didn't complete in time — the sandbox may still be tearing down. Check [`sandbox/list`](iii://sandbox/list) after a few seconds.

## Example

```json
{ "sandbox_id": "550e8400-e29b-41d4-a716-446655440000", "wait": false }
```

Response:
```json
{ "sandbox_id": "550e8400-e29b-41d4-a716-446655440000", "stopped": true }
```

## Errors

- **S001** `sandbox_id` is not a valid UUID.
- **S002** sandbox not found (unknown id, or already stopped and reaped — reaping removes the registry entry).
