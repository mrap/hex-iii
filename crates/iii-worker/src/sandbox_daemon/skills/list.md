# sandbox/list — list running sandboxes

Read the current sandbox roster. Pure read, safe to poll.

- id: `sandbox::list`
- timeout: 10s
- idempotent: yes
- request: `ListRequest {}`
- response: `ListResponse { sandboxes: [SandboxSummary] }`

`SandboxSummary`:
- `sandbox_id` — UUID; reuse as the target for [`sandbox/exec`](iii://sandbox/exec) and [`sandbox/stop`](iii://sandbox/stop).
- `name` — optional human label set at create time; `null` when unset.
- `image` — image reference the sandbox booted from.
- `age_secs` — seconds since the sandbox was created.
- `exec_in_progress` — another `sandbox::exec` against this id while this is `true` returns **S003**.
- `stopped` — running sandboxes have `stopped: false`. There is no `running` field; derive it from `!stopped`.

## Example

```json
{}
```

Response:
```json
{
  "sandboxes": [
    {
      "sandbox_id": "550e8400-e29b-41d4-a716-446655440000",
      "name": null,
      "image": "ghcr.io/iii-hq/node:latest",
      "age_secs": 142,
      "exec_in_progress": false,
      "stopped": false
    }
  ]
}
```

Also reachable inline as a section URI: [`iii://fn/sandbox/list`](iii://fn/sandbox/list).

## Errors

`sandbox::list` doesn't surface errors in practice — it always returns the current roster (possibly empty). An unexpected registry read failure surfaces as **S300** (platform).
