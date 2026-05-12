# sandbox/create — spin up a sandbox

Pull an image from the catalog and start a sandbox VM. Returns a UUID `sandbox_id` you reuse for [`sandbox/exec`](iii://sandbox/exec) and [`sandbox/stop`](iii://sandbox/stop).

- id: `sandbox::create`
- timeout: 600s (image pull dominates cold starts)
- idempotent: no (each call spawns a new sandbox)
- request: `CreateRequest { image, cpus?, memory_mb?, name?, network?, idle_timeout_secs?, env: [] }`
- response: `CreateResponse { sandbox_id, image }`

- `image` — preset (`python`, `node`) or full OCI ref in the catalog. Empty catalog denies every call (fail-closed).
- `env` — `Vec<String>` of `"K=V"` entries, NOT a map.
- `name` — optional human label, surfaced in [`sandbox/list`](iii://sandbox/list).

There is no `cwd` field; the workdir is fixed by the rootfs.

## Example

```json
{
  "image": "ghcr.io/iii-hq/node:latest",
  "env": ["NODE_ENV=production"],
  "cpus": 2,
  "memory_mb": 1024
}
```

Response:
```json
{
  "sandbox_id": "550e8400-e29b-41d4-a716-446655440000",
  "image": "ghcr.io/iii-hq/node:latest"
}
```

`sandbox_id` is a UUID, not the `sbx_*` opaque-id shape some other engines use.

## Errors

- **S001** invalid request (malformed `image`, missing required field).
- **S100** image not in catalog. Add it to the catalog or use a preset.
- **S101** rootfs missing on disk — run `iii worker add <image-ref>` first.
- **S102** auto-install failed (transient — retry).
- **S300** VM boot failed.
- **S400** resource limit hit (too many concurrent sandboxes).
