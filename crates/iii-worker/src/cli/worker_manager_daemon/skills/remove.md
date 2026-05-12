# worker/remove — uninstall workers

Remove a worker's entry from `iii.config.yaml`. The engine's file watcher tears down any running sandbox. Cached artifacts under `~/.iii/managed/{name}/` are **not** deleted — call [`worker/clear`](iii://worker/clear) for that.

- id: `worker::remove`
- timeout: 30s
- idempotent: yes
- request: `RemoveOptions { names: [], all?, yes }`
- response: `RemoveOutcome { removed: [string] }`

Either explicit `names` *or* `all: true`, never both, never neither. `yes: true` is always required.

## Example

```json
{ "names": ["image-resize"], "yes": true }
```

Also valid: `{ "names": ["image-resize", "skills"], "yes": true }`, `{ "all": true, "yes": true }`.

## Errors

- **W103** `names` empty and `all` unset, or both set.
- **W104** `yes` is not `true`.
- **W900** filesystem failure writing `iii.config.yaml`.
