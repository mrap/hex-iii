# worker/add — install a worker

Install from the iii registry or an OCI image. Writes the entry to `iii.config.yaml`, caches the artifact under `~/.iii/managed/{name}/`, and pins the resolved version in `iii.lock`. Calling twice yields the same outcome.

- id: `worker::add`
- timeout: 600s (registry pull + binary fetch + ready wait)
- idempotent: yes
- request: `AddOptions { source, force?, reset_config?, wait? }`
- response: `AddOutcome { name, version?, status, awaited_ready, config_path }`

`source` variants:
- `{ "kind": "registry", "name": "image-resize", "version": "0.1.2" }` — registry slug, optional pinned semver.
- `{ "kind": "oci", "reference": "ghcr.io/iii-hq/node:latest" }` — full OCI ref.
- `{ "kind": "local", "path": "./..." }` — **CLI only**; over the trigger this returns **W102**.

`status` values: `installed` (new), `already_current` (lockfile match), `repaired` (cache was corrupt), `replaced` (different version was installed before).

## Example

```json
{
  "source": { "kind": "registry", "name": "image-resize", "version": "0.1.2" },
  "force": false,
  "reset_config": false,
  "wait": true
}
```

## Errors

- **W101** missing/malformed `source`.
- **W102** local path via trigger.
- **W110** worker name not in registry.
- **W900** OCI pull, network, or filesystem failure (see `details.message`).
