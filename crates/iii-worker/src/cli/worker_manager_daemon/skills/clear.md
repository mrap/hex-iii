# worker/clear — wipe cached worker artifacts

Delete cached artifacts under `~/.iii/managed/{name}/`. The worker's entry in `iii.config.yaml` and `iii.lock` is **not** touched — call [`worker/remove`](iii://worker/remove) for that. Useful after `worker/remove` to reclaim disk, or before `worker/add --force` to force a clean re-download.

- id: `worker::clear`
- timeout: 30s
- idempotent: yes (second call clears 0 bytes)
- request: `ClearOptions { names: [], all?, yes }`
- response: `ClearOutcome { cleared_bytes }`

Same target rules as [`worker/remove`](iii://worker/remove): either `names` *or* `all: true`, never both, never neither, always `yes: true`.

## Example

```json
{ "all": true, "yes": true }
```

Response:
```json
{ "cleared_bytes": 812727797 }
```

## Errors

- **W103** target missing or ambiguous.
- **W104** consent missing.
- **W900** filesystem failure (rare).
