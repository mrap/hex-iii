# worker/list — list workers and run state

Read the union of every worker in `iii.config.yaml`, every artifact on disk under `~/.iii/managed/`, and every running process.

- id: `worker::list`
- timeout: 10s
- idempotent: yes (pure read)
- request: `ListOptions { running_only? }`
- response: `ListOutcome { workers: [WorkerEntry] }`

`WorkerEntry`:
- `name` — config name.
- `version` — string for registry-tracked workers; **omitted from JSON** for engine builtins that aren't lock-tracked.
- `running` — bool.
- `pid` — u32 when discoverable via ps; `null` for engine builtins.

## Example

Request:
```json
{ "running_only": false }
```

Response:
```json
{
  "workers": [
    { "name": "iii-stream",   "running": true, "pid": null },
    { "name": "image-resize", "running": true, "pid": 37037, "version": "0.1.2" },
    { "name": "skills",       "running": true, "pid": 37036, "version": "0.2.4" }
  ]
}
```

## Errors

- **W900** filesystem read failure (rare — config/lock both unreadable).
