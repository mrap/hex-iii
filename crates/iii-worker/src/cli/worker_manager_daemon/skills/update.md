# worker/update — re-resolve registry versions

Re-resolve each named worker against the registry, download newer artifacts if available, and rewrite `iii.lock`. Configs in `iii.config.yaml` are preserved.

- id: `worker::update`
- timeout: 600s
- idempotent: yes
- request: `UpdateOptions { names: [] }`
- response: `UpdateOutcome { updated: [{ name, from_version, to_version }] }`

`names: []` updates every installed registry-backed worker. `updated` contains one entry per worker that actually changed version; workers already at latest are omitted.

## Example

Request:
```json
{ "names": [] }
```

After a no-op:
```json
{ "updated": [] }
```

After a real change:
```json
{ "updated": [{ "name": "image-resize", "from_version": "0.1.2", "to_version": "0.1.3" }] }
```

## Errors

- **W110** name not installed.
- **W900** registry / network / filesystem failure.
